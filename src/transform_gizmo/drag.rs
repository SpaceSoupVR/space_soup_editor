use glam::{Quat, Vec2, Vec3};

use space_soup::renderer::Camera;

use super::math::{closest_point_on_line_to_ray, perpendicular_basis, ray_plane_intersect, screen_ray};
use super::{Axis, DragState, GizmoMode, TransformGizmo};

impl TransformGizmo {
    pub fn begin_drag(&mut self, axis: Axis, mouse: Vec2, camera: &Camera, viewport: (f32, f32)) {
        self.selected_axis = Some(axis);
        self.current_angle_deg = None;
        match self.mode {
            GizmoMode::Translate => {
                let hit = self.translate_drag_point(axis, mouse, camera, viewport).unwrap_or(self.position);
                self.drag = Some(DragState::Translate { anchor_offset: hit - self.position });
            }
            GizmoMode::Rotate => {
                let axis_dir = match axis {
                    Axis::X => self.basis() * Vec3::X,
                    Axis::Y => self.basis() * Vec3::Y,
                    _ => self.basis() * Vec3::Z,
                };
                let (u, v) = perpendicular_basis(axis_dir);
                let start_angle = self.angle_on_plane(mouse, camera, viewport, u, v);
                self.drag = Some(DragState::Rotate { axis, start_rot: self.rotation, start_angle, plane_u: u, plane_v: v });
            }
            GizmoMode::Scale => {
                self.drag = Some(DragState::Scale { axis, start_scale: self.scale, start_mouse: mouse });
            }
        }
    }

    pub fn drag(&mut self, mouse: Vec2, camera: &Camera, viewport: (f32, f32)) {
        let Some(state) = &self.drag else { return };
        match *state {
            DragState::Translate { anchor_offset } => {
                let Some(axis) = self.selected_axis else { return };
                if let Some(hit) = self.translate_drag_point(axis, mouse, camera, viewport) {
                    self.position = hit - anchor_offset;
                }
            }
            DragState::Rotate { axis, start_rot, start_angle, plane_u, plane_v, .. } => {
                let angle = self.angle_on_plane(mouse, camera, viewport, plane_u, plane_v);
                let delta = angle - start_angle;
                self.current_angle_deg = Some(delta.to_degrees());
                let axis_dir = match axis {
                    Axis::X => self.basis() * Vec3::X,
                    Axis::Y => self.basis() * Vec3::Y,
                    _ => self.basis() * Vec3::Z,
                };
                self.rotation = Quat::from_axis_angle(axis_dir, delta) * start_rot;
            }
            DragState::Scale { axis, start_scale, start_mouse } => {
                let delta_px = mouse.y - start_mouse.y; // drag up = scale up
                let factor = (1.0 - delta_px * 0.005).max(0.01);
                self.scale = match axis {
                    Axis::X => Vec3::new(start_scale.x * factor, start_scale.y, start_scale.z),
                    Axis::Y => Vec3::new(start_scale.x, start_scale.y * factor, start_scale.z),
                    Axis::Z => Vec3::new(start_scale.x, start_scale.y, start_scale.z * factor),
                    _ => start_scale * factor, // XYZ = uniform
                };
            }
        }
    }

    pub fn end_drag(&mut self) {
        self.drag = None;
        self.selected_axis = None;
        self.current_angle_deg = None;
    }

    /// Angle (radians) of the drag ray's plane-intersection point relative to
    /// `plane_u`, measured in the (plane_u, plane_v) basis through the
    /// gizmo's position. Mouse motion around this angle drives rotation.
    fn angle_on_plane(&self, mouse: Vec2, camera: &Camera, viewport: (f32, f32), u: Vec3, v: Vec3) -> f32 {
        let (origin, dir) = screen_ray(camera, viewport, mouse);
        let hit = ray_plane_intersect(origin, dir, self.position, u.cross(v)).unwrap_or(self.position);
        let rel = hit - self.position;
        rel.dot(v).atan2(rel.dot(u))
    }

    fn translate_drag_point(&self, axis: Axis, mouse: Vec2, camera: &Camera, viewport: (f32, f32)) -> Option<Vec3> {
        let (origin, dir) = screen_ray(camera, viewport, mouse);
        match axis {
            Axis::X | Axis::Y | Axis::Z => {
                closest_point_on_line_to_ray(self.position, self.axis_dir(axis), origin, dir)
            }
            Axis::XY | Axis::XZ | Axis::YZ => {
                let b = self.basis();
                let n = match axis {
                    Axis::XY => b * Vec3::Z,
                    Axis::XZ => b * Vec3::Y,
                    _ => b * Vec3::X,
                };
                ray_plane_intersect(origin, dir, self.position, n)
            }
            Axis::XYZ => {
                let n = (camera.position - self.position).normalize_or_zero();
                ray_plane_intersect(origin, dir, self.position, n)
            }
        }
    }
}
