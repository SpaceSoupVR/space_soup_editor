use glam::{EulerRot, Quat};

use agate::theme as t;
use agate::{Theme, Ui, WidgetId};

use space_soup::ui2d::Color;
use space_soup_engine::Scene;

use super::super::layout::{Layout, PAD, ROW_H};
use super::super::EditTarget;
use super::split_row;

use agate::TextEditor;

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw(
    ui: &mut Ui,
    theme: &Theme,
    layout: &Layout,
    editing: &Option<EditTarget>,
    editor: &TextEditor,
    scene: &mut Scene,
    game_dir: &std::path::Path,
    selected_object: &mut Option<String>,
    scene_dirty: &mut bool,
    open_script_editor: &mut Option<String>,
    open_grab_pose_editor: &mut Option<String>,
    open_anim_sim_editor: &mut Option<String>,
    content_height: &mut f32,
    editable: bool,
    rot_edit: &mut Option<(String, [f32; 3])>,
    _packet: &space_soup_engine::DebugPacket,
) {
    ui.panel_bordered(layout.inspector, t::SIDEBAR_BG);

    let ix = layout.inspector[0];
    let iy = layout.inspector[1];
    let iw = layout.inspector[2];
    let hdr = match editing {
        Some(EditTarget::SceneFile) => "EDITOR",
        Some(EditTarget::ObjectScript(_)) => "SCRIPT",
        None => "INSPECTOR",
    };
    let clip_ins = layout.inspector;
    let body_top = iy + theme.px(ROW_H + 10.0);

    if editing.is_some() {
        draw_header(ui, theme, ix, iy, iw, hdr, clip_ins);
        draw_editor_info(ui, theme, ix, iw, body_top, clip_ins, editor, editing);
        return;
    }

    let sel_id = selected_object
        .clone()
        .filter(|id| scene.find_object(id).is_some());
    match sel_id {
        Some(id) => {
            // The cards are taller than the panel in small windows — scroll
            // them so the bottom buttons stay reachable instead of being
            // clipped at the panel edge.
            let scroll_id = WidgetId::of("inspector_scroll");
            let (_, scroll_y) = ui.scroll_area(scroll_id, layout.inspector, *content_height);
            ui.label_styled(
                ix + theme.px(PAD),
                iy + theme.px(12.0) - scroll_y,
                hdr,
                theme.small(),
                t::TEXT_SECONDARY,
                iw,
                Some(clip_ins),
            );
            let bottom_y = draw_object_cards(
                ui,
                theme,
                layout,
                ix,
                iw,
                body_top - scroll_y,
                clip_ins,
                scene,
                game_dir,
                &id,
                selected_object,
                scene_dirty,
                open_script_editor,
                open_grab_pose_editor,
                open_anim_sim_editor,
                editable,
                rot_edit,
            );
            let ch = (bottom_y + scroll_y) - iy + theme.px(12.0);
            ui.end_scroll_area(scroll_id, layout.inspector, ch);
            *content_height = ch;
        }
        None => {
            draw_header(ui, theme, ix, iy, iw, hdr, clip_ins);
            ui.label_styled(
                ix + theme.px(PAD),
                body_top,
                "Nothing selected.",
                theme.body(),
                t::TEXT_SECONDARY,
                iw - theme.px(PAD * 2.0),
                Some(clip_ins),
            );
            ui.label_styled(
                ix + theme.px(PAD),
                body_top + theme.px(22.0),
                "Click an object in the viewport\nor the Objects list to edit it.",
                theme.small(),
                t::TEXT_DISABLED,
                iw - theme.px(PAD * 2.0),
                Some(clip_ins),
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_header(
    ui: &mut Ui,
    theme: &Theme,
    ix: f32,
    iy: f32,
    iw: f32,
    hdr: &str,
    clip_ins: [f32; 4],
) {
    ui.label_styled(
        ix + theme.px(PAD),
        iy + theme.px(12.0),
        hdr,
        theme.small(),
        t::TEXT_SECONDARY,
        iw,
        Some(clip_ins),
    );
}

fn draw_editor_info(
    ui: &mut Ui,
    theme: &Theme,
    ix: f32,
    iw: f32,
    body_top: f32,
    clip_ins: [f32; 4],
    editor: &TextEditor,
    editing: &Option<EditTarget>,
) {
    let (ln, col) = editor.cursor_line_col();
    let target_line = match editing {
        Some(EditTarget::SceneFile) => format!("file:   {}", editor.file_name()),
        Some(EditTarget::ObjectScript(id)) => format!("object: {id}"),
        None => String::new(),
    };
    let body = format!(
        "{target_line}\nlines:  {}\nLn {}, Col {}\nmodified: {}\n\nShortcuts:\n\u{2318}S save  \u{2318}Z undo\n\u{2318}C/\u{2318}X/\u{2318}V\n\u{2318}A select all",
        editor.line_count(), ln, col,
        if editor.dirty { "yes" } else { "no" },
    );
    ui.label_styled(
        ix + theme.px(PAD),
        body_top,
        &body,
        theme.small(),
        t::TEXT_PRIMARY,
        iw - theme.px(PAD * 2.0),
        Some(clip_ins),
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_object_cards(
    ui: &mut Ui,
    theme: &Theme,
    layout: &Layout,
    ix: f32,
    iw: f32,
    body_top: f32,
    clip_ins: [f32; 4],
    scene: &mut Scene,
    game_dir: &std::path::Path,
    id: &str,
    selected_object: &mut Option<String>,
    scene_dirty: &mut bool,
    open_script_editor: &mut Option<String>,
    open_grab_pose_editor: &mut Option<String>,
    open_anim_sim_editor: &mut Option<String>,
    editable: bool,
    rot_edit: &mut Option<(String, [f32; 3])>,
) -> f32 {
    let cards = layout.inspector_cards(theme, body_top);

    let (obj_position, obj_half_size, obj_rotation, obj_color, has_script, has_mesh) = {
        let obj = scene.find_object(id).unwrap();
        (
            obj.cuboid.position,
            obj.cuboid.half_size,
            obj.cuboid.rotation,
            obj.cuboid.color,
            obj.script.is_some(),
            obj.mesh.is_some(),
        )
    };

    ui.label_styled(
        cards.name_row[0],
        cards.name_row[1] + (cards.name_row[3] - theme.body()) * 0.5,
        id,
        theme.body(),
        t::TEXT_PRIMARY,
        cards.name_row[2],
        Some(clip_ins),
    );

    let label_w = theme.px(18.0);
    let field_gap = theme.px(6.0);
    let axes = ["X", "Y", "Z"];
    let hdr_h = theme.px(22.0);

    ui.separator(cards.pos_card[0], cards.pos_card[1], cards.pos_card[2]);
    ui.fill(
        [
            cards.pos_card[0],
            cards.pos_card[1],
            cards.pos_card[2],
            hdr_h,
        ],
        t::SURFACE,
    );
    ui.label_styled(
        cards.pos_card[0] + theme.px(PAD),
        cards.pos_card[1] + theme.px(5.0),
        "POSITION",
        theme.small(),
        t::TEXT_SECONDARY,
        cards.pos_card[2],
        None,
    );
    let pos_vals = [obj_position.x, obj_position.y, obj_position.z];
    for i in 0..3usize {
        let (label_r, input_r) = split_row(cards.pos_rows[i], label_w, field_gap);
        ui.label_styled(
            label_r[0],
            label_r[1] + (label_r[3] - theme.body()) * 0.5,
            axes[i],
            theme.body(),
            t::TEXT_SECONDARY,
            label_r[2],
            None,
        );
        if editable {
            let wid = WidgetId::of(&format!("pos_{i}_{id}"));
            if let Some(v) =
                ui.drag_float_clipped(wid, input_r, pos_vals[i], 0.005, "", Some(clip_ins))
            {
                if let Some(obj) = scene.find_object_mut(id) {
                    match i {
                        0 => obj.cuboid.position.x = v,
                        1 => obj.cuboid.position.y = v,
                        _ => obj.cuboid.position.z = v,
                    }
                    *scene_dirty = true;
                }
            }
        } else {
            draw_readonly_value(ui, theme, input_r, &format!("{:.3}", pos_vals[i]), clip_ins);
        }
    }

    ui.separator(cards.sz_card[0], cards.sz_card[1], cards.sz_card[2]);
    ui.fill(
        [cards.sz_card[0], cards.sz_card[1], cards.sz_card[2], hdr_h],
        t::SURFACE,
    );
    ui.label_styled(
        cards.sz_card[0] + theme.px(PAD),
        cards.sz_card[1] + theme.px(5.0),
        "SIZE",
        theme.small(),
        t::TEXT_SECONDARY,
        cards.sz_card[2],
        None,
    );
    let sz_vals = [
        obj_half_size.x * 2.0,
        obj_half_size.y * 2.0,
        obj_half_size.z * 2.0,
    ];
    for i in 0..3usize {
        let (label_r, input_r) = split_row(cards.sz_rows[i], label_w, field_gap);
        ui.label_styled(
            label_r[0],
            label_r[1] + (label_r[3] - theme.body()) * 0.5,
            axes[i],
            theme.body(),
            t::TEXT_SECONDARY,
            label_r[2],
            None,
        );
        if editable {
            let wid = WidgetId::of(&format!("sz_{i}_{id}"));
            if let Some(v) =
                ui.drag_float_clipped(wid, input_r, sz_vals[i], 0.005, "", Some(clip_ins))
            {
                if let Some(obj) = scene.find_object_mut(id) {
                    let half = (v * 0.5).max(0.005);
                    match i {
                        0 => obj.cuboid.half_size.x = half,
                        1 => obj.cuboid.half_size.y = half,
                        _ => obj.cuboid.half_size.z = half,
                    }
                    *scene_dirty = true;
                }
            }
        } else {
            draw_readonly_value(ui, theme, input_r, &format!("{:.3}", sz_vals[i]), clip_ins);
        }
    }

    ui.separator(cards.rot_card[0], cards.rot_card[1], cards.rot_card[2]);
    ui.fill(
        [
            cards.rot_card[0],
            cards.rot_card[1],
            cards.rot_card[2],
            hdr_h,
        ],
        t::SURFACE,
    );
    ui.label_styled(
        cards.rot_card[0] + theme.px(PAD),
        cards.rot_card[1] + theme.px(5.0),
        "ROTATION",
        theme.small(),
        t::TEXT_SECONDARY,
        cards.rot_card[2],
        None,
    );
    // Axis-indexed [X, Y, Z]; glam's YXZ decomposition returns (Y, X, Z).
    // While a drag is in progress the sticky `rot_edit` euler is shown/edited
    // instead of a fresh decompose, which would collapse Y and Z together near
    // the X = ±90° gimbal lock.
    let (ey, ex, ez) = obj_rotation.to_euler(EulerRot::YXZ);
    let rot_deg = match rot_edit {
        Some((rid, deg)) if rid == id => *deg,
        _ => [ex.to_degrees(), ey.to_degrees(), ez.to_degrees()],
    };
    let mut rot_dragging = false;
    for i in 0..3usize {
        let (label_r, input_r) = split_row(cards.rot_rows[i], label_w, field_gap);
        ui.label_styled(
            label_r[0],
            label_r[1] + (label_r[3] - theme.body()) * 0.5,
            axes[i],
            theme.body(),
            t::TEXT_SECONDARY,
            label_r[2],
            None,
        );
        if editable {
            let wid = WidgetId::of(&format!("rot_{i}_{id}"));
            if let Some(v) =
                ui.drag_float_clipped(wid, input_r, rot_deg[i], 0.5, "", Some(clip_ins))
            {
                let mut deg = rot_deg;
                deg[i] = v;
                if let Some(obj) = scene.find_object_mut(id) {
                    obj.cuboid.rotation = Quat::from_euler(
                        EulerRot::YXZ,
                        deg[1].to_radians(),
                        deg[0].to_radians(),
                        deg[2].to_radians(),
                    );
                    *scene_dirty = true;
                }
                *rot_edit = Some((id.to_string(), deg));
                rot_dragging = true;
            }
        } else {
            draw_readonly_value(ui, theme, input_r, &format!("{:.1}", rot_deg[i]), clip_ins);
        }
    }
    if !rot_dragging {
        *rot_edit = None;
    }

    ui.separator(cards.col_card[0], cards.col_card[1], cards.col_card[2]);
    ui.fill(
        [
            cards.col_card[0],
            cards.col_card[1],
            cards.col_card[2],
            hdr_h,
        ],
        t::SURFACE,
    );
    ui.label_styled(
        cards.col_card[0] + theme.px(PAD),
        cards.col_card[1] + theme.px(5.0),
        "COLOR",
        theme.small(),
        t::TEXT_SECONDARY,
        cards.col_card[2],
        None,
    );
    ui.color_swatch(
        cards.col_row,
        Color(obj_color.0, obj_color.1, obj_color.2, 255),
    );

    if has_mesh && ui.button_secondary(cards.btn_voxelize, "Voxelize") {
        match super::super::scene_bridge::voxelize_object(scene, game_dir, id) {
            Ok(new_id) => {
                *selected_object = Some(new_id);
                *scene_dirty = true;
            }
            Err(e) => log::warn!("space_soup_editor: voxelize '{id}' failed: {e}"),
        }
    }

    let script_label = if has_script {
        "Edit Script"
    } else {
        "Add Script"
    };
    if ui.button_secondary(cards.btn_script, script_label) {
        if !has_script {
            if let Some(obj) = scene.find_object_mut(id) {
                obj.script = Some(default_script_stub(id));
                *scene_dirty = true;
            }
        }
        *open_script_editor = Some(id.to_string());
    }

    if ui.button_secondary(cards.btn_grab_pose, "Edit Grab Pose") {
        *open_grab_pose_editor = Some(id.to_string());
    }

    if ui.button_secondary(cards.btn_anim_sim, "Simulate Animations") {
        *open_anim_sim_editor = Some(id.to_string());
    }

    if ui.button_secondary(cards.btn_dup, "Duplicate") {
        if let Some(src) = scene.find_object(id).cloned() {
            let mut copy = src;
            copy.id = unique_id(scene, id);
            copy.cuboid.position += glam::Vec3::new(0.1, 0.0, 0.1);
            let new_id = copy.id.clone();
            scene.objects.push(copy);
            *selected_object = Some(new_id);
            *scene_dirty = true;
        }
    }
    if ui.button_danger(cards.btn_del, "Delete") {
        scene.objects.retain(|o| o.id != id);
        *selected_object = None;
        *scene_dirty = true;
    }

    let hint_y = cards.bottom_y + theme.px(14.0);
    let hint = if editable {
        "Drag a value left/right to slide it."
    } else {
        "Values are read-only here \u{2014} switch to Edit mode to change them."
    };
    ui.label_styled(
        ix + theme.px(PAD),
        hint_y,
        hint,
        theme.small(),
        t::TEXT_SECONDARY,
        iw - theme.px(PAD * 2.0),
        Some(clip_ins),
    );

    hint_y + theme.px(20.0)
}

/// A value shown in a field-shaped slot but not editable (non-Edit view modes).
fn draw_readonly_value(ui: &mut Ui, theme: &Theme, r: [f32; 4], text: &str, clip: [f32; 4]) {
    ui.label_styled(
        r[0] + theme.px(8.0),
        r[1] + (r[3] - theme.body()) * 0.5,
        text,
        theme.body(),
        t::TEXT_DISABLED,
        r[2] - theme.px(16.0),
        Some(clip),
    );
}

fn default_script_stub(id: &str) -> String {
    format!("// Script for '{id}'\n\nfn on_update(dt) {{\n}}\n")
}

fn unique_id(scene: &Scene, base: &str) -> String {
    let stem = base.trim_end_matches(|c: char| c.is_ascii_digit() || c == '_');
    let stem = if stem.is_empty() { base } else { stem };
    let mut n = 2;
    loop {
        let candidate = format!("{stem}_{n}");
        if scene.find_object(&candidate).is_none() {
            return candidate;
        }
        n += 1;
    }
}
