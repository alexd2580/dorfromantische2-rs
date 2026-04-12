//! Visualize the viewport detection pipeline.
//! Takes a game screenshot and the map, and shows each step:
//! 1. Screenshot → binary mask (tile vs background)
//! 2. Unproject through game camera → top-down silhouette
//! 3. Map silhouette at same scale
//! 4. Cross-correlation to find offset
//!
//! Run with: cargo run --no-default-features --example viewport_detect -- calibration_02.png
#![allow(clippy::too_many_arguments, dead_code)]

use dorfromantische2_rs::coords::{ScreenPos, WorldPos};
use dorfromantische2_rs::game::game_camera::GameCamera;
use dorfromantische2_rs::hex;
use dorfromantische2_rs::map::Map;
use dorfromantische2_rs::raw_data::SaveGame;
use opencv::core as cv_core;
use opencv::imgproc;
use opencv::prelude::*;
use std::io::Cursor;
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let calibrate = args.iter().any(|a| a == "--calibrate");
    let screenshot_path = args
        .iter()
        .skip(1)
        .find(|a| !a.starts_with("--"))
        .cloned()
        .unwrap_or_else(|| "calibration/calibration_02.png".into());

    let t = Instant::now();
    let data = std::fs::read("calibration/savegame.sav").unwrap();
    let parsed = nrbf_rs::parse_nrbf(&mut Cursor::new(&data));
    let sg = SaveGame::try_from(&parsed).unwrap();
    let map = Map::from(&sg);
    println!("Savegame: {:.1}ms", t.elapsed().as_secs_f64() * 1000.0);

    // Precompute map silhouette once (doesn't depend on camera params).
    let t = Instant::now();
    let map_ppu = {
        let mut mx0 = f32::MAX;
        let mut mx1 = f32::MIN;
        let mut my0 = f32::MAX;
        let mut my1 = f32::MIN;
        for pos in map.iter_tile_positions() {
            let w = hex::hex_to_world(pos);
            mx0 = mx0.min(w.x());
            mx1 = mx1.max(w.x());
            my0 = my0.min(w.y());
            my1 = my1.max(w.y());
        }
        2000.0 / (mx1 - mx0).max(my1 - my0)
    };
    let (map_sil, map_bounds) = render_map_silhouette(&map, map_ppu);
    println!(
        "Map silhouette: {}x{} (ppu={:.2}) in {:.1}ms",
        map_bounds.width,
        map_bounds.height,
        map_ppu,
        t.elapsed().as_secs_f64() * 1000.0
    );

    let t = Instant::now();
    let screenshot = image::open(&screenshot_path).unwrap().to_rgb8();
    let (sw, sh) = (screenshot.width() as usize, screenshot.height() as usize);
    println!(
        "Screenshot load: {:.1}ms",
        t.elapsed().as_secs_f64() * 1000.0
    );

    let t = Instant::now();
    let mask = binarize_screenshot(&screenshot);
    save_gray("step1_mask.png", &mask, sw, sh);
    println!("Binarize: {:.1}ms", t.elapsed().as_secs_f64() * 1000.0);

    if calibrate {
        run_calibration(&mask, sw, sh, &map_sil, &map_bounds, map_ppu);
    } else if args.iter().any(|a| a == "--grid") {
        run_grid_search(&mask, sw, sh, &map_sil, &map_bounds, map_ppu);
    } else if args.iter().any(|a| a == "--fine-pitch") {
        run_fine_pitch(&map_sil, &map_bounds, map_ppu);
    } else if args.iter().any(|a| a == "--sweep-pitch") {
        run_sweep_pitch(&map_sil, &map_bounds, map_ppu);
    } else {
        run_detect(&mask, sw, sh, &map_sil, &map_bounds, map_ppu);
    }
}

/// Score a set of camera parameters: unproject, scale, correlate.
fn score_params(
    mask: &[u8],
    sw: usize,
    sh: usize,
    map_sil: &[u8],
    map_bounds: &Bounds,
    map_ppu: f32,
    cam: &GameCamera,
    tiles_across: f32,
) -> f32 {
    let screen_size = (sw as u32, sh as u32);
    let (unprojected, coverage, unproj_bounds) = unproject_mask(mask, sw, sh, cam, screen_size);
    if unproj_bounds.width == 0 || unproj_bounds.height == 0 {
        return -1.0;
    }
    let target_w = tiles_across * 1.5 * map_ppu;
    let scale = target_w / unproj_bounds.width as f32;
    let scaled_tw = (unproj_bounds.width as f32 * scale).max(1.0) as usize;
    let scaled_th = (unproj_bounds.height as f32 * scale).max(1.0) as usize;
    if scaled_tw > map_bounds.width
        || scaled_th > map_bounds.height
        || scaled_tw < 10
        || scaled_th < 10
    {
        return -1.0;
    }
    let scaled = scale_image(
        &unprojected,
        unproj_bounds.width,
        unproj_bounds.height,
        scaled_tw,
        scaled_th,
    );
    let scaled_mask = scale_image(
        &coverage,
        unproj_bounds.width,
        unproj_bounds.height,
        scaled_tw,
        scaled_th,
    );
    let (_ox, _oy, score) = find_best_offset_fast(
        map_sil,
        map_bounds.width,
        map_bounds.height,
        &scaled,
        scaled_tw,
        scaled_th,
        &scaled_mask,
    );
    score
}

