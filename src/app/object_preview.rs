use std::collections::HashMap;

use glam::{Quat, Vec3};

use space_soup::renderer::mesh_pipeline::ModelUniform;
use space_soup::renderer::{Color3, Cuboid, GltfMesh, MeshInstance};
use space_soup_engine::Scene;

use crate::transform_gizmo::TransformGizmo;

use super::orbit_camera::OrbitCamera;
use super::App;

pub(crate) struct ObjectPreviewState {
    pub object_id: String,
    pub orbit: OrbitCamera,
    pub pos_snap: Option<f32>,
    pub rot_snap_deg: Option<f32>,
    pub show_skeleton: bool,
    pub content_height: f32,
}

impl ObjectPreviewState {
    fn new(object_id: String, target: glam::Vec3, framing_radius: f32) -> Self {
        let mut orbit = OrbitCamera::new(framing_radius);
        orbit.target = target;
        Self {
            object_id,
            orbit,
            pos_snap: None,
            rot_snap_deg: None,
            show_skeleton: false,
            content_height: 600.0,
        }
    }
}

pub(crate) fn open(app: &mut App, object_id: String) {
    let Some(obj) = app.runtime.scene().find_object(&object_id) else {
        return;
    };
    let framing = obj.cuboid.half_size.length().max(0.05);
    let target = obj.cuboid.position;

    app.grab_pose_editor = None;
    app.xform_gizmo = TransformGizmo::new();
    app.object_preview = Some(ObjectPreviewState::new(object_id, target, framing));
}

pub(crate) fn close(app: &mut App) {
    app.object_preview = None;
    super::scene_bridge::save_if_dirty(app);
}

pub(crate) fn update_transforms(
    state: &ObjectPreviewState,
    scene: &Scene,
    mesh_cache: &mut HashMap<String, (GltfMesh, ModelUniform)>,
) {
    let Some(obj) = scene.find_object(&state.object_id) else {
        return;
    };
    let Some(mesh_ref) = &obj.mesh else {
        return;
    };
    let Some((mesh, _)) = mesh_cache.get_mut(&mesh_ref.path) else {
        return;
    };
    mesh.position = obj.cuboid.position;
    mesh.rotation = obj.cuboid.rotation;
    mesh.scale = mesh_ref.scale;
}

pub(crate) fn collect_cuboid(state: &ObjectPreviewState, scene: &Scene) -> Vec<Cuboid> {
    let Some(obj) = scene.find_object(&state.object_id) else {
        return Vec::new();
    };
    if obj.mesh.is_some() {
        return Vec::new();
    }
    let col = obj.cuboid.color;
    let mut c = Cuboid::solid(
        obj.cuboid.position,
        obj.cuboid.half_size,
        Color3(col.0, col.1, col.2, 255),
    );
    c.rotation = obj.cuboid.rotation;
    vec![c]
}

const JOINT_COLOR: Color3 = Color3(90, 225, 255, 255);
const BONE_COLOR: Color3 = Color3(40, 150, 200, 220);
const JOINT_HALF: f32 = 0.015;
const BONE_THICKNESS: f32 = 0.006;

/// Joint markers + bone segments for the previewed object's mesh skin, in
/// its bind pose (not any animated/finger-curl-blended pose — this is a
/// static "here's the generated skeleton" view, not a live rig preview).
pub(crate) fn collect_skeleton_cuboids(
    state: &ObjectPreviewState,
    scene: &Scene,
    mesh_cache: &HashMap<String, (GltfMesh, ModelUniform)>,
) -> Vec<Cuboid> {
    let mut out = Vec::new();
    if !state.show_skeleton {
        return out;
    }
    let Some(obj) = scene.find_object(&state.object_id) else {
        return out;
    };
    let Some(mesh_ref) = &obj.mesh else {
        return out;
    };
    let Some((mesh, _)) = mesh_cache.get(&mesh_ref.path) else {
        return out;
    };
    let Some(skin) = &mesh.skin else {
        return out;
    };

    let world = mesh.model_matrix();
    let local_joints = skin.hierarchical_transforms(&skin.joint_local_bind);
    let world_positions: Vec<Vec3> = local_joints
        .iter()
        .map(|m| world.transform_point3(m.transform_point3(Vec3::ZERO)))
        .collect();

    for &pos in &world_positions {
        out.push(Cuboid::solid(pos, Vec3::splat(JOINT_HALF), JOINT_COLOR));
    }

    for (i, parent) in skin.joint_parents.iter().enumerate() {
        let Some(p) = parent else { continue };
        let a = world_positions[*p];
        let b = world_positions[i];
        let dir = b - a;
        let len = dir.length();
        if len < 1e-5 {
            continue;
        }
        let mut segment = Cuboid::solid(
            (a + b) * 0.5,
            Vec3::new(BONE_THICKNESS, BONE_THICKNESS, len * 0.5),
            BONE_COLOR,
        );
        segment.rotation = Quat::from_rotation_arc(Vec3::Z, dir / len);
        out.push(segment);
    }

    out
}

pub(crate) fn collect_mesh_instances<'a>(
    state: &ObjectPreviewState,
    scene: &Scene,
    mesh_cache: &'a HashMap<String, (GltfMesh, ModelUniform)>,
) -> Vec<MeshInstance<'a>> {
    let Some(obj) = scene.find_object(&state.object_id) else {
        return Vec::new();
    };
    let Some(mesh_ref) = &obj.mesh else {
        return Vec::new();
    };
    match mesh_cache.get(&mesh_ref.path) {
        Some((mesh, model)) => vec![MeshInstance { mesh, model }],
        None => Vec::new(),
    }
}
