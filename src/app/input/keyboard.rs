use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{Key, NamedKey};

use super::super::anim_sim_editor;
use super::super::discover::winit_key_to_agate;
use super::super::grab_pose_editor;
use super::super::snap;
use super::super::{App, EditorTool, ViewMode};

/// Entry point for every key press *and release*. In the base-editor viewport
/// WASDQE (+ Space/Ctrl) fly the camera: those keys update the held-key state
/// (and are consumed) whenever fly mode is live. Everything else is handled on
/// press only.
pub(crate) fn handle_key_event(app: &mut App, event: &KeyEvent) {
    let pressed = event.state == ElementState::Pressed;
    if fly_active(app) {
        if let Some(dir) = fly_dir(&event.logical_key) {
            match dir {
                FlyDir::Forward => app.fly.forward = pressed,
                FlyDir::Back => app.fly.back = pressed,
                FlyDir::Left => app.fly.left = pressed,
                FlyDir::Right => app.fly.right = pressed,
                FlyDir::Up => app.fly.up = pressed,
                FlyDir::Down => app.fly.down = pressed,
            }
            return;
        }
    }
    if pressed {
        handle_key(app, event);
    }
}

/// Fly the base-editor camera only in the plain edit viewport — not while a
/// sub-editor is open or a text field / code editor has focus (so typing is
/// never eaten).
fn fly_active(app: &App) -> bool {
    let typing = app.editing.is_some()
        || app.ui.as_ref().map(|u| u.text_focused()).unwrap_or(false);
    app.view_mode == ViewMode::Edit
        && app.grab_pose_editor.is_none()
        && app.anim_sim_editor.is_none()
        && !typing
}

enum FlyDir {
    Forward,
    Back,
    Left,
    Right,
    Up,
    Down,
}

/// Maps a fly key to a direction. W/A/S/D move; E/Space go up, Q/Ctrl go down.
fn fly_dir(key: &Key) -> Option<FlyDir> {
    match key {
        Key::Character(s) => match s.as_str() {
            "w" | "W" => Some(FlyDir::Forward),
            "s" | "S" => Some(FlyDir::Back),
            "a" | "A" => Some(FlyDir::Left),
            "d" | "D" => Some(FlyDir::Right),
            "e" | "E" => Some(FlyDir::Up),
            "q" | "Q" => Some(FlyDir::Down),
            _ => None,
        },
        Key::Named(NamedKey::Space) => Some(FlyDir::Up),
        Key::Named(NamedKey::Control) => Some(FlyDir::Down),
        _ => None,
    }
}

pub(crate) fn handle_key(app: &mut App, event: &KeyEvent) {
    if app.grab_pose_editor.is_some() {
        grab_pose_key(app, event);
        return;
    }
    if app.anim_sim_editor.is_some() {
        anim_sim_key(app, event);
        return;
    }

    if app.editing.is_some() && app.editor_focused {
        editor_key(app, event);
        return;
    }

    let cmd = app.mods.super_key() || app.mods.control_key();
    if !cmd {
        if let Some(txt) = &event.text {
            for ch in txt.chars() {
                if !ch.is_control() {
                    app.text_input.push(ch);
                }
            }
        }
    }
    if !cmd {
        if let Key::Character(s) = &event.logical_key {
            // W/E/R are the WASDQE fly keys (handled in handle_key_event); the
            // gizmo modes live on the viewport toolbar instead.
            match s.as_str() {
                "g" | "G" if app.tool == EditorTool::Rigging && app.rig_selection.len() == 1 => {
                    let id = app.rig_selection[0].clone();
                    snap::seed_grip_pose(app.runtime.scene_mut(), &id, app.snap_hand, None);
                    app.scene_dirty = true;
                    app.rig_selection.clear();
                    app.selected_object = Some(id);
                }
                _ => {}
            }
        }
    }
    if let Some(nk) = winit_key_to_agate(&event.logical_key) {
        app.named_keys.push(nk);
    }
}

fn grab_pose_key(app: &mut App, ev: &KeyEvent) {
    let cmd = app.mods.super_key() || app.mods.control_key();
    let shift = app.mods.shift_key();
    let text_focused = app
        .ui
        .as_ref()
        .map(|u| u.text_focused())
        .unwrap_or(false);

    match &ev.logical_key {
        Key::Named(NamedKey::Escape) => {
            let confirming = app
                .grab_pose_editor
                .as_ref()
                .map(|s| s.confirm_exit)
                .unwrap_or(false);
            if confirming {
                grab_pose_editor::cancel_exit(app);
            } else {
                grab_pose_editor::request_exit(app);
            }
            app.redraw_now();
            return;
        }
        Key::Character(s) if cmd => {
            match s.as_str() {
                "z" | "Z" => {
                    if shift {
                        grab_pose_editor::redo(app)
                    } else {
                        grab_pose_editor::undo(app)
                    }
                }
                _ => {}
            }
            app.redraw_now();
            return;
        }
        _ => {}
    }

    if text_focused {
        // Route typing into the focused panel text input (e.g. the point name
        // field) — without this, characters never reach the UI here.
        if !cmd {
            if let Some(txt) = &ev.text {
                for ch in txt.chars() {
                    if !ch.is_control() {
                        app.text_input.push(ch);
                    }
                }
            }
        }
        if let Some(nk) = winit_key_to_agate(&ev.logical_key) {
            app.named_keys.push(nk);
        }
    }
    app.redraw_now();
}