fn run_calibration(
    mask: &[u8],
    sw: usize,
    sh: usize,
    map_sil: &[u8],
    map_bounds: &Bounds,
    map_ppu: f32,
) {
    println!("\n=== CALIBRATION: Gradient descent ===");

    // Current best params.
    let mut pitch = 51.3_f32.to_radians();
    let mut fov_y = 46.0_f32.to_radians();
    let mut distance = 343.0_f32;
    let mut tiles_across = 37.1_f32;

    let make_cam = |pitch, fov_y, distance| GameCamera {
        pitch,
        yaw: 0.0,
        fov_y,
        distance,
        look_at: WorldPos::ZERO,
    };
    let mut best_score = score_params(
        mask,
        sw,
        sh,
        map_sil,
        map_bounds,
        map_ppu,
        &make_cam(pitch, fov_y, distance),
        tiles_across,
    );
    println!(
        "Initial: pitch={:.1}° fov={:.1}° dist={:.1} tiles={:.1} → score={:.4}",
        pitch.to_degrees(),
        fov_y.to_degrees(),
        distance,
        tiles_across,
        best_score
    );

    // Gradient descent: try small perturbations along each axis.
    let mut step_pitch = 1.0_f32.to_radians();
    let mut step_fov = 1.0_f32.to_radians();
    let mut step_dist = 20.0_f32;
    let mut step_tiles = 2.0_f32;

    for iteration in 0..60 {
        let mut improved = false;

        // Try each parameter in both directions.
        let params: [(f32, f32, f32, f32); 8] = [
            (pitch + step_pitch, fov_y, distance, tiles_across),
            (pitch - step_pitch, fov_y, distance, tiles_across),
            (pitch, fov_y + step_fov, distance, tiles_across),
            (pitch, fov_y - step_fov, distance, tiles_across),
            (pitch, fov_y, distance + step_dist, tiles_across),
            (pitch, fov_y, distance - step_dist, tiles_across),
            (pitch, fov_y, distance, tiles_across + step_tiles),
            (pitch, fov_y, distance, tiles_across - step_tiles),
        ];

        for &(p, f, d, t) in &params {
            if p < 10.0_f32.to_radians() || p > 80.0_f32.to_radians() {
                continue;
            }
            if f < 10.0_f32.to_radians() || f > 90.0_f32.to_radians() {
                continue;
            }
            if !(10.0..=1000.0).contains(&d) {
                continue;
            }
            if !(15.0..=60.0).contains(&t) {
                continue;
            }

            let s = score_params(
                mask,
                sw,
                sh,
                map_sil,
                map_bounds,
                map_ppu,
                &make_cam(p, f, d),
                t,
            );
            if s > best_score {
                best_score = s;
                pitch = p;
                fov_y = f;
                distance = d;
                tiles_across = t;
                improved = true;
            }
        }

        if iteration % 5 == 0 || !improved {
            println!(
                "  [{:2}] pitch={:.2}° fov={:.2}° dist={:.1} tiles={:.1} → score={:.4}{}",
                iteration,
                pitch.to_degrees(),
                fov_y.to_degrees(),
                distance,
                tiles_across,
                best_score,
                if improved { "" } else { "  (shrinking steps)" }
            );
        }

        if !improved {
            step_pitch *= 0.5;
            step_fov *= 0.5;
            step_dist *= 0.5;
            step_tiles *= 0.5;
            // Converged?
            if step_pitch < 0.01_f32.to_radians()
                && step_fov < 0.01_f32.to_radians()
                && step_dist < 0.1
                && step_tiles < 0.05
            {
                println!("  Converged at iteration {iteration}.");
                break;
            }
        }
    }

    println!("\n=== BEST PARAMETERS ===");
    println!("  pitch:        {:.2}°", pitch.to_degrees());
    println!("  fov_y:        {:.2}°", fov_y.to_degrees());
    println!("  distance:     {:.1}", distance);
    println!("  tiles_across: {:.1}", tiles_across);
    println!("  score:        {:.4}", best_score);

    // Run final detection with best params and save images.
    let cam = GameCamera {
        pitch,
        yaw: 0.0,
        fov_y,
        distance,
        look_at: WorldPos::ZERO,
    };
    let screen_size = (sw as u32, sh as u32);
    let (unprojected, coverage, unproj_bounds) = unproject_mask(mask, sw, sh, &cam, screen_size);
    save_gray(
        "step2_unprojected.png",
        &unprojected,
        unproj_bounds.width,
        unproj_bounds.height,
    );
    save_gray(
        "step2_coverage.png",
        &coverage,
        unproj_bounds.width,
        unproj_bounds.height,
    );
    save_gray(
        "step3_map_silhouette.png",
        map_sil,
        map_bounds.width,
        map_bounds.height,
    );

    let target_w = tiles_across * 1.5 * map_ppu;
    let scale = target_w / unproj_bounds.width as f32;
    let scaled_tw = (unproj_bounds.width as f32 * scale) as usize;
    let scaled_th = (unproj_bounds.height as f32 * scale) as usize;
    let scaled_template = scale_image(
        &unprojected,
        unproj_bounds.width,
        unproj_bounds.height,
        scaled_tw,
        scaled_th,
    );
    let scaled_mask = scale_image(
        &coverage,
        unproj_bounds.width,
        unproj_bounds.height,
        scaled_tw,
        scaled_th,
    );
    let (best_x, best_y, _) = find_best_offset(
        map_sil,
        map_bounds.width,
        map_bounds.height,
        &scaled_template,
        scaled_tw,
        scaled_th,
        &scaled_mask,
    );

    let center_px_x = best_x as f32 + scaled_tw as f32 / 2.0;
    let center_px_y = best_y as f32 + scaled_th as f32 / 2.0;
    let center_x = map_bounds.min_x + center_px_x / map_ppu;
    let center_y = map_bounds.max_y - center_px_y / map_ppu;
    println!("  center:       ({:.1}, {:.1})", center_x, center_y);
    let hex_pos = hex::world_to_hex(WorldPos::new(center_x, center_y));
    println!("  hex:          ({}, {})", hex_pos.x(), hex_pos.y());

    save_overlay(
        "step5_overlay.png",
        map_sil,
        map_bounds.width,
        map_bounds.height,
        &scaled_template,
        scaled_tw,
        scaled_th,
        best_x,
        best_y,
    );
    println!("  Saved step5_overlay.png");
}

