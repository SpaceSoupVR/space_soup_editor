//! Interactive VR Grab Pose Editor — an isolated, single-object editing
//! mode (entered from the Inspector's "Edit Grab Pose" button) for
//! authoring named `grip_points` (see `space_soup_engine::GripPointDef`):
//! the physics-joint-based grab system where any hand can grab any named
//! point on an object, and two hands can hold two different points on the
//! same object at once (e.g. a rifle's `stock`/`barrel`). Follows the same
//! mode-switch pattern as the text-editor tab (`App.editing`) and the Snap
//! tool (`snap.rs`): no second window, just a different set of things drawn
//! into the one existing viewport while this mode is active.
//!
//! Everything here is rendered relative to the target object's own local
//! origin — the object is drawn at `Vec3::ZERO` with its authored
//! `cuboid.rotation` as the reference basis (matching exactly the
//! `obj_mat` used at grab time in `quest_app`), so a point's local
//! offset always looks the same in this editor as it will in-game,
//! regardless of the object's real position/rotation out in the scene.
//!
//! The gizmo always edits the *active* point (`GrabPoseEditorState::active_point`,
//! an index into `obj.grip_points`); a single hand mesh previews at that
//! point using whichever hand (`preview_hand`) is selected, while every
//! other point on the object is shown as a small color-coded marker
//! (cyan-ish for `Snap`, orange-ish for `Free`) so multi-point objects like
//! the rifle can be authored with both grips visible at once.

use std::collections::HashMap;

use glam::{EulerRot, Mat4, Quat, Vec3};

use space_soup::renderer::mesh_pipeline::ModelUniform;
use space_soup::renderer::{Color3, Cuboid, GltfMesh, MeshInstance, Renderer};
use space_soup_engine::{GripKind, GripPointDef, Hand, Scene};

use crate::transform_gizmo::{GizmoMode, TransformGizmo};

use super::snap::hand_glb_path;
use super::App;

fn identity_quat_arr() -> [f32; 4] { Quat::IDENTITY.to_array() }
fn one_vec3_arr() -> [f32; 3] { Vec3::ONE.to_array() }

fn default_grip_point(existing: &[GripPointDef]) -> GripPointDef {
    let mut n = 1;
    let name = loop {
        let candidate = if n == 1 { "grip".to_string() } else { format!("grip_{n}") };
        if !existing.iter().any(|p| p.name == candidate) { break candidate; }
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
            undo: Vec::new(),
            redo: Vec::new(),
            drag_before: None,
        }
    }

    pub fn can_undo(&self) -> bool { !self.undo.is_empty() }
    pub fn can_redo(&self) -> bool { !self.redo.is_empty() }
}

/// The frame in which grip point offsets are defined: the object's own
/// authored rotation (matching `quest_app`'s `obj_mat` exactly, minus
/// position), composed with `preview_rotation` — the free "let me look at
/// it from another angle" spin that's only ever nonzero while
/// `preview_mode` is on, so this collapses back to exactly the runtime
/// basis whenever the gizmo is actually being edited.
fn reference_transform(preview_rotation: Quat, obj_base_rotation: Quat) -> Mat4 {
    Mat4::from_rotation_translation(preview_rotation * obj_base_rotation, Vec3::ZERO)
}

fn point_root(reference: Mat4, point: &GripPointDef) -> Mat4 {
    let offset_mat = Mat4::from_rotation_translation(
        Quat::from_array(point.local_rot),
        Vec3::from(point.local_pos),
    );
    reference * offset_mat
}

/// Opens the editor for `object_id`, seeding one default grip point if the
/// object doesn't have any yet, so there's always something for the gizmo
/// to target.
pub(crate) fn open(app: &mut App, object_id: String) {
    if let Some(obj) = app.runtime.scene_mut().find_object_mut(&object_id) {
        if obj.grip_points.is_empty() {
            let point = default_grip_point(&obj.grip_points);
            obj.grip_points.push(point);
        }
    }
    app.scene_dirty = true;

    let framing = app.runtime.scene().find_object(&object_id)
        .map(|o| o.cuboid.half_size.length().max(0.05))
        .unwrap_or(0.3);

    app.grab_pose_gizmo = TransformGizmo::new();
    app.grab_pose_editor = Some(GrabPoseEditorState::new(object_id, framing));
}

