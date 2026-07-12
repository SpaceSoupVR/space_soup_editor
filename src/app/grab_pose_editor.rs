use std::collections::HashMap;

use glam::{EulerRot, Mat4, Quat, Vec3};

use space_soup::renderer::mesh_pipeline::ModelUniform;
use space_soup::renderer::{Color3, Cuboid, GltfMesh, MeshInstance, Renderer};
use space_soup_engine::{GameObject, GripKind, GripPointDef, Hand, Scene};

use super::snap::{compute_skin_matrices, hand_glb_path};
use super::App;

/// Cache key for the grab-pose editor's own copy of a hand mesh. Distinct from
/// the raw `hand_glb_path` key so we always hold a *skinned* instance (with a
/// joint bind group). The main scene view preloads the same GLB under its raw
/// path as a plain, unskinned mesh (setup.rs); reusing that entry would give the
/// grab-pose hand no joint bind group, and the renderer draws such a mesh in
/// neither pass — i.e. it would be invisible.
pub(crate) fn hand_cache_key(hand: Hand) -> String {
    format!("__grabpose_skinned__/{}", hand_glb_path(hand))
}

fn identity_quat_arr() -> [f32; 4] {
    Quat::IDENTITY.to_array()
}
fn one_vec3_arr() -> [f32; 3] {
    Vec3::ONE.to_array()
}

fn default_grip_point(existing: &[GripPointDef], hand: Hand) -> GripPointDef {
    let base = match hand {
        Hand::Left => "left_grip",
        Hand::Right => "right_grip",
    };
    let mut n = 1;
    let name = loop {
        let candidate = if n == 1 {
            base.to_string()
        } else {
            format!("{base}_{n}")
        };
        if !existing.iter().any(|p| p.name == candidate) {
            break candidate;
        }
        n += 1;
    };
    GripPointDef {
        name,
        kind: GripKind::Snap,
        hand,
        local_pos: [0.0; 3],
        local_rot: identity_quat_arr(),
        hand_offset_scale: one_vec3_arr(),
        finger_curl: HashMap::new(),
    }
}

/// Which grip points the viewport and point list show — display-only
/// organization, never touching saved data. `All` (the default) shows
/// everything; a handed view shows just that hand's points.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum HandView {
    All,
    Left,
    Right,
}

impl HandView {
    pub(crate) fn shows(self, point: &GripPointDef) -> bool {
        match self {
            HandView::All => true,
            HandView::Left => point.hand == Hand::Left,
            HandView::Right => point.hand == Hand::Right,
        }
    }
}

#[derive(Clone, PartialEq)]
struct GripSnapshot {
    points: Vec<GripPointDef>,
}

impl GripSnapshot {
    fn of(obj: &GameObject) -> Self {
        Self {
            points: obj.grip_points.clone(),
        }
    }

    fn restore(&self, obj: &mut GameObject) {
        obj.grip_points = self.points.clone();
    }
}

/// Identifies a continuous drag on one field so its many per-frame edits fold
/// into a single undo entry instead of flooding (and evicting) the stack.
/// `(point_index, field_code)`.
#[derive(Clone, Copy, PartialEq, Eq)]
struct EditKey(usize, u8);

fn field_code(f: PoseField) -> u8 {
    match f {
        PoseField::Pos(i) => 1 + i as u8,
        PoseField::Rot(i) => 4 + i as u8,
        PoseField::Scale(i) => 7 + i as u8,
    }
}

const FINGER_FIELD_BASE: u8 = 10;
/// Field codes for the snap-step drag fields (kept clear of pose/finger codes).
const POS_SNAP_STEP_CODE: u8 = 30;
const ROT_SNAP_STEP_CODE: u8 = 31;

/// One undoable state: the grip points plus the editor's snap tool settings, so
/// those toggles/steps are undoable too, not just the pose fields.
#[derive(Clone, PartialEq)]
struct GripUndoState {
    data: GripSnapshot,
    pos_snap: Option<f32>,
    rot_snap_deg: Option<f32>,
}

impl GripUndoState {
    fn capture(obj: &GameObject, state: &GrabPoseEditorState) -> Self {
        Self {
            data: GripSnapshot::of(obj),
            pos_snap: state.pos_snap,
            rot_snap_deg: state.rot_snap_deg,
        }
    }

    fn restore(&self, obj: &mut GameObject, state: &mut GrabPoseEditorState) {
        self.data.restore(obj);
        state.pos_snap = self.pos_snap;
        state.rot_snap_deg = self.rot_snap_deg;
    }
}

