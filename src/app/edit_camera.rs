use glam::{EulerRot, Quat, Vec3};

pub(crate) struct EditCamera {
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
}

impl EditCamera {
    pub fn new(position: Vec3) -> Self {
        Self {
            position,
            yaw: 0.0,
            pitch: -0.25,
        }
    }

    pub fn rotation(&self) -> Quat {
        Quat::from_euler(EulerRot::YXZ, self.yaw, self.pitch, 0.0)
    }

    pub fn forward(&self) -> Vec3 {
        self.rotation() * Vec3::NEG_Z
    }

    pub fn right(&self) -> Vec3 {
        Vec3::new(self.yaw.cos(), 0.0, -self.yaw.sin())
    }

    pub fn look(&mut self, dx: f32, dy: f32) {
        self.yaw -= dx * 0.006;

        self.pitch = (self.pitch - dy * 0.006).clamp(-1.55, 1.55);
    }

    pub fn pan(&mut self, dx: f32, dy: f32) {
        let right = self.right();
        let speed = 0.012 * self.speed_scale();

        self.position += right * dx * speed - Vec3::Y * dy * speed;
    }

    pub fn dolly(&mut self, d: f32) {
        self.position += self.forward() * d * self.speed_scale();
    }

    // Flat pan/dolly speed feels fine near the origin but increasingly
    // sluggish the further the camera flies out, since a fixed
    // units-per-pixel rate never keeps pace with how far away the scene
    // looks — mirrors OrbitCamera's distance-proportional pan speed.
    fn speed_scale(&self) -> f32 {
        self.position.length().max(1.0)
    }
}