pub(crate) fn close(app: &mut App) {
    app.grab_pose_editor = None;
}

/// Loads `left_hand.glb`/`right_hand.glb` into `mesh_cache` if not already
/// present — same lazy-load pattern as `snap::update_preview`.
pub(crate) fn ensure_hand_meshes_loaded(
    renderer: &Renderer,
    mesh_cache: &mut HashMap<String, (GltfMesh, ModelUniform)>,
    game_dir: &std::path::Path,
) {
    for hand in [Hand::Left, Hand::Right] {
        let path = hand_glb_path(hand);
        if mesh_cache.contains_key(path) { continue; }
        let full_path = game_dir.join(path);
        match GltfMesh::load(&renderer.device, &renderer.queue, renderer.mesh_texture_layout(), &full_path) {
            Ok(mesh) => {
                let model_uniform = renderer.create_model_uniform();
                mesh_cache.insert(path.to_string(), (mesh, model_uniform));
            }
            Err(e) => log::warn!("space_soup_editor: grab pose editor couldn't load {path}: {e}"),
        }
    }
}

/// Updates the cached `GltfMesh` transforms (object mesh, if any, plus the
/// preview hand at the active point) for this frame — mirrors the
/// per-frame mutation loop in `render/mod.rs::redraw` for the normal scene.
pub(crate) fn update_transforms(
    state: &GrabPoseEditorState,
    scene: &Scene,
    mesh_cache: &mut HashMap<String, (GltfMesh, ModelUniform)>,
) {
    let Some(obj) = scene.find_object(&state.object_id) else { return };
    let reference = reference_transform(state.preview_rotation, obj.cuboid.rotation);

    if let Some(mesh_ref) = &obj.mesh {
        if let Some((mesh, _)) = mesh_cache.get_mut(&mesh_ref.path) {
            let (_, rot, pos) = reference.to_scale_rotation_translation();
            mesh.position = pos;
            mesh.rotation = rot;
            mesh.scale = mesh_ref.scale;
        }
    }

    let Some(point) = obj.grip_points.get(state.active_point) else { return };
    if let Some((mesh, _)) = mesh_cache.get_mut(hand_glb_path(state.preview_hand)) {
        let root = point_root(reference, point);
        let (_, rot, pos) = root.to_scale_rotation_translation();
        mesh.position = pos;
        mesh.rotation = rot;
        mesh.scale = Vec3::from(point.hand_offset_scale);
    }
}

const MARKER_HALF: f32 = 0.015;

fn marker_color(kind: GripKind, active: bool) -> Color3 {
    match (kind, active) {
        (GripKind::Snap, true)  => Color3(90, 225, 255, 255),
        (GripKind::Snap, false) => Color3(40, 110, 140, 255),
        (GripKind::Free, true)  => Color3(255, 195, 60, 255),
        (GripKind::Free, false) => Color3(150, 105, 30, 255),
    }
}