fn run_grid_search(
    _mask: &[u8],
    _sw: usize,
    _sh: usize,
    map_sil: &[u8],
    map_bounds: &Bounds,
    map_ppu: f32,
) {
    // Load all calibration images.
    let cal_paths: Vec<String> = (1..=4)
        .map(|i| format!("calibration/calibration_{:02}.png", i))
        .filter(|p| std::path::Path::new(p).exists())
        .collect();
    println!(
        "Using {} calibration images: {:?}",
        cal_paths.len(),
        cal_paths
    );

    let masks: Vec<(Vec<u8>, usize, usize)> = cal_paths
        .iter()
        .map(|p| {
            let img = image::open(p).unwrap().to_rgb8();
            let (sw, sh) = (img.width() as usize, img.height() as usize);
            let mask = binarize_screenshot(&img);
            (mask, sw, sh)
        })
        .collect();

    println!("\n=== GRID SEARCH: pitch × fov × tiles ===");
    println!(
        "{:>7} {:>5} {:>6} {:>8} {:>8}  {:>7}  per-image",
        "pitch", "fov", "tiles", "tmpl_w", "tmpl_h", "avg_sc"
    );
    let mut best = (0.0_f32, 0.0_f32, 0.0_f32, f32::NEG_INFINITY);
    for pitch_deg in (42..=56).step_by(2) {
        for fov_deg in (40..=56).step_by(2) {
            for tiles_x10 in (330..=400).step_by(10) {
                let tiles = tiles_x10 as f32 / 10.0;
                let pitch = (pitch_deg as f32).to_radians();
                let fov = (fov_deg as f32).to_radians();

                let mut scores = Vec::new();
                let mut tw_last = 0;
                let mut th_last = 0;
                let mut skip = false;

                for (mask, sw, sh) in &masks {
                    let cam = GameCamera {
                        pitch,
                        yaw: 0.0,
                        fov_y: fov,
                        distance: 343.0,
                        look_at: WorldPos::ZERO,
                    };
                    let screen_size = (*sw as u32, *sh as u32);
                    let (unproj, cov, ub) = unproject_mask(mask, *sw, *sh, &cam, screen_size);
                    if ub.width == 0 || ub.height == 0 {
                        skip = true;
                        break;
                    }
                    let target_w = tiles * 1.5 * map_ppu;
                    let scale = target_w / ub.width as f32;
                    let tw = (ub.width as f32 * scale) as usize;
                    let th = (ub.height as f32 * scale) as usize;
                    if tw > map_bounds.width || th > map_bounds.height || tw < 10 || th < 10 {
                        skip = true;
                        break;
                    }
                    let scaled = scale_image(&unproj, ub.width, ub.height, tw, th);
                    let smask = scale_image(&cov, ub.width, ub.height, tw, th);
                    let (_, _, score) = find_best_offset(
                        map_sil,
                        map_bounds.width,
                        map_bounds.height,
                        &scaled,
                        tw,
                        th,
                        &smask,
                    );
                    scores.push(score);
                    tw_last = tw;
                    th_last = th;
                }
                if skip || scores.is_empty() {
                    continue;
                }

                let avg: f32 = scores.iter().sum::<f32>() / scores.len() as f32;
                let detail: String = scores
                    .iter()
                    .map(|s| format!("{s:.3}"))
                    .collect::<Vec<_>>()
                    .join(" ");
                println!("{pitch_deg:>6}° {fov_deg:>4}° {tiles:>5.1} {tw_last:>8} {th_last:>8}  {avg:>7.4}  [{detail}]");
                if avg > best.3 {
                    best = (pitch_deg as f32, fov_deg as f32, tiles, avg);
                }
            }
        }
    }
    println!(
        "\nBest: pitch={}° fov={}° tiles={:.1} avg_score={:.4}",
        best.0, best.1, best.2, best.3
    );
}

