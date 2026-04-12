use glam::{UVec2, Vec2};

use crate::{
    coords::{PixelPos, WorldPos},
    data::HexPos,
    hex::COS_30,
    map::Map,
};

use super::lerp::Interpolated;

pub const MIN_ZOOM: f32 = 5.0;
pub const MAX_ZOOM: f32 = 500.0;
pub const DEFAULT_ZOOM: f32 = 20.0;
pub const GOTO_ZOOM: f32 = 70.0;

pub struct Camera {
    /// Current origin coordinates (center of screen).
    pub origin: Interpolated<Vec2>,
    /// Rotation relative to origin (TODO unused currently).
    pub rotation: f32,
    /// Current world size (how many tiles are visible).
    pub inv_scale: Interpolated<f32>,
    /// Size of the window.
    pub size: UVec2,
    /// Aspect ratio of the window.
    pub aspect_ratio: f32,
    /// Fraction of window width that is visible (not covered by sidebar/panels).
    pub visible_fraction: Vec2,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            origin: Interpolated::new(Vec2::ZERO),
            rotation: 0.0,
            inv_scale: Interpolated::new(DEFAULT_ZOOM),
            size: UVec2::ZERO,
            aspect_ratio: 0.0,
            visible_fraction: Vec2::ONE,
        }
    }
}

impl Camera {
    pub fn resize(&mut self, size: UVec2) {
        self.size = size;
        let f32_size = size.as_vec2();
        self.aspect_ratio = f32_size.x / f32_size.y;
    }

    pub fn on_scroll(&mut self, y: f32, mouse: PixelPos) {
        let world_before = self.pixel_to_world(mouse);
        let new_scale = (*self.inv_scale - y * 5.0).clamp(MIN_ZOOM, MAX_ZOOM);
        self.inv_scale.set(new_scale);
        let world_after = self.pixel_to_world(mouse);
        // Shift origin so the world point under the mouse stays fixed.
        self.origin
            .set(*self.origin + world_before.0 - world_after.0);
    }

    pub fn goto(&mut self, pos: HexPos) {
        self.goto_world(crate::hex::hex_to_world(pos));
    }

    pub fn goto_world(&mut self, world: WorldPos) {
        let sidebar_offset = (1.0 - self.visible_fraction.x) * 0.5 * self.aspect_ratio * GOTO_ZOOM;
        self.origin
            .set_target(world.0 - Vec2::new(sidebar_offset, 0.0));
        self.inv_scale.set_target(GOTO_ZOOM);
    }

    pub fn zoom_fit(&mut self, map: &Map) {
        use crate::hex::hex_to_world;

        let offset = map.index_offset;
        let size = map.index_size;

        // Compute world-space bounding box from all occupied tiles.
        let mut world_min = Vec2::new(f32::MAX, f32::MAX);
        let mut world_max = Vec2::new(f32::MIN, f32::MIN);
        let mut any = false;
        for (key, entry) in map.tile_index.iter().enumerate() {
            if entry.is_none() {
                continue;
            }
            let col = key as i32 % size.x + offset.x;
            let row = key as i32 / size.x + offset.y;
            let w = hex_to_world(HexPos::new(col, row)).0;
            world_min = world_min.min(w);
            world_max = world_max.max(w);
            any = true;
        }
        if !any {
            return;
        }
        // 2-tile margin.
        world_min -= Vec2::new(3.0, 2.0 * 2.0 * COS_30);
        world_max += Vec2::new(3.0, 2.0 * 2.0 * COS_30);

        let world_center = (world_min + world_max) * 0.5;
        let world_size = world_max - world_min;

        // Visible world = inv_scale * (aspect, 1). Sidebar/panel reduce usable area.
        let effective_aspect = self.aspect_ratio * self.visible_fraction.x;
        let fit_width = world_size.x / effective_aspect;
        let fit_height = world_size.y / self.visible_fraction.y;
        let inv_scale = fit_width.max(fit_height);

        // Shift center to account for sidebar (left) and navbar (top).
        let sidebar_offset = (1.0 - self.visible_fraction.x) * 0.5 * self.aspect_ratio * inv_scale;
        let navbar_offset = (1.0 - self.visible_fraction.y) * 0.5 * inv_scale;
        self.origin
            .set_target(world_center - Vec2::new(sidebar_offset, -navbar_offset));
        self.inv_scale.set_target(inv_scale);
    }

