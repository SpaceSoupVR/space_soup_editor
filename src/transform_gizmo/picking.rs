use glam::{Vec2, Vec3};

use space_soup::renderer::Camera;

use super::math::{dist_point_segment, point_in_quad, project_to_screen};
use super::{Axis, GizmoMode, TransformGizmo};

impl TransformGizmo {
    pub fn raycast_gizmo(
        &self,
        mouse: Vec2,
        camera: &Camera,
        viewport: (f32, f32),
    ) -> Option<Axis> {
        const PICK_THRESHOLD_PX: f32 = 12.0;
        let scale = self.screen_scale(camera, viewport);
        let origin_screen = project_to_screen(camera, viewport, self.position)?;

        let mut best: Option<(Axis, f32)> = None;
        let mut consider = |axis: Axis, dist: f32| {
            if dist < PICK_THRESHOLD_PX && best.is_none_or(|(_, d)| dist < d) {
                best = Some((axis, dist));
            }
        };

        match self.mode {
            GizmoMode::Translate => {
                let len = 0.8 * scale;
                for axis in [Axis::X, Axis::Y, Axis::Z] {
                    if let Some(tip) = project_to_screen(
                        camera,
                        viewport,
                        self.position + self.axis_dir(axis) * len,
                    ) {
                        consider(axis, dist_point_segment(mouse, origin_screen, tip));
                    }
                }
                for axis in [Axis::XY, Axis::XZ, Axis::YZ] {
                    if let Some(d) = self.plane_pick_distance(axis, scale, mouse, camera, viewport)
                    {
                        consider(axis, d);
                    }
                }
                consider(Axis::XYZ, mouse.distance(origin_screen));
            }
            GizmoMode::Scale => {
                let len = 0.5 * scale;
                for axis in [Axis::X, Axis::Y, Axis::Z] {
                    if let Some(tip) = project_to_screen(
                        camera,
                        viewport,
                        self.position + self.axis_dir(axis) * len,
                    ) {
                        consider(axis, dist_point_segment(mouse, origin_screen, tip));
                    }
                }
                consider(Axis::XYZ, mouse.distance(origin_screen));
            }
            GizmoMode::Rotate => {
                for axis in [Axis::X, Axis::Y, Axis::Z] {
                    if let Some(d) = self.ring_pick_distance(axis, scale, mouse, camera, viewport) {
                        consider(axis, d);
                    }
                }
            }
        }
        best.map(|(a, _)| a)
    }

    fn plane_corners(&self, axis: Axis, scale: f32) -> [Vec3; 4] {
        let b = self.basis();
        let (u, v) = match axis {
            Axis::XY => (b * Vec3::X, b * Vec3::Y),
            Axis::XZ => (b * Vec3::X, b * Vec3::Z),
            _ => (b * Vec3::Y, b * Vec3::Z),
        };
        let center = self.position + (u + v) * 0.24 * scale;
        let h = 0.08 * scale;
        [
            center - u * h - v * h,
            center + u * h - v * h,
            center + u * h + v * h,
            center - u * h + v * h,
        ]
    }

    fn plane_pick_distance(
        &self,
        axis: Axis,
        scale: f32,
        mouse: Vec2,
        camera: &Camera,
        viewport: (f32, f32),
    ) -> Option<f32> {
        let corners = self.plane_corners(axis, scale);
        let screen: Vec<Vec2> = corners
            .iter()
            .filter_map(|&c| project_to_screen(camera, viewport, c))
            .collect();
        if screen.len() < 4 {
            return None;
        }
        if point_in_quad(mouse, &screen) {
            return Some(0.0);
        }
        (0..4)
            .map(|i| dist_point_segment(mouse, screen[i], screen[(i + 1) % 4]))
            .fold(None, |acc: Option<f32>, d| {
                Some(acc.map_or(d, |a| a.min(d)))
            })
    }

    fn ring_pick_distance(
        &self,
        axis: Axis,
        scale: f32,
        mouse: Vec2,
        camera: &Camera,
        viewport: (f32, f32),
    ) -> Option<f32> {
        let b = self.basis();
        let (u, v) = match axis {
            Axis::X => (b * Vec3::Y, b * Vec3::Z),
            Axis::Y => (b * Vec3::Z, b * Vec3::X),
            _ => (b * Vec3::X, b * Vec3::Y),
        };
        let r = 0.85 * scale;
        const N: usize = 32;
        let mut best: Option<f32> = None;
        let mut prev: Option<Vec2> = None;
        for i in 0..=N {
            let a = i as f32 / N as f32 * std::f32::consts::TAU;
            let world = self.position + (u * a.cos() + v * a.sin()) * r;
            if let Some(s) = project_to_screen(camera, viewport, world) {
                if let Some(p) = prev {
                    let d = dist_point_segment(mouse, p, s);
                    best = Some(best.map_or(d, |b| b.min(d)));
                }
                prev = Some(s);
            }
        }
        best
    }
}
