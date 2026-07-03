//! UI for the Interactive VR Grab Pose Editor — drawn in place of the
//! navigator/inspector/toolbar-mode-buttons/viewport_overlay whenever
//! `App.grab_pose_editor` is `Some`. Follows the same "panel is a plain
//! function over pre-computed `Layout` rects" pattern as `inspector.rs`.

use glam::{EulerRot, Quat};

use agate::theme as t;
use agate::{Theme, Ui, WidgetId};

use space_soup_engine::{Hand, Scene};

use crate::transform_gizmo::{GizmoMode, GizmoSpace, TransformGizmo};

use super::super::grab_pose_editor::{GrabPoseEditorState, PoseField};
use super::super::layout::{Layout, PAD, ROW_H};

#[derive(Default)]
pub(crate) struct GrabPosePanelActions {
    pub close: bool,
    pub reset: bool,
    pub undo: bool,
    pub redo: bool,
    pub field_edit: Option<(PoseField, f32)>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw(
    ui: &mut Ui,
    theme: &Theme,
    layout: &Layout,
    state: &mut GrabPoseEditorState,
    gizmo: &mut TransformGizmo,
    scene: &Scene,
) -> GrabPosePanelActions {
    let mut actions = GrabPosePanelActions::default();

    // --- Top bar ---
    let bar = layout.editor_tab;
    ui.fill(bar, t::TOOLBAR_BG);
    ui.separator(bar[0], bar[1] + bar[3] - theme.px(1.0), bar[2]);
    let title = format!("Grab Pose Editor \u{2014} {}", state.object_id);
    ui.label_styled(
        bar[0] + theme.px(PAD), bar[1] + (bar[3] - theme.body()) * 0.5,
        &title, theme.body(), t::TEXT_PRIMARY, bar[2] - theme.px(160.0), Some(bar),
    );
    let done_w = theme.px(90.0);
    let done_h = theme.px(28.0);
    let done_r = [bar[0] + bar[2] - theme.px(PAD) - done_w, bar[1] + (bar[3] - done_h) * 0.5, done_w, done_h];
    if ui.button_secondary(done_r, "Done") { actions.close = true; }

    // --- Side panel ---
    ui.panel_bordered(layout.inspector, t::SIDEBAR_BG);
    let ix = layout.inspector[0];
    let iw = layout.inspector[2];
    let pad = theme.px(PAD);
    let cx = ix + pad;
    let cw = iw - pad * 2.0;
    let row_h = theme.px(ROW_H);
    let gap = theme.px(8.0);
    let tw = theme.px(40.0);
    let th = theme.px(22.0);
    let mut y = layout.inspector[1] + theme.px(16.0);

    ui.label_styled(cx, y, "HAND VISIBILITY", theme.small(), t::TEXT_SECONDARY, cw, None);
    y += theme.px(20.0);
    if let Some(v) = ui.toggle([cx, y, tw, th], state.hand_visible(Hand::Left), "Left hand") {
        state.set_hand_visible(Hand::Left, v);
    }
    y += row_h;
    if let Some(v) = ui.toggle([cx, y, tw, th], state.hand_visible(Hand::Right), "Right hand") {
        state.set_hand_visible(Hand::Right, v);
    }
    y += row_h + gap;

    ui.label_styled(cx, y, "EDITING HAND", theme.small(), t::TEXT_SECONDARY, cw, None);
    y += theme.px(20.0);
    let active_idx = if state.active_hand == Hand::Left { 0 } else { 1 };
    if let Some(i) = ui.tabs([cx, y, cw, row_h], active_idx, &["Left Hand", "Right Hand"]) {
        state.active_hand = if i == 0 { Hand::Left } else { Hand::Right };
    }
    y += row_h + gap;

    ui.label_styled(cx, y, "GIZMO", theme.small(), t::TEXT_SECONDARY, cw, None);
    y += theme.px(20.0);
    let mode_idx = match gizmo.mode { GizmoMode::Translate => 0, GizmoMode::Rotate => 1, GizmoMode::Scale => 2 };
    if let Some(i) = ui.tabs([cx, y, cw, row_h], mode_idx, &["Move", "Rotate", "Scale"]) {
        gizmo.mode = [GizmoMode::Translate, GizmoMode::Rotate, GizmoMode::Scale][i];
    }
    y += row_h + gap * 0.5;
    if let Some(v) = ui.toggle([cx, y, tw, th], gizmo.space == GizmoSpace::Local, "Local space") {
        gizmo.space = if v { GizmoSpace::Local } else { GizmoSpace::World };
    }
    y += row_h + gap;

    let obj = scene.find_object(&state.object_id);
    let grip = obj.and_then(|o| o.grip_pose(state.active_hand));
    let pos = grip.map(|g| g.hand_offset_pos).unwrap_or([0.0; 3]);
    let rot_deg = grip.map(|g| {
        let q = Quat::from_array(g.hand_offset_rot);
        let (ex, ey, ez) = q.to_euler(EulerRot::YXZ);
        [ex.to_degrees(), ey.to_degrees(), ez.to_degrees()]
    }).unwrap_or([0.0; 3]);
    let scale = grip.map(|g| g.hand_offset_scale).unwrap_or([1.0; 3]);

    let axes = ["X", "Y", "Z"];
    let label_w = theme.px(18.0);
    let field_gap = theme.px(6.0);
    let fh = theme.px(26.0);
    let field_gap_y = theme.px(4.0);

    ui.separator(cx, y, cw);
    y += theme.px(10.0);
    ui.label_styled(cx, y, "POSITION (m)", theme.small(), t::TEXT_SECONDARY, cw, None);
    y += theme.px(20.0);
    for (i, axis) in axes.iter().enumerate() {
        let (label_r, input_r) = split_row([cx, y, cw, fh], label_w, field_gap);
        ui.label_styled(label_r[0], label_r[1] + (label_r[3] - theme.body()) * 0.5,
            axis, theme.body(), t::TEXT_SECONDARY, label_r[2], None);
        let wid = WidgetId::of(&format!("grabpose_pos_{i}"));
        if let Some(v) = ui.drag_float(wid, input_r, pos[i], 0.001, "") {
            actions.field_edit = Some((PoseField::Pos(i), v));
        }
        y += fh + field_gap_y;
    }
    y += theme.px(6.0);

    ui.label_styled(cx, y, "ROTATION (deg)", theme.small(), t::TEXT_SECONDARY, cw, None);
    y += theme.px(20.0);
    for (i, axis) in axes.iter().enumerate() {
        let (label_r, input_r) = split_row([cx, y, cw, fh], label_w, field_gap);
        ui.label_styled(label_r[0], label_r[1] + (label_r[3] - theme.body()) * 0.5,
            axis, theme.body(), t::TEXT_SECONDARY, label_r[2], None);
        let wid = WidgetId::of(&format!("grabpose_rot_{i}"));
        if let Some(v) = ui.drag_float(wid, input_r, rot_deg[i], 0.2, "") {
            actions.field_edit = Some((PoseField::Rot(i), v));
        }
        y += fh + field_gap_y;
    }
    y += theme.px(6.0);

    ui.label_styled(cx, y, "SCALE (visual only)", theme.small(), t::TEXT_SECONDARY, cw, None);
    y += theme.px(20.0);
    for (i, axis) in axes.iter().enumerate() {
        let (label_r, input_r) = split_row([cx, y, cw, fh], label_w, field_gap);
        ui.label_styled(label_r[0], label_r[1] + (label_r[3] - theme.body()) * 0.5,
            axis, theme.body(), t::TEXT_SECONDARY, label_r[2], None);
        let wid = WidgetId::of(&format!("grabpose_scale_{i}"));
        if let Some(v) = ui.drag_float(wid, input_r, scale[i], 0.005, "") {
            actions.field_edit = Some((PoseField::Scale(i), v.max(0.01)));
        }
        y += fh + field_gap_y;
    }
    y += theme.px(4.0);

    ui.separator(cx, y, cw);
    y += theme.px(10.0);
    if let Some(v) = ui.checkbox([cx, y, tw, th], state.pos_snap.is_some(), "Snap position") {
        state.pos_snap = if v { Some(0.01) } else { None };
    }
    y += row_h;
    if let Some(step) = state.pos_snap {
        let wid = WidgetId::of("grabpose_pos_snap_step");
        if let Some(v) = ui.drag_float(wid, [cx, y, cw, fh], step, 0.001, "step (m)") {
            state.pos_snap = Some(v.max(0.001));
        }
        y += fh + field_gap_y;
    }
    if let Some(v) = ui.checkbox([cx, y, tw, th], state.rot_snap_deg.is_some(), "Snap rotation") {
        state.rot_snap_deg = if v { Some(5.0) } else { None };
    }
    y += row_h;
    if let Some(step) = state.rot_snap_deg {
        let wid = WidgetId::of("grabpose_rot_snap_step");
        if let Some(v) = ui.drag_float(wid, [cx, y, cw, fh], step, 0.05, "step (deg)") {
            state.rot_snap_deg = Some(v.max(0.5));
        }
        y += fh + field_gap_y;
    }
    y += theme.px(6.0);

    ui.separator(cx, y, cw);
    y += theme.px(10.0);
    if let Some(v) = ui.toggle([cx, y, tw, th], state.preview_mode, "Live Grab Preview") {
        state.preview_mode = v;
        state.preview_rotation = Quat::IDENTITY;
    }
    y += row_h + gap;

    let bw = (cw - theme.px(16.0)) / 3.0;
    let bh = theme.px(30.0);
    if ui.button_secondary([cx, y, bw, bh], "Reset") { actions.reset = true; }

    let can_undo = state.can_undo();
    let (ubg, ufg) = if can_undo { (t::CONTROL_BG, t::TEXT_PRIMARY) } else { (t::CONTROL_ACTIVE, t::TEXT_DISABLED) };
    if ui.button_styled([cx + bw + theme.px(8.0), y, bw, bh], "Undo", ubg, ufg) && can_undo {
        actions.undo = true;
    }

    let can_redo = state.can_redo();
    let (rbg, rfg) = if can_redo { (t::CONTROL_BG, t::TEXT_PRIMARY) } else { (t::CONTROL_ACTIVE, t::TEXT_DISABLED) };
    if ui.button_styled([cx + (bw + theme.px(8.0)) * 2.0, y, bw, bh], "Redo", rbg, rfg) && can_redo {
        actions.redo = true;
    }

    actions
}

fn split_row(row: [f32; 4], label_w: f32, field_gap: f32) -> ([f32; 4], [f32; 4]) {
    let label_r = [row[0], row[1], label_w, row[3]];
    let input_r = [row[0] + label_w + field_gap, row[1], row[2] - label_w - field_gap, row[3]];
    (label_r, input_r)
}