fn run_sweep_pitch(map_sil: &[u8], map_bounds: &Bounds, map_ppu: f32) {
    let cal_paths: Vec<String> = (1..=99)
        .map(|i| format!("calibration/calibration_{:02}.png", i))
        .take_while(|p| std::path::Path::new(p).exists())
        .collect();
    println!("Using {} calibration images", cal_paths.len());
    let images: Vec<(String, Vec<u8>, usize, usize)> = cal_paths
        .iter()
        .map(|p| {
            let img = image::open(p).unwrap().to_rgb8();
            let (sw, sh) = (img.width() as usize, img.height() as usize);
            let mask = binarize_screenshot(&img);
            (p.clone(), mask, sw, sh)
        })
        .collect();

    let fov = GameCamera::default().fov_y;
    let dist = GameCamera::default().distance;
    let pitch = GameCamera::default().pitch;

    for tiles in [61.0_f32] {
        for (path, mask, sw, sh) in &images {
            let img_name = path.trim_end_matches(".png");
            let cam = GameCamera {
                pitch,
                yaw: 0.0,
                fov_y: fov,
                distance: dist,
                look_at: WorldPos::ZERO,
            };
            let screen_size = (*sw as u32, *sh as u32);
            let (unproj, cov, ub) = unproject_mask(mask, *sw, *sh, &cam, screen_size);
            if ub.width == 0 || ub.height == 0 {
                continue;
            }
            let target_w = tiles * 1.5 * map_ppu;
            let scale = target_w / ub.width as f32;
            let tw = (ub.width as f32 * scale) as usize;
            let th = (ub.height as f32 * scale) as usize;
            if tw > map_bounds.width || th > map_bounds.height || tw < 10 || th < 10 {
                continue;
            }
            let scaled = scale_image(&unproj, ub.width, ub.height, tw, th);
            let smask = scale_image(&cov, ub.width, ub.height, tw, th);
            let (ox, oy, score) = find_best_offset(
                map_sil,
                map_bounds.width,
                map_bounds.height,
                &scaled,
                tw,
                th,
                &smask,
            );
            let cx = map_bounds.min_x + (ox as f32 + tw as f32 / 2.0) / map_ppu;
            let cy = map_bounds.max_y - (oy as f32 + th as f32 / 2.0) / map_ppu;
            let hex = hex::world_to_hex(WorldPos::new(cx, cy));
            println!(
                "tiles={tiles:.0} {img_name}: score={score:.0} hex=({},{}) tmpl={}x{}",
                hex.x(),
                hex.y(),
                tw,
                th
            );
            let out = format!("overlay_{img_name}_tiles{tiles:.0}.png");
            save_overlay(
                &out,
                map_sil,
                map_bounds.width,
                map_bounds.height,
                &scaled,
                tw,
                th,
                ox,
                oy,
            );
        }
    }
}

