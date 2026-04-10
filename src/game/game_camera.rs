//! Game camera projection model.
//!
//! Converts between the game's 3D perspective view and the 2D top-down
//! world coordinates used by the map. Parameters were calibrated by
//! matching a screenshot against the map silhouette.

use glam::Vec2;

/// Projection parameters for the Dorfromantik game camera.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct GameCamera {
    /// Pitch angle in radians (angle from vertical/ground normal).
    pub pitch: f32,
    /// Yaw angle in radians (rotation around vertical axis).
    pub yaw: f32,
    /// Vertical field of view in radians.
    pub fov_y: f32,
    /// Camera distance from look_at point.
    pub distance: f32,
    /// World position the camera is looking at (center of viewport).
    pub look_at: Vec2,
    /// Screenshot resolution.
    pub screen_width: u32,
    pub screen_height: u32,
}

impl Default for GameCamera {
    fn default() -> Self {
        Self {
            // From Dorfromantik Unity scene: CameraParent X-rotation = 33° from horizontal.
            // This code measures pitch from vertical (ground normal), so 90° - 33° = 57°.
            pitch: 57.0_f32.to_radians(),
            yaw: 0.0,
            fov_y: 30.0_f32.to_radians(),
            // Derived from tiles_across=61: width = 61*1.5 = 91.5,
            // distance = width / (2 * tan(fov_y/2) * aspect).
            distance: 96.0,
            look_at: Vec2::ZERO,
            screen_width: 2560,
            screen_height: 1440,
        }
    }
}

#[allow(dead_code)]
impl GameCamera {
    fn aspect(&self) -> f32 {
        self.screen_width as f32 / self.screen_height as f32
    }

    /// Project a world-space (top-down) point to screen-space (0..1, 0..1).
    /// Returns None if the point is behind the camera.
    pub fn world_to_screen(&self, world: Vec2) -> Option<Vec2> {
        let dx = world.x - self.look_at.x;
        let dy = world.y - self.look_at.y;

        // Camera is SOUTH of look_at, looking NORTH (+Y).
        // dy > 0 means further north = further from camera = smaller on screen.
        let cam_depth = self.distance + dy * self.pitch.sin();
        let cam_x = dx;
        // dy > 0 appears higher on screen (toward top).
        let cam_y = dy * self.pitch.cos();

        if cam_depth <= 0.0 {
            return None;
        }

        let half_h = (self.fov_y / 2.0).tan();
        let aspect = self.aspect();
        let screen_x = cam_x / (cam_depth * half_h * aspect);
        let screen_y = cam_y / (cam_depth * half_h);

        // Screen: (0,0)=top-left, (1,1)=bottom-right.
        // North (dy>0) → top of screen → smaller Y.
        Some(Vec2::new(0.5 + screen_x * 0.5, 0.5 - screen_y * 0.5))
    }

    /// Project a world point to pixel coordinates.
    pub fn world_to_pixel(&self, world: Vec2) -> Option<Vec2> {
        let screen = self.world_to_screen(world)?;
        Some(Vec2::new(
            screen.x * self.screen_width as f32,
            screen.y * self.screen_height as f32,
        ))
    }

    /// Unproject a screen-space point (0..1, 0..1) back to world coordinates.
    /// Solves the inverse of the perspective projection.
    pub fn screen_to_world(&self, screen: Vec2) -> Vec2 {
        let half_h = (self.fov_y / 2.0).tan();
        let aspect = self.aspect();

        // Invert screen mapping: screen_x = 0.5 + cam_x/(cam_depth*half_h*aspect) * 0.5
        //                        screen_y = 0.5 - cam_y/(cam_depth*half_h) * 0.5
        let sx = (screen.x - 0.5) * 2.0;
        let sy = -(screen.y - 0.5) * 2.0; // flip Y back

        // From forward projection:
        //   cam_depth = distance + dy * sin(pitch)
        //   cam_y = dy * cos(pitch)
        //   cam_y / (cam_depth * half_h) = sy
        //   dy * cos(pitch) = sy * half_h * (distance + dy * sin(pitch))
        //   dy * cos(pitch) = sy * half_h * distance + sy * half_h * dy * sin(pitch)
        //   dy * (cos(pitch) - sy * half_h * sin(pitch)) = sy * half_h * distance
        let sp = self.pitch.sin();
        let cp = self.pitch.cos();
        let denom = cp - sy * half_h * sp;

        if denom.abs() < 1e-10 {
            return self.look_at;
        }

        let dy = sy * half_h * self.distance / denom;
        let cam_depth = self.distance + dy * sp;
        let dx = sx * cam_depth * half_h * aspect;

        Vec2::new(self.look_at.x + dx, self.look_at.y + dy)
    }

    /// Unproject pixel coordinates to world coordinates.
    pub fn pixel_to_world(&self, pixel: Vec2) -> Vec2 {
        let screen = Vec2::new(
            pixel.x / self.screen_width as f32,
            pixel.y / self.screen_height as f32,
        );
        self.screen_to_world(screen)
    }

    /// Convert a hex position to world then to screen pixel.
    pub fn hex_to_pixel(&self, pos: glam::IVec2) -> Option<Vec2> {
        self.world_to_pixel(crate::hex::hex_to_world(pos))
    }

    /// Convert a screen pixel to the nearest hex position.
    pub fn pixel_to_hex(&self, pixel: Vec2) -> glam::IVec2 {
        crate::hex::world_to_hex(self.pixel_to_world(pixel))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_world_screen_roundtrip_center() {
        let cam = GameCamera::default();
        // The look_at point should project to screen center.
        let screen = cam.world_to_screen(cam.look_at).unwrap();
        assert!((screen.x - 0.5).abs() < 0.01, "x={}", screen.x);
        assert!((screen.y - 0.5).abs() < 0.01, "y={}", screen.y);
    }

    #[test]
    fn test_screen_world_roundtrip() {
        let cam = GameCamera {
            look_at: Vec2::new(100.0, -50.0),
            ..GameCamera::default()
        };

        // Test several screen points.
        for &(sx, sy) in &[(0.3, 0.3), (0.5, 0.5), (0.7, 0.4), (0.2, 0.7)] {
            let screen = Vec2::new(sx, sy);
            let world = cam.screen_to_world(screen);
            let back = cam.world_to_screen(world);
            if let Some(back) = back {
                assert!(
                    (back.x - sx).abs() < 0.01 && (back.y - sy).abs() < 0.01,
                    "Roundtrip failed: ({sx},{sy}) -> ({:.3},{:.3}) -> ({:.3},{:.3})",
                    world.x,
                    world.y,
                    back.x,
                    back.y
                );
            }
        }
    }

    #[test]
    fn test_pixel_roundtrip() {
        let cam = GameCamera {
            look_at: Vec2::new(100.0, -50.0),
            ..GameCamera::default()
        };

        let pixel = Vec2::new(1280.0, 720.0); // center
        let world = cam.pixel_to_world(pixel);
        let back = cam.world_to_pixel(world).unwrap();
        assert!((back.x - pixel.x).abs() < 1.0);
        assert!((back.y - pixel.y).abs() < 1.0);
    }
}
