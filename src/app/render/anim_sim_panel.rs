//! Side panel for the Animation Simulation Editor: animation list, keyframe
//! editor, playback controls, and controller-binding rows. Pure view — all
//! data mutations are returned as `AnimSimPanelActions` and applied by
//! `render/mod.rs` through `anim_sim_editor` functions.

use glam::Quat;

use agate::theme as t;
use agate::{Theme, Ui, WidgetId};

use space_soup_engine::{Animation, BindingScope, Easing, Keyframe, PlayMode, Scene};

use super::super::anim_sim_editor::{
    self, AnimSimEditorState, KeyChannel, KeyField, SNAP_STEPS, SPEED_STEPS,
};
use super::super::layout::{Layout, PAD, ROW_H};
use super::confirm::{draw_exit_confirm, ExitChoice};
use super::split_row;

#[derive(Default)]
pub(crate) struct AnimSimPanelActions {
    pub save: bool,
    pub request_exit: bool,
    pub exit_discard: bool,
    pub exit_save: bool,
    pub cancel_exit: bool,
    pub undo: bool,
    pub redo: bool,
    pub recenter: bool,
    /// Outer Some = fired; inner is the new snap grid (None = snapping off).
    pub set_snap_step: Option<Option<f32>>,
    pub set_speed: Option<f32>,

    pub select_anim: Option<usize>,
    pub add_anim: bool,
    pub delete_anim: bool,
    pub rename_anim: Option<String>,
    pub set_looping: Option<bool>,
    pub set_easing: Option<Easing>,
    pub copy_anim: bool,
    pub paste_anim: bool,

    pub play: bool,
    pub pause: bool,
    pub stop: bool,
    pub seek: Option<f32>,

    pub select_key: Option<usize>,
    pub add_key: bool,
    pub capture_pose: bool,
    pub delete_key: bool,
    pub copy_key: bool,
    pub paste_key: bool,
    pub key_field_edit: Option<(KeyField, f32)>,
    pub toggle_channel: Option<KeyChannel>,

    pub add_binding: bool,
    pub remove_binding: Option<usize>,
    pub cycle_binding_button: Option<usize>,
    pub cycle_binding_anim: Option<usize>,
    pub binding_mode: Option<(usize, PlayMode)>,
    pub binding_scope: Option<(usize, BindingScope)>,
}

fn section(ui: &mut Ui, theme: &Theme, cx: f32, cw: f32, y: &mut f32, label: &str, clip: [f32; 4]) {
    ui.separator(cx, *y, cw);
    *y += theme.px(10.0);
    ui.label_styled(cx, *y, label, theme.small(), t::TEXT_SECONDARY, cw, Some(clip));
    *y += theme.px(20.0);
}

fn key_row_label(key: &Keyframe) -> String {
    let mut ch = String::new();
    ch.push(if key.position.is_some() { 'P' } else { '\u{00b7}' });
    ch.push(if key.rotation.is_some() { 'R' } else { '\u{00b7}' });
    ch.push(if key.scale.is_some() { 'S' } else { '\u{00b7}' });
    format!("{:>6.2}s   [{ch}]", key.t)
}