fn run_fine_pitch(map_sil: &[u8], map_bounds: &Bounds, map_ppu: f32) {
    let cal_paths: Vec<String> = (1..=4)
        .map(|i| format!("calibration/calibration_{:02}.png", i))
        .filter(|p| std::path::Path::new(p).exists())
        .collect();
    let masks: Vec<(Vec<u8>, usize, usize)> = cal_paths
        .iter()
        .map(|p| {
            let img = image::open(p).unwrap().to_rgb8();
            let (sw, sh) = (img.width() as usize, img.height() as usize);
            (binarize_screenshot(&img), sw, sh)
        })
        .collect();

    let fov = GameCamera::default().fov_y;
    let dist = GameCamera::default().distance;
    let tiles = 50.0_f32;

    println!(
        "{:>7} {:>8} {:>8}  {:>7}  per-image",
        "pitch", "tmpl_w", "tmpl_h", "avg_sc"
    );
    let mut best = (0.0_f32, f32::NEG_INFINITY);
    // Sweep pitch from 44° to 56° in 0.5° steps.
    for pitch_x2 in 88..=112 {
        let pitch_deg = pitch_x2 as f32 / 2.0;
        let pitch = pitch_deg.to_radians();
        let mut scores = Vec::new();
        let mut tw_last = 0;
        let mut th_last = 0;
        let mut skip = false;
        for (mask, sw, sh) in &masks {
            let cam = GameCamera {
                pitch,
                yaw: 0.0,
                fov_y: fov,
                distance: dist,
                look_at: WorldPos::ZERO,
            };
            let screen_size = (*sw as u32, *sh as u32);
            let (unproj, cov, ub) = unproject_mask(mask, *sw, *sh, &cam, screen_size);
            if ub.width == 0 || ub.height == 0 {
                skip = true;
                break;
            }
            let target_w = tiles * 1.5 * map_ppu;
            let scale = target_w / ub.width as f32;
            let tw = (ub.width as f32 * scale) as usize;
            let th = (ub.height as f32 * scale) as usize;
            if tw > map_bounds.width || th > map_bounds.height || tw < 10 || th < 10 {
                skip = true;
                break;
            }
            let scaled = scale_image(&unproj, ub.width, ub.height, tw, th);
            let smask = scale_image(&cov, ub.width, ub.height, tw, th);
            let (_, _, score) = find_best_offset(
                map_sil,
                map_bounds.width,
                map_bounds.height,
                &scaled,
                tw,
                th,
                &smask,
            );
            scores.push(score);
            tw_last = tw;
            th_last = th;
        }
        if skip || scores.is_empty() {
            continue;
        }
        let avg: f32 = scores.iter().sum::<f32>() / scores.len() as f32;
        let detail: String = scores
            .iter()
            .map(|s| format!("{s:.3}"))
            .collect::<Vec<_>>()
            .join(" ");
        println!("{pitch_deg:>6.1}° {tw_last:>8} {th_last:>8}  {avg:>7.4}  [{detail}]");
        if avg > best.1 {
            best = (pitch_deg, avg);
        }
    }
    println!("\nBest: pitch={:.1}° avg_score={:.4}", best.0, best.1);
}

fn run_detect(
    mask: &[u8],
    sw: usize,
    sh: usize,
    map_sil: &[u8],
    map_bounds: &Bounds,
    map_ppu: f32,
) {
    run_detect_with(
        mask,
        sw,
        sh,
        map_sil,
        map_bounds,
        map_ppu,
        GameCamera::default(),
        61.0,
    );
}

fn run_detect_with(
    mask: &[u8],
    sw: usize,
    sh: usize,
    map_sil: &[u8],
    map_bounds: &Bounds,
    map_ppu: f32,
    cam: GameCamera,
    tiles_across: f32,
) {
    let screen_size = (sw as u32, sh as u32);
    let (unprojected, coverage, unproj_bounds) = unproject_mask(mask, sw, sh, &cam, screen_size);
    save_gray(
        "step2_unprojected.png",
        &unprojected,
        unproj_bounds.width,
        unproj_bounds.height,
    );
    save_gray(
        "step2_coverage.png",
        &coverage,
        unproj_bounds.width,
        unproj_bounds.height,
    );
    save_gray(
        "step3_map_silhouette.png",
        map_sil,
        map_bounds.width,
        map_bounds.height,
    );

    let target_w = tiles_across * 1.5 * map_ppu;
    let scale = target_w / unproj_bounds.width as f32;
    let scaled_tw = (unproj_bounds.width as f32 * scale) as usize;
    let scaled_th = (unproj_bounds.height as f32 * scale) as usize;
    println!(
        "Template {}x{} vs map {}x{}",
        scaled_tw, scaled_th, map_bounds.width, map_bounds.height
    );
    let scaled_template = scale_image(
        &unprojected,
        unproj_bounds.width,
        unproj_bounds.height,
        scaled_tw,
        scaled_th,
    );
    let scaled_mask = scale_image(
        &coverage,
        unproj_bounds.width,
        unproj_bounds.height,
        scaled_tw,
        scaled_th,
    );

    // Find top-3 peaks with non-maximum suppression.
    let min_dist = scaled_tw.max(scaled_th) as i32;
    let peaks = find_top_offsets(
        map_sil,
        map_bounds.width,
        map_bounds.height,
        &scaled_template,
        scaled_tw,
        scaled_th,
        &scaled_mask,
        3,
        min_dist,
    );
    for (i, &(ox, oy, score)) in peaks.iter().enumerate() {
        let cx = map_bounds.min_x + (ox as f32 + scaled_tw as f32 / 2.0) / map_ppu;
        let cy = map_bounds.max_y - (oy as f32 + scaled_th as f32 / 2.0) / map_ppu;
        let hex = hex::world_to_hex(WorldPos::new(cx, cy));
        let marker = if i == 0 { " <-- best" } else { "" };
        println!("  #{}: offset=({ox}, {oy}) score={score:.4} center=({cx:.1}, {cy:.1}) hex=({}, {}){marker}",
            i + 1, hex.x(), hex.y());
    }

    if let Some(&(ox, oy, _)) = peaks.first() {
        save_overlay(
            "step5_overlay.png",
            map_sil,
            map_bounds.width,
            map_bounds.height,
            &scaled_template,
            scaled_tw,
            scaled_th,
            ox,
            oy,
        );
        println!("Saved step5_overlay.png");
    }
}

