use winit::event::{MouseScrollDelta, TouchPhase};

use super::super::layout::{in_rect, Layout};
use super::super::{App, ViewMode};
use agate::Theme;

pub(crate) fn pinch(app: &mut App, delta: f64, phase: TouchPhase) {
    if phase == TouchPhase::Cancelled {
        return;
    }

    if app.grab_pose_editor.is_some() {
        if over_grab_pose_viewport(app) {
            if let Some(state) = app.grab_pose_editor.as_mut() {
                state.orbit.zoom(delta as f32 * -0.8);
            }
            app.redraw_now();
        }
    } else if app.anim_sim_editor.is_some() {
        if over_anim_sim_viewport(app) {
            if let Some(state) = app.anim_sim_editor.as_mut() {
                state.orbit.zoom(delta as f32 * -0.8);
            }
            app.redraw_now();
        }
    } else if app.object_preview.is_some() {
        if over_grab_pose_viewport(app) {
            if let Some(state) = app.object_preview.as_mut() {
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
    } else if app.anim_sim_editor.is_some() {
        if over_anim_sim_viewport(app) {
            let cmd = app.mods.super_key() || app.mods.control_key();
            if let Some(state) = app.anim_sim_editor.as_mut() {
                if cmd {
                    state.orbit.zoom(-dy * 0.002);
                } else {
                    state.orbit.pan(dx, dy);
                }
            }
        }
    } else if app.object_preview.is_some() {
        if over_grab_pose_viewport(app) {
            let cmd = app.mods.super_key() || app.mods.control_key();
            if let Some(state) = app.object_preview.as_mut() {
                if cmd {
                    state.orbit.zoom(-dy * 0.002);
                } else {
                    state.orbit.pan(dx, dy);
                }
            }
        }
    } else if app.ribbon_tab == super::super::RibbonTab::Insert && over_ribbon(app) {
        let (win_w, win_h) = app.win_size();
        let theme = Theme::new(app.scale);
        let layout = Layout::new(win_w, win_h, &theme);
        let total = super::super::PRIMITIVE_PALETTE_COUNT + app.available_models.len();
        let max_scroll = layout.ribbon_chip_max_scroll(&theme, total);
        // Horizontal chip row: honor trackpad x-scroll, fall back to the
        // wheel's y for mice.
        let d = if dx.abs() > dy.abs() { dx } else { dy };
        app.model_scroll_y = (app.model_scroll_y + d).clamp(0.0, max_scroll);
    } else if over_viewport(app) && app.editing.is_none() && app.view_mode == ViewMode::Edit {
        if app.mods.super_key() || app.mods.control_key() {
            app.edit_camera.dolly(-dy * 0.006);
        } else {
            app.edit_camera.pan(dx, dy);
        }
    }
    app.redraw_now();
}

fn over_ribbon(app: &App) -> bool {
    let (win_w, win_h) = app.win_size();
    let theme = Theme::new(app.scale);
    let layout = Layout::new(win_w, win_h, &theme);
    in_rect(app.mouse_pos, layout.ribbon)
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

fn over_anim_sim_viewport(app: &App) -> bool {
    let (win_w, win_h) = app.win_size();
    let theme = Theme::new(app.scale);
    let layout = Layout::new(win_w, win_h, &theme);
    in_rect(app.mouse_pos, layout.anim_sim_viewport()) && !in_rect(app.mouse_pos, layout.inspector)
}
