use std::collections::HashMap;

use glam::{EulerRot, Mat4, Quat, Vec3};

use space_soup::renderer::mesh_pipeline::ModelUniform;
use space_soup::renderer::{Color3, Cuboid, GltfMesh, MeshInstance, Renderer};
use space_soup_engine::{GripKind, GripPointDef, Hand, Scene};

use crate::transform_gizmo::{GizmoMode, TransformGizmo};

use super::snap::{compute_skin_matrices, hand_glb_path, snap_rotation, snap_vec3};
use super::App;

/// Cache key for the grab-pose editor's own copy of a hand mesh. Distinct from
/// the raw `hand_glb_path` key so we always hold a *skinned* instance (with a
/// joint bind group). The main scene view preloads the same GLB under its raw
/// path as a plain, unskinned mesh (setup.rs); reusing that entry would give the
/// grab-pose hand no joint bind group, and the renderer draws such a mesh in
/// neither pass — i.e. it would be invisible.
fn hand_cache_key(hand: Hand) -> String {
    format!("__grabpose_skinned__/{}", hand_glb_path(hand))
}

fn identity_quat_arr() -> [f32; 4] {
    Quat::IDENTITY.to_array()
}
fn one_vec3_arr() -> [f32; 3] {
    Vec3::ONE.to_array()
}

fn default_grip_point(existing: &[GripPointDef]) -> GripPointDef {
    let mut n = 1;
    let name = loop {
        let candidate = if n == 1 {
            "grip".to_string()
        } else {
            format!("grip_{n}")
        };
        if !existing.iter().any(|p| p.name == candidate) {
            break candidate;
        }
        n += 1;
    };
    GripPointDef {
        name,
        kind: GripKind::Snap,
        local_pos: [0.0; 3],
        local_rot: identity_quat_arr(),
        hand_offset_scale: one_vec3_arr(),
        finger_curl: HashMap::new(),
    }
}

#[derive(Clone)]
struct GrabPointEdit {
    point_index: usize,
    before: GripPointDef,
    after: GripPointDef,
}

pub(crate) struct GrabPoseEditorState {
    pub object_id: String,
    pub orbit: super::orbit_camera::OrbitCamera,
    pub active_point: usize,
    pub preview_hand: Hand,
    pub preview_mode: bool,
    pub preview_rotation: Quat,
    pub pos_snap: Option<f32>,
    pub rot_snap_deg: Option<f32>,

    pub content_height: f32,

    undo: Vec<GrabPointEdit>,
    redo: Vec<GrabPointEdit>,
    drag_before: Option<GripPointDef>,
}