/// Draws the easing curve (plus a playhead marker) as a dotted polyline made
/// of tiny fills — agate has no polyline primitive.
fn draw_easing_preview(
    ui: &mut Ui,
    theme: &Theme,
    r: [f32; 4],
    easing: Easing,
    norm_time: f32,
) {
    ui.fill(r, t::CONTROL_BG);
    let pad = theme.px(6.0);
    let inner = [r[0] + pad, r[1] + pad, r[2] - pad * 2.0, r[3] - pad * 2.0];
    let dot = theme.px(2.0);
    const STEPS: usize = 48;
    for i in 0..=STEPS {
        let x01 = i as f32 / STEPS as f32;
        let y01 = easing.apply(x01);
        let x = inner[0] + x01 * inner[2] - dot * 0.5;
        let y = inner[1] + (1.0 - y01) * inner[3] - dot * 0.5;
        ui.fill([x, y, dot, dot], t::TEXT_SECONDARY);
    }
    // Playhead marker on the curve.
    let x01 = norm_time.clamp(0.0, 1.0);
    let y01 = easing.apply(x01);
    let d = theme.px(5.0);
    ui.fill(
        [
            inner[0] + x01 * inner[2] - d * 0.5,
            inner[1] + (1.0 - y01) * inner[3] - d * 0.5,
            d,
            d,
        ],
        t::ACCENT_HI,
    );
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw(
    ui: &mut Ui,
    theme: &Theme,
    layout: &Layout,
    state: &mut AnimSimEditorState,
    scene: &Scene,
    has_anim_clipboard: bool,
    has_key_clipboard: bool,
) -> AnimSimPanelActions {
    let mut actions = AnimSimPanelActions::default();

    // -- Top bar ------------------------------------------------------------
    let bar = layout.editor_tab;
    let draw_top_bar = |ui: &mut Ui| {
        ui.fill(bar, t::TOOLBAR_BG);
        ui.separator(bar[0], bar[1] + bar[3] - theme.px(1.0), bar[2]);
        let title = format!("Animation Simulator \u{2014} {}", state.object_id);
        ui.label_styled(
            bar[0] + theme.px(PAD),
            bar[1] + (bar[3] - theme.body()) * 0.5,
            &title,
            theme.body(),
            t::TEXT_PRIMARY,
            bar[2] - theme.px(340.0),
            Some(bar),
        );
    };

    // The unsaved-changes dialog replaces all other interaction (immediate-mode
    // widgets underneath would still take clicks).
    if state.confirm_exit {
        draw_top_bar(ui);
        ui.panel_bordered(layout.inspector, t::SIDEBAR_BG);
        match draw_exit_confirm(ui, theme, layout) {
            Some(ExitChoice::Exit) => actions.exit_discard = true,
            Some(ExitChoice::SaveExit) => actions.exit_save = true,
            Some(ExitChoice::Return) => actions.cancel_exit = true,
            None => {}
        }
        return actions;
    }

    draw_top_bar(ui);
    let done_h = theme.px(28.0);
    let btn_y = bar[1] + (bar[3] - done_h) * 0.5;
    let tb_gap = theme.px(8.0);
    let dirty = state.dirty(scene);

    let exit_w = theme.px(70.0);
    let exit_r = [bar[0] + bar[2] - theme.px(PAD) - exit_w, btn_y, exit_w, done_h];
    if ui.button_secondary(exit_r, "Exit") {
        actions.request_exit = true;
    }
    ui.tooltip(exit_r, "Close the editor \u{2014} asks first if you have unsaved changes");

    let save_w = theme.px(70.0);
    let save_r = [exit_r[0] - tb_gap - save_w, btn_y, save_w, done_h];
    if dirty {
        if ui.button_success(save_r, "Save") {
            actions.save = true;
        }
        ui.tooltip(save_r, "Keep these animations (Save Scene in the main view writes to disk)");
    } else {
        ui.button_disabled(save_r, "Save", t::CONTROL_ACTIVE, t::TEXT_DISABLED);
        ui.tooltip(save_r, "No changes to save");
    }

    // Undo/Redo live in the top bar (always reachable, not down in the scroll).
    let ur_w = theme.px(60.0);
    let redo_r = [save_r[0] - tb_gap - ur_w, btn_y, ur_w, done_h];
    let undo_r = [redo_r[0] - tb_gap - ur_w, btn_y, ur_w, done_h];
    if state.can_undo() {
        if ui.button_secondary(undo_r, "Undo") {
            actions.undo = true;
        }
    } else {
        ui.button_disabled(undo_r, "Undo", t::CONTROL_ACTIVE, t::TEXT_DISABLED);
    }
    ui.tooltip(undo_r, "Take back the last change (\u{2318}Z)");
    if state.can_redo() {
        if ui.button_secondary(redo_r, "Redo") {
            actions.redo = true;
        }
    } else {
        ui.button_disabled(redo_r, "Redo", t::CONTROL_ACTIVE, t::TEXT_DISABLED);
    }
    ui.tooltip(redo_r, "Bring back what you undid (\u{21e7}\u{2318}Z)");

    // Recenter sits just left of Undo.
    let rc_w = theme.px(80.0);
    let recenter_r = [undo_r[0] - tb_gap - rc_w, btn_y, rc_w, done_h];
    if ui.button_secondary(recenter_r, "Recenter") {
        actions.recenter = true;
    }
    ui.tooltip(
        recenter_r,
        "Point the camera back at the object if you've panned or zoomed away",
    );

    // -- Panel scaffolding ----------------------------------------------------
    ui.panel_bordered(layout.inspector, t::SIDEBAR_BG);
    let ix = layout.inspector[0];
    let iw = layout.inspector[2];
    let pad = theme.px(PAD);
    let cx = ix + pad;
    let cw = iw - pad * 2.0;
    let row_h = theme.px(ROW_H);
    let gap = theme.px(8.0);
    let fh = theme.px(26.0);
    let field_gap_y = theme.px(4.0);
    let label_w = theme.px(18.0);
    let field_gap = theme.px(6.0);
    let axes = ["X", "Y", "Z"];

    let content_area = layout.inspector;
    let scroll_id = WidgetId::of("animsim_scroll");
    let (_, scroll_y) = ui.scroll_area(scroll_id, content_area, state.content_height);
    // Confine every widget drawn below to the panel so scrolled-past content
    // (buttons, tabs, sliders — not just the self-clipping labels) stays hidden
    // instead of spilling over the top bar and 3D view.
    ui.push_clip(content_area);
    let y_start = layout.inspector[1] + theme.px(16.0) - scroll_y;
    let mut y = y_start;

    let obj = scene.find_object(&state.object_id);
    let anims: &[Animation] = obj.map(|o| o.animations.as_slice()).unwrap_or(&[]);
    let bindings = obj.map(|o| o.animation_bindings.as_slice()).unwrap_or(&[]);
    let anim = anims.get(state.selected_anim);
    let duration = anim.map(|a| a.duration()).unwrap_or(0.0);
    let key = anim.and_then(|a| state.selected_key.and_then(|i| a.keyframes.get(i)));

    // -- Animation list -------------------------------------------------------
    ui.label_styled(
        cx,
        y,
        "ANIMATIONS",
        theme.small(),
        t::TEXT_SECONDARY,
        cw,
        Some(content_area),
    );
    y += theme.px(20.0);
    let list_row_h = theme.px(24.0);
    for (i, a) in anims.iter().enumerate() {
        let loop_tag = if a.looping { "  \u{21bb}" } else { "" };
        let label = format!("{}  ({:.2}s){loop_tag}", a.name, a.duration());
        let row_r = [cx, y, cw, list_row_h];
        if ui.list_row_clipped(row_r, &label, i == state.selected_anim, Some(content_area)) {
            actions.select_anim = Some(i);
        }
        ui.tooltip(row_r, "Click to preview & edit this animation");
        y += list_row_h + theme.px(2.0);
    }
    y += theme.px(4.0);
    let bw2 = (cw - theme.px(8.0)) / 2.0;
    let add_anim_r = [cx, y, bw2, theme.px(26.0)];
    if ui.button_secondary(add_anim_r, "+ Add") {
        actions.add_anim = true;
    }
    ui.tooltip(add_anim_r, "Create a new animation on this object");
    let can_delete = anims.len() > 1;
    let del_r = [cx + bw2 + theme.px(8.0), y, bw2, theme.px(26.0)];
    if can_delete {
        if ui.button_styled(del_r, "Delete", t::CONTROL_BG, t::TEXT_PRIMARY) {
            actions.delete_anim = true;
        }
        ui.tooltip(del_r, "Remove the selected animation");
    } else {
        ui.button_disabled(del_r, "Delete", t::CONTROL_ACTIVE, t::TEXT_DISABLED);
        ui.tooltip(del_r, "Objects keep at least one animation");
    }
    y += theme.px(26.0) + theme.px(4.0);
    let copy_anim_r = [cx, y, bw2, theme.px(26.0)];
    if ui.button_secondary(copy_anim_r, "Copy Anim") {
        actions.copy_anim = true;
    }
    ui.tooltip(copy_anim_r, "Copy this animation \u{2014} paste it onto any object");
    let paste_r = [cx + bw2 + theme.px(8.0), y, bw2, theme.px(26.0)];
    if has_anim_clipboard {
        if ui.button_styled(paste_r, "Paste Anim", t::CONTROL_BG, t::TEXT_PRIMARY) {
            actions.paste_anim = true;
        }
        ui.tooltip(paste_r, "Add the copied animation to this object");
    } else {
        ui.button_disabled(paste_r, "Paste Anim", t::CONTROL_ACTIVE, t::TEXT_DISABLED);
        ui.tooltip(paste_r, "Nothing copied yet \u{2014} use Copy Anim first");
    }
    y += theme.px(26.0) + gap;

    // Name / loop / easing for the selected animation.
    ui.label_styled(
        cx,
        y,
        "NAME",
        theme.small(),
        t::TEXT_SECONDARY,
        cw,
        Some(content_area),
    );
    y += theme.px(18.0);
    let name_wid = WidgetId::of("animsim_anim_name");
    let current_name = anim.map(|a| a.name.as_str()).unwrap_or("");
    let name_r = [cx, y, cw, theme.px(26.0)];
    if let Some(new_name) = ui.text_input(name_wid, name_r, current_name, "animation name") {
        actions.rename_anim = Some(new_name);
    }
    ui.tooltip(name_r, "Rename it \u{2014} scripts & bindings that use it update too");
    y += theme.px(26.0) + gap * 0.5;

    let looping = anim.map(|a| a.looping).unwrap_or(false);
    let loop_r = [cx, y, theme.px(40.0), theme.px(22.0)];
    if let Some(v) = ui.checkbox(loop_r, looping, "Looping") {
        actions.set_looping = Some(v);
    }
    ui.tooltip(
        [cx, y, cw, theme.px(22.0)],
        "Looping: restart automatically when it reaches the end",
    );
    y += row_h;

    ui.label_styled(
        cx,
        y,
        "EASING",
        theme.small(),
        t::TEXT_SECONDARY,
        cw,
        Some(content_area),
    );
    y += theme.px(18.0);
    let easing = anim.map(|a| a.easing).unwrap_or_default();
    let easing_idx = match easing {
        Easing::Linear => 0,
        Easing::EaseIn => 1,
        Easing::EaseOut => 2,
        Easing::EaseInOut => 3,
    };
    let easing_r = [cx, y, cw, row_h];
    if let Some(i) = ui.tabs(easing_r, easing_idx, &["Lin", "In", "Out", "InOut"]) {
        actions.set_easing = Some(match i {
            1 => Easing::EaseIn,
            2 => Easing::EaseOut,
            3 => Easing::EaseInOut,
            _ => Easing::Linear,
        });
    }
    ui.tooltip(easing_r, "How motion ramps between keyframes (soft start/stop)");
    y += row_h + theme.px(4.0);
    let curve_r = [cx, y, cw, theme.px(64.0)];
    let norm_t = if duration > 0.0 {
        state.player.elapsed / duration
    } else {
        0.0
    };
    draw_easing_preview(ui, theme, curve_r, easing, norm_t);
    ui.tooltip(curve_r, "The easing curve: time across, progress up. Dot = playhead");
    y += curve_r[3] + gap;

    // -- Playback controls ------------------------------------------------------
    section(ui, theme, cx, cw, &mut y, "PLAYBACK", content_area);
    let bw3 = (cw - theme.px(16.0)) / 3.0;
    let bh = theme.px(28.0);
    let play_r = [cx, y, bw3, bh];
    if state.playing {
        if ui.button_secondary(play_r, "Pause") {
            actions.pause = true;
        }
        ui.tooltip(play_r, "Freeze the preview at the current time (Space)");
    } else {
        if ui.button_success(play_r, "Play") {
            actions.play = true;
        }
        ui.tooltip(play_r, "Preview the animation in the 3D view (Space)");
    }
    let stop_r = [cx + bw3 + theme.px(8.0), y, bw3, bh];
    if ui.button_secondary(stop_r, "Stop") {
        actions.stop = true;
    }
    ui.tooltip(stop_r, "Stop and rewind to 0.00s");
    ui.label_styled(
        cx + (bw3 + theme.px(8.0)) * 2.0,
        y + (bh - theme.body()) * 0.5,
        &format!("{:.2}/{:.2}s", state.player.elapsed, duration),
        theme.body(),
        t::TEXT_PRIMARY,
        bw3 + theme.px(8.0),
        Some(content_area),
    );
    y += bh + gap * 0.5;

    let scrub_id = WidgetId::of("animsim_scrub");
    let scrub_r = [cx, y, cw, fh];
    if duration > 0.0 {
        if let Some(v) = ui.slider(scrub_id, scrub_r, state.player.elapsed, 0.0..=duration) {
            actions.seek = Some(v);
        }
    } else {
        ui.progress_bar(scrub_r, 0.0);
    }
    ui.tooltip(scrub_r, "Scrub: drag to jump the playhead to any moment");
    y += fh + gap * 0.5;

    ui.label_styled(
        cx,
        y,
        "SPEED",
        theme.small(),
        t::TEXT_SECONDARY,
        cw,
        Some(content_area),
    );
    y += theme.px(18.0);
    let speed_idx = SPEED_STEPS
        .iter()
        .position(|s| (s - state.speed).abs() < 0.01)
        .unwrap_or(2);
    let speed_r = [cx, y, cw, row_h];
    if let Some(i) = ui.tabs(speed_r, speed_idx, &["\u{00bc}x", "\u{00bd}x", "1x", "2x", "4x"]) {
        actions.set_speed = Some(SPEED_STEPS[i]);
    }
    ui.tooltip(speed_r, "Preview speed only \u{2014} the saved animation is unchanged");
    y += row_h + gap * 0.5;

    if let Some(v) = ui.checkbox(
        [cx, y, theme.px(40.0), theme.px(22.0)],
        state.snap_step.is_some(),
        "Snap keyframe times",
    ) {
        actions.set_snap_step = Some(if v { Some(0.1) } else { None });
    }
    ui.tooltip(
        [cx, y, cw, theme.px(22.0)],
        "Round new/edited keyframe times to a neat grid",
    );
    y += row_h;
    if let Some(step) = state.snap_step {
        let snap_idx = SNAP_STEPS
            .iter()
            .position(|s| (s - step).abs() < 0.001)
            .unwrap_or(1);
        let snap_r = [cx, y, cw, row_h];
        if let Some(i) = ui.tabs(snap_r, snap_idx, &["0.05", "0.1", "0.25", "0.5"]) {
            actions.set_snap_step = Some(Some(SNAP_STEPS[i]));
        }
        ui.tooltip(snap_r, "Grid size, in seconds");
        y += row_h + gap * 0.5;
    }

    // -- Keyframes ---------------------------------------------------------------
    section(ui, theme, cx, cw, &mut y, "KEYFRAMES", content_area);
    let keyframes: &[Keyframe] = anim.map(|a| a.keyframes.as_slice()).unwrap_or(&[]);
    for (i, k) in keyframes.iter().enumerate() {
        let row_r = [cx, y, cw, list_row_h];
        if ui.list_row_clipped(
            row_r,
            &key_row_label(k),
            state.selected_key == Some(i),
            Some(content_area),
        ) {
            actions.select_key = Some(i);
        }
        ui.tooltip(row_r, "A saved pose: P=position R=rotation S=scale");
        y += list_row_h + theme.px(2.0);
    }
    y += theme.px(4.0);
    let add_key_r = [cx, y, bw2, theme.px(26.0)];
    if ui.button_secondary(add_key_r, "+ Key @ Playhead") {
        actions.add_key = true;
    }
    ui.tooltip(add_key_r, "Save the previewed pose as a keyframe here (K)");
    let cap_r = [cx + bw2 + theme.px(8.0), y, bw2, theme.px(26.0)];
    if ui.button_secondary(cap_r, "Capture Pose") {
        actions.capture_pose = true;
    }
    ui.tooltip(cap_r, "Snapshot the object's real scene transform as a keyframe");
    y += theme.px(26.0) + theme.px(4.0);
    let has_key = key.is_some();
    let del_key_r = [cx, y, bw3, theme.px(26.0)];
    if has_key {
        if ui.button_styled(del_key_r, "Delete", t::CONTROL_BG, t::TEXT_PRIMARY) {
            actions.delete_key = true;
        }
    } else {
        ui.button_disabled(del_key_r, "Delete", t::CONTROL_ACTIVE, t::TEXT_DISABLED);
    }
    ui.tooltip(del_key_r, "Remove the selected keyframe (Del)");
    let copy_key_r = [cx + bw3 + theme.px(8.0), y, bw3, theme.px(26.0)];
    if has_key {
        if ui.button_styled(copy_key_r, "Copy", t::CONTROL_BG, t::TEXT_PRIMARY) {
            actions.copy_key = true;
        }
    } else {
        ui.button_disabled(copy_key_r, "Copy", t::CONTROL_ACTIVE, t::TEXT_DISABLED);
    }
    ui.tooltip(copy_key_r, "Copy the selected keyframe (\u{2318}C)");
    let paste_key_r = [cx + (bw3 + theme.px(8.0)) * 2.0, y, bw3, theme.px(26.0)];
    if has_key_clipboard {
        if ui.button_styled(paste_key_r, "Paste", t::CONTROL_BG, t::TEXT_PRIMARY) {
            actions.paste_key = true;
        }
    } else {
        ui.button_disabled(paste_key_r, "Paste", t::CONTROL_ACTIVE, t::TEXT_DISABLED);
    }
    ui.tooltip(paste_key_r, "Paste the copied keyframe at the playhead (\u{2318}V)");
    y += theme.px(26.0) + gap;

    // -- Selected keyframe fields ---------------------------------------------------
    if let Some(k) = key {
        section(ui, theme, cx, cw, &mut y, "SELECTED KEYFRAME", content_area);

        let (label_r, input_r) = split_row([cx, y, cw, fh], theme.px(24.0), field_gap);
        ui.label_styled(
            label_r[0],
            label_r[1] + (label_r[3] - theme.body()) * 0.5,
            "t",
            theme.body(),
            t::TEXT_SECONDARY,
            label_r[2],
            Some(content_area),
        );
        let wid = WidgetId::of("animsim_key_t");
        if let Some(v) = ui.drag_float_clipped(wid, input_r, k.t, 0.01, "", Some(content_area)) {
            actions.key_field_edit = Some((KeyField::T, v.max(0.0)));
        }
        ui.tooltip(input_r, "When this keyframe happens, in seconds \u{2014} drag to change");
        y += fh + field_gap_y + theme.px(2.0);

        // Channel toggles: click to add/remove a channel from this keyframe.
        let ch_defs = [
            (
                "Pos",
                KeyChannel::Position,
                k.position.is_some(),
                "Toggle: does this keyframe move the object?",
            ),
            (
                "Rot",
                KeyChannel::Rotation,
                k.rotation.is_some(),
                "Toggle: does this keyframe turn the object?",
            ),
            (
                "Scl",
                KeyChannel::Scale,
                k.scale.is_some(),
                "Toggle: does this keyframe resize the object?",
            ),
        ];
        let cbw = (cw - theme.px(16.0)) / 3.0;
        for (i, (label, ch, on, tip)) in ch_defs.iter().enumerate() {
            let r = [cx + i as f32 * (cbw + theme.px(8.0)), y, cbw, theme.px(24.0)];
            let (bg, fg) = if *on {
                (t::ACCENT_DIM, t::ACCENT_HI)
            } else {
                (t::CONTROL_BG, t::TEXT_DISABLED)
            };
            if ui.button_styled(r, label, bg, fg) {
                actions.toggle_channel = Some(*ch);
            }
            ui.tooltip(r, tip);
        }
        y += theme.px(24.0) + gap * 0.75;

        if let Some(p) = k.position {
            ui.label_styled(
                cx,
                y,
                "POSITION (m)",
                theme.small(),
                t::TEXT_SECONDARY,
                cw,
                Some(content_area),
            );
            ui.tooltip(
                [cx, y, cw, theme.px(16.0)],
                "Where the object sits at this keyframe \u{2014} drag values below",
            );
            y += theme.px(18.0);
            let vals = [p.x, p.y, p.z];
            for (i, axis) in axes.iter().enumerate() {
                let (label_r, input_r) = split_row([cx, y, cw, fh], label_w, field_gap);
                ui.label_styled(
                    label_r[0],
                    label_r[1] + (label_r[3] - theme.body()) * 0.5,
                    axis,
                    theme.body(),
                    t::TEXT_SECONDARY,
                    label_r[2],
                    Some(content_area),
                );
                let wid = WidgetId::of(&format!("animsim_key_pos_{i}"));
                if let Some(v) =
                    ui.drag_float_clipped(wid, input_r, vals[i], 0.005, "", Some(content_area))
                {
                    actions.key_field_edit = Some((KeyField::Pos(i), v));
                }
                y += fh + field_gap_y;
            }
            y += theme.px(4.0);
        }

        if let Some(q) = k.rotation {
            ui.label_styled(
                cx,
                y,
                "ROTATION (deg)",
                theme.small(),
                t::TEXT_SECONDARY,
                cw,
                Some(content_area),
            );
            ui.tooltip(
                [cx, y, cw, theme.px(16.0)],
                "How the object is turned at this keyframe, in degrees",
            );
            y += theme.px(18.0);
            // Rotation is shown/edited in the display (rest-relative) frame so
            // the axes stay independent (see `euler_for_key`). Rest reads (0,0,0).
            let disp = obj
                .map(anim_sim_editor::display_rotation_offset)
                .unwrap_or(Quat::IDENTITY);
            let key_idx = state.selected_key.unwrap_or(0);
            let deg = state.euler_for_key(state.selected_anim, key_idx, q, disp);
            for (i, axis) in axes.iter().enumerate() {
                let (label_r, input_r) = split_row([cx, y, cw, fh], label_w, field_gap);
                ui.label_styled(
                    label_r[0],
                    label_r[1] + (label_r[3] - theme.body()) * 0.5,
                    axis,
                    theme.body(),
                    t::TEXT_SECONDARY,
                    label_r[2],
                    Some(content_area),
                );
                let wid = WidgetId::of(&format!("animsim_key_rot_{i}"));
                if let Some(v) =
                    ui.drag_float_clipped(wid, input_r, deg[i], 0.5, "", Some(content_area))
                {
                    actions.key_field_edit = Some((KeyField::RotEuler(i), v));
                }
                y += fh + field_gap_y;
            }
            y += theme.px(4.0);
        }

        if let Some(s) = k.scale {
            ui.label_styled(
                cx,
                y,
                "SCALE (half size, m)",
                theme.small(),
                t::TEXT_SECONDARY,
                cw,
                Some(content_area),
            );
            ui.tooltip(
                [cx, y, cw, theme.px(16.0)],
                "Object size at this keyframe (box half-extents in meters)",
            );
            y += theme.px(18.0);
            let vals = [s.x, s.y, s.z];
            for (i, axis) in axes.iter().enumerate() {
                let (label_r, input_r) = split_row([cx, y, cw, fh], label_w, field_gap);
                ui.label_styled(
                    label_r[0],
                    label_r[1] + (label_r[3] - theme.body()) * 0.5,
                    axis,
                    theme.body(),
                    t::TEXT_SECONDARY,
                    label_r[2],
                    Some(content_area),
                );
                let wid = WidgetId::of(&format!("animsim_key_scale_{i}"));
                if let Some(v) =
                    ui.drag_float_clipped(wid, input_r, vals[i], 0.005, "", Some(content_area))
                {
                    actions.key_field_edit = Some((KeyField::Scale(i), v.max(0.001)));
                }
                y += fh + field_gap_y;
            }
            y += theme.px(4.0);
        }
    }

    // -- Controller bindings -----------------------------------------------------------
    section(ui, theme, cx, cw, &mut y, "CONTROLLER BINDINGS", content_area);
    for (i, b) in bindings.iter().enumerate() {
        // Row 1: button cycler + animation cycler + remove.
        let rm_w = theme.px(24.0);
        let half = (cw - rm_w - theme.px(16.0)) / 2.0;
        let btn_label = format!("Btn: {}", anim_sim_editor::button_label(&b.button));
        let btn_r = [cx, y, half, theme.px(26.0)];
        if ui.button_secondary(btn_r, &btn_label) {
            actions.cycle_binding_button = Some(i);
        }
        ui.tooltip(btn_r, "Controller button that fires this \u{2014} click to cycle");
        let anim_label = if b.animation.is_empty() {
            "Anim: \u{2014}".to_string()
        } else {
            format!("Anim: {}", b.animation)
        };
        let anim_r = [cx + half + theme.px(8.0), y, half, theme.px(26.0)];
        if ui.button_secondary(anim_r, &anim_label) {
            actions.cycle_binding_anim = Some(i);
        }
        ui.tooltip(anim_r, "Animation it plays \u{2014} click to cycle through them");
        let rm_r = [cx + (half + theme.px(8.0)) * 2.0, y, rm_w, theme.px(26.0)];
        if ui.button_danger(rm_r, "\u{00d7}") {
            actions.remove_binding = Some(i);
        }
        ui.tooltip(rm_r, "Delete this binding");
        y += theme.px(26.0) + theme.px(4.0);

        // Row 2: play mode + scope.
        let mode_idx = if b.play_mode == PlayMode::Sequential { 1 } else { 0 };
        let mode_r = [cx, y, half, theme.px(24.0)];
        if let Some(m) = ui.tabs(mode_r, mode_idx, &["Simul", "Seq"]) {
            actions.binding_mode = Some((
                i,
                if m == 1 {
                    PlayMode::Sequential
                } else {
                    PlayMode::Simultaneous
                },
            ));
        }
        ui.tooltip(mode_r, "Simul: plays right away \u{00b7} Seq: waits its turn in a queue");
        let scope_idx = if b.scope == BindingScope::GlobalAnywhere { 1 } else { 0 };
        let scope_r = [cx + half + theme.px(8.0), y, half + rm_w + theme.px(8.0), theme.px(24.0)];
        if let Some(s) = ui.tabs(scope_r, scope_idx, &["Held", "Global"]) {
            actions.binding_scope = Some((
                i,
                if s == 1 {
                    BindingScope::GlobalAnywhere
                } else {
                    BindingScope::ContextualHold
                },
            ));
        }
        ui.tooltip(scope_r, "Held: only while holding this object \u{00b7} Global: anywhere");
        y += theme.px(24.0) + gap * 0.75;
    }
    if bindings.is_empty() {
        ui.label_styled(
            cx,
            y,
            "No bindings. Click a Btn/Anim chip to cycle its value.",
            theme.small(),
            t::TEXT_DISABLED,
            cw,
            Some(content_area),
        );
        y += theme.px(20.0);
    }
    let add_bind_r = [cx, y, cw, theme.px(26.0)];
    if ui.button_secondary(add_bind_r, "+ Add Binding") {
        actions.add_binding = true;
    }
    ui.tooltip(add_bind_r, "Make a controller button play an animation in-game");
    y += theme.px(26.0) + gap;

    // -- Hotkeys hint (Undo/Redo live in the top bar) --------------------------------
    ui.separator(cx, y, cw);
    y += theme.px(10.0);
    ui.label_styled(
        cx,
        y,
        "Space play/pause \u{00b7} K add key \u{00b7} Del remove key\n\u{2190}/\u{2192} step playhead \u{00b7} \u{2318}Z undo",
        theme.small(),
        t::TEXT_DISABLED,
        cw,
        Some(content_area),
    );
    y += theme.px(36.0) + theme.px(16.0);

    let content_height = y - y_start + scroll_y;
    ui.pop_clip();
    ui.end_scroll_area(scroll_id, content_area, content_height);
    state.content_height = content_height;

    // -- Hover info box ---------------------------------------------------------
    // Show the hovered control's help text in a fixed strip just below the top
    // bar instead of a floating tooltip that overlaps the panel and 3D view.
    let hint = ui.hovered_hint();
    ui.clear_tooltip();
    let info_r = [bar[0], bar[1] + bar[3], bar[2], theme.px(24.0)];
    ui.fill(info_r, t::SURFACE_RAISED);
    ui.separator(info_r[0], info_r[1] + info_r[3] - theme.px(1.0), info_r[2]);
    let (info_text, info_color) = match &hint {
        Some(h) => (h.as_str(), t::TEXT_PRIMARY),
        None => ("Hover a control for a description.", t::TEXT_DISABLED),
    };
    ui.label_styled(
        info_r[0] + theme.px(PAD),
        info_r[1] + (info_r[3] - theme.body()) * 0.5,
        info_text,
        theme.body(),
        info_color,
        info_r[2] - theme.px(PAD) * 2.0,
        Some(info_r),
    );

    actions
}