fn binarize_screenshot(img: &image::RgbImage) -> Vec<u8> {
    let (w, h) = (img.width() as usize, img.height() as usize);
    let mut mask = vec![0u8; w * h];
    for y in 0..h {
        for x in 0..w {
            let p = img.get_pixel(x as u32, y as u32);
            let r = p[0] as f32;
            let g = p[1] as f32;
            let b = p[2] as f32;
            // Pink background: high R, medium G/B, R > G, R > B.
            let is_pink = r > 180.0 && g > 140.0 && b > 140.0 && r > g && r > b && (r - g) > 20.0;
            // Light blue (water): similar to pink but B >= G.
            let is_water = r > 160.0 && g > 180.0 && b > 200.0;
            // Sky/fog at top.
            let is_sky = r > 200.0 && g > 190.0 && b > 190.0 && y < h / 5;
            mask[y * w + x] = if is_pink || is_water || is_sky {
                0
            } else {
                255
            };
        }
    }
    // Mask out UI corners.
    for y in 0..h / 8 {
        for x in (w * 3 / 4)..w {
            mask[y * w + x] = 0;
        }
    }
    for y in (h * 3 / 4)..h {
        for x in (w * 3 / 4)..w {
            mask[y * w + x] = 0;
        }
    }
    for y in 0..h / 8 {
        for x in 0..w / 8 {
            mask[y * w + x] = 0;
        }
    }
    mask
}

#[derive(Clone, Debug)]
struct Bounds {
    min_x: f32,
    max_x: f32,
    min_y: f32,
    max_y: f32,
    width: usize,
    height: usize,
    pixels_per_unit: f32,
}

/// Nearest-neighbor downscale of a grayscale image.
fn scale_image(src: &[u8], sw: usize, sh: usize, dw: usize, dh: usize) -> Vec<u8> {
    let mut dst = vec![0u8; dw * dh];
    for dy in 0..dh {
        for dx in 0..dw {
            let sx = dx * sw / dw;
            let sy = dy * sh / dh;
            dst[dy * dw + dx] = src[sy * sw + sx];
        }
    }
    dst
}

/// Returns (image, coverage_mask, bounds).
/// coverage_mask: 255 where the screen pixel was visible, 0 where off-screen/UI cutout.
fn screen_coverage_mask(sw: usize, sh: usize) -> Vec<u8> {
    let mut cov = vec![255u8; sw * sh];
    // Top-right: score display.
    for y in 0..sh / 8 {
        for x in (sw * 3 / 4)..sw {
            cov[y * sw + x] = 0;
        }
    }
    // Bottom-right: next tile + tile count.
    for y in (sh * 3 / 4)..sh {
        for x in (sw * 3 / 4)..sw {
            cov[y * sw + x] = 0;
        }
    }
    // Top-left: compass/logo.
    for y in 0..sh / 8 {
        for x in 0..sw / 8 {
            cov[y * sw + x] = 0;
        }
    }
    cov
}

