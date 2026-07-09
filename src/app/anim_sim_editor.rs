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

#[derive(Clone)]
struct AnimEdit {
    before: AnimSnapshot,
    after: AnimSnapshot,
}

const UNDO_CAP: usize = 100;

pub(crate) struct AnimSimEditorState {
    pub object_id: String,
    pub orbit: super::orbit_camera::OrbitCamera,

    pub selected_anim: usize,
    pub selected_key: Option<usize>,

    /// Internal preview player; `elapsed` doubles as the scrub playhead.
    pub player: AnimationPlayer,
    pub playing: bool,
    pub speed: f32,

    /// Keyframe-time snapping grid (None = off).
    pub snap_step: Option<f32>,

    pub content_height: f32,

    undo: Vec<AnimEdit>,
    redo: Vec<AnimEdit>,
}

impl AnimSimEditorState {
    fn new(object_id: String, framing_radius: f32, target: Vec3, first_anim: &Animation) -> Self {
        let mut orbit = super::orbit_camera::OrbitCamera::new(framing_radius);
        orbit.target = target;
        Self {
            object_id,
            orbit,
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
            content_height: 1600.0,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
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
    Keyframe {
        t,
        position: Some(obj.cuboid.position),
        rotation: Some(obj.cuboid.rotation),
        scale: Some(obj.cuboid.half_size),
        color: Some(obj.cuboid.color),
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

pub(crate) fn open(app: &mut App, object_id: String) {
    if let Some(obj) = app.runtime.scene_mut().find_object_mut(&object_id) {
        if obj.animations.is_empty() {
            let anim = default_animation(obj);
            obj.animations.push(anim);
            app.scene_dirty = true;
        }
    }

    let Some(obj) = app.runtime.scene().find_object(&object_id) else {
        return;
    };
    let framing = obj.cuboid.half_size.length().max(0.05);
    let target = obj.cuboid.position;
    let first = obj.animations[0].clone();

    app.anim_sim_editor = Some(AnimSimEditorState::new(object_id, framing, target, &first));
}

pub(crate) fn close(app: &mut App) {
    app.anim_sim_editor = None;
    super::scene_bridge::save_if_dirty(app);
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
// Undo/redo plumbing — every mutation goes through `with_edit`.
// ---------------------------------------------------------------------------

fn with_edit(app: &mut App, f: impl FnOnce(&mut GameObject, &mut AnimSimEditorState)) {
    let Some(mut state) = app.anim_sim_editor.take() else {
        return;
    };
    let obj_id = state.object_id.clone();
    if let Some(obj) = app.runtime.scene_mut().find_object_mut(&obj_id) {
        let before = AnimSnapshot::of(obj);
        f(obj, &mut state);
        let after = AnimSnapshot::of(obj);
        if before != after {
            state.undo.push(AnimEdit { before, after });
            if state.undo.len() > UNDO_CAP {
                state.undo.remove(0);
            }
            state.redo.clear();
            app.scene_dirty = true;
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
        if let Some(obj) = app.runtime.scene_mut().find_object_mut(&state.object_id) {
            edit.before.restore(obj);
            app.scene_dirty = true;
        }
        state.redo.push(edit);
    }
    clamp_selection(&mut state, app.runtime.scene());
    app.anim_sim_editor = Some(state);
}

pub(crate) fn redo(app: &mut App) {
    let Some(mut state) = app.anim_sim_editor.take() else {
        return;
    };
    if let Some(edit) = state.redo.pop() {
        if let Some(obj) = app.runtime.scene_mut().find_object_mut(&state.object_id) {
            edit.after.restore(obj);
            app.scene_dirty = true;
        }
        state.undo.push(edit);
    }
    clamp_selection(&mut state, app.runtime.scene());
    app.anim_sim_editor = Some(state);
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
    let mut state = app.anim_sim_editor.take().unwrap();
    clamp_selection(&mut state, app.runtime.scene());
    state.selected_key = current_anim(&state, app.runtime.scene())
        .filter(|a| !a.keyframes.is_empty())
        .map(|_| 0);
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
            color: baked.color.or(Some(obj.cuboid.color)),
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
    /// RGBA channel 0..=3 in 0..=255.
    Color(usize),
}

pub(crate) fn edit_key_field(app: &mut App, field: KeyField, value: f32) {
    with_edit(app, |obj, state| {
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
            }
            KeyField::Pos(axis) => {
                let mut p = key.position.unwrap_or(obj.cuboid.position);
                p[axis] = value;
                key.position = Some(p);
            }
            KeyField::RotEuler(axis) => {
                let q = key.rotation.unwrap_or(obj.cuboid.rotation);
                // glam returns YXZ euler in (Y, X, Z) order — store axis-indexed
                // as [X, Y, Z] so editing one axis doesn't drag the others.
                let (ey, ex, ez) = q.to_euler(EulerRot::YXZ);
                let mut deg = [ex.to_degrees(), ey.to_degrees(), ez.to_degrees()];
                deg[axis] = value;
                key.rotation = Some(Quat::from_euler(
                    EulerRot::YXZ,
                    deg[1].to_radians(),
                    deg[0].to_radians(),
                    deg[2].to_radians(),
                ));
            }
            KeyField::Scale(axis) => {
                let mut s = key.scale.unwrap_or(obj.cuboid.half_size);
                s[axis] = value.max(0.001);
                key.scale = Some(s);
            }
            KeyField::Color(ch) => {
                let mut c = key.color.unwrap_or(obj.cuboid.color);
                let v = value.round().clamp(0.0, 255.0) as u8;
                match ch {
                    0 => c.0 = v,
                    1 => c.1 = v,
                    2 => c.2 = v,
                    _ => c.3 = v,
                }
                key.color = Some(c);
            }
        }
    });
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum KeyChannel {
    Position,
    Rotation,
    Scale,
    Color,
}

/// Toggle a channel on/off for the selected keyframe. Enabling seeds it from
/// the object's current transform.
pub(crate) fn toggle_key_channel(app: &mut App, channel: KeyChannel) {
    with_edit(app, |obj, state| {
        let Some(i) = state.selected_key else { return };
        let (pos, rot, half, col) = (
            obj.cuboid.position,
            obj.cuboid.rotation,
            obj.cuboid.half_size,
            obj.cuboid.color,
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
            KeyChannel::Scale => key.scale = if key.scale.is_some() { None } else { Some(half) },
            KeyChannel::Color => key.color = if key.color.is_some() { None } else { Some(col) },
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
    let mut out = Vec::new();

    if obj.mesh.is_none() {
        let pos = s.position.unwrap_or(obj.cuboid.position);
        let rot = s.rotation.unwrap_or(obj.cuboid.rotation);
        let half = s.scale.unwrap_or(obj.cuboid.half_size);
        let col = s.color.unwrap_or(obj.cuboid.color);
        let mut c = Cuboid::solid(pos, half, Color3(col.0, col.1, col.2, col.3));
        c.rotation = rot;
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
            let mut c = Cuboid::solid(p, Vec3::splat(half), color);
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
    if let Some((mesh, _)) = mesh_cache.get_mut(&mesh_ref.path) {
        mesh.position = s.position.unwrap_or(obj.cuboid.position);
        mesh.rotation = s.rotation.unwrap_or(obj.cuboid.rotation) * mesh_ref.rotation_offset;
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
