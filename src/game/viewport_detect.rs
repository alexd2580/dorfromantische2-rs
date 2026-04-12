//! Viewport detection: locate the game's current view on the map by
//! capturing a screenshot, binarizing it, unprojecting through the game
//! camera, and template-matching against a precomputed map silhouette.

use opencv::core as cv_core;
use opencv::imgproc;

use super::game_camera::GameCamera;
use crate::coords::{ScreenPos, WorldPos};
use crate::data::HexPos;
use crate::hex;

/// How many hex tiles fit across the viewport horizontally.
/// Calibrated empirically from overlay matching.
const TILES_ACROSS: f32 = 61.0;

/// Precomputed map silhouette for template matching.
pub struct MapSilhouette {
    pub image: Vec<u8>,
    pub width: usize,
    pub height: usize,
    pub min_x: f32,
    pub max_y: f32,
    pub pixels_per_unit: f32,
}

impl MapSilhouette {
    /// Render the map as a top-down silhouette with padding for edge viewports.
    pub fn from_positions(positions: &[HexPos]) -> Self {
        let mut min_x = f32::MAX;
        let mut max_x = f32::MIN;
        let mut min_y = f32::MAX;
        let mut max_y = f32::MIN;

        for &pos in positions {
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

        // Extra padding so template can slide beyond map edges.
        let pad = 50.0;
        min_x -= pad;
        max_x += pad;
        min_y -= pad;
        max_y += pad;

        let ppu = 5.49; // pixels per world unit, matching the example
        let w = ((max_x - min_x) * ppu) as usize;
        let h = ((max_y - min_y) * ppu) as usize;
        let mut image = vec![0u8; w * h];

        let r = (1.2 * ppu) as i32;
        let r_sq = r * r;
        for &pos in positions {
            let world = hex::hex_to_world(pos);
            let cx = ((world.x() - min_x) * ppu) as i32;
            let cy = ((max_y - world.y()) * ppu) as i32;
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx * dx + dy * dy <= r_sq {
                        let x = cx + dx;
                        let y = cy + dy;
                        if x >= 0 && x < w as i32 && y >= 0 && y < h as i32 {
                            image[y as usize * w + x as usize] = 255;
                        }
                    }
                }
            }
        }

        Self {
            image,
            width: w,
            height: h,
            min_x,
            max_y,
            pixels_per_unit: ppu,
        }
    }
}

/// Detect the game viewport position from a screenshot.
/// Returns the world-space center of the detected viewport, or None.
/// Result of viewport detection: the detected center and the screenshot dimensions.
pub struct DetectResult {
    pub center: WorldPos,
    pub screen_size: (u32, u32),
}

pub fn detect_viewport(
    screenshot: &image::RgbImage,
    map_sil: &MapSilhouette,
    cam: &GameCamera,
) -> Option<DetectResult> {
    let (sw, sh) = (screenshot.width() as usize, screenshot.height() as usize);
    let screen_size = (sw as u32, sh as u32);
    log::debug!("detect: screenshot {}x{}", sw, sh);

    let mask = binarize_screenshot(screenshot);
    let up = unproject_mask(&mask, sw, sh, cam, screen_size)?;

    let target_w = TILES_ACROSS * 1.5 * map_sil.pixels_per_unit;
    let scale = target_w / up.width as f32;
    let tw = (up.width as f32 * scale) as usize;
    let th = (up.height as f32 * scale) as usize;
    if tw > map_sil.width || th > map_sil.height || tw < 10 || th < 10 {
        return None;
    }

    let scaled = scale_image(&up.image, up.width, up.height, tw, th);
    let smask = scale_image(&up.coverage, up.width, up.height, tw, th);
    let (ox, oy, _score) = find_best_offset(
        &map_sil.image,
        map_sil.width,
        map_sil.height,
        &scaled,
        tw,
        th,
        &smask,
    )?;

    // The viewport center in world space is at cam.look_at = (0, 0) since we
    // zero it before detection. Find where (0, 0) falls in the unprojected
    // template, then map that pixel position to the map silhouette.
    let center_px_in_unproj_x = (0.0 - up.min_x) * up.ppu;
    let center_px_in_unproj_y = (up.max_y - 0.0) * up.ppu;
    // Scale to template size.
    let center_in_tmpl_x = center_px_in_unproj_x * scale;
    let center_in_tmpl_y = center_px_in_unproj_y * scale;

    let cx = map_sil.min_x + (ox as f32 + center_in_tmpl_x) / map_sil.pixels_per_unit;
    let cy = map_sil.max_y - (oy as f32 + center_in_tmpl_y) / map_sil.pixels_per_unit;

    log::debug!(
        "detect: unproj {}x{} min_x={:.1} max_y={:.1} ppu={:.2} | tmpl {}x{} scale={:.3} | \
         center_in_unproj=({:.1},{:.1}) center_in_tmpl=({:.1},{:.1}) | \
         offset=({},{}) | result=({:.1},{:.1})",
        up.width,
        up.height,
        up.min_x,
        up.max_y,
        up.ppu,
        tw,
        th,
        scale,
        center_px_in_unproj_x,
        center_px_in_unproj_y,
        center_in_tmpl_x,
        center_in_tmpl_y,
        ox,
        oy,
        cx,
        cy,
    );

    Some(DetectResult {
        center: WorldPos::new(cx, cy),
        screen_size,
    })
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
            let is_pink = r > 180.0 && g > 140.0 && b > 140.0 && r > g && r > b && (r - g) > 20.0;
            let is_water = r > 160.0 && g > 180.0 && b > 200.0;
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

fn screen_coverage_mask(sw: usize, sh: usize) -> Vec<u8> {
    let mut cov = vec![255u8; sw * sh];
    for y in 0..sh / 8 {
        for x in (sw * 3 / 4)..sw {
            cov[y * sw + x] = 0;
        }
    }
    for y in (sh * 3 / 4)..sh {
        for x in (sw * 3 / 4)..sw {
            cov[y * sw + x] = 0;
        }
    }
    for y in 0..sh / 8 {
        for x in 0..sw / 8 {
            cov[y * sw + x] = 0;
        }
    }
    cov
}

struct UnprojResult {
    image: Vec<u8>,
    coverage: Vec<u8>,
    width: usize,
    height: usize,
    min_x: f32,
    max_y: f32,
    ppu: f32,
}

fn unproject_mask(
    mask: &[u8],
    sw: usize,
    sh: usize,
    cam: &GameCamera,
    screen_size: (u32, u32),
) -> Option<UnprojResult> {
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

    if w == 0 || h == 0 {
        return None;
    }

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

    Some(UnprojResult {
        image: img,
        coverage,
        width: w,
        height: h,
        min_x,
        max_y,
        ppu,
    })
}

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

fn find_best_offset(
    map_img: &[u8],
    mw: usize,
    mh: usize,
    template: &[u8],
    tw: usize,
    th: usize,
    mask: &[u8],
) -> Option<(i32, i32, f32)> {
    if tw == 0 || th == 0 || mw == 0 || mh == 0 || tw > mw || th > mh {
        return None;
    }
    if map_img.len() != mw * mh || template.len() != tw * th || mask.len() != tw * th {
        log::error!("find_best_offset: size mismatch map={}x{}={} got {}, tmpl={}x{}={} got {}, mask got {}",
            mw, mh, mw*mh, map_img.len(), tw, th, tw*th, template.len(), mask.len());
        return None;
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
    .ok()?;
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
    .ok()?;
    Some((min_loc.x, min_loc.y, -min_val as f32))
}