fn unproject_mask(
    mask: &[u8],
    sw: usize,
    sh: usize,
    cam: &GameCamera,
    screen_size: (u32, u32),
) -> (Vec<u8>, Vec<u8>, Bounds) {
    // Compute world bounds from the four screen corners.
    let corners = [
        ScreenPos::new(0.0, 0.0),
        ScreenPos::new(1.0, 0.0),
        ScreenPos::new(0.0, 1.0),
        ScreenPos::new(1.0, 1.0),
    ];
    let world_corners: Vec<WorldPos> = corners
        .iter()
        .map(|&s| cam.screen_to_world(s, screen_size))
        .collect();

    let mut min_x = f32::MAX;
    let mut max_x = f32::MIN;
    let mut min_y = f32::MAX;
    let mut max_y = f32::MIN;
    for &c in &world_corners {
        min_x = min_x.min(c.x());
        max_x = max_x.max(c.x());
        min_y = min_y.min(c.y());
        max_y = max_y.max(c.y());
    }

    let margin = 2.0;
    min_x -= margin;
    max_x += margin;
    min_y -= margin;
    max_y += margin;

    let max_img_dim = 400.0_f32;
    let world_w = max_x - min_x;
    let world_h = max_y - min_y;
    let ppu = max_img_dim / world_w.max(world_h);
    let w = ((max_x - min_x) * ppu) as usize;
    let h = ((max_y - min_y) * ppu) as usize;

    let screen_cov = screen_coverage_mask(sw, sh);
    let mut img = vec![0u8; w * h];
    let mut coverage = vec![0u8; w * h];

    // Iterate world space, project back to screen to sample mask.
    for py in 0..h {
        for px in 0..w {
            let world = WorldPos::new(min_x + px as f32 / ppu, max_y - py as f32 / ppu);
            if let Some(screen) = cam.world_to_screen(world, screen_size) {
                let sx = (screen.0.x * sw as f32) as i32;
                let sy = (screen.0.y * sh as f32) as i32;
                if sx >= 0 && sx < sw as i32 && sy >= 0 && sy < sh as i32 {
                    let si = sy as usize * sw + sx as usize;
                    if screen_cov[si] > 0 {
                        img[py * w + px] = mask[si];
                        coverage[py * w + px] = 255;
                    }
                }
            }
        }
    }

    (
        img,
        coverage,
        Bounds {
            min_x,
            max_x,
            min_y,
            max_y,
            width: w,
            height: h,
            pixels_per_unit: ppu,
        },
    )
}

fn render_map_silhouette(map: &Map, ppu: f32) -> (Vec<u8>, Bounds) {
    // Find world bounds.
    let mut min_x = f32::MAX;
    let mut max_x = f32::MIN;
    let mut min_y = f32::MAX;
    let mut max_y = f32::MIN;

    for pos in map.iter_tile_positions() {
        let world = hex::hex_to_world(pos);
        min_x = min_x.min(world.x());
        max_x = max_x.max(world.x());
        min_y = min_y.min(world.y());
        max_y = max_y.max(world.y());
    }

    let margin = 2.0;
    min_x -= margin;
    max_x += margin;
    min_y -= margin;
    max_y += margin;

    // Extra padding so the template can slide beyond the map edges.
    let pad = 50.0;
    min_x -= pad;
    max_x += pad;
    min_y -= pad;
    max_y += pad;

    let w = ((max_x - min_x) * ppu) as usize;
    let h = ((max_y - min_y) * ppu) as usize;
    let mut img = vec![0u8; w * h];

    // Fill a circle for each tile. Radius ~0.8 world units * ppu.
    let r = (1.2 * ppu) as i32;
    let r_sq = r * r;
    for pos in map.iter_tile_positions() {
        let world = hex::hex_to_world(pos);
        let cx = ((world.x() - min_x) * ppu) as i32;
        let cy = ((max_y - world.y()) * ppu) as i32;
        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy <= r_sq {
                    let x = cx + dx;
                    let y = cy + dy;
                    if x >= 0 && x < w as i32 && y >= 0 && y < h as i32 {
                        img[y as usize * w + x as usize] = 255;
                    }
                }
            }
        }
    }

    (
        img,
        Bounds {
            min_x,
            max_x,
            min_y,
            max_y,
            width: w,
            height: h,
            pixels_per_unit: ppu,
        },
    )
}

fn make_mat(data: &[u8], w: usize, h: usize) -> cv_core::Mat {
    unsafe {
        cv_core::Mat::new_rows_cols_with_data_unsafe(
            h as i32,
            w as i32,
            cv_core::CV_8UC1,
            data.as_ptr() as *mut std::ffi::c_void,
            cv_core::Mat_AUTO_STEP,
        )
        .unwrap()
    }
}

/// Use OpenCV matchTemplate with mask (TM_CCORR_NORMED).
fn find_best_offset(
    map_img: &[u8],
    mw: usize,
    mh: usize,
    template: &[u8],
    tw: usize,
    th: usize,
    mask: &[u8],
) -> (i32, i32, f32) {
    if tw == 0 || th == 0 || mw == 0 || mh == 0 || tw > mw || th > mh {
        return (0, 0, -1.0);
    }
    let map_mat = make_mat(map_img, mw, mh);
    let tmpl_mat = make_mat(template, tw, th);
    let mask_mat = make_mat(mask, tw, th);
    let mut result = cv_core::Mat::default();
    imgproc::match_template(
        &map_mat,
        &tmpl_mat,
        &mut result,
        imgproc::TM_SQDIFF,
        &mask_mat,
    )
    .unwrap();
    let mut min_val = 0.0;
    let mut min_loc = cv_core::Point::new(0, 0);
    cv_core::min_max_loc(
        &result,
        Some(&mut min_val),
        None,
        Some(&mut min_loc),
        None,
        &cv_core::no_array(),
    )
    .unwrap();
    // Return negative so higher = better (consistent with callers).
    (min_loc.x, min_loc.y, -min_val as f32)
}

