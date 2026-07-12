use glam::{EulerRot, Quat, Vec3};

pub(crate) struct OrbitCamera {
    pub target: Vec3,
    pub distance: f32,
    pub yaw: f32,
    pub pitch: f32,
}

impl OrbitCamera {
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

    /// Snap back onto a subject after panning/zooming away: re-aims the target
    /// and resets the distance to frame it, keeping the current view angle.
    pub fn refocus(&mut self, target: Vec3, framing_radius: f32) {
        self.target = target;
        self.distance = (framing_radius * 3.0).max(0.5);
    }

    pub fn orbit(&mut self, dx: f32, dy: f32) {
        self.yaw -= dx * 0.006;
        self.pitch = (self.pitch - dy * 0.006).clamp(-1.55, 1.55);
    }

    pub fn pan(&mut self, dx: f32, dy: f32) {
        let rot = self.rotation();
        let right = rot * Vec3::X;
        let up = rot * Vec3::Y;
        let speed = self.distance * 0.0015;
        self.target += -right * dx * speed + up * dy * speed;
    }

    pub fn zoom(&mut self, d: f32) {
        self.distance = (self.distance + d).clamp(0.15, 50.0);
    }
}
