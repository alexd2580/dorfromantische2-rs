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
