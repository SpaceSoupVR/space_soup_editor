use glam::Vec3;

use space_soup::renderer::{Camera, MeshInstance};

use super::assets::GizmoAssets;
use super::geometry::all_axes_for;
use super::math::billboard_rotation;
use super::{Axis, GizmoMode, TransformGizmo};

impl TransformGizmo {
    /// Computes a world-space scale that keeps the gizmo's on-screen size
    /// constant (~TARGET_PIXELS tall) regardless of camera distance — the
    /// standard "constant screen size" trick used by Unity/Blender gizmos,
    /// since handles that scaled with world distance would become
    /// unusably tiny far away or unusably huge up close.
    ///
    /// FIX: the original version had no floor or ceiling on the resulting
    /// world-space scale. As `dist -> 0` (camera right on top of the
    /// object), the computed world scale shrinks toward zero too — the
    /// gizmo stays pinned at exactly TARGET_PIXELS on screen, but its
    /// *world-space* footprint becomes smaller than the object itself,
    /// so it visually reads as "buried inside the model." Clamping the
    /// result keeps handles usable at both extremes.
    pub(crate) fn screen_scale(&self, camera: &Camera, viewport: (f32, f32)) -> f32 {
        const TARGET_PIXELS: f32 = 90.0;
        const MIN_WORLD_SCALE: f32 = 0.5;
        const MAX_WORLD_SCALE: f32 = 3.0;

        let dist = (self.position - camera.position).length().max(0.01);

        // projection().y_axis.y == 1/tan(fov_y/2) for a standard perspective_rh matrix.
        // Dividing by it converts "world units at this distance" → "fraction of half-height".
        // Multiplying by (viewport_h / 2) gives pixels, so we solve for the world-unit
        // size that produces TARGET_PIXELS of screen size.
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

        // Pass 1 — mutate GltfMesh transforms (position/rotation/scale).
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

        // Pass 2 — collect shared references now that mutation is done.
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