/// Builds this frame's cuboids: the target object itself (only when it has
/// no mesh — meshed objects are drawn via `collect_mesh_instances`) plus a
/// small color-coded marker at every grip point (bright = active point,
/// dim = the others) so multi-point objects show all their grips at once.
pub(crate) fn collect_cuboids(state: &GrabPoseEditorState, scene: &Scene) -> Vec<Cuboid> {
    let Some(obj) = scene.find_object(&state.object_id) else { return Vec::new() };

    let reference = reference_transform(state.preview_rotation, obj.cuboid.rotation);
    let mut out = Vec::new();

    if obj.mesh.is_none() {
        let (_, rot, pos) = reference.to_scale_rotation_translation();
        let col = obj.cuboid.color;
        let mut c = Cuboid::solid(pos, obj.cuboid.half_size, Color3(col.0, col.1, col.2, col.3));
        c.rotation = rot;
        out.push(c);
    }

    for (i, point) in obj.grip_points.iter().enumerate() {
        let active = i == state.active_point;
        let root = point_root(reference, point);
        let (_, rot, pos) = root.to_scale_rotation_translation();
        let half = if active { MARKER_HALF * 1.6 } else { MARKER_HALF };
        let mut c = Cuboid::solid(pos, Vec3::splat(half), marker_color(point.kind, active));
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
    let Some(obj) = scene.find_object(&state.object_id) else { return out };

    if let Some(mesh_ref) = &obj.mesh {
        if let Some((mesh, model)) = mesh_cache.get(&mesh_ref.path) {
            out.push(MeshInstance { mesh, model });
        }
    }

    if obj.grip_points.get(state.active_point).is_some() {
        if let Some((mesh, model)) = mesh_cache.get(hand_glb_path(state.preview_hand)) {
            out.push(MeshInstance { mesh, model });
        }
    }
    out
}

/// Positions `app.grab_pose_gizmo` at the active grip point's current root
/// transform, unless a drag is already in progress (matching
/// `scene::sync_gizmo_and_collect`'s `is_dragging` guard) or preview mode
/// is on (the gizmo is hidden there — see `render/mod.rs`).
pub(crate) fn sync_gizmo(app: &mut App) {
    let Some(state) = &app.grab_pose_editor else { return };
    if state.preview_mode { return; }
    let obj_id = state.object_id.clone();
    let active_point = state.active_point;
    let preview_rotation = state.preview_rotation;
    let is_dragging = app.gizmo_dragging;

    let Some(obj) = app.runtime.scene().find_object(&obj_id) else { return };
    let Some(point) = obj.grip_points.get(active_point) else { return };
    let reference = reference_transform(preview_rotation, obj.cuboid.rotation);
    let root = point_root(reference, point);
    let (_, rot, pos) = root.to_scale_rotation_translation();
    let scale = Vec3::from(point.hand_offset_scale);

    if !is_dragging {
        app.grab_pose_gizmo.set_position(pos);
        app.grab_pose_gizmo.set_rotation(rot);
        app.grab_pose_gizmo.set_scale(scale);
    }
}

fn snap_vec3(v: Vec3, step: f32) -> Vec3 {
    if step <= 0.0 { return v; }
    Vec3::new((v.x / step).round() * step, (v.y / step).round() * step, (v.z / step).round() * step)
}

/// Snaps each of the three UI-labeled euler components to the nearest
/// multiple of `step_deg` — same `EulerRot::YXZ` convention (and the same
/// slightly-confusing (ex, ey, ez) <-> (Y, X, Z) argument order) already
/// used by `render/inspector.rs`'s rotation fields, kept identical here so
/// both editors agree on what a given (X, Y, Z) degree triple means.
fn snap_rotation(q: Quat, step_deg: f32) -> Quat {
    if step_deg <= 0.0 { return q; }
    let (ex, ey, ez) = q.to_euler(EulerRot::YXZ);
    let snap = |a: f32| (a.to_degrees() / step_deg).round() * step_deg;
    Quat::from_euler(EulerRot::YXZ, snap(ey).to_radians(), snap(ex).to_radians(), snap(ez).to_radians())
}

/// Writes the dragged `grab_pose_gizmo` transform back into the active grip
/// point — Translate/Rotate solve for the offset via the same
/// inverse-compose trick `snap::seed_grip_pose` uses
/// (`offset = reference^-1 * root`); Scale is stored directly since it's
/// visual-only (the preview hand mesh) and not defined relative to
/// anything.
pub(crate) fn apply_gizmo_drag(app: &mut App) {
    let Some(state) = &app.grab_pose_editor else { return };
    let obj_id = state.object_id.clone();
    let active_point = state.active_point;
    let pos_snap = state.pos_snap;
    let rot_snap_deg = state.rot_snap_deg;
    let mode = app.grab_pose_gizmo.mode;

    let mut gizmo_pos = app.grab_pose_gizmo.get_position();
    let mut gizmo_rot = app.grab_pose_gizmo.get_rotation();
    let gizmo_scale = app.grab_pose_gizmo.get_scale();

    if mode == GizmoMode::Translate {
        if let Some(step) = pos_snap { gizmo_pos = snap_vec3(gizmo_pos, step); }
    } else if mode == GizmoMode::Rotate {
        if let Some(step) = rot_snap_deg { gizmo_rot = snap_rotation(gizmo_rot, step); }
    }

    let Some(obj) = app.runtime.scene_mut().find_object_mut(&obj_id) else { return };
    let reference = reference_transform(Quat::IDENTITY, obj.cuboid.rotation);
    let Some(point) = obj.grip_points.get_mut(active_point) else { return };

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
        let Some(state) = app.grab_pose_editor.as_ref() else { return };
        (state.object_id.clone(), state.active_point)
    };
    let before = app.runtime.scene().find_object(&obj_id)
        .and_then(|o| o.grip_points.get(active_point).cloned());
    if let Some(state) = app.grab_pose_editor.as_mut() {
        state.drag_before = before;
    }
}

