//! Pure math helpers with no GPU or App dependency.

use glam::{Quat, Vec2, Vec3};

use space_soup::renderer::Camera;

pub(crate) fn normalize3(p: [f32; 3]) -> [f32; 3] {
    let len = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt().max(1e-8);
    [p[0] / len, p[1] / len, p[2] / len]
}

/// Rotates (x,y,z) so the base shape's +Z axis ends up pointing along +X.
/// Derived from the standard right-handed R_y(+90°) rotation matrix.
pub(crate) fn rot_y90(p: [f32; 3]) -> [f32; 3] {
    [p[2], p[1], -p[0]]
}

/// Rotates (x,y,z) so the base shape's +Z axis ends up pointing along +Y.
/// Derived from the standard right-handed R_x(-90°) rotation matrix.
pub(crate) fn rot_x_neg90(p: [f32; 3]) -> [f32; 3] {
    [p[0], p[2], -p[1]]
}

pub(crate) fn screen_ray(camera: &Camera, viewport: (f32, f32), mouse: Vec2) -> (Vec3, Vec3) {
    let view_proj = camera.projection() * camera.view();
    let inv = view_proj.inverse();
    let ndc_x = (mouse.x / viewport.0) * 2.0 - 1.0;
    let ndc_y = 1.0 - (mouse.y / viewport.1) * 2.0;
    let near4 = inv * glam::Vec4::new(ndc_x, ndc_y, -1.0, 1.0);
    let far4 = inv * glam::Vec4::new(ndc_x, ndc_y, 1.0, 1.0);
    let near = Vec3::new(near4.x, near4.y, near4.z) / near4.w;
    let far = Vec3::new(far4.x, far4.y, far4.z) / far4.w;
    (near, (far - near).normalize_or_zero())
}

pub(crate) fn project_to_screen(camera: &Camera, viewport: (f32, f32), world: Vec3) -> Option<Vec2> {
    let view_proj = camera.projection() * camera.view();
    let clip = view_proj * glam::Vec4::new(world.x, world.y, world.z, 1.0);
    if clip.w <= 0.0001 {
        return None; // behind the camera
    }
    let ndc_x = clip.x / clip.w;
    let ndc_y = clip.y / clip.w;
    let sx = (ndc_x * 0.5 + 0.5) * viewport.0;
    let sy = (1.0 - (ndc_y * 0.5 + 0.5)) * viewport.1;
    Some(Vec2::new(sx, sy))
}

pub(crate) fn ray_plane_intersect(origin: Vec3, dir: Vec3, plane_point: Vec3, normal: Vec3) -> Option<Vec3> {
    let denom = dir.dot(normal);
    if denom.abs() < 1e-6 {
        return None;
    }
    let t = (plane_point - origin).dot(normal) / denom;
    if t < 0.0 {
        return None;
    }
    Some(origin + dir * t)
}

/// Closest point *on the gizmo's axis line* to the camera ray through the
/// mouse — the standard way professional tools turn a 2D drag into 1D
/// motion along a 3D axis.
pub(crate) fn closest_point_on_line_to_ray(
    line_point: Vec3,
    line_dir: Vec3,
    ray_origin: Vec3,
    ray_dir: Vec3,
) -> Option<Vec3> {
    let a = line_dir.normalize_or_zero();
    let b = ray_dir.normalize_or_zero();
    let r = line_point - ray_origin;
    let ab = a.dot(b);
    let denom = 1.0 - ab * ab;
    if denom.abs() < 1e-6 {
        // Axis points ~straight at the camera — fall back to a
        // camera-facing plane through the line point.
        return ray_plane_intersect(ray_origin, ray_dir, line_point, b);
    }
    let t = (ab * r.dot(b) - r.dot(a)) / denom;
    Some(line_point + a * t)
}

pub(crate) fn perpendicular_basis(n: Vec3) -> (Vec3, Vec3) {
    let n = n.normalize_or_zero();
    let helper = if n.x.abs() < 0.9 { Vec3::X } else { Vec3::Y };
    let u = n.cross(helper).normalize_or_zero();
    let v = n.cross(u);
    (u, v)
}

pub(crate) fn dist_point_segment(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let len_sq = ab.length_squared().max(1e-6);
    let t = ((p - a).dot(ab) / len_sq).clamp(0.0, 1.0);
    (p - (a + ab * t)).length()
}

pub(crate) fn point_in_quad(p: Vec2, q: &[Vec2]) -> bool {
    let mut sign = 0.0_f32;
    for i in 0..4 {
        let a = q[i];
        let b = q[(i + 1) % 4];
        let cross = (b - a).perp_dot(p - a);
        if sign == 0.0 {
            sign = cross.signum();
        } else if cross != 0.0 && cross.signum() != sign {
            return false;
        }
    }
    true
}

pub(crate) fn billboard_rotation(from: Vec3, to: Vec3) -> Quat {
    let dir = (to - from).normalize_or_zero();
    Quat::from_rotation_arc(Vec3::Z, dir)
}