//! Calibrate the game camera projection by matching a screenshot against the map silhouette.
//! Run with: cargo run --no-default-features --example calibrate_camera

use dorfromantische2_rs::hex;
use dorfromantische2_rs::map::Map;
use dorfromantische2_rs::raw_data::SaveGame;
use glam::Vec2;
use std::io::Cursor;

/// Camera projection parameters.
#[derive(Clone, Debug)]
struct CameraParams {
    /// Pitch angle in radians (0 = looking straight down, PI/2 = looking horizontally).
    pitch: f32,
    /// Yaw angle in radians (rotation of view around vertical axis).
    yaw: f32,
    /// Vertical field of view in radians.
    fov_y: f32,
    /// World position the camera is looking at.
    look_at: Vec2,
    /// Camera distance from look_at point.
    distance: f32,
}

impl CameraParams {
    /// Project a world-space (top-down) point to screen-space (0..1, 0..1).
    /// Returns None if the point is behind the camera.
    fn project(&self, world: Vec2, aspect: f32) -> Option<(f32, f32)> {
        // Apply yaw rotation around look_at point.
        let dx0 = world.x - self.look_at.x;
        let dy0 = world.y - self.look_at.y;
        let cos_yaw = self.yaw.cos();
        let sin_yaw = self.yaw.sin();
        let dx = dx0 * cos_yaw - dy0 * sin_yaw;
        let dy = dx0 * sin_yaw + dy0 * cos_yaw;

        // Camera looks down at pitch angle.
        let cam_depth = self.distance - dy * self.pitch.sin();
        let cam_x = dx;
        let cam_y = -dy * self.pitch.cos();

        if cam_depth <= 0.0 {
            return None;
        }

        let half_h = (self.fov_y / 2.0).tan();
        let screen_x = cam_x / (cam_depth * half_h * aspect);
        let screen_y = cam_y / (cam_depth * half_h);

        Some((0.5 + screen_x * 0.5, 0.5 + screen_y * 0.5))
    }
}

/// Render the map silhouette into a binary image at the given resolution.
fn render_silhouette(map: &Map, width: usize, height: usize, params: &CameraParams) -> Vec<u8> {
    let mut img = vec![0u8; width * height]; // 0 = empty, 255 = tile
    let aspect = width as f32 / height as f32;

    for pos in map.iter_tile_positions() {
        let world = hex::hex_to_world(pos);
        if let Some((sx, sy)) = params.project(world.0, aspect) {
            let px = (sx * width as f32) as i32;
            let py = (sy * height as f32) as i32;
            // Draw a small dot for each tile.
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let x = px + dx;
                    let y = py + dy;
                    if x >= 0 && x < width as i32 && y >= 0 && y < height as i32 {
                        img[y as usize * width + x as usize] = 255;
                    }
                }
            }
        }
    }
    img
}

/// Convert a screenshot to a binary mask (tile = 255, background = 0).
fn screenshot_to_mask(img: &image::RgbImage) -> Vec<u8> {
    let (w, h) = (img.width() as usize, img.height() as usize);
    let mut mask = vec![0u8; w * h];

    for y in 0..h {
        for x in 0..w {
            let p = img.get_pixel(x as u32, y as u32);
            let r = p[0] as f32;
            let g = p[1] as f32;
            let b = p[2] as f32;
            // The background is pink (#FFB6C1 ish). Tiles are NOT pink.
            // Detect pink: high R, medium-high G, medium-high B, R > G, R > B.
            let is_pink = r > 180.0 && g > 140.0 && b > 140.0 && r > g && r > b && (r - g) > 20.0;
            // Also detect the UI areas (score, buttons) — exclude edges.
            mask[y * w + x] = if is_pink { 0 } else { 255 };
        }
    }
    mask
}

/// Compute normalized cross-correlation between two binary images.
fn correlate(a: &[u8], b: &[u8], width: usize, height: usize) -> f32 {
    assert_eq!(a.len(), b.len());
    let n = width * height;

    let mean_a: f32 = a.iter().map(|&x| x as f32).sum::<f32>() / n as f32;
    let mean_b: f32 = b.iter().map(|&x| x as f32).sum::<f32>() / n as f32;

    let mut cov = 0.0f32;
    let mut var_a = 0.0f32;
    let mut var_b = 0.0f32;

    for i in 0..n {
        let da = a[i] as f32 - mean_a;
        let db = b[i] as f32 - mean_b;
        cov += da * db;
        var_a += da * da;
        var_b += db * db;
    }

    let denom = (var_a * var_b).sqrt();
    if denom < 1e-10 {
        return 0.0;
    }
    cov / denom
}

