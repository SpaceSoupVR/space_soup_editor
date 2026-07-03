use winit::event::{MouseScrollDelta, TouchPhase};

use super::super::layout::{in_rect, Layout};
use super::super::{App, ViewMode};
use agate::Theme;

pub(crate) fn pinch(app: &mut App, delta: f64, phase: TouchPhase) {
    if phase == TouchPhase::Cancelled { return; }

    if app.grab_pose_editor.is_some() {
        if over_grab_pose_viewport(app) {
            if let Some(state) = app.grab_pose_editor.as_mut() {
                state.orbit.zoom(delta as f32 * -0.8);
            }
            app.redraw_now();
        }
    } else if app.view_mode == ViewMode::Edit && app.editing.is_none() && over_viewport(app) {
        app.edit_camera.dolly(delta as f32 * 0.8);
        app.redraw_now();
    }
}

pub(crate) fn mouse_wheel(app: &mut App, delta: MouseScrollDelta) {
    let (dx, dy) = match delta {
        MouseScrollDelta::LineDelta(x, y) => (x * 20.0, y * 20.0),
        MouseScrollDelta::PixelDelta(p) => (p.x as f32, p.y as f32),
    };
    // Always accumulate scroll_y — agate's nav-panel scroll_area reads this
    // every frame and only applies it while that panel is hovered, so it
    // must keep flowing even when the camera itself shouldn't move.
    app.scroll_y += dy * 0.05;

    if app.grab_pose_editor.is_some() {
        if over_grab_pose_viewport(app) {
            let cmd = app.mods.super_key() || app.mods.control_key();
            if let Some(state) = app.grab_pose_editor.as_mut() {
                if cmd {
                    state.orbit.zoom(-dy * 0.002);
                } else {
                    state.orbit.pan(dx, dy);
                }
            }
        }
    } else if over_viewport(app) && app.editing.is_none() && app.view_mode == ViewMode::Edit {
        if app.mods.super_key() || app.mods.control_key() {
            app.edit_camera.dolly(-dy * 0.002);
        } else {
            app.edit_camera.pan(dx, dy);
        }
    }
    app.redraw_now();
}

fn over_viewport(app: &App) -> bool {
    let (win_w, win_h) = app.win_size();
    let theme = Theme::new(app.scale);
    let layout = Layout::new(win_w, win_h, &theme);
    in_rect(app.mouse_pos, layout.center)
        && !in_rect(app.mouse_pos, layout.navigator)
        && !in_rect(app.mouse_pos, layout.inspector)
}

fn over_grab_pose_viewport(app: &App) -> bool {
    let (win_w, win_h) = app.win_size();
    let theme = Theme::new(app.scale);
    let layout = Layout::new(win_w, win_h, &theme);
    in_rect(app.mouse_pos, layout.grab_pose_viewport()) && !in_rect(app.mouse_pos, layout.inspector)
}