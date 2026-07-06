use glam::{EulerRot, Quat};

use agate::theme as t;
use agate::{Theme, Ui, WidgetId};

use space_soup_engine::{GripKind, Hand, Scene};

use super::super::grab_pose_editor::{self, GrabPoseEditorState, PoseField};
use super::super::layout::{Layout, PAD, ROW_H};

#[derive(Default)]
pub(crate) struct GrabPosePanelActions {
    pub close: bool,
    pub reset: bool,
    pub undo: bool,
    pub redo: bool,
    pub field_edit: Option<(PoseField, f32)>,

    pub finger_curl_edit: Option<(usize, f32)>,
    pub select_point: Option<usize>,
    pub add_point: bool,
    pub delete_point: bool,
    pub rename_point: Option<String>,
    pub set_kind: Option<GripKind>,
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
    ui.fill(bar, t::TOOLBAR_BG);
    ui.separator(bar[0], bar[1] + bar[3] - theme.px(1.0), bar[2]);
    let title = format!("Grab Pose Editor \u{2014} {}", state.object_id);
    ui.label_styled(
        bar[0] + theme.px(PAD),
        bar[1] + (bar[3] - theme.body()) * 0.5,
        &title,
        theme.body(),
        t::TEXT_PRIMARY,
        bar[2] - theme.px(160.0),
        Some(bar),
    );
    let done_w = theme.px(90.0);
    let done_h = theme.px(28.0);
    let done_r = [
        bar[0] + bar[2] - theme.px(PAD) - done_w,
        bar[1] + (bar[3] - done_h) * 0.5,
        done_w,
        done_h,
    ];
    if ui.button_secondary(done_r, "Done") {
        actions.close = true;
    }

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
    let y_start = layout.inspector[1] + theme.px(16.0) - scroll_y;
    let mut y = y_start;