/// Fast version — same implementation, OpenCV is already fast.
fn find_best_offset_fast(
    map_img: &[u8],
    mw: usize,
    mh: usize,
    template: &[u8],
    tw: usize,
    th: usize,
    mask: &[u8],
) -> (i32, i32, f32) {
    find_best_offset(map_img, mw, mh, template, tw, th, mask)
}

/// Return top-N peaks with non-maximum suppression (min_dist between peaks).
fn find_top_offsets(
    map_img: &[u8],
    mw: usize,
    mh: usize,
    template: &[u8],
    tw: usize,
    th: usize,
    mask: &[u8],
    n: usize,
    min_dist: i32,
) -> Vec<(i32, i32, f32)> {
    if tw == 0 || th == 0 || mw == 0 || mh == 0 || tw > mw || th > mh {
        return vec![];
    }
    let map_mat = make_mat(map_img, mw, mh);
    let tmpl_mat = make_mat(template, tw, th);
    let mask_mat = make_mat(mask, tw, th);
    let mut result = cv_core::Mat::default();
    imgproc::match_template(
        &map_mat,
        &tmpl_mat,
        &mut result,
        imgproc::TM_SQDIFF,
        &mask_mat,
    )
    .unwrap();

    let rw = result.cols();
    let rh = result.rows();
    let mut peaks = Vec::new();

    for _ in 0..n {
        let mut min_val = 0.0;
        let mut min_loc = cv_core::Point::new(0, 0);
        cv_core::min_max_loc(
            &result,
            Some(&mut min_val),
            None,
            Some(&mut min_loc),
            None,
            &cv_core::no_array(),
        )
        .unwrap();
        peaks.push((min_loc.x, min_loc.y, -min_val as f32));
        let x0 = (min_loc.x - min_dist).max(0);
        let y0 = (min_loc.y - min_dist).max(0);
        let x1 = (min_loc.x + min_dist).min(rw - 1);
        let y1 = (min_loc.y + min_dist).min(rh - 1);
        let roi = cv_core::Rect::new(x0, y0, x1 - x0 + 1, y1 - y0 + 1);
        let mut roi_mat = cv_core::Mat::roi_mut(&mut result, roi).unwrap();
        roi_mat
            .set_to(&cv_core::Scalar::all(f64::MAX), &cv_core::no_array())
            .unwrap();
    }
    peaks
}

fn save_gray(path: &str, data: &[u8], w: usize, h: usize) {
    if w == 0 || h == 0 {
        return;
    }
    let img = image::GrayImage::from_raw(w as u32, h as u32, data.to_vec()).unwrap();
    img.save(path).unwrap();
}

fn save_overlay(
    path: &str,
    map_img: &[u8],
    mw: usize,
    mh: usize,
    template: &[u8],
    tw: usize,
    th: usize,
    ox: i32,
    oy: i32,
) {
    let mut overlay = image::RgbImage::new(mw as u32, mh as u32);

    // Map silhouette in dark blue.
    for y in 0..mh {
        for x in 0..mw {
            let v = map_img[y * mw + x];
            overlay.put_pixel(x as u32, y as u32, image::Rgb([0, 0, v / 2]));
        }
    }

    // Template overlay in yellow.
    for ty in 0..th {
        for tx in 0..tw {
            let x = ox as usize + tx;
            let y = oy as usize + ty;
            if x < mw && y < mh && template[ty * tw + tx] > 0 {
                let p = overlay.get_pixel(x as u32, y as u32);
                overlay.put_pixel(x as u32, y as u32, image::Rgb([255, 255, p[2]]));
            }
        }
    }

    // Draw bounding box of template in red.
    for x in ox.max(0) as usize..(ox as usize + tw).min(mw) {
        if oy >= 0 && (oy as usize) < mh {
            overlay.put_pixel(x as u32, oy as u32, image::Rgb([255, 0, 0]));
        }
        let bottom = (oy as usize + th).min(mh - 1);
        overlay.put_pixel(x as u32, bottom as u32, image::Rgb([255, 0, 0]));
    }
    for y in oy.max(0) as usize..(oy as usize + th).min(mh) {
        if ox >= 0 && (ox as usize) < mw {
            overlay.put_pixel(ox as u32, y as u32, image::Rgb([255, 0, 0]));
        }
        let right = (ox as usize + tw).min(mw - 1);
        overlay.put_pixel(right as u32, y as u32, image::Rgb([255, 0, 0]));
    }

    overlay.save(path).unwrap();
}
