//! Free-fly edit camera — position + look direction only. There is
//! deliberately no orbit target/pivot: "center" was removed so the camera
//! can go anywhere and turn a full 360° without anything to orbit around.

use glam::{EulerRot, Quat, Vec3};

pub(crate) struct EditCamera {
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
}

impl EditCamera {
    pub fn new(position: Vec3) -> Self {
        Self { position, yaw: 0.0, pitch: -0.25 }
    }

    pub fn rotation(&self) -> Quat {
        Quat::from_euler(EulerRot::YXZ, self.yaw, self.pitch, 0.0)
    }

    pub fn forward(&self) -> Vec3 {
        self.rotation() * Vec3::NEG_Z
    }

    /// World-space horizontal right vector, derived from yaw only (not
    /// pitch) so panning stays level instead of tilting with the camera.
    pub fn right(&self) -> Vec3 {
        Vec3::new(self.yaw.cos(), 0.0, -self.yaw.sin())
    }

    /// Click-drag (mouse) or the gizmo's orbit-ring icon — rotates the
    /// camera in place. Yaw is unconstrained (full 360°); pitch is clamped
    /// just shy of straight up/down to avoid a gimbal flip.
    pub fn look(&mut self, dx: f32, dy: f32) {
        self.yaw -= dx * 0.006;
        // Drag up -> look up: cursor-up means dy is negative, and positive
        // pitch is "look up", so pitch moves opposite to dy.
        self.pitch = (self.pitch - dy * 0.006).clamp(-1.55, 1.55);
    }

    /// Two-finger trackpad drag, or the gizmo's hand icon — strafes the
    /// camera left/right and moves it up/down in world space.
    pub fn pan(&mut self, dx: f32, dy: f32) {
        let right = self.right();
        let speed = 0.004;
        // Direct mapping: drag right -> moves right, drag down -> moves down.
        self.position += right * dx * speed - Vec3::Y * dy * speed;
    }

    /// Pinch, Cmd/Ctrl + swipe, or the gizmo's magnifier icon — moves the
    /// camera forward/backward along its current view direction.
    pub fn dolly(&mut self, d: f32) {
        self.position += self.forward() * d;
    }
}
