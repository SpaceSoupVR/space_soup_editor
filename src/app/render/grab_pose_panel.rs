//! Side panel for the Grab Pose Editor: grip point list, per-point pose
//! fields, finger curls, and view options. Pure view — all data mutations are
//! returned as `GrabPosePanelActions` and applied by `render/mod.rs` through
//! `grab_pose_editor` functions.

use glam::Quat;

use agate::theme as t;
use agate::{Theme, Ui, WidgetId};

use space_soup_engine::{GripKind, Hand, Scene};

use super::super::grab_pose_editor::{self, GrabPoseEditorState, HandView, PoseField};
use super::super::layout::{Layout, PAD, ROW_H};
use super::confirm::{draw_exit_confirm, ExitChoice};
use super::split_row;

#[derive(Default)]
pub(crate) struct GrabPosePanelActions {
    pub save: bool,
    pub request_exit: bool,
    pub exit_discard: bool,
    pub exit_save: bool,
    pub cancel_exit: bool,
    pub reset: bool,
    pub undo: bool,
    pub redo: bool,
    pub recenter: bool,
    /// Outer Some = fired; inner is the new snap value (None = snapping off).
    pub set_pos_snap: Option<Option<f32>>,
    pub set_rot_snap: Option<Option<f32>>,
    pub set_pos_snap_step: Option<f32>,
    pub set_rot_snap_step: Option<f32>,
    pub field_edit: Option<(PoseField, f32)>,

    pub finger_curl_edit: Option<(usize, f32)>,
    pub select_point: Option<usize>,
    pub add_point: Option<Hand>,
    pub delete_point: bool,
    pub rename_point: Option<String>,
    pub set_kind: Option<GripKind>,
    pub set_view: Option<HandView>,
}

fn hand_tag(hand: Hand) -> &'static str {
    match hand {
        Hand::Left => "L",
        Hand::Right => "R",
    }
}

