//! Model of the Dorfromantik game's 3D perspective camera.
//!
//! Dorfromantik renders its hex map from a fixed-pitch perspective camera
//! that looks down at the board from the south. This module replicates
//! that projection mathematically so we can:
//!
//! - **Unproject** a game screenshot back to top-down world coordinates
//!   (used by viewport detection to figure out where the game is looking).
//! - **Project** world positions to screen/pixel positions (used by game
//!   navigation to compute mouse-drag vectors).
//!
//! The camera parameters (pitch, FOV, distance) were extracted from the
//! Dorfromantik Unity scene files and calibrated against screenshots.

use crate::coords::{PixelPos, ScreenPos, WorldPos};
use crate::data::HexPos;

/// From Dorfromantik Unity scene: CameraParent X-rotation = 33° from horizontal.
/// This code measures pitch from vertical (ground normal), so 90° - 33° = 57°.
const PITCH: f32 = 57.0 * std::f32::consts::PI / 180.0;

/// Vertical field of view from the Unity Camera component.
const FOV_Y: f32 = 30.0 * std::f32::consts::PI / 180.0;

/// Camera distance from the look-at point, in world units (1 hex = 1.5 world units wide).
/// Derived from tiles_across=61: width = 61 * 1.5 = 91.5,
/// distance = width / (2 * tan(fov_y/2) * aspect_ratio).
const DISTANCE: f32 = 96.0;

/// A replica of the Dorfromantik game's perspective camera.
///
/// This is NOT the solver's UI camera (that's [`render::camera::Camera`]).
/// It models the actual game's 3D viewpoint so we can convert between
/// what the game shows on screen and the flat hex-grid world the solver
/// works in.
#[derive(Clone, Debug)]
pub struct GameCamera {
    /// Pitch angle in radians (angle from vertical/ground normal).
    pub pitch: f32,
    /// Yaw angle in radians (rotation around vertical axis).
    pub yaw: f32,
    /// Vertical field of view in radians.
    pub fov_y: f32,
    /// Camera distance from look_at point, in world units (1 hex = 1.5 wu wide).
    pub distance: f32,
    /// World position the camera is looking at (center of viewport).
    pub look_at: WorldPos,
}

impl Default for GameCamera {
    fn default() -> Self {
        Self {
            pitch: PITCH,
            yaw: 0.0,
            fov_y: FOV_Y,
            distance: DISTANCE,
            look_at: WorldPos::ZERO,
        }
    }
}

impl GameCamera {
    fn aspect(&self, screen_size: (u32, u32)) -> f32 {
        screen_size.0 as f32 / screen_size.1 as f32
    }

    /// Project a world-space (top-down) point to screen-space (0..1, 0..1).
    /// Returns None if the point is behind the camera.
    pub fn world_to_screen(&self, world: WorldPos, screen_size: (u32, u32)) -> Option<ScreenPos> {
        let dx = world.x() - self.look_at.x();
        let dy = world.y() - self.look_at.y();

        let cam_depth = self.distance + dy * self.pitch.sin();
        let cam_x = dx;
        let cam_y = dy * self.pitch.cos();

        if cam_depth <= 0.0 {
            return None;
        }

        let half_h = (self.fov_y / 2.0).tan();
        let aspect = self.aspect(screen_size);
        let screen_x = cam_x / (cam_depth * half_h * aspect);
        let screen_y = cam_y / (cam_depth * half_h);

        Some(ScreenPos::new(0.5 + screen_x * 0.5, 0.5 - screen_y * 0.5))
    }

    /// Project a world point to pixel coordinates.
    pub fn world_to_pixel(&self, world: WorldPos, screen_size: (u32, u32)) -> Option<PixelPos> {
        let screen = self.world_to_screen(world, screen_size)?;
        Some(PixelPos::new(
            screen.0.x * screen_size.0 as f32,
            screen.0.y * screen_size.1 as f32,
        ))
    }

    /// Unproject a screen-space point (0..1, 0..1) back to world coordinates.
    pub fn screen_to_world(&self, screen: ScreenPos, screen_size: (u32, u32)) -> WorldPos {
        let half_h = (self.fov_y / 2.0).tan();
        let aspect = self.aspect(screen_size);

        let sx = (screen.0.x - 0.5) * 2.0;
        let sy = -(screen.0.y - 0.5) * 2.0;

        let sp = self.pitch.sin();
        let cp = self.pitch.cos();
        let denom = cp - sy * half_h * sp;

        if denom.abs() < 1e-10 {
            return self.look_at;
        }

        let dy = sy * half_h * self.distance / denom;
        let cam_depth = self.distance + dy * sp;
        let dx = sx * cam_depth * half_h * aspect;

        WorldPos::new(self.look_at.x() + dx, self.look_at.y() + dy)
    }