fn anim_sim_key(app: &mut App, ev: &KeyEvent) {
    let cmd = app.mods.super_key() || app.mods.control_key();
    let shift = app.mods.shift_key();
    let text_focused = app
        .ui
        .as_ref()
        .map(|u| u.text_focused())
        .unwrap_or(false);

    // Cmd-chords work even while typing. (No Esc-to-close: too easy to lose
    // your place by accident — use the Done button.)
    match &ev.logical_key {
        Key::Character(s) if cmd => {
            match s.as_str() {
                "z" | "Z" => {
                    if shift {
                        anim_sim_editor::redo(app);
                    } else {
                        anim_sim_editor::undo(app);
                    }
                }
                "c" | "C" if !text_focused => anim_sim_editor::copy_key(app),
                "v" | "V" if !text_focused => anim_sim_editor::paste_key(app),
                _ => {}
            }
            app.redraw_now();
            return;
        }
        _ => {}
    }

    if text_focused {
        // Route typing into the focused panel text input.
        if !cmd {
            if let Some(txt) = &ev.text {
                for ch in txt.chars() {
                    if !ch.is_control() {
                        app.text_input.push(ch);
                    }
                }
            }
        }
        if let Some(nk) = winit_key_to_agate(&ev.logical_key) {
            app.named_keys.push(nk);
        }
    } else if !cmd {
        // Single-key hotkeys.
        match &ev.logical_key {
            Key::Named(NamedKey::Space) => {
                if !ev.repeat {
                    anim_sim_editor::toggle_play(app);
                }
            }
            Key::Named(NamedKey::Delete) | Key::Named(NamedKey::Backspace) => {
                anim_sim_editor::delete_key(app);
            }
            Key::Named(NamedKey::ArrowLeft) => anim_sim_editor::step_playhead(app, -1.0),
            Key::Named(NamedKey::ArrowRight) => anim_sim_editor::step_playhead(app, 1.0),
            Key::Character(s) => match s.as_str() {
                "k" | "K" => anim_sim_editor::add_key_at_playhead(app),
                _ => {}
            },
            _ => {}
        }
    }
    app.redraw_now();
}

fn editor_key(app: &mut App, ev: &KeyEvent) {
    let cmd = app.mods.super_key() || app.mods.control_key();
    let shift = app.mods.shift_key();
    let ed = &mut app.editor;
    match &ev.logical_key {
        Key::Named(NamedKey::ArrowLeft) => ed.move_left(shift),
        Key::Named(NamedKey::ArrowRight) => ed.move_right(shift),
        Key::Named(NamedKey::ArrowUp) => ed.move_up(shift),
        Key::Named(NamedKey::ArrowDown) => ed.move_down(shift),
        Key::Named(NamedKey::Home) => ed.move_home(shift),
        Key::Named(NamedKey::End) => ed.move_end(shift),
        Key::Named(NamedKey::PageUp) => ed.page_up(shift),
        Key::Named(NamedKey::PageDown) => ed.page_down(shift),
        Key::Named(NamedKey::Backspace) => ed.backspace(),
        Key::Named(NamedKey::Delete) => ed.delete_forward(),
        Key::Named(NamedKey::Enter) => ed.newline(),
        Key::Named(NamedKey::Tab) => ed.insert_str("  "),
        Key::Named(NamedKey::Space) => ed.insert_char(' '),
        Key::Character(s) if cmd => match s.as_str() {
            "s" | "S" => {
                let _ = ed.save();
            }
            "a" | "A" => ed.select_all(),
            "c" | "C" => ed.copy(),
            "x" | "X" => ed.cut(),
            "v" | "V" => ed.paste(),
            "z" | "Z" => {
                if shift {
                    ed.redo()
                } else {
                    ed.undo()
                }
            }
            _ => {}
        },
        _ => {
            if !cmd {
                if let Some(txt) = &ev.text {
                    for ch in txt.chars() {
                        if !ch.is_control() {
                            ed.insert_char(ch);
                        }
                    }
                }
            }
        }
    }
}
