//! Minimal 3D vector maths. No external dependency so the generator builds
//! from the same lockfile as the plugin.

use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

pub const fn v3(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3 { x, y, z }
}

impl Vec3 {
    pub const ZERO: Vec3 = v3(0.0, 0.0, 0.0);

    pub const fn splat(v: f32) -> Self {
        v3(v, v, v)
    }

    pub fn dot(self, o: Self) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }

    pub fn cross(self, o: Self) -> Self {
        v3(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }

    pub fn length(self) -> f32 {
        self.dot(self).sqrt()
    }

    pub fn normalise(self) -> Self {
        let l = self.length();
        if l > 1e-20 {
            self / l
        } else {
            Self::ZERO
        }
    }

    /// Component-wise product, for tinting a colour by a light.
    pub fn mul(self, o: Self) -> Self {
        v3(self.x * o.x, self.y * o.y, self.z * o.z)
    }

    pub fn lerp(self, o: Self, t: f32) -> Self {
        self + (o - self) * t
    }

    /// Rotation about the Z axis, which is how a knob turns.
    pub fn rotate_z(self, radians: f32) -> Self {
        let (s, c) = radians.sin_cos();
        v3(self.x * c - self.y * s, self.x * s + self.y * c, self.z)
    }

    /// An arbitrary unit vector perpendicular to `self`, for building a basis.
    pub fn any_perpendicular(self) -> Self {
        let a = if self.x.abs() < 0.9 {
            v3(1.0, 0.0, 0.0)
        } else {
            v3(0.0, 1.0, 0.0)
        };
        self.cross(a).normalise()
    }
}

impl Add for Vec3 {
    type Output = Self;
    fn add(self, o: Self) -> Self {
        v3(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}
impl AddAssign for Vec3 {
    fn add_assign(&mut self, o: Self) {
        *self = *self + o;
    }
}
impl Sub for Vec3 {
    type Output = Self;
    fn sub(self, o: Self) -> Self {
        v3(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}
impl Mul<f32> for Vec3 {
    type Output = Self;
    fn mul(self, s: f32) -> Self {
        v3(self.x * s, self.y * s, self.z * s)
    }
}
impl Div<f32> for Vec3 {
    type Output = Self;
    fn div(self, s: f32) -> Self {
        v3(self.x / s, self.y / s, self.z / s)
    }
}
impl Neg for Vec3 {
    type Output = Self;
    fn neg(self) -> Self {
        v3(-self.x, -self.y, -self.z)
    }
}