    /// Unproject pixel coordinates to world coordinates.
    pub fn pixel_to_world(&self, pixel: PixelPos, screen_size: (u32, u32)) -> WorldPos {
        let screen = ScreenPos::new(
            pixel.x() / screen_size.0 as f32,
            pixel.y() / screen_size.1 as f32,
        );
        self.screen_to_world(screen, screen_size)
    }

    /// Convert a hex position to world then to screen pixel.
    pub fn hex_to_pixel(&self, pos: HexPos, screen_size: (u32, u32)) -> Option<PixelPos> {
        self.world_to_pixel(crate::hex::hex_to_world(pos), screen_size)
    }

    /// Convert a screen pixel to the nearest hex position.
    pub fn pixel_to_hex(&self, pixel: PixelPos, screen_size: (u32, u32)) -> HexPos {
        crate::hex::world_to_hex(self.pixel_to_world(pixel, screen_size))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_SCREEN: (u32, u32) = (2560, 1440);

    #[test]
    fn test_world_screen_roundtrip_center() {
        let cam = GameCamera::default();
        // The look_at point should project to screen center.
        let screen = cam.world_to_screen(cam.look_at, TEST_SCREEN).unwrap();
        assert!((screen.0.x - 0.5).abs() < 0.01, "x={}", screen.0.x);
        assert!((screen.0.y - 0.5).abs() < 0.01, "y={}", screen.0.y);
    }

    #[test]
    fn test_screen_world_roundtrip() {
        let cam = GameCamera {
            look_at: WorldPos::new(100.0, -50.0),
            ..GameCamera::default()
        };

        // Test several screen points.
        for &(sx, sy) in &[(0.3, 0.3), (0.5, 0.5), (0.7, 0.4), (0.2, 0.7)] {
            let screen = ScreenPos::new(sx, sy);
            let world = cam.screen_to_world(screen, TEST_SCREEN);
            let back = cam.world_to_screen(world, TEST_SCREEN);
            if let Some(back) = back {
                assert!(
                    (back.0.x - sx).abs() < 0.01 && (back.0.y - sy).abs() < 0.01,
                    "Roundtrip failed: ({sx},{sy}) -> ({:.3},{:.3}) -> ({:.3},{:.3})",
                    world.x(),
                    world.y(),
                    back.0.x,
                    back.0.y
                );
            }
        }
    }

    #[test]
    fn test_pixel_roundtrip() {
        let cam = GameCamera {
            look_at: WorldPos::new(100.0, -50.0),
            ..GameCamera::default()
        };

        let pixel = PixelPos::new(1280.0, 720.0); // center
        let world = cam.pixel_to_world(pixel, TEST_SCREEN);
        let back = cam.world_to_pixel(world, TEST_SCREEN).unwrap();
        assert!((back.x() - pixel.x()).abs() < 1.0);
        assert!((back.y() - pixel.y()).abs() < 1.0);
    }

    #[test]
    fn test_pixel_roundtrip_multiple() {
        let cam = GameCamera::default();
        let pixels = [
            PixelPos::new(640.0, 360.0),
            PixelPos::new(200.0, 100.0),
            PixelPos::new(2000.0, 1200.0),
            PixelPos::new(1280.0, 200.0),
        ];
        for pixel in pixels {
            let world = cam.pixel_to_world(pixel, TEST_SCREEN);
            let back = cam.world_to_pixel(world, TEST_SCREEN).unwrap();
            assert!(
                (back.x() - pixel.x()).abs() < 1.0 && (back.y() - pixel.y()).abs() < 1.0,
                "Pixel roundtrip failed: {pixel:?} -> {world:?} -> {back:?}"
            );
        }
    }

    #[test]
    fn test_screen_center_maps_to_look_at() {
        let cam = GameCamera {
            look_at: WorldPos::new(42.0, -17.0),
            ..GameCamera::default()
        };
        let world = cam.screen_to_world(ScreenPos::new(0.5, 0.5), TEST_SCREEN);
        assert!(
            (world.x() - cam.look_at.x()).abs() < 0.01
                && (world.y() - cam.look_at.y()).abs() < 0.01,
            "Screen center should map to look_at, got {world:?} vs {:?}",
            cam.look_at
        );
    }

    #[test]
    fn test_behind_camera_returns_none() {
        let cam = GameCamera::default();
        // A point very far south (negative y) should be behind the camera.
        let far_behind = WorldPos::new(0.0, -500.0);
        assert!(
            cam.world_to_screen(far_behind, TEST_SCREEN).is_none(),
            "Point far behind camera should return None"
        );
    }

    #[test]
    fn test_left_right_symmetry() {
        let cam = GameCamera::default();
        // Two points symmetric about the look_at x-axis.
        let left = WorldPos::new(cam.look_at.x() - 10.0, cam.look_at.y() + 5.0);
        let right = WorldPos::new(cam.look_at.x() + 10.0, cam.look_at.y() + 5.0);

        let sl = cam.world_to_screen(left, TEST_SCREEN).unwrap();
        let sr = cam.world_to_screen(right, TEST_SCREEN).unwrap();

        // Same y coordinate.
        assert!(
            (sl.0.y - sr.0.y).abs() < 1e-4,
            "Symmetric points should have same screen y: {sl:?} vs {sr:?}"
        );
        // Symmetric about screen center x (0.5).
        assert!(
            ((sl.0.x - 0.5) + (sr.0.x - 0.5)).abs() < 1e-4,
            "Symmetric points should be symmetric about center: {sl:?} vs {sr:?}"
        );
    }

    #[test]
    fn test_north_maps_to_top_of_screen() {
        let cam = GameCamera::default();
        // A point north of look_at (positive dy) should have screen y < 0.5.
        let north = WorldPos::new(cam.look_at.x(), cam.look_at.y() + 10.0);
        let screen = cam.world_to_screen(north, TEST_SCREEN).unwrap();
        assert!(
            screen.0.y < 0.5,
            "North (positive dy) should map to top of screen (y < 0.5), got y={}",
            screen.0.y
        );

        // A point south of look_at should have screen y > 0.5.
        let south = WorldPos::new(cam.look_at.x(), cam.look_at.y() - 10.0);
        let screen = cam.world_to_screen(south, TEST_SCREEN).unwrap();
        assert!(
            screen.0.y > 0.5,
            "South (negative dy) should map to bottom of screen (y > 0.5), got y={}",
            screen.0.y
        );
    }

    #[test]
    fn test_trapezoid_wider_at_top() {
        let cam = GameCamera::default();
        // Unproject left and right edges at top of screen vs bottom.
        let top_left = cam.screen_to_world(ScreenPos::new(0.0, 0.1), TEST_SCREEN);
        let top_right = cam.screen_to_world(ScreenPos::new(1.0, 0.1), TEST_SCREEN);
        let top_width = (top_right.x() - top_left.x()).abs();

        let bot_left = cam.screen_to_world(ScreenPos::new(0.0, 0.9), TEST_SCREEN);
        let bot_right = cam.screen_to_world(ScreenPos::new(1.0, 0.9), TEST_SCREEN);
        let bot_width = (bot_right.x() - bot_left.x()).abs();

        assert!(
            top_width > bot_width,
            "Top of screen (far from camera) should be wider: top_width={top_width:.2}, bot_width={bot_width:.2}"
        );
    }

    #[test]
    fn test_world_to_screen_roundtrip_grid() {
        let cam = GameCamera {
            look_at: WorldPos::new(20.0, 30.0),
            ..GameCamera::default()
        };
        // Test a grid of screen coordinates.
        for sx_i in 1..=9 {
            for sy_i in 1..=9 {
                let sx = sx_i as f32 / 10.0;
                let sy = sy_i as f32 / 10.0;
                let screen = ScreenPos::new(sx, sy);
                let world = cam.screen_to_world(screen, TEST_SCREEN);
                if let Some(back) = cam.world_to_screen(world, TEST_SCREEN) {
                    assert!(
                        (back.0.x - sx).abs() < 0.001 && (back.0.y - sy).abs() < 0.001,
                        "Roundtrip failed at ({sx},{sy}): back=({:.4},{:.4})",
                        back.0.x,
                        back.0.y,
                    );
                }
            }
        }
    }
}