#[derive(Clone)]
struct GripEdit {
    before: GripUndoState,
    after: GripUndoState,
    /// The drag this entry belongs to, if any; used to coalesce follow-up frames.
    coalesce: Option<EditKey>,
}

const UNDO_CAP: usize = 100;

pub(crate) struct GrabPoseEditorState {
    pub object_id: String,
    pub orbit: super::orbit_camera::OrbitCamera,
    pub active_point: usize,
    pub hand_view: HandView,
    pub pos_snap: Option<f32>,
    pub rot_snap_deg: Option<f32>,

    /// The "you have unsaved changes" dialog is up; the panel draws only it.
    pub confirm_exit: bool,

    pub content_height: f32,

    /// Grip points as of open/last save. Exit-without-saving restores this, so
    /// edits stay local to the editor until saved (main-view Save Scene then
    /// persists them to disk with the rest of the scene).
    saved: GripSnapshot,

    /// Sticky euler angles for the grip rotation currently being edited:
    /// `(point_index, [x, y, z] degrees)`. Editing rotation through a quat
    /// round-trip hits gimbal lock (in YXZ order the middle X axis locks near
    /// ±90°, collapsing Y and Z so they move together). Holding the euler here
    /// and only rebuilding the quat keeps the three fields independent.
    rot_edit: Option<(usize, [f32; 3])>,

    /// The drag currently coalescing into the top undo entry; cleared each frame
    /// no field edit arrives (i.e. the drag ended).
    active_coalesce: Option<EditKey>,

    undo: Vec<GripEdit>,
    redo: Vec<GripEdit>,
}

