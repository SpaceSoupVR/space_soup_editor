use glam::EulerRot;

use agate::theme as t;
use agate::{Theme, Ui, WidgetId};

use space_soup::ui2d::Color;
use space_soup_engine::Scene;

use super::super::layout::{Layout, PAD};
use super::super::object_preview::ObjectPreviewState;
use super::super::scene_bridge;

#[derive(Default)]
pub(crate) struct ObjectPreviewPanelActions {
    pub open_script_editor: bool,
    pub open_grab_pose_editor: bool,
    pub deleted: bool,
}

pub(crate) fn draw(
    ui: &mut Ui,
    theme: &Theme,
    layout: &Layout,
    state: &mut ObjectPreviewState,
    scene: &mut Scene,
    game_dir: &std::path::Path,
    scene_dirty: &mut bool,
) -> ObjectPreviewPanelActions {
    let mut actions = ObjectPreviewPanelActions::default();

    ui.panel_bordered(layout.inspector, t::SIDEBAR_BG);
    let ix = layout.inspector[0];
    let iw = layout.inspector[2];
    let pad = theme.px(PAD);
    let cx = ix + pad;
    let cw = iw - pad * 2.0;
    let fh = theme.px(26.0);
    let field_gap_y = theme.px(4.0);
    let row_h = theme.px(30.0);
    let gap = theme.px(8.0);
    let tw = theme.px(40.0);
    let th = theme.px(22.0);

    let content_area = layout.inspector;
    let scroll_id = WidgetId::of("preview_scroll");
    let (_, scroll_y) = ui.scroll_area(scroll_id, content_area, state.content_height);
    let y_start = layout.inspector[1] + theme.px(16.0) - scroll_y;
    let mut y = y_start;

    let Some(obj) = scene.find_object(&state.object_id) else {
        return actions;
    };
    let has_mesh = obj.mesh.is_some();
    let pos = obj.cuboid.position.to_array();
    let (ex, ey, ez) = obj.cuboid.rotation.to_euler(EulerRot::YXZ);
    let rot_deg = [ex.to_degrees(), ey.to_degrees(), ez.to_degrees()];
    let size = [
        obj.cuboid.half_size.x * 2.0,
        obj.cuboid.half_size.y * 2.0,
        obj.cuboid.half_size.z * 2.0,
    ];
    let color = obj.cuboid.color;

    ui.label_styled(
        cx,
        y,
        &state.object_id,
        theme.body(),
        t::TEXT_PRIMARY,
        cw,
        Some(content_area),
    );
    y += theme.px(26.0);

    let axes = ["X", "Y", "Z"];
    let label_w = theme.px(18.0);
    let field_gap = theme.px(6.0);

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
    let mut new_pos = pos;
    let mut pos_changed = false;
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
        let wid = WidgetId::of(&format!("preview_pos_{i}"));
        if let Some(v) = ui.drag_float_clipped(wid, input_r, pos[i], 0.001, "", Some(content_area))
        {
            new_pos[i] = v;
            pos_changed = true;
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
    let mut new_rot_deg = rot_deg;
    let mut rot_changed = false;
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
        let wid = WidgetId::of(&format!("preview_rot_{i}"));
        if let Some(v) =
            ui.drag_float_clipped(wid, input_r, rot_deg[i], 0.2, "", Some(content_area))
        {
            new_rot_deg[i] = v;
            rot_changed = true;
        }
        y += fh + field_gap_y;
    }
    y += theme.px(6.0);

    ui.label_styled(
        cx,
        y,
        "SIZE (m)",
        theme.small(),
        t::TEXT_SECONDARY,
        cw,
        Some(content_area),
    );
    y += theme.px(20.0);
    let mut new_size = size;
    let mut size_changed = false;
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
        let wid = WidgetId::of(&format!("preview_size_{i}"));
        if let Some(v) =
            ui.drag_float_clipped(wid, input_r, size[i], 0.005, "", Some(content_area))
        {
            new_size[i] = v.max(0.01);
            size_changed = true;
        }
        y += fh + field_gap_y;
    }
    y += theme.px(4.0);

    if !has_mesh {
        ui.separator(cx, y, cw);
        y += theme.px(10.0);
        ui.label_styled(
            cx,
            y,
            "COLOR",
            theme.small(),
            t::TEXT_SECONDARY,
            cw,
            Some(content_area),
        );
        y += theme.px(20.0);
        let swatch_r = [cx, y, cw, fh];
        ui.color_swatch(swatch_r, Color(color.0, color.1, color.2, 255));
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
        let wid = WidgetId::of("preview_pos_snap_step");
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
        let wid = WidgetId::of("preview_rot_snap_step");
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

    if has_mesh {
        if let Some(v) = ui.checkbox([cx, y, tw, th], state.show_skeleton, "Show skeleton") {
            state.show_skeleton = v;
        }
        y += row_h;
    }

    ui.separator(cx, y, cw);
    y += theme.px(10.0);
    if has_mesh && ui.button_secondary([cx, y, cw, theme.px(30.0)], "Voxelize") {
        match scene_bridge::voxelize_object(scene, game_dir, &state.object_id) {
            Ok(new_id) => {
                state.object_id = new_id;
                *scene_dirty = true;
            }
            Err(e) => log::warn!("space_soup_editor: voxelize '{}' failed: {e}", state.object_id),
        }
    }
    y += theme.px(30.0) + gap;

    if ui.button_secondary([cx, y, cw, theme.px(30.0)], "Edit Script") {
        actions.open_script_editor = true;
    }
    y += theme.px(30.0) + gap;

    if ui.button_secondary([cx, y, cw, theme.px(30.0)], "Edit Grab Pose") {
        actions.open_grab_pose_editor = true;
    }
    y += theme.px(30.0) + gap;

    if ui.button_danger([cx, y, cw, theme.px(30.0)], "Delete") {
        actions.deleted = true;
    }
    y += theme.px(30.0) + theme.px(16.0);

    if pos_changed {
        if let Some(obj) = scene.find_object_mut(&state.object_id) {
            obj.cuboid.position = new_pos.into();
            *scene_dirty = true;
        }
    }
    if rot_changed {
        if let Some(obj) = scene.find_object_mut(&state.object_id) {
            obj.cuboid.rotation = glam::Quat::from_euler(
                EulerRot::YXZ,
                new_rot_deg[1].to_radians(),
                new_rot_deg[0].to_radians(),
                new_rot_deg[2].to_radians(),
            );
            *scene_dirty = true;
        }
    }
    if size_changed {
        if let Some(obj) = scene.find_object_mut(&state.object_id) {
            obj.cuboid.half_size = glam::Vec3::new(
                (new_size[0] * 0.5).max(0.005),
                (new_size[1] * 0.5).max(0.005),
                (new_size[2] * 0.5).max(0.005),
            );
            *scene_dirty = true;
        }
    }

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
