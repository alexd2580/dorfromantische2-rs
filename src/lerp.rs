use std::ops::Deref;

use glam::Vec2;

pub trait Lerp {
    fn lerp(source: Self, target: Self, mix: f32) -> Self;
}

impl Lerp for Vec2 {
    fn lerp(source: Self, target: Self, mix: f32) -> Self {
        source.lerp(target, mix)
    }
}

impl Lerp for f32 {
    fn lerp(source: Self, target: Self, mix: f32) -> Self {
        source + (target - source) * mix
    }
}

pub struct Interpolated<T> {
    value: T,
    source: T,
    pub target: T,
    mix: f32,
}

impl<T> Deref for Interpolated<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T: Lerp + Copy> Interpolated<T> {
    pub fn new(value: T) -> Self {
        Self {
            value,
            source: value,
            target: value,
            mix: 1.0,
        }
    }

    fn smoothstep(x: f32) -> f32 {
        -2.0 * x.powi(3) + 3.0 * x.powi(2)
    }

    pub fn tick(&mut self) {
        self.mix = 1f32.min(self.mix + 1.0 / 60.0);
        self.value = Lerp::lerp(self.source, self.target, Self::smoothstep(self.mix));
    }

    pub fn set(&mut self, new_value: T) {
        self.value = new_value;
        self.target = new_value;
        self.mix = 1.0;
    }

    pub fn set_target(&mut self, new_target: T) {
        self.source = self.value;
        self.target = new_target;
        self.mix = 0.0;
    }
}