impl GrabPoseEditorState {
    fn new(object_id: String, framing_radius: f32, saved: GripSnapshot) -> Self {
        Self {
            object_id,
            orbit: super::orbit_camera::OrbitCamera::new(framing_radius),
            active_point: 0,
            hand_view: HandView::All,
            pos_snap: None,
            rot_snap_deg: None,
            confirm_exit: false,
            content_height: 900.0,
            saved,
            rot_edit: None,
            active_coalesce: None,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    /// Ends any in-progress drag coalescing. Called each frame no field edit is
    /// received, so the next drag starts a fresh undo entry.
    pub(crate) fn end_coalesce(&mut self) {
        self.active_coalesce = None;
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Whether the grip points differ from the last save (or open).
    pub fn dirty(&self, scene: &Scene) -> bool {
        scene
            .find_object(&self.object_id)
            .map(|o| GripSnapshot::of(o) != self.saved)
            .unwrap_or(false)
    }

    /// Euler angles (degrees, `[x, y, z]`) to show/edit for a grip point's
    /// rotation. Prefers the sticky `rot_edit` value mid-edit so repeated drags
    /// don't re-decompose the quat (which collapses Y/Z near gimbal lock).
    pub(crate) fn euler_for_point(&self, point_idx: usize, q: Quat) -> [f32; 3] {
        if let Some((p, deg)) = self.rot_edit {
            if p == point_idx {
                return deg;
            }
        }
        let (ey, ex, ez) = q.to_euler(EulerRot::YXZ);
        [ex.to_degrees(), ey.to_degrees(), ez.to_degrees()]
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

/// The object sits at the origin in its native orientation; the orbit camera
/// provides the viewing angle.
fn reference_transform() -> Mat4 {
    Mat4::IDENTITY
}

fn point_root(reference: Mat4, point: &GripPointDef) -> Mat4 {
    let offset_mat = Mat4::from_rotation_translation(
        Quat::from_array(point.local_rot),
        Vec3::from(point.local_pos),
    );
    reference * offset_mat
}

// ---------------------------------------------------------------------------
// Open / save / exit — edits live in the scene while the editor is open, but
// only stick if saved; exiting restores the last-saved state. Saving here only
// commits to the in-memory scene: writing to disk stays with the main view's
// Save Scene button.
// ---------------------------------------------------------------------------

pub(crate) fn open(app: &mut App, object_id: String) {
    if let Some(obj) = app.runtime.scene_mut().find_object_mut(&object_id) {
        if obj.grip_points.is_empty() {
            let point = default_grip_point(&obj.grip_points, Hand::Right);
            obj.grip_points.push(point);
        }
    }

    let Some(obj) = app.runtime.scene().find_object(&object_id) else {
        return;
    };
    let framing = obj.cuboid.half_size.length().max(0.05);
    let saved = GripSnapshot::of(obj);

    app.grab_pose_editor = Some(GrabPoseEditorState::new(object_id, framing, saved));
}

fn close(app: &mut App) {
    app.grab_pose_editor = None;
}

/// Commit the current grip points as the saved state (kept when exiting).
pub(crate) fn save(app: &mut App) {
    let Some(state) = app.grab_pose_editor.as_mut() else {
        return;
    };
    if let Some(obj) = app.runtime.scene().find_object(&state.object_id) {
        state.saved = GripSnapshot::of(obj);
        app.scene_dirty = true;
    }
}

/// Exit button / Escape: close right away when clean, else ask.
pub(crate) fn request_exit(app: &mut App) {
    let dirty = app
        .grab_pose_editor
        .as_ref()
        .map(|s| s.dirty(app.runtime.scene()))
        .unwrap_or(false);
    if let Some(state) = app.grab_pose_editor.as_mut() {
        if dirty {
            state.confirm_exit = true;
        } else {
            close(app);
        }
    }
}

/// Confirm dialog "Exit": throw away everything since the last save.
pub(crate) fn exit_discard(app: &mut App) {
    if let Some(state) = app.grab_pose_editor.as_ref() {
        let saved = state.saved.clone();
        let obj_id = state.object_id.clone();
        if let Some(obj) = app.runtime.scene_mut().find_object_mut(&obj_id) {
            saved.restore(obj);
        }
    }
    close(app);
}

/// Confirm dialog "Save then Exit".
pub(crate) fn exit_save(app: &mut App) {
    save(app);
    close(app);
}

/// Confirm dialog "Return": stay in the editor.
pub(crate) fn cancel_exit(app: &mut App) {
    if let Some(state) = app.grab_pose_editor.as_mut() {
        state.confirm_exit = false;
    }
}

/// Point the camera back at the object (drawn at the origin) after
/// panning/zooming away.
pub(crate) fn recenter_view(app: &mut App) {
    let Some(state) = app.grab_pose_editor.as_ref() else {
        return;
    };
    let framing = app
        .runtime
        .scene()
        .find_object(&state.object_id)
        .map(|o| o.cuboid.half_size.length().max(0.05))
        .unwrap_or(0.3);
    if let Some(state) = app.grab_pose_editor.as_mut() {
        state.orbit.refocus(Vec3::ZERO, framing);
    }
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

/// The active point, only when the current hand view shows it (a handed view
/// with no points of that hand leaves nothing to edit or preview).
fn visible_active_point<'a>(state: &GrabPoseEditorState, scene: &'a Scene) -> Option<&'a GripPointDef> {
    scene
        .find_object(&state.object_id)?
        .grip_points
        .get(state.active_point)
        .filter(|p| state.hand_view.shows(p))
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
    let reference = reference_transform();

    if let Some(mesh_ref) = &obj.mesh {
        if let Some((mesh, _)) = mesh_cache.get_mut(&mesh_ref.path) {
            let (_, rot, pos) = reference.to_scale_rotation_translation();
            mesh.position = pos;
            mesh.rotation = rot;
            mesh.scale = mesh_ref.scale;
        }
    }

    let Some(point) = visible_active_point(state, scene) else {
        return;
    };
    let hand_key = hand_cache_key(point.hand);
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

    let reference = reference_transform();
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
        // The hand view filters what shows (organization only). The active
        // point shows the posed hand instead of a marker cube — only the other
        // (unselected) points get a cube so they stay visible/pickable.
        if !state.hand_view.shows(point) || i == state.active_point {
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

    if let Some(point) = visible_active_point(state, scene) {
        if let Some((mesh, model)) = mesh_cache.get(&hand_cache_key(point.hand)) {
            out.push(MeshInstance { mesh, model });
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Undo/redo plumbing — every mutation goes through `with_edit`, which
// snapshots the whole grip-point list (so add/delete/rename/kind are
// undoable too, not just pose fields). Note edits do NOT mark the scene
// dirty: that happens on save (exiting without saving restores everything).
// ---------------------------------------------------------------------------

fn with_edit(app: &mut App, f: impl FnOnce(&mut GameObject, &mut GrabPoseEditorState)) {
    with_edit_coalesced(app, None, f);
}

/// Like `with_edit`, but when `token` matches the drag already folding into the
/// top undo entry, the entry's `after` is extended in place rather than pushing
/// a new one. Keeps a whole drag as one undoable step so undo isn't reduced to
/// nudging back single frames (which also used to evict older real edits).
fn with_edit_coalesced(
    app: &mut App,
    token: Option<EditKey>,
    f: impl FnOnce(&mut GameObject, &mut GrabPoseEditorState),
) {
    let Some(mut state) = app.grab_pose_editor.take() else {
        return;
    };
    let obj_id = state.object_id.clone();
    if let Some(obj) = app.runtime.scene_mut().find_object_mut(&obj_id) {
        let before = GripUndoState::capture(obj, &state);
        f(obj, &mut state);
        let after = GripUndoState::capture(obj, &state);
        if before != after {
            let merge = token.is_some()
                && state.active_coalesce == token
                && state.redo.is_empty()
                && state.undo.last().map_or(false, |e| e.coalesce == token);
            if merge {
                state.undo.last_mut().unwrap().after = after;
            } else {
                state.undo.push(GripEdit {
                    before,
                    after,
                    coalesce: token,
                });
                if state.undo.len() > UNDO_CAP {
                    state.undo.remove(0);
                }
                state.redo.clear();
            }
            state.active_coalesce = token;
        }
        state.active_point = state.active_point.min(obj.grip_points.len().saturating_sub(1));
    }
    app.grab_pose_editor = Some(state);
}

pub(crate) fn undo(app: &mut App) {
    let Some(mut state) = app.grab_pose_editor.take() else {
        return;
    };
    if let Some(edit) = state.undo.pop() {
        let obj_id = state.object_id.clone();
        if let Some(obj) = app.runtime.scene_mut().find_object_mut(&obj_id) {
            edit.before.restore(obj, &mut state);
            state.active_point = state.active_point.min(obj.grip_points.len().saturating_sub(1));
        }
        state.redo.push(edit);
    }
    state.rot_edit = None;
    state.active_coalesce = None;
    app.grab_pose_editor = Some(state);
}

pub(crate) fn redo(app: &mut App) {
    let Some(mut state) = app.grab_pose_editor.take() else {
        return;
    };
    if let Some(edit) = state.redo.pop() {
        let obj_id = state.object_id.clone();
        if let Some(obj) = app.runtime.scene_mut().find_object_mut(&obj_id) {
            edit.after.restore(obj, &mut state);
            state.active_point = state.active_point.min(obj.grip_points.len().saturating_sub(1));
        }
        state.undo.push(edit);
    }
    state.rot_edit = None;
    state.active_coalesce = None;
    app.grab_pose_editor = Some(state);
}

// ---------------------------------------------------------------------------
// Snap tool settings — routed through `with_edit` so they're undoable too.
// ---------------------------------------------------------------------------

pub(crate) fn set_pos_snap(app: &mut App, snap: Option<f32>) {
    with_edit(app, |_, state| state.pos_snap = snap);
}

pub(crate) fn set_rot_snap(app: &mut App, snap: Option<f32>) {
    with_edit(app, |_, state| state.rot_snap_deg = snap);
}

pub(crate) fn set_pos_snap_step(app: &mut App, step: f32) {
    let token = Some(EditKey(usize::MAX, POS_SNAP_STEP_CODE));
    with_edit_coalesced(app, token, |_, state| state.pos_snap = Some(step));
}

pub(crate) fn set_rot_snap_step(app: &mut App, step: f32) {
    let token = Some(EditKey(usize::MAX, ROT_SNAP_STEP_CODE));
    with_edit_coalesced(app, token, |_, state| state.rot_snap_deg = Some(step));
}

// ---------------------------------------------------------------------------
// Grip point operations
// ---------------------------------------------------------------------------

pub(crate) fn apply_finger_curl(app: &mut App, group_idx: usize, value: f32) {
    let token = app
        .grab_pose_editor
        .as_ref()
        .map(|s| EditKey(s.active_point, FINGER_FIELD_BASE + group_idx as u8));
    with_edit_coalesced(app, token, |obj, state| {
        let Some(point) = obj.grip_points.get_mut(state.active_point) else {
            return;
        };
        let Some((_, bones)) = FINGER_GROUPS.get(group_idx) else {
            return;
        };
        let v = value.clamp(0.0, 1.0);
        for bone in *bones {
            point.finger_curl.insert(bone.to_string(), v);
        }
    });
}

pub(crate) fn reset_active_point(app: &mut App) {
    with_edit(app, |obj, state| {
        state.rot_edit = None;
        if let Some(point) = obj.grip_points.get_mut(state.active_point) {
            let name = point.name.clone();
            let kind = point.kind;
            let hand = point.hand;
            *point = GripPointDef {
                name,
                kind,
                ..default_grip_point(&[], hand)
            };
        }
    });
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
        state.rot_edit = None;
    }
}

/// Switch the hand view; keeps the selection on a point that view can show.
pub(crate) fn set_hand_view(app: &mut App, view: HandView) {
    let Some(state) = app.grab_pose_editor.as_mut() else {
        return;
    };
    state.hand_view = view;
    let obj_id = state.object_id.clone();
    let active = state.active_point;
    let reselect = app.runtime.scene().find_object(&obj_id).and_then(|obj| {
        match obj.grip_points.get(active) {
            Some(p) if view.shows(p) => None,
            _ => obj.grip_points.iter().position(|p| view.shows(p)),
        }
    });
    if let (Some(i), Some(state)) = (reselect, app.grab_pose_editor.as_mut()) {
        state.active_point = i;
        state.rot_edit = None;
    }
}

pub(crate) fn add_point(app: &mut App, hand: Hand) {
    with_edit(app, |obj, state| {
        let point = default_grip_point(&obj.grip_points, hand);
        obj.grip_points.push(point);
        state.active_point = obj.grip_points.len() - 1;
        state.rot_edit = None;
        // Jump the view to where the new point lives, so it never lands
        // somewhere the current view hides.
        if state.hand_view != HandView::All {
            state.hand_view = match hand {
                Hand::Left => HandView::Left,
                Hand::Right => HandView::Right,
            };
        }
    });
}

pub(crate) fn delete_active_point(app: &mut App) {
    with_edit(app, |obj, state| {
        if obj.grip_points.len() <= 1 {
            return;
        }
        obj.grip_points.remove(state.active_point);
        state.rot_edit = None;
    });
}

pub(crate) fn rename_active_point(app: &mut App, new_name: String) {
    let trimmed = new_name.trim().to_string();
    if trimmed.is_empty() {
        return;
    }
    with_edit(app, |obj, state| {
        let taken = obj
            .grip_points
            .iter()
            .enumerate()
            .any(|(i, p)| i != state.active_point && p.name == trimmed);
        if taken {
            return;
        }
        if let Some(point) = obj.grip_points.get_mut(state.active_point) {
            point.name = trimmed;
        }
    });
}

pub(crate) fn set_active_point_kind(app: &mut App, kind: GripKind) {
    with_edit(app, |obj, state| {
        if let Some(point) = obj.grip_points.get_mut(state.active_point) {
            point.kind = kind;
        }
    });
}

#[derive(Clone, Copy)]
pub(crate) enum PoseField {
    Pos(usize),
    /// Euler degrees, axis-indexed [X, Y, Z] (stored back via YXZ order).
    Rot(usize),
    Scale(usize),
}

pub(crate) fn apply_field_edit(app: &mut App, field: PoseField, value: f32) {
    // Fold this field's drag frames into one undo entry.
    let token = app
        .grab_pose_editor
        .as_ref()
        .map(|s| EditKey(s.active_point, field_code(field)));
    with_edit_coalesced(app, token, |obj, state| {
        let Some(point) = obj.grip_points.get_mut(state.active_point) else {
            return;
        };
        match field {
            PoseField::Pos(i) => point.local_pos[i] = value,
            PoseField::Rot(i) => {
                // Axis-indexed [X, Y, Z]; glam's YXZ decomposition returns
                // (Y, X, Z), rebuilt below in that order. The sticky euler from
                // `euler_for_point` keeps the axes independent while editing —
                // re-decomposing the quat every frame collapses Y and Z near
                // the X = ±90° gimbal lock (they'd move together).
                let q = Quat::from_array(point.local_rot);
                let mut deg = state.euler_for_point(state.active_point, q);
                deg[i] = value;
                let nq = Quat::from_euler(
                    EulerRot::YXZ,
                    deg[1].to_radians(), // Y
                    deg[0].to_radians(), // X
                    deg[2].to_radians(), // Z
                );
                point.local_rot = nq.to_array();
                state.rot_edit = Some((state.active_point, deg));
            }
            PoseField::Scale(i) => point.hand_offset_scale[i] = value,
        }
    });
}
