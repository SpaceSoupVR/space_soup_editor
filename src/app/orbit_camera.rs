//! Pivoted orbit camera used by the Grab Pose Editor's isolated viewport.
//! Unlike `EditCamera` (deliberately pivot-less free-fly, for the main
//! scene view), this camera always looks at `target` — the object being
//! edited sits at `Vec3::ZERO` in that isolated view, so `target` stays at
//! the origin unless the user pans.

use glam::{EulerRot, Quat, Vec3};

pub(crate) struct OrbitCamera {
    pub target: Vec3,
    pub distance: f32,
    pub yaw: f32,
    pub pitch: f32,
}

impl OrbitCamera {
    /// `framing_radius` should roughly cover the object's bounding size —
    /// the starting distance is sized off it so the object opens up already
    /// framed in view, per "display the object centered in front of the
    /// camera."
    pub fn new(framing_radius: f32) -> Self {
        Self {
            target: Vec3::ZERO,
            distance: (framing_radius * 3.0).max(0.5),
            yaw: 0.6,
            pitch: -0.3,
        }
    }

    pub fn rotation(&self) -> Quat {
        Quat::from_euler(EulerRot::YXZ, self.yaw, self.pitch, 0.0)
    }

    pub fn eye_position(&self) -> Vec3 {
        self.target - self.rotation() * Vec3::NEG_Z * self.distance
    }

    /// Click-drag — rotates the camera around `target`.
    pub fn orbit(&mut self, dx: f32, dy: f32) {
        self.yaw -= dx * 0.006;
        self.pitch = (self.pitch - dy * 0.006).clamp(-1.55, 1.55);
    }

    /// Two-finger trackpad drag / middle-mouse-drag — moves the pivot in
    /// the camera's local right/up plane.
    pub fn pan(&mut self, dx: f32, dy: f32) {
        let rot = self.rotation();
        let right = rot * Vec3::X;
        let up = rot * Vec3::Y;
        let speed = self.distance * 0.0015;
        self.target += -right * dx * speed + up * dy * speed;
    }

    /// Scroll / pinch — moves the camera toward or away from `target`.
    pub fn zoom(&mut self, d: f32) {
        self.distance = (self.distance + d).clamp(0.15, 50.0);
    }
}
