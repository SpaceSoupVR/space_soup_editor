//! Animation Simulation Editor — full-screen overlay (cloned from the grab
//! pose editor's structure) for authoring keyframe animations and controller
//! bindings on a single `GameObject`, with live preview playback.

use glam::{EulerRot, Quat, Vec3};

use space_soup::renderer::mesh_pipeline::ModelUniform;
use space_soup::renderer::{Color3, Cuboid, GltfMesh, MeshInstance};
use space_soup_engine::animation::{sample, AnimationPlayer, Sample};
use space_soup_engine::{Animation, AnimationBinding, Easing, GameObject, Keyframe, Scene};

use super::App;

/// Snap grid choices for keyframe times (seconds).
pub(crate) const SNAP_STEPS: [f32; 4] = [0.05, 0.1, 0.25, 0.5];
/// Preview speed choices.
pub(crate) const SPEED_STEPS: [f32; 5] = [0.25, 0.5, 1.0, 2.0, 4.0];

/// Human labels paired with the engine's canonical binding button ids.
pub(crate) const BUTTON_OPTIONS: [(&str, &str); 6] = [
    ("A", "btn_a"),
    ("B", "btn_b"),
    ("X", "btn_x"),
    ("Y", "btn_y"),
    ("Trigger", "trigger"),
    ("Grip", "grip"),
];

pub(crate) fn button_label(id: &str) -> &'static str {
    BUTTON_OPTIONS
        .iter()
        .find(|(_, bid)| *bid == id)
        .map(|(label, _)| *label)
        .unwrap_or("?")
}

#[derive(Clone, PartialEq)]
struct AnimSnapshot {
    animations: Vec<Animation>,
    bindings: Vec<AnimationBinding>,
}

impl AnimSnapshot {
    fn of(obj: &GameObject) -> Self {
        Self {
            animations: obj.animations.clone(),
            bindings: obj.animation_bindings.clone(),
        }
    }

    fn restore(&self, obj: &mut GameObject) {
        obj.animations = self.animations.clone();
        obj.animation_bindings = self.bindings.clone();
    }
}

/// Identifies a continuous drag on one field so its many per-frame edits fold
/// into a single undo entry instead of flooding (and evicting) the stack.
/// `(anim_index, key_index, field_code)`.
#[derive(Clone, Copy, PartialEq, Eq)]
struct EditKey(usize, usize, u8);

fn field_code(f: KeyField) -> u8 {
    match f {
        KeyField::T => 0,
        KeyField::Pos(i) => 1 + i as u8,
        KeyField::RotEuler(i) => 4 + i as u8,
        KeyField::Scale(i) => 7 + i as u8,
    }
}

/// One undoable state: the object's animation data plus the editor tool
/// settings (snap grid, preview speed) so those are undoable too, not just
/// keyframe edits.
#[derive(Clone, PartialEq)]
struct AnimUndoState {
    data: AnimSnapshot,
    snap_step: Option<f32>,
    speed: f32,
}

impl AnimUndoState {
    fn capture(obj: &GameObject, state: &AnimSimEditorState) -> Self {
        Self {
            data: AnimSnapshot::of(obj),
            snap_step: state.snap_step,
            speed: state.speed,
        }
    }

    fn restore(&self, obj: &mut GameObject, state: &mut AnimSimEditorState) {
        self.data.restore(obj);
        state.snap_step = self.snap_step;
        state.speed = self.speed;
    }
}

#[derive(Clone)]
struct AnimEdit {
    before: AnimUndoState,
    after: AnimUndoState,
    /// The drag this entry belongs to, if any; used to coalesce follow-up frames.
    coalesce: Option<EditKey>,
}

const UNDO_CAP: usize = 100;

pub(crate) struct AnimSimEditorState {
    pub object_id: String,
    pub orbit: super::orbit_camera::OrbitCamera,

    /// The preview draws the object relative to this world point (rendered at
    /// the origin), so it opens centred and the object's world position in the
    /// base editor never shifts it here. Anchored to the animation's first
    /// positioned keyframe (fallback: the object's rest position); see
    /// [`compute_display_origin`].
    display_origin: Vec3,

    pub selected_anim: usize,
    pub selected_key: Option<usize>,

    /// Internal preview player; `elapsed` doubles as the scrub playhead.
    pub player: AnimationPlayer,
    pub playing: bool,
    pub speed: f32,

    /// Keyframe-time snapping grid (None = off).
    pub snap_step: Option<f32>,

    /// The "you have unsaved changes" dialog is up; the panel draws only it.
    pub confirm_exit: bool,