pub(crate) fn end_drag_commit(app: &mut App) {
    let (obj_id, active_point, before) = {
        let Some(state) = app.grab_pose_editor.as_ref() else { return };
        let Some(before) = state.drag_before.clone() else { return };
        (state.object_id.clone(), state.active_point, before)
    };
    let after = app.runtime.scene().find_object(&obj_id)
        .and_then(|o| o.grip_points.get(active_point).cloned());
    if let Some(state) = app.grab_pose_editor.as_mut() {
        state.drag_before = None;
        if let Some(after) = after {
            let changed = before.local_pos != after.local_pos
                || before.local_rot != after.local_rot
                || before.hand_offset_scale != after.hand_offset_scale;
            if changed {
                state.undo.push(GrabPointEdit { point_index: active_point, before, after });
                state.redo.clear();
            }
        }
    }
}

pub(crate) fn reset_active_point(app: &mut App) {
    begin_drag_snapshot(app);
    let (obj_id, active_point) = {
        let Some(state) = app.grab_pose_editor.as_ref() else { return };
        (state.object_id.clone(), state.active_point)
    };
    if let Some(obj) = app.runtime.scene_mut().find_object_mut(&obj_id) {
        if let Some(point) = obj.grip_points.get_mut(active_point) {
            let name = point.name.clone();
            let kind = point.kind;
            *point = GripPointDef { name, kind, ..default_grip_point(&[]) };
            app.scene_dirty = true;
        }
    }
    end_drag_commit(app);
}