fn main() {
    println!("Loading savegame...");
    let data = std::fs::read("calibration/savegame.sav").unwrap();
    let parsed = nrbf_rs::parse_nrbf(&mut Cursor::new(&data));
    let sg = SaveGame::try_from(&parsed).unwrap();
    let map = Map::from(&sg);

    println!("Loading screenshot...");
    let screenshot = image::open("calibration/calibration_01.png")
        .unwrap()
        .to_rgb8();
    let (sw, sh) = (screenshot.width() as usize, screenshot.height() as usize);
    let screenshot_mask = screenshot_to_mask(&screenshot);

    // Save the screenshot mask for inspection.
    let mask_img =
        image::GrayImage::from_raw(sw as u32, sh as u32, screenshot_mask.clone()).unwrap();
    mask_img.save("calibration_mask.png").unwrap();
    println!("Saved screenshot mask to calibration_mask.png");

    // Find the big lake area — the screenshot is near the southern part of the map.
    // From earlier analysis, big lake is around hex (30, -100).
    // World coords: hex_to_world((30, -100)) = (45, -147.2)
    let lake_center = hex::hex_to_world(dorfromantische2_rs::data::HexPos::new(30, -100));
    println!(
        "Lake center (world): ({:.1}, {:.1})",
        lake_center.x(),
        lake_center.y()
    );

    // Downscale for faster correlation.
    let scale = 4;
    let cw = sw / scale;
    let ch = sh / scale;
    let mut screenshot_small: Vec<u8> = {
        let mut buf = Vec::with_capacity(cw * ch);
        for y in 0..ch {
            for x in 0..cw {
                buf.push(screenshot_mask[y * scale * sw + x * scale]);
            }
        }
        buf
    };

    println!("Starting grid search...");

    // Mask out UI areas in screenshot (corners).
    let mask_ui = |mask: &mut Vec<u8>, w: usize, h: usize| {
        for y in 0..h / 10 {
            for x in (w * 7 / 10)..w {
                mask[y * w + x] = 0;
            }
        }
        for y in (h * 7 / 10)..h {
            for x in (w * 7 / 10)..w {
                mask[y * w + x] = 0;
            }
        }
        for y in 0..h / 10 {
            for x in 0..w / 10 {
                mask[y * w + x] = 0;
            }
        }
    };
    mask_ui(&mut screenshot_small, cw, ch);

    let mut best_params = CameraParams {
        pitch: 49.0_f32.to_radians(),
        yaw: 0.0,
        fov_y: 47.0_f32.to_radians(),
        look_at: Vec2::new(130.0, -2.2),
        distance: 350.0,
    };
    let mut best_score = {
        let sil = render_silhouette(&map, cw, ch, &best_params);
        correlate(&screenshot_small, &sil, cw, ch)
    };
    println!("Starting score: {best_score:.4}");

    // Gradient descent: perturb each parameter, keep improvements.
    // pitch_deg, yaw_deg, fov_deg, dist, ox, oy
    let mut step_sizes = [2.0_f32, 5.0, 2.0, 20.0, 5.0, 5.0];
    for iteration in 0..300 {
        let mut improved = false;
        for dim in 0..6 {
            for sign in [-1.0_f32, 1.0] {
                let mut trial = best_params.clone();
                match dim {
                    0 => trial.pitch += (sign * step_sizes[0]).to_radians(),
                    1 => trial.yaw += (sign * step_sizes[1]).to_radians(),
                    2 => trial.fov_y += (sign * step_sizes[2]).to_radians(),
                    3 => trial.distance += sign * step_sizes[3],
                    4 => trial.look_at.x += sign * step_sizes[4],
                    5 => trial.look_at.y += sign * step_sizes[5],
                    _ => unreachable!(),
                }
                if trial.distance < 50.0 || trial.fov_y < 10.0_f32.to_radians() {
                    continue;
                }

                let sil = render_silhouette(&map, cw, ch, &trial);
                let score = correlate(&screenshot_small, &sil, cw, ch);

                if score > best_score {
                    best_score = score;
                    best_params = trial;
                    improved = true;
                    println!(
                        "  [{iteration}] score={best_score:.4}, pitch={:.1}°, yaw={:.1}°, fov={:.1}°, dist={:.0}, look=({:.1}, {:.1})",
                        best_params.pitch.to_degrees(),
                        best_params.yaw.to_degrees(),
                        best_params.fov_y.to_degrees(),
                        best_params.distance,
                        best_params.look_at.x,
                        best_params.look_at.y,
                    );
                }
            }
        }
        if !improved {
            // Reduce step sizes.
            for s in &mut step_sizes {
                *s *= 0.7;
            }
            if step_sizes.iter().all(|&s| s < 0.1) {
                println!("Converged at iteration {iteration}");
                break;
            }
        }
    }

    println!("\n=== Best parameters ===");
    println!("Pitch: {:.1}°", best_params.pitch.to_degrees());
    println!("FOV Y: {:.1}°", best_params.fov_y.to_degrees());
    println!("Distance: {:.1}", best_params.distance);
    println!(
        "Look at: ({:.1}, {:.1})",
        best_params.look_at.x, best_params.look_at.y
    );
    println!("Score: {best_score:.4}");

    // Render the best match for visual inspection.
    let best_silhouette = render_silhouette(&map, sw, sh, &best_params);
    let sil_img = image::GrayImage::from_raw(sw as u32, sh as u32, best_silhouette).unwrap();
    sil_img.save("calibration_projected.png").unwrap();
    println!("Saved projected silhouette to calibration_projected.png");

    // Render overlay: screenshot mask in red, silhouette in green, overlap in yellow.
    let mut overlay = image::RgbImage::new(sw as u32, sh as u32);
    let best_sil_full = render_silhouette(&map, sw, sh, &best_params);
    for y in 0..sh {
        for x in 0..sw {
            let idx = y * sw + x;
            let s = screenshot_mask[idx];
            let p = best_sil_full[idx];
            overlay.put_pixel(x as u32, y as u32, image::Rgb([s, p, 0]));
        }
    }
    overlay.save("calibration_overlay.png").unwrap();
    println!("Saved overlay to calibration_overlay.png");
}
