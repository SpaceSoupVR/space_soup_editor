use winit::event::KeyEvent;
use winit::keyboard::{Key, NamedKey};

use crate::transform_gizmo::GizmoMode;

use super::super::discover::winit_key_to_agate;
use super::super::grab_pose_editor;
use super::super::snap;
use super::super::{App, EditorTool};

pub(crate) fn handle_key(app: &mut App, event: &KeyEvent) {
    if app.grab_pose_editor.is_some() {
        grab_pose_key(app, event);
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
            match s.as_str() {
                "w" | "W" => app.xform_gizmo.mode = GizmoMode::Translate,
                "e" | "E" => app.xform_gizmo.mode = GizmoMode::Rotate,
                "r" | "R" => app.xform_gizmo.mode = GizmoMode::Scale,
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
    match &ev.logical_key {
        Key::Named(NamedKey::Escape) => grab_pose_editor::close(app),
        Key::Character(s) if cmd => match s.as_str() {
            "z" | "Z" => {
                if shift {
                    grab_pose_editor::redo(app)
                } else {
                    grab_pose_editor::undo(app)
                }
            }
            _ => {}
        },
        _ => {}
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
