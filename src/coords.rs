//! Type-safe coordinate spaces.
//!
//! Prevents accidentally passing a world-space position where a pixel-space
//! position is expected, or vice versa.

use std::fmt;
use std::ops::{Add, Sub};

use glam::{IVec2, Vec2};

/// A discrete hex grid position in axial coordinates.
/// x → 2 o'clock, y → north.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct HexPos(pub IVec2);

impl HexPos {
    pub const ZERO: Self = Self(IVec2::ZERO);

    pub fn new(x: i32, y: i32) -> Self {
        Self(IVec2::new(x, y))
    }

    pub fn x(self) -> i32 {
        self.0.x
    }

    pub fn y(self) -> i32 {
        self.0.y
    }
}

impl Add for HexPos {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
    }
}

impl Sub for HexPos {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self(self.0 - rhs.0)
    }
}

impl fmt::Display for HexPos {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {})", self.0.x, self.0.y)
    }
}

/// A position in the top-down hex world coordinate system.
/// Produced by `hex_to_world`, consumed by camera projections.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WorldPos(pub Vec2);

/// A position in normalized screen coordinates (0..1, 0..1).
/// (0,0) = top-left, (1,1) = bottom-right.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScreenPos(pub Vec2);

/// A position in window pixel coordinates.
/// (0,0) = top-left, increases right and down.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PixelPos(pub Vec2);

// --- WorldPos ops ---

impl WorldPos {
    pub const ZERO: Self = Self(Vec2::ZERO);

    pub fn new(x: f32, y: f32) -> Self {
        Self(Vec2::new(x, y))
    }

    pub fn x(self) -> f32 {
        self.0.x
    }

    pub fn y(self) -> f32 {
        self.0.y
    }
}

impl Add for WorldPos {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
    }
}

impl Sub for WorldPos {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self(self.0 - rhs.0)
    }
}

// --- ScreenPos ops ---

impl ScreenPos {
    pub fn new(x: f32, y: f32) -> Self {
        Self(Vec2::new(x, y))
    }
}

// --- PixelPos ops ---

impl PixelPos {
    pub fn new(x: f32, y: f32) -> Self {
        Self(Vec2::new(x, y))
    }

    pub fn x(self) -> f32 {
        self.0.x
    }

    pub fn y(self) -> f32 {
        self.0.y
    }
}

impl Add for PixelPos {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
    }
}

impl Sub for PixelPos {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self(self.0 - rhs.0)
    }
}

// --- CameraMode ---

/// How the solver's camera couples with the game's camera.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CameraMode {
    /// No game camera coupling.
    Off,
    /// Read camera_pos.txt, render trapezoid on the map.
    TrackGame,
    /// Bidirectional: game camera tracks solver and vice versa.
    Duplex,
}

// --- UnityCameraPos ---

/// The 3D camera position in Unity world space.
/// x = east/west, y = height above ground, z = north/south.
/// Written by the hardpatched game to camera_pos.txt.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct UnityCameraPos {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// Full camera state read from camera_pos.txt.
#[derive(Clone, Copy, Debug)]
pub struct UnityCameraState {
    pub pos: UnityCameraPos,
    /// Pitch in degrees from horizontal (e.g. 33°).
    pub pitch_deg: f32,
    /// Yaw in degrees (0° = default, increases clockwise).
    pub yaw_deg: f32,
    /// Vertical field of view in degrees (e.g. 30°).
    pub fov_deg: f32,
    /// CameraAnchor local Z position (negative = zoom distance, default -10).
    pub anchor_z: f32,
}

impl UnityCameraState {
    /// Parse "x y z rotX rotY rotZ fov anchorZ" from camera_pos.txt.
    pub fn parse(s: &str) -> Option<Self> {
        let parts: Vec<f32> = s
            .split_whitespace()
            .filter_map(|p| p.parse().ok())
            .collect();
        if parts.len() < 7 {
            return None;
        }
        Some(Self {
            pos: UnityCameraPos {
                x: parts[0],
                y: parts[1],
                z: parts[2],
            },
            pitch_deg: parts[3],
            yaw_deg: parts[4],
            fov_deg: parts[6],
            anchor_z: if parts.len() >= 8 { parts[7] } else { -10.0 },
        })
    }

    /// Compute the ground-plane look-at point in our 2D world coordinates.
    /// Unity X = WorldPos.x, Unity Z = WorldPos.y.
    /// The camera looks down at `pitch_deg` from horizontal, rotated by `yaw_deg`.
    pub fn look_at(&self) -> WorldPos {
        let pitch = self.pitch_deg.to_radians();
        let yaw = self.yaw_deg.to_radians();

        let sin_pitch = pitch.sin();
        if sin_pitch.abs() < 0.001 {
            return WorldPos::new(self.pos.x, self.pos.z);
        }

        let view_dist = self.pos.y / sin_pitch;
        let horiz_dist = pitch.cos() * view_dist;

        // Default look direction (yaw=0): camera is behind (negative Z) the target,
        // looking forward (+Z). So the look-at point is AHEAD of the camera.
        let look_x = self.pos.x + yaw.sin() * horiz_dist;
        let look_z = self.pos.z + yaw.cos() * horiz_dist;

        WorldPos::new(look_x, look_z)
    }

    /// The view distance along the camera's look axis (from camera to ground).
    pub fn view_distance(&self) -> f32 {
        let sin_pitch = self.pitch_deg.to_radians().sin();
        if sin_pitch.abs() < 0.001 {
            return self.pos.y;
        }
        self.pos.y / sin_pitch
    }
}

impl UnityCameraPos {
    /// Compute the Unity camera position for a given ground-plane target.
    /// Inverse of `UnityCameraState::look_at`.
    pub fn from_look_at(
        target: WorldPos,
        pitch_deg: f32,
        yaw_deg: f32,
        view_distance: f32,
    ) -> Self {
        let pitch = pitch_deg.to_radians();
        let yaw = yaw_deg.to_radians();

        let height = pitch.sin() * view_distance;
        let horiz_dist = pitch.cos() * view_distance;

        let cam_x = target.x() + yaw.sin() * horiz_dist;
        let cam_z = target.y() + yaw.cos() * horiz_dist;

        Self {
            x: cam_x,
            y: height,
            z: cam_z,
        }
    }

    /// Format as "x y z" for camera_set.txt.
    pub fn to_set_string(self) -> String {
        format!("{:.4} {:.4} {:.4}", self.x, self.y, self.z)
    }
}