/// Top bar with title; returns the y to place buttons at.
fn draw_top_bar(ui: &mut Ui, theme: &Theme, bar: [f32; 4], object_id: &str) -> f32 {
    ui.fill(bar, t::TOOLBAR_BG);
    ui.separator(bar[0], bar[1] + bar[3] - theme.px(1.0), bar[2]);
    let title = format!("Grab Pose Editor \u{2014} {object_id}");
    ui.label_styled(
        bar[0] + theme.px(PAD),
        bar[1] + (bar[3] - theme.body()) * 0.5,
        &title,
        theme.body(),
        t::TEXT_PRIMARY,
        bar[2] - theme.px(340.0),
        Some(bar),
    );
    bar[1] + (bar[3] - theme.px(28.0)) * 0.5
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw(
    ui: &mut Ui,
    theme: &Theme,
    layout: &Layout,
    state: &mut GrabPoseEditorState,
    scene: &Scene,
) -> GrabPosePanelActions {
    let mut actions = GrabPosePanelActions::default();
    let bar = layout.editor_tab;

    // The unsaved-changes dialog replaces all other interaction (immediate-mode
    // widgets underneath would still take clicks).
    if state.confirm_exit {
        draw_top_bar(ui, theme, bar, &state.object_id);
        ui.panel_bordered(layout.inspector, t::SIDEBAR_BG);
        match draw_exit_confirm(ui, theme, layout) {
            Some(ExitChoice::Exit) => actions.exit_discard = true,
            Some(ExitChoice::SaveExit) => actions.exit_save = true,
            Some(ExitChoice::Return) => actions.cancel_exit = true,
            None => {}
        }
        return actions;
    }

    // -- Top bar ------------------------------------------------------------
    let btn_y = draw_top_bar(ui, theme, bar, &state.object_id);
    let bh = theme.px(28.0);
    let tb_gap = theme.px(8.0);
    let dirty = state.dirty(scene);

    let exit_w = theme.px(70.0);
    let exit_r = [bar[0] + bar[2] - theme.px(PAD) - exit_w, btn_y, exit_w, bh];
    if ui.button_secondary(exit_r, "Exit") {
        actions.request_exit = true;
    }
    ui.tooltip(exit_r, "Close the editor \u{2014} asks first if you have unsaved changes");

    let save_w = theme.px(70.0);
    let save_r = [exit_r[0] - tb_gap - save_w, btn_y, save_w, bh];
    if dirty {
        if ui.button_success(save_r, "Save") {
            actions.save = true;
        }
        ui.tooltip(save_r, "Keep these grip points (Save Scene in the main view writes to disk)");
    } else {
        ui.button_disabled(save_r, "Save", t::CONTROL_ACTIVE, t::TEXT_DISABLED);
        ui.tooltip(save_r, "No changes to save");
    }

    let ur_w = theme.px(60.0);
    let redo_r = [save_r[0] - tb_gap - ur_w, btn_y, ur_w, bh];
    let undo_r = [redo_r[0] - tb_gap - ur_w, btn_y, ur_w, bh];
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
    let recenter_r = [undo_r[0] - tb_gap - rc_w, btn_y, rc_w, bh];
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
    let tw = theme.px(40.0);
    let th = theme.px(22.0);

    let content_area = layout.inspector;
    let scroll_id = WidgetId::of("grabpose_scroll");
    let (_, scroll_y) = ui.scroll_area(scroll_id, content_area, state.content_height);
    // Confine every widget drawn below to the panel so scrolled-past content
    // (buttons, tabs, sliders — not just the self-clipping labels) stays hidden
    // instead of spilling over the top bar and 3D view.
    ui.push_clip(content_area);
    let y_start = layout.inspector[1] + theme.px(16.0) - scroll_y;
    let mut y = y_start;

    let obj = scene.find_object(&state.object_id);
    let points = obj.map(|o| o.grip_points.as_slice()).unwrap_or(&[]);
    // Only a point the current view shows is editable/previewed.
    let active = points
        .get(state.active_point)
        .filter(|p| state.hand_view.shows(p));

    // -- Hand view (display-only) ----------------------------------------------
    ui.label_styled(
        cx,
        y,
        "HAND VIEW",
        theme.small(),
        t::TEXT_SECONDARY,
        cw,
        Some(content_area),
    );
    y += theme.px(18.0);
    let view_idx = match state.hand_view {
        HandView::All => 0,
        HandView::Left => 1,
        HandView::Right => 2,
    };
    let view_r = [cx, y, cw, row_h];
    if let Some(i) = ui.tabs(view_r, view_idx, &["All", "Left", "Right"]) {
        actions.set_view = Some(match i {
            1 => HandView::Left,
            2 => HandView::Right,
            _ => HandView::All,
        });
    }
    ui.tooltip(
        view_r,
        "View only \u{2014} show all points or just one hand's, to keep things organized. Nothing changes in the game",
    );
    y += row_h + gap;

    // -- Grip point list --------------------------------------------------------
    ui.label_styled(
        cx,
        y,
        "GRIP POINTS",
        theme.small(),
        t::TEXT_SECONDARY,
        cw,
        Some(content_area),
    );
    y += theme.px(20.0);
    let list_row_h = theme.px(24.0);
    let mut listed = 0;
    for (i, point) in points.iter().enumerate() {
        if !state.hand_view.shows(point) {
            continue;
        }
        listed += 1;
        let kind = match point.kind {
            GripKind::Snap => "Snap",
            GripKind::Free => "Free",
            GripKind::Pinch => "Pinch",
        };
        let label = format!("[{}] {}  ({kind})", hand_tag(point.hand), point.name);
        let row_r = [cx, y, cw, list_row_h];
        if ui.list_row_clipped(row_r, &label, i == state.active_point, Some(content_area)) {
            actions.select_point = Some(i);
        }
        ui.tooltip(row_r, "Click to pose this point. [L] = left hand, [R] = right hand");
        y += list_row_h + theme.px(2.0);
    }
    if listed == 0 {
        let hint = match state.hand_view {
            HandView::Left => "No left-hand points yet.",
            HandView::Right => "No right-hand points yet.",
            HandView::All => "No grip points yet.",
        };
        ui.label_styled(
            cx,
            y,
            hint,
            theme.small(),
            t::TEXT_DISABLED,
            cw,
            Some(content_area),
        );
        y += theme.px(20.0);
    }
    y += theme.px(4.0);
    let bw = (cw - theme.px(8.0)) / 2.0;
    let add_l_r = [cx, y, bw, theme.px(26.0)];
    if ui.button_secondary(add_l_r, "+ Left Hand") {
        actions.add_point = Some(Hand::Left);
    }
    ui.tooltip(add_l_r, "Add a grip point only the left hand can grab in-game");
    let add_r_r = [cx + bw + theme.px(8.0), y, bw, theme.px(26.0)];
    if ui.button_secondary(add_r_r, "+ Right Hand") {
        actions.add_point = Some(Hand::Right);
    }
    ui.tooltip(add_r_r, "Add a grip point only the right hand can grab in-game");
    y += theme.px(26.0) + theme.px(4.0);
    let can_delete = points.len() > 1 && active.is_some();
    let del_r = [cx, y, bw, theme.px(26.0)];
    if can_delete {
        if ui.button_styled(del_r, "Delete", t::CONTROL_BG, t::TEXT_PRIMARY) {
            actions.delete_point = true;
        }
        ui.tooltip(del_r, "Remove the selected grip point");
    } else {
        ui.button_disabled(del_r, "Delete", t::CONTROL_ACTIVE, t::TEXT_DISABLED);
        ui.tooltip(del_r, "Objects keep at least one grip point");
    }
    y += theme.px(26.0) + gap;

    // -- Selected point (only when the view shows one) ---------------------------
    if let Some(active) = active {
        ui.separator(cx, y, cw);
        y += theme.px(10.0);

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
        let name_wid = WidgetId::of("grabpose_point_name");
        let name_r = [cx, y, cw, theme.px(26.0)];
        if let Some(new_name) = ui.text_input(name_wid, name_r, &active.name, "point name") {
            actions.rename_point = Some(new_name);
        }
        ui.tooltip(name_r, "Rename it \u{2014} scripts refer to grip points by name");
        y += theme.px(26.0) + gap * 0.5;

        ui.label_styled(
            cx,
            y,
            "KIND",
            theme.small(),
            t::TEXT_SECONDARY,
            cw,
            Some(content_area),
        );
        y += theme.px(18.0);
        let kind_idx = match active.kind {
            GripKind::Snap => 0,
            GripKind::Free => 1,
            GripKind::Pinch => 2,
        };
        let kind_r = [cx, y, cw, row_h];
        if let Some(i) = ui.tabs(kind_r, kind_idx, &["Snap", "Free", "Pinch"]) {
            actions.set_kind = Some(match i {
                1 => GripKind::Free,
                2 => GripKind::Pinch,
                _ => GripKind::Snap,
            });
        }
        ui.tooltip(kind_r, "How the hand attaches when this point is grabbed");
        y += row_h + gap * 0.5;
        let kind_hint = match active.kind {
            GripKind::Free => "Free: hand position is followed, rotation stays with the object.",
            GripKind::Pinch => "Pinch: same lock as Snap, but only grabbable via trigger specifically (not squeeze) — for a small control like a slide, not a full-hand grip.",
            GripKind::Snap => "Snap: hand fully locks to this point's position and rotation.",
        };
        ui.label_styled(
            cx,
            y,
            kind_hint,
            theme.small(),
            t::TEXT_DISABLED,
            cw,
            Some(content_area),
        );
        y += theme.px(28.0) + gap * 0.5;

        // -- Pose fields ---------------------------------------------------------
        let pos = active.local_pos;
        // Axis-indexed [X, Y, Z] eulers via the sticky edit value, so the fields
        // stay independent near gimbal lock (see `euler_for_point`).
        let rot_deg =
            state.euler_for_point(state.active_point, Quat::from_array(active.local_rot));
        let scale = active.hand_offset_scale;

        let axes = ["X", "Y", "Z"];
        let label_w = theme.px(18.0);
        let field_gap = theme.px(6.0);
        let fh = theme.px(26.0);
        let field_gap_y = theme.px(4.0);

        ui.separator(cx, y, cw);
        y += theme.px(10.0);
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
            "Where the hand sits on the object \u{2014} drag values below",
        );
        y += theme.px(20.0);
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
            let wid = WidgetId::of(&format!("grabpose_pos_{i}"));
            if let Some(v) =
                ui.drag_float_clipped(wid, input_r, pos[i], 0.001, "", Some(content_area))
            {
                actions.field_edit = Some((PoseField::Pos(i), v));
            }
            y += fh + field_gap_y;
        }
        y += theme.px(6.0);

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
            "How the hand is turned at this point, in degrees",
        );
        y += theme.px(20.0);
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
            let wid = WidgetId::of(&format!("grabpose_rot_{i}"));
            if let Some(v) =
                ui.drag_float_clipped(wid, input_r, rot_deg[i], 0.2, "", Some(content_area))
            {
                actions.field_edit = Some((PoseField::Rot(i), v));
            }
            y += fh + field_gap_y;
        }
        y += theme.px(6.0);

        ui.label_styled(
            cx,
            y,
            "SCALE (visual only)",
            theme.small(),
            t::TEXT_SECONDARY,
            cw,
            Some(content_area),
        );
        ui.tooltip(
            [cx, y, cw, theme.px(16.0)],
            "Resizes the hand model at this point \u{2014} physics is unaffected",
        );
        y += theme.px(20.0);
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
            let wid = WidgetId::of(&format!("grabpose_scale_{i}"));
            if let Some(v) =
                ui.drag_float_clipped(wid, input_r, scale[i], 0.005, "", Some(content_area))
            {
                actions.field_edit = Some((PoseField::Scale(i), v.max(0.01)));
            }
            y += fh + field_gap_y;
        }
        y += theme.px(4.0);

        // -- Finger curls ----------------------------------------------------------
        ui.separator(cx, y, cw);
        y += theme.px(10.0);
        ui.label_styled(
            cx,
            y,
            "FINGER CURL",
            theme.small(),
            t::TEXT_SECONDARY,
            cw,
            Some(content_area),
        );
        ui.tooltip(
            [cx, y, cw, theme.px(16.0)],
            "How closed each finger is at this grip: 0 = open, 1 = full fist",
        );
        y += theme.px(20.0);
        let finger_label_w = theme.px(46.0);
        for (i, (name, _bones)) in grab_pose_editor::FINGER_GROUPS.iter().enumerate() {
            let (label_r, input_r) = split_row([cx, y, cw, fh], finger_label_w, field_gap);
            ui.label_styled(
                label_r[0],
                label_r[1] + (label_r[3] - theme.body()) * 0.5,
                name,
                theme.body(),
                t::TEXT_SECONDARY,
                label_r[2],
                Some(content_area),
            );
            let wid = WidgetId::of(&format!("grabpose_finger_{i}"));
            let v = grab_pose_editor::finger_curl_value(active, i);
            if let Some(nv) = ui.drag_float_clipped(wid, input_r, v, 0.005, "", Some(content_area))
            {
                actions.finger_curl_edit = Some((i, nv.clamp(0.0, 1.0)));
            }
            y += fh + field_gap_y;
        }
        y += theme.px(4.0);
    }

    // -- Snapping / preview -----------------------------------------------------------
    let fh = theme.px(26.0);
    let field_gap_y = theme.px(4.0);
    ui.separator(cx, y, cw);
    y += theme.px(10.0);
    let pos_snap_r = [cx, y, tw, th];
    if let Some(v) = ui.checkbox(pos_snap_r, state.pos_snap.is_some(), "Snap position") {
        actions.set_pos_snap = Some(if v { Some(0.01) } else { None });
    }
    ui.tooltip([cx, y, cw, th], "Round position edits to a neat grid");
    y += row_h;
    if let Some(step) = state.pos_snap {
        let wid = WidgetId::of("grabpose_pos_snap_step");
        let step_r = [cx, y, cw, fh];
        if let Some(v) =
            ui.drag_float_clipped(wid, step_r, step, 0.001, "step (m)", Some(content_area))
        {
            actions.set_pos_snap_step = Some(v.max(0.001));
        }
        ui.tooltip(step_r, "Grid size, in meters");
        y += fh + field_gap_y;
    }
    let rot_snap_r = [cx, y, tw, th];
    if let Some(v) = ui.checkbox(rot_snap_r, state.rot_snap_deg.is_some(), "Snap rotation") {
        actions.set_rot_snap = Some(if v { Some(5.0) } else { None });
    }
    ui.tooltip([cx, y, cw, th], "Round rotation edits to a neat grid");
    y += row_h;
    if let Some(step) = state.rot_snap_deg {
        let wid = WidgetId::of("grabpose_rot_snap_step");
        let step_r = [cx, y, cw, fh];
        if let Some(v) =
            ui.drag_float_clipped(wid, step_r, step, 0.05, "step (deg)", Some(content_area))
        {
            actions.set_rot_snap_step = Some(v.max(0.5));
        }
        ui.tooltip(step_r, "Grid size, in degrees");
        y += fh + field_gap_y;
    }
    y += theme.px(6.0);

    ui.separator(cx, y, cw);
    y += theme.px(10.0);

    if active.is_some() {
        let reset_r = [cx, y, cw, theme.px(30.0)];
        if ui.button_secondary(reset_r, "Reset Grip") {
            actions.reset = true;
        }
        ui.tooltip(
            reset_r,
            "Zero this grip's pose and finger curls (name, hand & kind are kept)",
        );
        y += theme.px(30.0);
    }
    y += theme.px(16.0);

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