    let obj = scene.find_object(&state.object_id);
    let points = obj.map(|o| o.grip_points.as_slice()).unwrap_or(&[]);
    let active = points.get(state.active_point);

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
    for (i, point) in points.iter().enumerate() {
        let label = match point.kind {
            GripKind::Snap => format!("{}  (Snap)", point.name),
            GripKind::Free => format!("{}  (Free)", point.name),
            GripKind::Pinch => format!("{}  (Pinch)", point.name),
        };
        if ui.list_row_clipped(
            [cx, y, cw, list_row_h],
            &label,
            i == state.active_point,
            Some(content_area),
        ) {
            actions.select_point = Some(i);
        }
        y += list_row_h + theme.px(2.0);
    }
    y += theme.px(4.0);
    let bw = (cw - theme.px(8.0)) / 2.0;
    if ui.button_secondary([cx, y, bw, theme.px(26.0)], "+ Add Point") {
        actions.add_point = true;
    }
    let can_delete = points.len() > 1;
    let del_r = [cx + bw + theme.px(8.0), y, bw, theme.px(26.0)];
    if can_delete {
        if ui.button_styled(del_r, "Delete", t::CONTROL_BG, t::TEXT_PRIMARY) {
            actions.delete_point = true;
        }
    } else {
        ui.button_disabled(del_r, "Delete", t::CONTROL_ACTIVE, t::TEXT_DISABLED);
    }
    y += theme.px(26.0) + gap;

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
    let current_name = active.map(|p| p.name.as_str()).unwrap_or("");
    if let Some(new_name) = ui.text_input(
        name_wid,
        [cx, y, cw, theme.px(26.0)],
        current_name,
        "point name",
    ) {
        actions.rename_point = Some(new_name);
    }
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
    let kind_idx = match active.map(|p| p.kind) {
        Some(GripKind::Free) => 1,
        Some(GripKind::Pinch) => 2,
        _ => 0,
    };
    if let Some(i) = ui.tabs([cx, y, cw, row_h], kind_idx, &["Snap", "Free", "Pinch"]) {
        actions.set_kind = Some(match i {
            1 => GripKind::Free,
            2 => GripKind::Pinch,
            _ => GripKind::Snap,
        });
    }
    y += row_h + gap * 0.5;
    let kind_hint = match active.map(|p| p.kind) {
        Some(GripKind::Free) => "Free: hand position is followed, rotation stays with the object.",
        Some(GripKind::Pinch) => "Pinch: same lock as Snap, but only grabbable via trigger specifically (not squeeze) — for a small control like a slide, not a full-hand grip.",
        _ => "Snap: hand fully locks to this point's position and rotation.",
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

    ui.label_styled(
        cx,
        y,
        "PREVIEW HAND",
        theme.small(),
        t::TEXT_SECONDARY,
        cw,
        Some(content_area),
    );
    y += theme.px(20.0);
    let hand_idx = if state.preview_hand == Hand::Left {
        0
    } else {
        1
    };
    if let Some(i) = ui.tabs([cx, y, cw, row_h], hand_idx, &["Left Hand", "Right Hand"]) {
        state.preview_hand = if i == 0 { Hand::Left } else { Hand::Right };
    }
    y += row_h + gap;

    let pos = active.map(|p| p.local_pos).unwrap_or([0.0; 3]);
    let rot_deg = active
        .map(|p| {
            let q = Quat::from_array(p.local_rot);
            let (ex, ey, ez) = q.to_euler(EulerRot::YXZ);
            [ex.to_degrees(), ey.to_degrees(), ez.to_degrees()]
        })
        .unwrap_or([0.0; 3]);
    let scale = active.map(|p| p.hand_offset_scale).unwrap_or([1.0; 3]);

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
        if let Some(v) = ui.drag_float_clipped(wid, input_r, pos[i], 0.001, "", Some(content_area))
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

    ui.separator(cx, y, cw);
    y += theme.px(10.0);
    ui.label_styled(
        cx,
        y,
        "FINGER CURL (dots preview only)",
        theme.small(),
        t::TEXT_SECONDARY,
        cw,
        Some(content_area),
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
        let v = active
            .map(|p| grab_pose_editor::finger_curl_value(p, i))
            .unwrap_or(0.0);
        if let Some(nv) = ui.drag_float_clipped(wid, input_r, v, 0.005, "", Some(content_area)) {
            actions.finger_curl_edit = Some((i, nv.clamp(0.0, 1.0)));
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
        if let Some(v) = ui.drag_float_clipped(
            wid,
            [cx, y, cw, fh],
            step,
            0.001,
            "step (m)",
            Some(content_area),
        ) {
            state.pos_snap = Some(v.max(0.001));
        }
        y += fh + field_gap_y;
    }
    if let Some(v) = ui.checkbox(
        [cx, y, tw, th],
        state.rot_snap_deg.is_some(),
        "Snap rotation",
    ) {
        state.rot_snap_deg = if v { Some(5.0) } else { None };
    }
    y += row_h;
    if let Some(step) = state.rot_snap_deg {
        let wid = WidgetId::of("grabpose_rot_snap_step");
        if let Some(v) = ui.drag_float_clipped(
            wid,
            [cx, y, cw, fh],
            step,
            0.05,
            "step (deg)",
            Some(content_area),
        ) {
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
    if ui.button_secondary([cx, y, bw, bh], "Reset") {
        actions.reset = true;
    }

    let can_undo = state.can_undo();
    let undo_r = [cx + bw + theme.px(8.0), y, bw, bh];
    if can_undo {
        if ui.button_styled(undo_r, "Undo", t::CONTROL_BG, t::TEXT_PRIMARY) {
            actions.undo = true;
        }
    } else {
        ui.button_disabled(undo_r, "Undo", t::CONTROL_ACTIVE, t::TEXT_DISABLED);
    }

    let can_redo = state.can_redo();
    let redo_r = [cx + (bw + theme.px(8.0)) * 2.0, y, bw, bh];
    if can_redo {
        if ui.button_styled(redo_r, "Redo", t::CONTROL_BG, t::TEXT_PRIMARY) {
            actions.redo = true;
        }
    } else {
        ui.button_disabled(redo_r, "Redo", t::CONTROL_ACTIVE, t::TEXT_DISABLED);
    }
    y += bh + theme.px(16.0);

    let content_height = y - y_start + scroll_y;
    ui.end_scroll_area(scroll_id, content_area, content_height);
    state.content_height = content_height;

    actions
}

fn split_row(row: [f32; 4], label_w: f32, field_gap: f32) -> ([f32; 4], [f32; 4]) {
    let label_r = [row[0], row[1], label_w, row[3]];
    let input_r = [
        row[0] + label_w + field_gap,
        row[1],
        row[2] - label_w - field_gap,
        row[3],
    ];
    (label_r, input_r)
}
