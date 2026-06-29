use std::collections::HashMap;

use space_soup::renderer::{GltfMesh, Renderer};
use space_soup::renderer::mesh_pipeline::ModelUniform;

use super::colors::{color_for, ColorState};
use super::geometry::{all_axes_for, geometry_for};
use super::glb::write_glb;
use super::{Axis, GizmoMode};

pub(crate) struct GizmoAssets {
    pub(crate) parts: HashMap<(GizmoMode, Axis, ColorState), (GltfMesh, ModelUniform)>,
}

impl GizmoAssets {
    pub fn load(renderer: &Renderer) -> Self {
        let cache_dir = std::env::temp_dir().join("space_soup_gizmo_cache");
        let _ = std::fs::create_dir_all(&cache_dir);

        let mut parts = HashMap::new();
        let mut attempted = 0usize;
        for &mode in &[GizmoMode::Translate, GizmoMode::Rotate, GizmoMode::Scale] {
            for &axis in all_axes_for(mode) {
                let Some(geo) = geometry_for(mode, axis) else { continue };
                for &state in &[ColorState::Normal, ColorState::Hover, ColorState::Selected] {
                    attempted += 1;
                    let color = color_for(axis, state);
                    let glb = write_glb(&geo, color);
                    let path = cache_dir.join(format!("{mode:?}_{axis:?}_{state:?}.glb"));
                    if let Err(e) = std::fs::write(&path, &glb) {
                        log::warn!("transform_gizmo: failed to write cache file {path:?}: {e}");
                        continue;
                    }
                    match GltfMesh::load(&renderer.device, &renderer.queue, renderer.mesh_texture_layout(), &path) {
                        Ok(gltf) => {
                            let model = renderer.create_model_uniform();
                            parts.insert((mode, axis, state), (gltf, model));
                        }
                        Err(e) => log::warn!("transform_gizmo: failed to load {path:?}: {e}"),
                    }
                }
            }
        }
        // Diagnostic: if this count is much lower than `attempted`, check the
        // log above for "failed to load" lines — that's a GLB/loader mismatch,
        // not a rendering bug.
        log::info!("transform_gizmo: loaded {}/{attempted} gizmo part variants", parts.len());
        Self { parts }
    }
}
