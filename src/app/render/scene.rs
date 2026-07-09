use std::collections::HashMap;

use glam::{Quat, Vec3};

use space_soup::renderer::mesh_pipeline::ModelUniform;
use space_soup::renderer::{
    billboard_rotation, Camera, Color3, Cuboid, GltfMesh, IconAssets, IconKind, MeshInstance,
    Renderer,
};
use space_soup_engine::{DebugPacket, GameObject, RenderCuboid};

use crate::scene_3d;
use crate::transform_gizmo::{GizmoAssets, TransformGizmo};

use super::super::ViewMode;

pub(crate) const GIZMO_ANCHOR_MARGIN: f32 = 0.35;

pub(crate) fn gizmo_anchor(obj: &GameObject) -> Vec3 {
    let clearance = obj.cuboid.half_size.y + GIZMO_ANCHOR_MARGIN;
    obj.cuboid.position + Vec3::new(0.0, clearance, 0.0)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_cuboids(
    render_cuboids: &[RenderCuboid],
    packet: &DebugPacket,
    head_pos: Vec3,
    head_rot: Quat,
    to_world: impl Fn(Vec3, Quat) -> (Vec3, Quat),
    objects: &[GameObject],
    selected_id: Option<&str>,
    is_edit: bool,
    dragging_new_model: bool,
    ghost_preview: Option<Vec3>,
) -> Vec<Cuboid> {
    let mut cuboids: Vec<Cuboid> = render_cuboids
        .iter()
        .map(scene_3d::engine_cuboid_to_render)
        .collect();
    cuboids.extend(scene_3d::ground_grid());
    cuboids.extend(scene_3d::build_player_overlay(
        head_pos,
        head_rot,
        &packet.left_hand,
        &packet.right_hand,
        to_world,
    ));

    if is_edit {
        for obj in objects {
            if obj.hidden {
                continue;
            }
            let selected = selected_id == Some(obj.id.as_str());
            let wire = if selected {
                Color3(255, 220, 40, 255)
            } else {
                Color3(255, 255, 255, 150)
            };
            if obj.mesh.is_some() {
                let mut c = Cuboid::wireframe(obj.cuboid.position, obj.cuboid.half_size, wire);
                c.rotation = obj.cuboid.rotation;
                cuboids.push(c);
            } else if selected {
                let mut c = Cuboid::wireframe(obj.cuboid.position, obj.cuboid.half_size, wire);
                c.rotation = obj.cuboid.rotation;
                cuboids.push(c);
            }
        }
    }

    if dragging_new_model {
        if let Some(pos) = ghost_preview {
            let half = Vec3::splat(0.25);
            cuboids.push(Cuboid::wireframe(
                Vec3::new(pos.x, half.y, pos.z),
                half,
                Color3(80, 220, 255, 160),
            ));
        }
    }
    cuboids
}

/// Camera-facing billboard markers for objects that have no mesh/cuboid body
/// of their own to click on — lights and sound sources. Each object gets its
/// own persistent `(GltfMesh, ModelUniform)` cache entry keyed by object id
/// (not by the shared icon texture path) so independent instances don't
/// clobber each other's transform, same reasoning as the model mesh cache.
pub(crate) fn collect_icon_instances<'a>(
    icon_assets: &Option<IconAssets>,
    icon_mesh_cache: &'a mut HashMap<String, (GltfMesh, ModelUniform)>,
    renderer: &Renderer,
    camera: &Camera,
    objects: &[GameObject],
) -> Vec<MeshInstance<'a>> {
    let Some(icon_assets) = icon_assets else {
        return Vec::new();
    };

    // `hidden` only suppresses a cuboid/mesh body — light/sound markers are
    // created hidden precisely so the icon is their only visible body.
    let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for obj in objects {
        let kind = if obj.light.is_some() {
            IconKind::Light
        } else if obj.sound.is_some() {
            IconKind::Sound
        } else {
            continue;
        };
        seen.insert(obj.id.as_str());

        let (mesh, _) = icon_mesh_cache
            .entry(obj.id.clone())
            .or_insert_with(|| (icon_assets.mesh_for(kind), renderer.create_model_uniform()));
        mesh.position = obj.cuboid.position;
        mesh.rotation = billboard_rotation(camera.rotation);
    }
    icon_mesh_cache.retain(|id, _| seen.contains(id.as_str()));

    icon_mesh_cache
        .values()
        .map(|(mesh, model)| MeshInstance { mesh, model })
        .collect()
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn sync_gizmo_and_collect<'a>(
    xform_gizmo: &mut TransformGizmo,
    gizmo_assets: &'a mut Option<GizmoAssets>,
    camera: &Camera,
    viewport: (f32, f32),
    objects: &[GameObject],
    selected_id: Option<&str>,
    view_mode: ViewMode,
    show_editor: bool,
    is_dragging: bool,
) -> Vec<MeshInstance<'a>> {
    if view_mode != ViewMode::Edit || show_editor {
        return Vec::new();
    }
    let Some(id) = selected_id else {
        return Vec::new();
    };
    let Some(obj) = objects.iter().find(|o| o.id == id) else {
        return Vec::new();
    };

    xform_gizmo.set_position(gizmo_anchor(obj));
    if !is_dragging {
        xform_gizmo.set_rotation(obj.cuboid.rotation);
        xform_gizmo.set_scale(obj.cuboid.half_size);
    }

    let Some(assets) = gizmo_assets.as_mut() else {
        return Vec::new();
    };
    xform_gizmo.collect_mesh_instances(assets, camera, viewport)
}
