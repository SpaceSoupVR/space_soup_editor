use glam::Vec3;

use super::App;

const CAMERA_FOV_Y_DEG: f32 = 60.0;

impl App {
    pub(crate) fn screen_ray(&self, sx: f32, sy: f32, win_w: f32, win_h: f32) -> (Vec3, Vec3) {
        let ndc_x = (sx / win_w) * 2.0 - 1.0;
        let ndc_y = 1.0 - (sy / win_h) * 2.0;
        let fov_y = CAMERA_FOV_Y_DEG.to_radians();
        let tan_half = (fov_y * 0.5).tan();
        let dir_cam = Vec3::new(
            ndc_x * tan_half * self.camera.aspect,
            ndc_y * tan_half,
            -1.0,
        ).normalize();
        (self.camera.position, self.camera.rotation * dir_cam)
    }

    pub(crate) fn pick_object(&self, sx: f32, sy: f32, w: f32, h: f32) -> Option<String> {
        let (o, d) = self.screen_ray(sx, sy, w, h);
        self.runtime.scene().objects.iter()
            .filter(|ob| !ob.hidden)
            .filter_map(|ob| {
                ray_aabb_hit(o, d, ob.cuboid.position, ob.cuboid.half_size).map(|t| (ob.id.clone(), t))
            })
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .map(|(id, _)| id)
    }
}

pub(crate) fn ray_aabb_hit(origin: Vec3, dir: Vec3, center: Vec3, half: Vec3) -> Option<f32> {
    let min = center - half;
    let max = center + half;
    let mut t_min = f32::NEG_INFINITY;
    let mut t_max = f32::INFINITY;
    for (o, d, lo, hi) in [
        (origin.x, dir.x, min.x, max.x),
        (origin.y, dir.y, min.y, max.y),
        (origin.z, dir.z, min.z, max.z),
    ] {
        if d.abs() < 1e-8 {
            if o < lo || o > hi { return None; }
        } else {
            let (mut t1, mut t2) = ((lo - o) / d, (hi - o) / d);
            if t1 > t2 { std::mem::swap(&mut t1, &mut t2); }
            t_min = t_min.max(t1);
            t_max = t_max.min(t2);
            if t_min > t_max { return None; }
        }
    }
    if t_max < 0.0 { None } else { Some(t_min.max(0.0)) }
}

pub(crate) fn ray_plane_intersect(origin: Vec3, dir: Vec3, plane_y: f32) -> Option<Vec3> {
    if dir.y.abs() < 1e-5 { return None; }
    let t = (plane_y - origin.y) / dir.y;
    if t < 0.0 { return None; }
    Some(origin + dir * t)
}