impl GrabPoseEditorState {
    fn new(object_id: String, framing_radius: f32) -> Self {
        Self {
            object_id,
            orbit: super::orbit_camera::OrbitCamera::new(framing_radius),
            active_point: 0,
            preview_hand: Hand::Right,
            preview_mode: false,
            preview_rotation: Quat::IDENTITY,
            pos_snap: None,
            rot_snap_deg: None,
            content_height: 900.0,
            undo: Vec::new(),
            redo: Vec::new(),
            drag_before: None,
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

pub(crate) const FINGER_GROUPS: [(&str, &[&str]); 5] = [
    ("Thumb", &["thumb1", "thumb2", "thumb3"]),
    ("Index", &["index1", "index2", "index3"]),
    ("Middle", &["middle1", "middle2", "middle3"]),
    ("Ring", &["ring1", "ring2", "ring3"]),
    ("Pinky", &["pinky0", "pinky1", "pinky2", "pinky3"]),
];

pub(crate) fn finger_curl_value(point: &GripPointDef, group_idx: usize) -> f32 {
    FINGER_GROUPS
        .get(group_idx)
        .and_then(|(_, bones)| bones.first())
        .and_then(|bone| point.finger_curl.get(*bone))
        .copied()
        .unwrap_or(0.0)
}

pub(crate) fn apply_finger_curl(app: &mut App, group_idx: usize, value: f32) {
    begin_drag_snapshot(app);
    let (obj_id, active_point) = {
        let Some(state) = app.grab_pose_editor.as_ref() else {
            return;
        };
        (state.object_id.clone(), state.active_point)
    };
    if let Some(obj) = app.runtime.scene_mut().find_object_mut(&obj_id) {
        if let Some(point) = obj.grip_points.get_mut(active_point) {
            if let Some((_, bones)) = FINGER_GROUPS.get(group_idx) {
                let v = value.clamp(0.0, 1.0);
                for bone in *bones {
                    point.finger_curl.insert(bone.to_string(), v);
                }
                app.scene_dirty = true;
            }
        }
    }
    end_drag_commit(app);
}

fn reference_transform(preview_rotation: Quat) -> Mat4 {
    Mat4::from_rotation_translation(preview_rotation, Vec3::ZERO)
}

fn point_root(reference: Mat4, point: &GripPointDef) -> Mat4 {
    let offset_mat = Mat4::from_rotation_translation(
        Quat::from_array(point.local_rot),
        Vec3::from(point.local_pos),
    );
    reference * offset_mat
}

pub(crate) fn open(app: &mut App, object_id: String) {
    if let Some(obj) = app.runtime.scene_mut().find_object_mut(&object_id) {
        if obj.grip_points.is_empty() {
            let point = default_grip_point(&obj.grip_points);
            obj.grip_points.push(point);
        }
    }
    app.scene_dirty = true;

    let framing = app
        .runtime
        .scene()
        .find_object(&object_id)
        .map(|o| o.cuboid.half_size.length().max(0.05))
        .unwrap_or(0.3);

    app.grab_pose_gizmo = TransformGizmo::new();
    app.grab_pose_editor = Some(GrabPoseEditorState::new(object_id, framing));
}

pub(crate) fn close(app: &mut App) {
    app.grab_pose_editor = None;
}

pub(crate) fn ensure_hand_meshes_loaded(
    renderer: &Renderer,
    mesh_cache: &mut HashMap<String, (GltfMesh, ModelUniform)>,
    game_dir: &std::path::Path,
) {
    for hand in [Hand::Left, Hand::Right] {
        let key = hand_cache_key(hand);
        if mesh_cache.contains_key(&key) {
            continue;
        }
        let full_path = game_dir.join(hand_glb_path(hand));
        match GltfMesh::load(
            &renderer.device,
            &renderer.queue,
            renderer.mesh_texture_layout(),
            &full_path,
        ) {
            Ok(mut mesh) => {
                mesh.create_skin_bind_group(&renderer.device, renderer.skin_joint_layout());
                let model_uniform = renderer.create_skinned_model_uniform();
                mesh_cache.insert(key, (mesh, model_uniform));
            }
            Err(e) => log::warn!(
                "space_soup_editor: grab pose editor couldn't load {}: {e}",
                full_path.display()
            ),
        }
    }
}

pub(crate) fn update_transforms(
    state: &GrabPoseEditorState,
    scene: &Scene,
    mesh_cache: &mut HashMap<String, (GltfMesh, ModelUniform)>,
    queue: &wgpu::Queue,
) {
    let Some(obj) = scene.find_object(&state.object_id) else {
        return;
    };
    let reference = reference_transform(state.preview_rotation);

    if let Some(mesh_ref) = &obj.mesh {
        if let Some((mesh, _)) = mesh_cache.get_mut(&mesh_ref.path) {
            let (_, rot, pos) = reference.to_scale_rotation_translation();
            mesh.position = pos;
            mesh.rotation = rot;
            mesh.scale = mesh_ref.scale;
        }
    }

    let Some(point) = obj.grip_points.get(state.active_point) else {
        return;
    };
    let hand_key = hand_cache_key(state.preview_hand);
    // The hand's world placement (grip root + preview scale) is baked entirely
    // into the joint matrices below. The skinned shader computes
    // `world = model * (joints * pos)`, so the mesh's own model matrix MUST stay
    // identity — otherwise `root` is applied twice and the hand is flung off the
    // grip point (which is why it looked like the hand wasn't rendering at all).
    let root = point_root(reference, point)
        * Mat4::from_scale(Vec3::from(point.hand_offset_scale));
    if let Some((mesh, _)) = mesh_cache.get(&hand_key) {
        if let Some(skin) = &mesh.skin {
            let mats = compute_skin_matrices(skin, root, &point.finger_curl);
            skin.update_joint_matrices(queue, &mats);
        }
    }
    if let Some((mesh, _)) = mesh_cache.get_mut(&hand_key) {
        mesh.position = Vec3::ZERO;
        mesh.rotation = Quat::IDENTITY;
        mesh.scale = Vec3::ONE;
    }
}

const MARKER_HALF: f32 = 0.015;

fn marker_color(kind: GripKind, active: bool) -> Color3 {
    match (kind, active) {
        (GripKind::Snap, true) => Color3(90, 225, 255, 255),
        (GripKind::Snap, false) => Color3(40, 110, 140, 255),
        (GripKind::Free, true) => Color3(255, 195, 60, 255),
        (GripKind::Free, false) => Color3(150, 105, 30, 255),
        (GripKind::Pinch, true) => Color3(255, 110, 220, 255),
        (GripKind::Pinch, false) => Color3(150, 60, 130, 255),
    }
}

pub(crate) fn collect_cuboids(state: &GrabPoseEditorState, scene: &Scene) -> Vec<Cuboid> {
    let Some(obj) = scene.find_object(&state.object_id) else {
        return Vec::new();
    };

    let reference = reference_transform(state.preview_rotation);
    let mut out = Vec::new();

    if obj.mesh.is_none() {
        let (_, rot, pos) = reference.to_scale_rotation_translation();
        let col = obj.cuboid.color;
        let mut c = Cuboid::solid(
            pos,
            obj.cuboid.half_size,
            Color3(col.0, col.1, col.2, col.3),
        );
        c.rotation = rot;
        out.push(c);
    }

    for (i, point) in obj.grip_points.iter().enumerate() {
        // The active point shows the posed hand instead of a marker cube — only
        // the other (unselected) points get a cube so they stay visible/pickable.
        if i == state.active_point {
            continue;
        }
        let root = point_root(reference, point);
        let (_, rot, pos) = root.to_scale_rotation_translation();
        let mut c = Cuboid::solid(pos, Vec3::splat(MARKER_HALF), marker_color(point.kind, false));
        c.rotation = rot;
        out.push(c);
    }

    out
}

pub(crate) fn collect_mesh_instances<'a>(
    state: &GrabPoseEditorState,
    scene: &Scene,
    mesh_cache: &'a HashMap<String, (GltfMesh, ModelUniform)>,
) -> Vec<MeshInstance<'a>> {
    let mut out = Vec::new();
    let Some(obj) = scene.find_object(&state.object_id) else {
        return out;
    };

    if let Some(mesh_ref) = &obj.mesh {
        if let Some((mesh, model)) = mesh_cache.get(&mesh_ref.path) {
            out.push(MeshInstance { mesh, model });
        }
    }

    if obj.grip_points.get(state.active_point).is_some() {
        if let Some((mesh, model)) = mesh_cache.get(&hand_cache_key(state.preview_hand)) {
            out.push(MeshInstance { mesh, model });
        }
    }
    out
}

/// Kept for a possible future re-enable; the grab pose editor no longer shows or drives this
/// gizmo in its viewport, so nothing currently calls this.
#[allow(dead_code)]
pub(crate) fn sync_gizmo(app: &mut App) {
    let Some(state) = &app.grab_pose_editor else {
        return;
    };
    if state.preview_mode {
        return;
    }
    let obj_id = state.object_id.clone();
    let active_point = state.active_point;
    let preview_rotation = state.preview_rotation;
    let is_dragging = app.gizmo_dragging;

    let Some(obj) = app.runtime.scene().find_object(&obj_id) else {
        return;
    };
    let Some(point) = obj.grip_points.get(active_point) else {
        return;
    };
    let reference = reference_transform(preview_rotation);
    let root = point_root(reference, point);
    let (_, rot, pos) = root.to_scale_rotation_translation();
    let scale = Vec3::from(point.hand_offset_scale);

    if !is_dragging {
        app.grab_pose_gizmo.set_position(pos);
        app.grab_pose_gizmo.set_rotation(rot);
        app.grab_pose_gizmo.set_scale(scale);
    }
}

/// Kept alongside `sync_gizmo` for a possible future re-enable; currently unused.
#[allow(dead_code)]
pub(crate) fn apply_gizmo_drag(app: &mut App) {
    let Some(state) = &app.grab_pose_editor else {
        return;
    };
    let obj_id = state.object_id.clone();
    let active_point = state.active_point;
    let pos_snap = state.pos_snap;
    let rot_snap_deg = state.rot_snap_deg;
    let mode = app.grab_pose_gizmo.mode;

    let mut gizmo_pos = app.grab_pose_gizmo.get_position();
    let mut gizmo_rot = app.grab_pose_gizmo.get_rotation();
    let gizmo_scale = app.grab_pose_gizmo.get_scale();

    if mode == GizmoMode::Translate {
        if let Some(step) = pos_snap {
            gizmo_pos = snap_vec3(gizmo_pos, step);
        }
    } else if mode == GizmoMode::Rotate {
        if let Some(step) = rot_snap_deg {
            gizmo_rot = snap_rotation(gizmo_rot, step);
        }
    }

    let Some(obj) = app.runtime.scene_mut().find_object_mut(&obj_id) else {
        return;
    };
    let reference = reference_transform(Quat::IDENTITY);
    let Some(point) = obj.grip_points.get_mut(active_point) else {
        return;
    };

    match mode {
        GizmoMode::Translate | GizmoMode::Rotate => {
            let root = Mat4::from_rotation_translation(gizmo_rot, gizmo_pos);
            let offset_mat = reference.inverse() * root;
            let (_, rot, pos) = offset_mat.to_scale_rotation_translation();
            point.local_pos = pos.to_array();
            point.local_rot = rot.to_array();
        }
        GizmoMode::Scale => {
            point.hand_offset_scale = gizmo_scale.to_array();
        }
    }
    app.scene_dirty = true;
}

pub(crate) fn begin_drag_snapshot(app: &mut App) {
    let (obj_id, active_point) = {
        let Some(state) = app.grab_pose_editor.as_ref() else {
            return;
        };
        (state.object_id.clone(), state.active_point)
    };
    let before = app
        .runtime
        .scene()
        .find_object(&obj_id)
        .and_then(|o| o.grip_points.get(active_point).cloned());
    if let Some(state) = app.grab_pose_editor.as_mut() {
        state.drag_before = before;
    }
}

pub(crate) fn end_drag_commit(app: &mut App) {
    let (obj_id, active_point, before) = {
        let Some(state) = app.grab_pose_editor.as_ref() else {
            return;
        };
        let Some(before) = state.drag_before.clone() else {
            return;
        };
        (state.object_id.clone(), state.active_point, before)
    };
    let after = app
        .runtime
        .scene()
        .find_object(&obj_id)
        .and_then(|o| o.grip_points.get(active_point).cloned());
    if let Some(state) = app.grab_pose_editor.as_mut() {
        state.drag_before = None;
        if let Some(after) = after {
            let changed = before.local_pos != after.local_pos
                || before.local_rot != after.local_rot
                || before.hand_offset_scale != after.hand_offset_scale
                || before.finger_curl != after.finger_curl;
            if changed {
                state.undo.push(GrabPointEdit {
                    point_index: active_point,
                    before,
                    after,
                });
                state.redo.clear();
            }
        }
    }
}

pub(crate) fn reset_active_point(app: &mut App) {
    begin_drag_snapshot(app);
    let (obj_id, active_point) = {
        let Some(state) = app.grab_pose_editor.as_ref() else {
            return;
        };
        (state.object_id.clone(), state.active_point)
    };
    if let Some(obj) = app.runtime.scene_mut().find_object_mut(&obj_id) {
        if let Some(point) = obj.grip_points.get_mut(active_point) {
            let name = point.name.clone();
            let kind = point.kind;
            *point = GripPointDef {
                name,
                kind,
                ..default_grip_point(&[])
            };
            app.scene_dirty = true;
        }
    }
    end_drag_commit(app);
}

pub(crate) fn undo(app: &mut App) {
    let (edit, obj_id) = {
        let Some(state) = app.grab_pose_editor.as_mut() else {
            return;
        };
        let Some(edit) = state.undo.pop() else { return };
        state.redo.push(edit.clone());
        (edit, state.object_id.clone())
    };
    if let Some(obj) = app.runtime.scene_mut().find_object_mut(&obj_id) {
        if let Some(point) = obj.grip_points.get_mut(edit.point_index) {
            *point = edit.before;
            app.scene_dirty = true;
        }
    }
}

pub(crate) fn redo(app: &mut App) {
    let (edit, obj_id) = {
        let Some(state) = app.grab_pose_editor.as_mut() else {
            return;
        };
        let Some(edit) = state.redo.pop() else { return };
        state.undo.push(edit.clone());
        (edit, state.object_id.clone())
    };
    if let Some(obj) = app.runtime.scene_mut().find_object_mut(&obj_id) {
        if let Some(point) = obj.grip_points.get_mut(edit.point_index) {
            *point = edit.after;
            app.scene_dirty = true;
        }
    }
}

pub(crate) fn select_point(app: &mut App, index: usize) {
    let Some(state) = app.grab_pose_editor.as_mut() else {
        return;
    };
    let obj_id = state.object_id.clone();
    let len = app
        .runtime
        .scene()
        .find_object(&obj_id)
        .map(|o| o.grip_points.len())
        .unwrap_or(0);
    if let Some(state) = app.grab_pose_editor.as_mut() {
        state.active_point = index.min(len.saturating_sub(1));
    }
}

pub(crate) fn add_point(app: &mut App) {
    let Some(state) = app.grab_pose_editor.as_ref() else {
        return;
    };
    let obj_id = state.object_id.clone();
    let Some(obj) = app.runtime.scene_mut().find_object_mut(&obj_id) else {
        return;
    };
    let point = default_grip_point(&obj.grip_points);
    obj.grip_points.push(point);
    let new_index = obj.grip_points.len() - 1;
    if let Some(state) = app.grab_pose_editor.as_mut() {
        state.active_point = new_index;
    }
    app.scene_dirty = true;
}

pub(crate) fn delete_active_point(app: &mut App) {
    let Some(state) = app.grab_pose_editor.as_ref() else {
        return;
    };
    let obj_id = state.object_id.clone();
    let active_point = state.active_point;
    let Some(obj) = app.runtime.scene_mut().find_object_mut(&obj_id) else {
        return;
    };
    if obj.grip_points.len() <= 1 {
        return;
    }
    obj.grip_points.remove(active_point);
    if let Some(state) = app.grab_pose_editor.as_mut() {
        state.active_point = active_point.min(obj.grip_points.len() - 1);
        state.undo.clear();
        state.redo.clear();
    }
    app.scene_dirty = true;
}

pub(crate) fn rename_active_point(app: &mut App, new_name: String) {
    let trimmed = new_name.trim();
    if trimmed.is_empty() {
        return;
    }
    let Some(state) = app.grab_pose_editor.as_ref() else {
        return;
    };
    let obj_id = state.object_id.clone();
    let active_point = state.active_point;
    let Some(obj) = app.runtime.scene_mut().find_object_mut(&obj_id) else {
        return;
    };
    if obj
        .grip_points
        .iter()
        .enumerate()
        .any(|(i, p)| i != active_point && p.name == trimmed)
    {
        return;
    }
    if let Some(point) = obj.grip_points.get_mut(active_point) {
        point.name = trimmed.to_string();
        app.scene_dirty = true;
    }
}

pub(crate) fn set_active_point_kind(app: &mut App, kind: GripKind) {
    let Some(state) = app.grab_pose_editor.as_ref() else {
        return;
    };
    let obj_id = state.object_id.clone();
    let active_point = state.active_point;
    if let Some(obj) = app.runtime.scene_mut().find_object_mut(&obj_id) {
        if let Some(point) = obj.grip_points.get_mut(active_point) {
            point.kind = kind;
            app.scene_dirty = true;
        }
    }
}

pub(crate) fn preview_drag(app: &mut App, dx: f32, dy: f32) {
    if let Some(state) = app.grab_pose_editor.as_mut() {
        let yaw = Quat::from_rotation_y(-dx * 0.006);
        let pitch = Quat::from_rotation_x(-dy * 0.006);
        state.preview_rotation = yaw * pitch * state.preview_rotation;
    }
}

#[derive(Clone, Copy)]
pub(crate) enum PoseField {
    Pos(usize),
    Rot(usize),
    Scale(usize),
}

pub(crate) fn apply_field_edit(app: &mut App, field: PoseField, value: f32) {
    begin_drag_snapshot(app);
    let (obj_id, active_point) = {
        let Some(state) = app.grab_pose_editor.as_ref() else {
            return;
        };
        (state.object_id.clone(), state.active_point)
    };
    if let Some(obj) = app.runtime.scene_mut().find_object_mut(&obj_id) {
        if let Some(point) = obj.grip_points.get_mut(active_point) {
            match field {
                PoseField::Pos(i) => point.local_pos[i] = value,
                PoseField::Rot(i) => {
                    let q = Quat::from_array(point.local_rot);
                    // glam returns YXZ euler in (Y, X, Z) order — store it
                    // axis-indexed as [X, Y, Z] so field i maps to one stable
                    // axis (otherwise editing X also drags Y).
                    let (ey, ex, ez) = q.to_euler(EulerRot::YXZ);
                    let mut deg = [ex.to_degrees(), ey.to_degrees(), ez.to_degrees()];
                    deg[i] = value;
                    let nq = Quat::from_euler(
                        EulerRot::YXZ,
                        deg[1].to_radians(), // Y
                        deg[0].to_radians(), // X
                        deg[2].to_radians(), // Z
                    );
                    point.local_rot = nq.to_array();
                }
                PoseField::Scale(i) => point.hand_offset_scale[i] = value,
            }
            app.scene_dirty = true;
        }
    }
    end_drag_commit(app);
}