pub(crate) fn undo(app: &mut App) {
    let (edit, obj_id) = {
        let Some(state) = app.grab_pose_editor.as_mut() else { return };
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
        let Some(state) = app.grab_pose_editor.as_mut() else { return };
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

/// Selects a different grip point to edit — clamped to the current
/// `grip_points` length so a stale index (e.g. after a delete) can't panic.
pub(crate) fn select_point(app: &mut App, index: usize) {
    let Some(state) = app.grab_pose_editor.as_mut() else { return };
    let obj_id = state.object_id.clone();
    let len = app.runtime.scene().find_object(&obj_id).map(|o| o.grip_points.len()).unwrap_or(0);
    if let Some(state) = app.grab_pose_editor.as_mut() {
        state.active_point = index.min(len.saturating_sub(1));
    }
}

/// Adds a new grip point (default name/pose) and selects it.
pub(crate) fn add_point(app: &mut App) {
    let Some(state) = app.grab_pose_editor.as_ref() else { return };
    let obj_id = state.object_id.clone();
    let Some(obj) = app.runtime.scene_mut().find_object_mut(&obj_id) else { return };
    let point = default_grip_point(&obj.grip_points);
    obj.grip_points.push(point);
    let new_index = obj.grip_points.len() - 1;
    if let Some(state) = app.grab_pose_editor.as_mut() {
        state.active_point = new_index;
    }
    app.scene_dirty = true;
}

/// Deletes the active grip point (a no-op if it's the last one — an object
/// being grab-pose-edited should always have at least one point to show).
pub(crate) fn delete_active_point(app: &mut App) {
    let Some(state) = app.grab_pose_editor.as_ref() else { return };
    let obj_id = state.object_id.clone();
    let active_point = state.active_point;
    let Some(obj) = app.runtime.scene_mut().find_object_mut(&obj_id) else { return };
    if obj.grip_points.len() <= 1 { return; }
    obj.grip_points.remove(active_point);
    if let Some(state) = app.grab_pose_editor.as_mut() {
        state.active_point = active_point.min(obj.grip_points.len() - 1);
        state.undo.clear();
        state.redo.clear();
    }
    app.scene_dirty = true;
}

/// Renames the active grip point — rejects blank names and names that
/// collide with another point on the same object (grip point names are
/// looked up by string at grab time, so they must stay unique per object).
pub(crate) fn rename_active_point(app: &mut App, new_name: String) {
    let trimmed = new_name.trim();
    if trimmed.is_empty() { return; }
    let Some(state) = app.grab_pose_editor.as_ref() else { return };
    let obj_id = state.object_id.clone();
    let active_point = state.active_point;
    let Some(obj) = app.runtime.scene_mut().find_object_mut(&obj_id) else { return };
    if obj.grip_points.iter().enumerate().any(|(i, p)| i != active_point && p.name == trimmed) {
        return;
    }
    if let Some(point) = obj.grip_points.get_mut(active_point) {
        point.name = trimmed.to_string();
        app.scene_dirty = true;
    }
}

pub(crate) fn set_active_point_kind(app: &mut App, kind: GripKind) {
    let Some(state) = app.grab_pose_editor.as_ref() else { return };
    let obj_id = state.object_id.clone();
    let active_point = state.active_point;
    if let Some(obj) = app.runtime.scene_mut().find_object_mut(&obj_id) {
        if let Some(point) = obj.grip_points.get_mut(active_point) {
            point.kind = kind;
            app.scene_dirty = true;
        }
    }
}

/// While in Live Grab Preview mode, dragging in the viewport spins the
/// object (a "trackball" style free rotate) instead of moving the gizmo —
/// the preview hand recomputes from `point_root`, so it visibly stays
/// attached as the object turns. Never persisted; resets to identity
/// whenever preview mode is turned off so editing always resumes from the
/// runtime-accurate basis (see `reference_transform`).
pub(crate) fn preview_drag(app: &mut App, dx: f32, dy: f32) {
    if let Some(state) = app.grab_pose_editor.as_mut() {
        let yaw = Quat::from_rotation_y(-dx * 0.006);
        let pitch = Quat::from_rotation_x(-dy * 0.006);
        state.preview_rotation = yaw * pitch * state.preview_rotation;
    }
}

/// Which numeric field in the side panel was edited — used by
/// `apply_field_edit` to write the value into the right slot of the active
/// grip point (with undo support, same as a gizmo drag).
#[derive(Clone, Copy)]
pub(crate) enum PoseField {
    Pos(usize),
    Rot(usize),
    Scale(usize),
}

/// Applies a typed edit from the side panel's position/rotation/scale
/// fields — same euler convention as `snap_rotation`/`render/inspector.rs`.
pub(crate) fn apply_field_edit(app: &mut App, field: PoseField, value: f32) {
    begin_drag_snapshot(app);
    let (obj_id, active_point) = {
        let Some(state) = app.grab_pose_editor.as_ref() else { return };
        (state.object_id.clone(), state.active_point)
    };
    if let Some(obj) = app.runtime.scene_mut().find_object_mut(&obj_id) {
        if let Some(point) = obj.grip_points.get_mut(active_point) {
            match field {
                PoseField::Pos(i) => point.local_pos[i] = value,
                PoseField::Rot(i) => {
                    let q = Quat::from_array(point.local_rot);
                    let (ex, ey, ez) = q.to_euler(EulerRot::YXZ);
                    let mut deg = [ex.to_degrees(), ey.to_degrees(), ez.to_degrees()];
                    deg[i] = value;
                    let nq = Quat::from_euler(EulerRot::YXZ, deg[1].to_radians(), deg[0].to_radians(), deg[2].to_radians());
                    point.local_rot = nq.to_array();
                }
                PoseField::Scale(i) => point.hand_offset_scale[i] = value,
            }
            app.scene_dirty = true;
        }
    }
    end_drag_commit(app);
}