    /// Compute world coordinates of pixel.
    pub fn pixel_to_world(&self, pos: PixelPos) -> WorldPos {
        let relative = pos.0 / self.size.as_vec2();
        let uv_2 = Vec2::new(1.0, -1.0) * (relative - 0.5);
        WorldPos(*self.origin + uv_2 * Vec2::new(self.aspect_ratio, 1.0) * *self.inv_scale)
    }

    /// Compute pixel coordinates of world position.
    pub fn world_to_pixel(&self, world: WorldPos) -> PixelPos {
        let uv_2 = (world.0 - *self.origin) / (*self.inv_scale * Vec2::new(self.aspect_ratio, 1.0));
        PixelPos((uv_2 * Vec2::new(1.0, -1.0) + 0.5) * self.size.as_vec2())
    }

    /// Compute pixel coordinates of hex position.
    pub fn hex_to_pixel(&self, pos: HexPos) -> PixelPos {
        self.world_to_pixel(crate::hex::hex_to_world(pos))
    }

    /// Convert a world-space distance to pixel-space distance.
    pub fn world_dist_to_pixels(&self, dist: f32) -> f32 {
        dist * self.size.y as f32 / *self.inv_scale
    }

    pub fn tick(&mut self) {
        self.origin.tick();
        self.inv_scale.tick();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_camera_default() {
        let cam = Camera::default();
        assert_eq!(cam.size, UVec2::ZERO);
        assert_eq!(cam.rotation, 0.0);
        assert_eq!(cam.aspect_ratio, 0.0);
        assert_eq!(cam.visible_fraction, Vec2::ONE);
        assert_eq!(*cam.inv_scale, DEFAULT_ZOOM);
        assert_eq!(*cam.origin, Vec2::ZERO);
    }

    #[test]
    fn test_resize_updates_aspect_ratio() {
        let mut cam = Camera::default();
        cam.resize(UVec2::new(1920, 1080));
        assert_eq!(cam.size, UVec2::new(1920, 1080));
        let expected = 1920.0_f32 / 1080.0;
        assert!((cam.aspect_ratio - expected).abs() < 1e-5);
    }

    #[test]
    fn test_pixel_world_roundtrip() {
        let mut cam = Camera::default();
        cam.resize(UVec2::new(800, 600));
        cam.inv_scale.set(50.0);

        let pixel = PixelPos::new(300.0, 200.0);
        let world = cam.pixel_to_world(pixel);
        let back = cam.world_to_pixel(world);
        assert!(
            (pixel.0 - back.0).length() < 0.01,
            "roundtrip failed: {pixel:?} -> {world:?} -> {back:?}"
        );
    }

    #[test]
    fn test_on_scroll_clamps() {
        let mut cam = Camera::default();
        cam.resize(UVec2::new(1280, 720));
        cam.inv_scale.set(DEFAULT_ZOOM);
        let center = PixelPos::new(640.0, 360.0);

        for _ in 0..200 {
            cam.on_scroll(1.0, center);
        }
        assert!(
            *cam.inv_scale >= MIN_ZOOM,
            "zoom went below min: {}",
            *cam.inv_scale
        );

        for _ in 0..400 {
            cam.on_scroll(-1.0, center);
        }
        assert!(
            *cam.inv_scale <= MAX_ZOOM,
            "zoom went above max: {}",
            *cam.inv_scale
        );
    }
}
