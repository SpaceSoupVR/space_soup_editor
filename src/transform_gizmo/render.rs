use glam::Vec3;

use space_soup::renderer::{Camera, MeshInstance};

use super::assets::GizmoAssets;
use super::geometry::all_axes_for;
use super::math::billboard_rotation;
use super::{Axis, GizmoMode, TransformGizmo};

impl TransformGizmo {
    pub(crate) fn screen_scale(&self, camera: &Camera, viewport: (f32, f32)) -> f32 {
        const TARGET_PIXELS: f32 = 90.0;
        const MIN_WORLD_SCALE: f32 = 0.5;
        const MAX_WORLD_SCALE: f32 = 3.0;

        let dist = (self.position - camera.position).length().max(0.01);

        let inv_tan_half_fov = camera.projection().y_axis.y;
        let pixels_per_world_unit_at_dist = inv_tan_half_fov * (viewport.1 / 2.0) / dist;

        (TARGET_PIXELS / pixels_per_world_unit_at_dist).clamp(MIN_WORLD_SCALE, MAX_WORLD_SCALE)
    }

    pub fn collect_mesh_instances<'a>(
        &self,
        assets: &'a mut GizmoAssets,
        camera: &Camera,
        viewport: (f32, f32),
    ) -> Vec<MeshInstance<'a>> {
        let scale = self.screen_scale(camera, viewport);
        let basis = self.basis();

        for &axis in all_axes_for(self.mode) {
            let state = self.state_for(axis);
            if let Some((gltf, _)) = assets.parts.get_mut(&(self.mode, axis, state)) {
                gltf.position = self.position;
                gltf.scale = Vec3::splat(scale);
                gltf.rotation = if axis == Axis::XYZ && self.mode != GizmoMode::Scale {
                    billboard_rotation(self.position, camera.position)
                } else {
                    basis
                };
            }
        }

        all_axes_for(self.mode)
            .iter()
            .filter_map(|&axis| {
                let state = self.state_for(axis);
                let (gltf, model) = assets.parts.get(&(self.mode, axis, state))?;
                Some(MeshInstance { mesh: gltf, model })
            })
            .collect()
    }
}