    pub content_height: f32,

    /// Animations + bindings as of open/last save. Exit-without-saving restores
    /// this, so edits stay local to the editor until saved (main-view Save
    /// Scene then persists them to disk with the rest of the scene).
    saved: AnimSnapshot,

    /// Sticky euler angles for the keyframe rotation currently being edited:
    /// `(anim_index, key_index, [x, y, z] degrees)`. Editing rotation through a
    /// quat round-trip hits gimbal lock (in YXZ order the middle X axis locks
    /// near ±90°, collapsing Y and Z). Holding the euler here and only rebuilding
    /// the quat keeps the three fields independent no matter the orientation.
    rot_edit: Option<(usize, usize, [f32; 3])>,

    /// The drag currently coalescing into the top undo entry; cleared each frame
    /// no field edit arrives (i.e. the drag ended).
    active_coalesce: Option<EditKey>,

    undo: Vec<AnimEdit>,
    redo: Vec<AnimEdit>,
}

impl AnimSimEditorState {
    fn new(
        object_id: String,
        display_origin: Vec3,
        framing_radius: f32,
        first_anim: &Animation,
        saved: AnimSnapshot,
    ) -> Self {
        // Object is drawn centred at the origin, so the camera targets the origin.
        let mut orbit = super::orbit_camera::OrbitCamera::new(framing_radius);
        orbit.target = Vec3::ZERO;
        Self {
            object_id,
            orbit,
            display_origin,
            selected_anim: 0,
            selected_key: if first_anim.keyframes.is_empty() {
                None
            } else {
                Some(0)
            },
            player: AnimationPlayer {
                anim_name: first_anim.name.clone(),
                elapsed: 0.0,
                looping: first_anim.looping,
                finished: false,
            },
            playing: false,
            speed: 1.0,
            snap_step: None,
            confirm_exit: false,
            content_height: 1600.0,
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

    /// Whether the animations/bindings differ from the last save (or open).
    pub fn dirty(&self, scene: &Scene) -> bool {
        scene
            .find_object(&self.object_id)
            .map(|o| AnimSnapshot::of(o) != self.saved)
            .unwrap_or(false)
    }

    /// Euler angles (degrees, `[x, y, z]`) to show/edit for a keyframe's
    /// rotation, expressed in the display (rest-relative) frame — `disp` is the
    /// display rotation offset, so `disp * q` is the rotation relative to rest.
    /// Editing here (rather than on the raw world quat) keeps the axes clean the
    /// way grab pose does, since the rest pose reads as (0, 0, 0) instead of a
    /// gimbal-locked laying-down orientation. Prefers the sticky `rot_edit` value
    /// mid-edit so repeated drags don't re-decompose.
    pub(crate) fn euler_for_key(
        &self,
        anim_idx: usize,
        key_idx: usize,
        q: Quat,
        disp: Quat,
    ) -> [f32; 3] {
        if let Some((a, k, deg)) = self.rot_edit {
            if a == anim_idx && k == key_idx {
                return deg;
            }
        }
        let (ey, ex, ez) = (disp * q).to_euler(EulerRot::YXZ);
        [ex.to_degrees(), ey.to_degrees(), ez.to_degrees()]
    }

    pub fn snap_time(&self, t: f32) -> f32 {
        match self.snap_step {
            Some(step) if step > 0.0 => (t / step).round() * step,
            _ => t,
        }
    }
}

fn unique_anim_name(existing: &[Animation], base: &str) -> String {
    let mut n = 1;
    loop {
        let candidate = if n == 1 && base != "anim" {
            base.to_string()
        } else {
            format!("{base}_{n}")
        };
        if !existing.iter().any(|a| a.name == candidate) {
            return candidate;
        }
        n += 1;
    }
}

/// A keyframe capturing the object's current scene transform. `scale` maps to
/// `cuboid.half_size` (matching how the runtime applies keyframe scale).
fn capture_keyframe(obj: &GameObject, t: f32) -> Keyframe {
    // The sim animates position + rotation + scale; color is left unset.
    Keyframe {
        t,
        position: Some(obj.cuboid.position),
        rotation: Some(obj.cuboid.rotation),
        scale: Some(obj.cuboid.half_size),
        color: None,
    }
}

fn default_animation(obj: &GameObject) -> Animation {
    Animation {
        name: unique_anim_name(&obj.animations, "anim"),
        keyframes: vec![capture_keyframe(obj, 0.0)],
        easing: Easing::default(),
        looping: false,
    }
}

// ---------------------------------------------------------------------------
// Open / save / exit — edits live in the scene while the editor is open, but
// only stick if saved; exiting restores the last-saved state. Saving here only
// commits to the in-memory scene: writing to disk stays with the main view's
// Save Scene button.
// ---------------------------------------------------------------------------

/// The display anchor (world origin of the preview) for an object + animation:
/// the animation's first positioned keyframe, falling back to the object's rest
/// position. Anchoring to the animation itself means the object's world
/// position in the base editor never shifts the preview, and it opens centred.
fn compute_display_origin(obj: &GameObject, anim: Option<&Animation>) -> Vec3 {
    anim.and_then(|a| a.keyframes.iter().find_map(|k| k.position))
        .unwrap_or(obj.cuboid.position)
}

pub(crate) fn open(app: &mut App, object_id: String) {
    if let Some(obj) = app.runtime.scene_mut().find_object_mut(&object_id) {
        if obj.animations.is_empty() {
            let anim = default_animation(obj);
            obj.animations.push(anim);
        }
    }

    let Some(obj) = app.runtime.scene().find_object(&object_id) else {
        return;
    };
    let first = obj.animations[0].clone();
    let origin = compute_display_origin(obj, Some(&first));
    let framing = obj.cuboid.half_size.length().max(0.05);
    let saved = AnimSnapshot::of(obj);

    app.anim_sim_editor = Some(AnimSimEditorState::new(
        object_id, origin, framing, &first, saved,
    ));
}

fn close(app: &mut App) {
    app.anim_sim_editor = None;
}

/// Commit the current animations/bindings as the saved state (kept when exiting).
pub(crate) fn save(app: &mut App) {
    let Some(state) = app.anim_sim_editor.as_mut() else {
        return;
    };
    if let Some(obj) = app.runtime.scene().find_object(&state.object_id) {
        state.saved = AnimSnapshot::of(obj);
        app.scene_dirty = true;
    }
}

/// Exit button: close right away when clean, else ask.
pub(crate) fn request_exit(app: &mut App) {
    let dirty = app
        .anim_sim_editor
        .as_ref()
        .map(|s| s.dirty(app.runtime.scene()))
        .unwrap_or(false);
    if let Some(state) = app.anim_sim_editor.as_mut() {
        if dirty {
            state.confirm_exit = true;
        } else {
            close(app);
        }
    }
}

/// Confirm dialog "Exit": throw away everything since the last save.
pub(crate) fn exit_discard(app: &mut App) {
    if let Some(state) = app.anim_sim_editor.as_ref() {
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
    if let Some(state) = app.anim_sim_editor.as_mut() {
        state.confirm_exit = false;
    }
}

/// Re-anchor and point the camera back at the object after panning/zooming
/// away. The object always sits at the origin, so this recentres on it.
pub(crate) fn recenter_view(app: &mut App) {
    let Some(state) = app.anim_sim_editor.as_ref() else {
        return;
    };
    let scene = app.runtime.scene();
    let Some(obj) = scene.find_object(&state.object_id) else {
        return;
    };
    let origin = compute_display_origin(obj, current_anim(state, scene));
    let framing = obj.cuboid.half_size.length().max(0.05);
    if let Some(state) = app.anim_sim_editor.as_mut() {
        state.display_origin = origin;
        state.orbit.refocus(Vec3::ZERO, framing);
    }
}

// ---------------------------------------------------------------------------
// Lookup helpers
// ---------------------------------------------------------------------------

pub(crate) fn current_anim<'a>(state: &AnimSimEditorState, scene: &'a Scene) -> Option<&'a Animation> {
    scene
        .find_object(&state.object_id)?
        .animations
        .get(state.selected_anim)
}

fn current_duration(state: &AnimSimEditorState, scene: &Scene) -> f32 {
    current_anim(state, scene).map(|a| a.duration()).unwrap_or(0.0)
}

/// Display-only rotation applied on top of the object's orientation in the
/// preview: cancels the resting rotation so the object shows upright/modeled.
/// Same for every mode; never persisted. Rotation editing also happens in this
/// frame, so the resting pose reads as (0, 0, 0) and the axes stay independent
/// (no gimbal lock from starting at a laying-down world orientation).
pub(crate) fn display_rotation_offset(obj: &GameObject) -> Quat {
    obj.cuboid.rotation.inverse()
}

/// Sampled preview values at the current playhead for the selected animation.
pub(crate) fn preview_sample(state: &AnimSimEditorState, scene: &Scene) -> Sample {
    match current_anim(state, scene) {
        Some(anim) => sample(anim, state.player.elapsed),
        None => Sample::default(),
    }
}

// ---------------------------------------------------------------------------
// Playback
// ---------------------------------------------------------------------------

pub(crate) fn update_playback(app: &mut App, dt: f32) {
    let Some(state) = app.anim_sim_editor.as_ref() else {
        return;
    };
    if !state.playing {
        return;
    }
    let duration = current_duration(state, app.runtime.scene());
    let looping = current_anim(state, app.runtime.scene())
        .map(|a| a.looping)
        .unwrap_or(false);
    if let Some(state) = app.anim_sim_editor.as_mut() {
        if duration <= 0.0 {
            state.playing = false;
            return;
        }
        state.player.looping = looping;
        state.player.finished = false;
        state.player.tick(dt * state.speed, duration);
        if state.player.finished {
            state.playing = false;
        }
    }
}

pub(crate) fn play(app: &mut App) {
    let restart = app
        .anim_sim_editor
        .as_ref()
        .map(|s| {
            let d = current_duration(s, app.runtime.scene());
            d > 0.0 && s.player.elapsed >= d
        })
        .unwrap_or(false);
    if let Some(state) = app.anim_sim_editor.as_mut() {
        if restart {
            state.player.elapsed = 0.0;
        }
        state.player.finished = false;
        state.playing = true;
    }
}

pub(crate) fn pause(app: &mut App) {
    if let Some(state) = app.anim_sim_editor.as_mut() {
        state.playing = false;
    }
}

pub(crate) fn stop(app: &mut App) {
    if let Some(state) = app.anim_sim_editor.as_mut() {
        state.playing = false;
        state.player.elapsed = 0.0;
        state.player.finished = false;
    }
}

pub(crate) fn toggle_play(app: &mut App) {
    let playing = app
        .anim_sim_editor
        .as_ref()
        .map(|s| s.playing)
        .unwrap_or(false);
    if playing {
        pause(app);
    } else {
        play(app);
    }
}

pub(crate) fn seek(app: &mut App, t: f32) {
    let duration = app
        .anim_sim_editor
        .as_ref()
        .map(|s| current_duration(s, app.runtime.scene()))
        .unwrap_or(0.0);
    if let Some(state) = app.anim_sim_editor.as_mut() {
        state.player.elapsed = t.clamp(0.0, duration.max(0.0));
        state.player.finished = false;
    }
}

pub(crate) fn step_playhead(app: &mut App, dir: f32) {
    let step = app
        .anim_sim_editor
        .as_ref()
        .and_then(|s| s.snap_step)
        .unwrap_or(0.05);
    let t = app
        .anim_sim_editor
        .as_ref()
        .map(|s| s.player.elapsed)
        .unwrap_or(0.0);
    seek(app, t + dir * step);
}

// ---------------------------------------------------------------------------
// Undo/redo plumbing — every mutation goes through `with_edit`. Note edits do
// NOT mark the scene dirty: that happens on save (exiting without saving
// restores everything).
// ---------------------------------------------------------------------------

fn with_edit(app: &mut App, f: impl FnOnce(&mut GameObject, &mut AnimSimEditorState)) {
    with_edit_coalesced(app, None, f);
}

/// Like `with_edit`, but when `token` matches the drag already folding into the
/// top undo entry, the entry's `after` is extended in place rather than pushing
/// a new one. Keeps a whole drag as one undoable step so undo isn't reduced to
/// nudging back single frames (which also used to evict older real edits).
fn with_edit_coalesced(
    app: &mut App,
    token: Option<EditKey>,
    f: impl FnOnce(&mut GameObject, &mut AnimSimEditorState),
) {
    let Some(mut state) = app.anim_sim_editor.take() else {
        return;
    };
    let obj_id = state.object_id.clone();
    if let Some(obj) = app.runtime.scene_mut().find_object_mut(&obj_id) {
        let before = AnimUndoState::capture(obj, &state);
        f(obj, &mut state);
        let after = AnimUndoState::capture(obj, &state);
        if before != after {
            let merge = token.is_some()
                && state.active_coalesce == token
                && state.redo.is_empty()
                && state.undo.last().map_or(false, |e| e.coalesce == token);
            if merge {
                state.undo.last_mut().unwrap().after = after;
            } else {
                state.undo.push(AnimEdit {
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
    }
    clamp_selection(&mut state, app.runtime.scene());
    app.anim_sim_editor = Some(state);
}

fn clamp_selection(state: &mut AnimSimEditorState, scene: &Scene) {
    let Some(obj) = scene.find_object(&state.object_id) else {
        return;
    };
    if obj.animations.is_empty() {
        state.selected_anim = 0;
        state.selected_key = None;
        return;
    }
    state.selected_anim = state.selected_anim.min(obj.animations.len() - 1);
    let anim = &obj.animations[state.selected_anim];
    state.player.anim_name = anim.name.clone();
    state.player.looping = anim.looping;
    state.selected_key = match state.selected_key {
        Some(_) if anim.keyframes.is_empty() => None,
        Some(i) => Some(i.min(anim.keyframes.len() - 1)),
        None => None,
    };
}

pub(crate) fn undo(app: &mut App) {
    let Some(mut state) = app.anim_sim_editor.take() else {
        return;
    };
    if let Some(edit) = state.undo.pop() {
        let obj_id = state.object_id.clone();
        if let Some(obj) = app.runtime.scene_mut().find_object_mut(&obj_id) {
            edit.before.restore(obj, &mut state);
        }
        state.redo.push(edit);
    }
    state.rot_edit = None;
    state.active_coalesce = None;
    clamp_selection(&mut state, app.runtime.scene());
    app.anim_sim_editor = Some(state);
}

pub(crate) fn redo(app: &mut App) {
    let Some(mut state) = app.anim_sim_editor.take() else {
        return;
    };
    if let Some(edit) = state.redo.pop() {
        let obj_id = state.object_id.clone();
        if let Some(obj) = app.runtime.scene_mut().find_object_mut(&obj_id) {
            edit.after.restore(obj, &mut state);
        }
        state.undo.push(edit);
    }
    state.rot_edit = None;
    state.active_coalesce = None;
    clamp_selection(&mut state, app.runtime.scene());
    app.anim_sim_editor = Some(state);
}

/// Tool settings go through `with_edit` too so they land on the undo stack.
pub(crate) fn set_snap_step(app: &mut App, step: Option<f32>) {
    with_edit(app, |_, state| state.snap_step = step);
}

pub(crate) fn set_speed(app: &mut App, speed: f32) {
    with_edit(app, |_, state| state.speed = speed);
}

// ---------------------------------------------------------------------------
// Animation list operations
// ---------------------------------------------------------------------------

pub(crate) fn select_anim(app: &mut App, index: usize) {
    let Some(state) = app.anim_sim_editor.as_mut() else {
        return;
    };
    state.selected_anim = index;
    state.playing = false;
    state.player.elapsed = 0.0;
    state.player.finished = false;
    state.selected_key = None;
    state.rot_edit = None;
    let mut state = app.anim_sim_editor.take().unwrap();
    clamp_selection(&mut state, app.runtime.scene());
    state.selected_key = current_anim(&state, app.runtime.scene())
        .filter(|a| !a.keyframes.is_empty())
        .map(|_| 0);
    // Re-anchor the preview to the newly selected animation so it stays centred.
    if let Some(obj) = app.runtime.scene().find_object(&state.object_id) {
        state.display_origin = compute_display_origin(obj, current_anim(&state, app.runtime.scene()));
    }
    app.anim_sim_editor = Some(state);
}

pub(crate) fn add_anim(app: &mut App) {
    with_edit(app, |obj, state| {
        let anim = default_animation(obj);
        obj.animations.push(anim);
        state.selected_anim = obj.animations.len() - 1;
        state.selected_key = Some(0);
        state.playing = false;
        state.player.elapsed = 0.0;
    });
}

pub(crate) fn delete_anim(app: &mut App) {
    with_edit(app, |obj, state| {
        if obj.animations.len() <= 1 {
            return;
        }
        let removed = obj.animations.remove(state.selected_anim);
        // Bindings pointing at the removed animation become unassigned.
        for b in &mut obj.animation_bindings {
            if b.animation == removed.name {
                b.animation = String::new();
            }
        }
        state.playing = false;
        state.player.elapsed = 0.0;
    });
}

pub(crate) fn rename_anim(app: &mut App, new_name: String) {
    let trimmed = new_name.trim().to_string();
    if trimmed.is_empty() {
        return;
    }
    with_edit(app, |obj, state| {
        let taken = obj
            .animations
            .iter()
            .enumerate()
            .any(|(i, a)| i != state.selected_anim && a.name == trimmed);
        if taken {
            return;
        }
        let Some(anim) = obj.animations.get_mut(state.selected_anim) else {
            return;
        };
        let old = anim.name.clone();
        anim.name = trimmed.clone();
        // Keep bindings pointing at the renamed animation.
        for b in &mut obj.animation_bindings {
            if b.animation == old {
                b.animation = trimmed.clone();
            }
        }
    });
}

pub(crate) fn set_looping(app: &mut App, looping: bool) {
    with_edit(app, |obj, state| {
        if let Some(anim) = obj.animations.get_mut(state.selected_anim) {
            anim.looping = looping;
        }
    });
}

pub(crate) fn set_easing(app: &mut App, easing: Easing) {
    with_edit(app, |obj, state| {
        if let Some(anim) = obj.animations.get_mut(state.selected_anim) {
            anim.easing = easing;
        }
    });
}

pub(crate) fn copy_anim(app: &mut App) {
    let anim = app
        .anim_sim_editor
        .as_ref()
        .and_then(|s| current_anim(s, app.runtime.scene()).cloned());
    if anim.is_some() {
        app.anim_clipboard = anim;
    }
}

/// Paste the clipboard animation onto this object (export/import across objects).
pub(crate) fn paste_anim(app: &mut App) {
    let Some(mut anim) = app.anim_clipboard.clone() else {
        return;
    };
    with_edit(app, |obj, state| {
        anim.name = unique_anim_name(&obj.animations, &anim.name);
        obj.animations.push(anim);
        state.selected_anim = obj.animations.len() - 1;
        state.selected_key = obj.animations[state.selected_anim]
            .keyframes
            .first()
            .map(|_| 0);
        state.playing = false;
        state.player.elapsed = 0.0;
    });
}

// ---------------------------------------------------------------------------
// Keyframe operations
// ---------------------------------------------------------------------------

fn sort_keyframes(anim: &mut Animation, follow: Option<&Keyframe>) -> Option<usize> {
    anim.keyframes
        .sort_by(|a, b| a.t.partial_cmp(&b.t).unwrap_or(std::cmp::Ordering::Equal));
    follow.and_then(|k| anim.keyframes.iter().position(|c| c == k))
}

pub(crate) fn select_key(app: &mut App, index: usize) {
    let Some(state) = app.anim_sim_editor.as_mut() else {
        return;
    };
    state.selected_key = Some(index);
    state.rot_edit = None;
    let mut state = app.anim_sim_editor.take().unwrap();
    clamp_selection(&mut state, app.runtime.scene());
    // Jump the playhead to the selected keyframe for quick inspection.
    if let (Some(i), Some(anim)) = (state.selected_key, current_anim(&state, app.runtime.scene()))
    {
        if let Some(k) = anim.keyframes.get(i) {
            state.player.elapsed = k.t;
            state.player.finished = false;
            state.playing = false;
        }
    }
    app.anim_sim_editor = Some(state);
}

/// Add a keyframe at the current playhead, baking the interpolated pose so the
/// motion is unchanged at that instant.
pub(crate) fn add_key_at_playhead(app: &mut App) {
    let baked = app
        .anim_sim_editor
        .as_ref()
        .map(|s| preview_sample(s, app.runtime.scene()));
    with_edit(app, |obj, state| {
        state.rot_edit = None;
        let t = state.snap_time(state.player.elapsed).max(0.0);
        let Some(anim) = obj.animations.get_mut(state.selected_anim) else {
            return;
        };
        let baked = baked.clone().unwrap_or_default();
        let key = Keyframe {
            t,
            position: baked.position.or(Some(obj.cuboid.position)),
            rotation: baked.rotation.or(Some(obj.cuboid.rotation)),
            scale: baked.scale.or(Some(obj.cuboid.half_size)),
            color: None,
        };
        let follow = key.clone();
        anim.keyframes.push(key);
        state.selected_key = sort_keyframes(anim, Some(&follow));
        state.player.elapsed = t;
    });
}

/// Capture the object's *actual* scene transform as a keyframe at the playhead.
pub(crate) fn capture_pose_key(app: &mut App) {
    with_edit(app, |obj, state| {
        state.rot_edit = None;
        let t = state.snap_time(state.player.elapsed).max(0.0);
        let key = capture_keyframe(obj, t);
        let follow = key.clone();
        let Some(anim) = obj.animations.get_mut(state.selected_anim) else {
            return;
        };
        anim.keyframes.push(key);
        state.selected_key = sort_keyframes(anim, Some(&follow));
        state.player.elapsed = t;
    });
}

pub(crate) fn delete_key(app: &mut App) {
    with_edit(app, |obj, state| {
        state.rot_edit = None;
        let Some(i) = state.selected_key else { return };
        let Some(anim) = obj.animations.get_mut(state.selected_anim) else {
            return;
        };
        if i < anim.keyframes.len() {
            anim.keyframes.remove(i);
        }
    });
}

pub(crate) fn copy_key(app: &mut App) {
    let key = app.anim_sim_editor.as_ref().and_then(|s| {
        let anim = current_anim(s, app.runtime.scene())?;
        anim.keyframes.get(s.selected_key?).cloned()
    });
    if key.is_some() {
        app.keyframe_clipboard = key;
    }
}

/// Paste the clipboard keyframe into the selected animation at the playhead.
pub(crate) fn paste_key(app: &mut App) {
    let Some(mut key) = app.keyframe_clipboard.clone() else {
        return;
    };
    with_edit(app, |obj, state| {
        state.rot_edit = None;
        key.t = state.snap_time(state.player.elapsed).max(0.0);
        let follow = key.clone();
        let Some(anim) = obj.animations.get_mut(state.selected_anim) else {
            return;
        };
        anim.keyframes.push(key);
        state.selected_key = sort_keyframes(anim, Some(&follow));
    });
}

#[derive(Clone, Copy)]
pub(crate) enum KeyField {
    T,
    Pos(usize),
    /// Euler degrees, axis-indexed [X, Y, Z] (stored back via YXZ order).
    RotEuler(usize),
    Scale(usize),
}

pub(crate) fn edit_key_field(app: &mut App, field: KeyField, value: f32) {
    // Fold this field's drag frames into one undo entry.
    let token = app.anim_sim_editor.as_ref().and_then(|s| {
        s.selected_key
            .map(|k| EditKey(s.selected_anim, k, field_code(field)))
    });
    with_edit_coalesced(app, token, |obj, state| {
        let Some(i) = state.selected_key else { return };
        let Some(anim) = obj.animations.get_mut(state.selected_anim) else {
            return;
        };
        let Some(key) = anim.keyframes.get_mut(i) else {
            return;
        };
        match field {
            KeyField::T => {
                key.t = state.snap_time(value).max(0.0);
                let follow = key.clone();
                state.selected_key = sort_keyframes(anim, Some(&follow));
                if let Some(new_i) = state.selected_key {
                    state.player.elapsed = anim.keyframes[new_i].t;
                }
                // Re-sorting can shuffle indices; drop any sticky rotation edit.
                state.rot_edit = None;
            }
            KeyField::Pos(axis) => {
                let mut p = key.position.unwrap_or(obj.cuboid.position);
                p[axis] = value;
                key.position = Some(p);
            }
            KeyField::RotEuler(axis) => {
                // Edit in the display (rest-relative) frame so the three axes
                // stay independent — same clean behaviour as the grab pose
                // editor, whose grip rotations start from identity. `disp * q` is
                // the rest-relative rotation; we edit its euler, then map back to
                // world with `disp.inverse()` (== the object's resting rotation).
                let disp = obj.cuboid.rotation.inverse();
                let q = key.rotation.unwrap_or(obj.cuboid.rotation);
                // Axis-indexed [X, Y, Z]; glam's YXZ decomposition returns
                // (Y, X, Z), rebuilt below in that order.
                let mut deg = state.euler_for_key(state.selected_anim, i, q, disp);
                deg[axis] = value;
                let d = Quat::from_euler(
                    EulerRot::YXZ,
                    deg[1].to_radians(),
                    deg[0].to_radians(),
                    deg[2].to_radians(),
                );
                key.rotation = Some(disp.inverse() * d);
                state.rot_edit = Some((state.selected_anim, i, deg));
            }
            KeyField::Scale(axis) => {
                let mut s = key.scale.unwrap_or(obj.cuboid.half_size);
                s[axis] = value.max(0.001);
                key.scale = Some(s);
            }
        }
    });
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum KeyChannel {
    Position,
    Rotation,
    Scale,
}

/// Toggle a channel on/off for the selected keyframe. Enabling seeds it from
/// the object's current transform.
pub(crate) fn toggle_key_channel(app: &mut App, channel: KeyChannel) {
    with_edit(app, |obj, state| {
        // Toggling rotation off/on resets the quat; drop any sticky euler.
        state.rot_edit = None;
        let Some(i) = state.selected_key else { return };
        let (pos, rot, half) = (
            obj.cuboid.position,
            obj.cuboid.rotation,
            obj.cuboid.half_size,
        );
        let Some(anim) = obj.animations.get_mut(state.selected_anim) else {
            return;
        };
        let Some(key) = anim.keyframes.get_mut(i) else {
            return;
        };
        match channel {
            KeyChannel::Position => {
                key.position = if key.position.is_some() { None } else { Some(pos) }
            }
            KeyChannel::Rotation => {
                key.rotation = if key.rotation.is_some() { None } else { Some(rot) }
            }
            KeyChannel::Scale => {
                key.scale = if key.scale.is_some() { None } else { Some(half) }
            }
        }
    });
}

// ---------------------------------------------------------------------------
// Controller bindings
// ---------------------------------------------------------------------------

pub(crate) fn add_binding(app: &mut App) {
    with_edit(app, |obj, state| {
        let animation = obj
            .animations
            .get(state.selected_anim)
            .map(|a| a.name.clone())
            .unwrap_or_default();
        obj.animation_bindings.push(AnimationBinding {
            button: "btn_a".to_string(),
            animation,
            play_mode: Default::default(),
            scope: Default::default(),
        });
    });
}

pub(crate) fn remove_binding(app: &mut App, index: usize) {
    with_edit(app, |obj, _| {
        if index < obj.animation_bindings.len() {
            obj.animation_bindings.remove(index);
        }
    });
}

pub(crate) fn edit_binding(app: &mut App, index: usize, f: impl FnOnce(&mut AnimationBinding)) {
    with_edit(app, |obj, _| {
        if let Some(b) = obj.animation_bindings.get_mut(index) {
            f(b);
        }
    });
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

const KEY_MARKER_HALF: f32 = 0.012;

/// Object cuboid (when it has no mesh) at the sampled preview transform, plus
/// small marker cubes at every keyframe that carries a position.
pub(crate) fn collect_cuboids(state: &AnimSimEditorState, scene: &Scene) -> Vec<Cuboid> {
    let Some(obj) = scene.find_object(&state.object_id) else {
        return Vec::new();
    };
    let s = preview_sample(state, scene);
    let disp = display_rotation_offset(obj);
    // World position -> centred preview space (object drawn at the origin).
    let to_view = |p: Vec3| p - state.display_origin;
    let mut out = Vec::new();

    if obj.mesh.is_none() {
        let pos = to_view(s.position.unwrap_or(obj.cuboid.position));
        let rot = s.rotation.unwrap_or(obj.cuboid.rotation);
        let half = s.scale.unwrap_or(obj.cuboid.half_size);
        let col = s.color.unwrap_or(obj.cuboid.color);
        let mut c = Cuboid::solid(pos, half, Color3(col.0, col.1, col.2, col.3));
        c.rotation = disp * rot;
        out.push(c);
    }

    if let Some(anim) = current_anim(state, scene) {
        for (i, key) in anim.keyframes.iter().enumerate() {
            let Some(p) = key.position else { continue };
            let selected = state.selected_key == Some(i);
            let (color, half) = if selected {
                (Color3(255, 220, 40, 255), KEY_MARKER_HALF * 1.4)
            } else {
                (Color3(90, 225, 255, 200), KEY_MARKER_HALF)
            };
            let mut c = Cuboid::solid(to_view(p), Vec3::splat(half), color);
            if let Some(r) = key.rotation {
                c.rotation = r;
            }
            out.push(c);
        }
    }

    out
}

/// Applies the sampled preview transform to the object's mesh (if any).
/// Matches the runtime: `rotation * mesh.rotation_offset`, mesh keeps its own
/// scale (keyframe scale only affects cuboid half-size).
pub(crate) fn update_transforms(
    state: &AnimSimEditorState,
    scene: &Scene,
    mesh_cache: &mut std::collections::HashMap<String, (GltfMesh, ModelUniform)>,
) {
    let Some(obj) = scene.find_object(&state.object_id) else {
        return;
    };
    let Some(mesh_ref) = &obj.mesh else {
        return;
    };
    let s = preview_sample(state, scene);
    let disp = display_rotation_offset(obj);
    if let Some((mesh, _)) = mesh_cache.get_mut(&mesh_ref.path) {
        mesh.position = s.position.unwrap_or(obj.cuboid.position) - state.display_origin;
        mesh.rotation =
            disp * s.rotation.unwrap_or(obj.cuboid.rotation) * mesh_ref.rotation_offset;
        mesh.scale = mesh_ref.scale;
    }
}

pub(crate) fn collect_mesh_instances<'a>(
    state: &AnimSimEditorState,
    scene: &Scene,
    mesh_cache: &'a std::collections::HashMap<String, (GltfMesh, ModelUniform)>,
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
    out
}
