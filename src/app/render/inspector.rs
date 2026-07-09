use glam::{EulerRot, Quat};

use agate::theme as t;
use agate::{Theme, Ui, WidgetId};

use space_soup::ui2d::Color;
use space_soup_engine::{LightKind, Scene, SoundSourceDef};

use super::super::layout::{InspectorCards, Layout, PAD, ROW_H};
use super::super::EditTarget;

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
    open_object_preview: &mut Option<String>,
    preview_sound: &mut Option<(String, f32, f32)>,
    content_height: &mut f32,
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
                open_object_preview,
                preview_sound,
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
    open_object_preview: &mut Option<String>,
    preview_sound: &mut Option<(String, f32, f32)>,
) -> f32 {
    let (has_light, has_sound) = {
        let obj = scene.find_object(id).unwrap();
        (obj.light.is_some(), obj.sound.is_some())
    };
    let cards = layout.inspector_cards(theme, body_top, has_light, has_sound);

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
        let wid = WidgetId::of(&format!("pos_{i}_{id}"));
        let val_str = format!("{:.3}", pos_vals[i]);
        if let Some(new_str) = ui.text_input(wid, input_r, &val_str, "") {
            if let Ok(v) = new_str.trim().parse::<f32>() {
                if let Some(obj) = scene.find_object_mut(id) {
                    match i {
                        0 => obj.cuboid.position.x = v,
                        1 => obj.cuboid.position.y = v,
                        _ => obj.cuboid.position.z = v,
                    }
                    *scene_dirty = true;
                }
            }
        }
    }

    if !has_light && !has_sound {
        draw_card_header(ui, theme, cards.sz_card, hdr_h, "SIZE");
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
            let wid = WidgetId::of(&format!("sz_{i}_{id}"));
            let val_str = format!("{:.3}", sz_vals[i]);
            if let Some(new_str) = ui.text_input(wid, input_r, &val_str, "") {
                if let Ok(v) = new_str.trim().parse::<f32>() {
                    if v > 0.0 {
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
                }
            }
        }
    } else if has_light {
        draw_light_card(ui, theme, &cards, id, scene, scene_dirty);
    } else if has_sound {
        draw_sound_card(ui, theme, &cards, id, scene, scene_dirty);
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
    let (ex, ey, ez) = obj_rotation.to_euler(EulerRot::YXZ);
    let rot_deg = [ex.to_degrees(), ey.to_degrees(), ez.to_degrees()];
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
        let wid = WidgetId::of(&format!("rot_{i}_{id}"));
        let val_str = format!("{:.1}", rot_deg[i]);
        if let Some(new_str) = ui.text_input(wid, input_r, &val_str, "") {
            if let Ok(v) = new_str.trim().parse::<f32>() {
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
            }
        }
    }

    if !has_light && !has_sound {
        draw_card_header(ui, theme, cards.col_card, hdr_h, "COLOR");
        ui.color_swatch(
            cards.col_row,
            Color(obj_color.0, obj_color.1, obj_color.2, 255),
        );
    }

    if has_mesh && ui.button_secondary(cards.btn_voxelize, "Voxelize") {
        match super::super::scene_bridge::voxelize_object(scene, game_dir, id) {
            Ok(new_id) => {
                *selected_object = Some(new_id);
                *scene_dirty = true;
            }
            Err(e) => log::warn!("space_soup_editor: voxelize '{id}' failed: {e}"),
        }
    } else if has_sound && ui.button_secondary(cards.btn_voxelize, "Play Preview") {
        if let Some(sound) = scene.find_object(id).and_then(|o| o.sound.clone()) {
            *preview_sound = Some((sound.clip, sound.volume, sound.pitch));
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

    if ui.button_secondary(cards.btn_preview, "Preview") {
        *open_object_preview = Some(id.to_string());
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
    ui.label_styled(
        ix + theme.px(PAD),
        hint_y,
        "Click a field and type a value.",
        theme.small(),
        t::TEXT_SECONDARY,
        iw - theme.px(PAD * 2.0),
        Some(clip_ins),
    );

    hint_y + theme.px(20.0)
}

fn draw_card_header(ui: &mut Ui, theme: &Theme, card: [f32; 4], hdr_h: f32, title: &str) {
    ui.separator(card[0], card[1], card[2]);
    ui.fill([card[0], card[1], card[2], hdr_h], t::SURFACE);
    ui.label_styled(
        card[0] + theme.px(PAD),
        card[1] + theme.px(5.0),
        title,
        theme.small(),
        t::TEXT_SECONDARY,
        card[2],
        None,
    );
}

/// A labeled text-input row, parsed as `f32` on edit. Returns the parsed
/// value when the user commits a valid edit this frame.
fn text_field_row(
    ui: &mut Ui,
    theme: &Theme,
    row: [f32; 4],
    label: &str,
    label_w: f32,
    id: &str,
    value: f32,
    decimals: usize,
) -> Option<f32> {
    let (label_r, input_r) = split_row(row, label_w, theme.px(6.0));
    ui.label_styled(
        label_r[0],
        label_r[1] + (label_r[3] - theme.body()) * 0.5,
        label,
        theme.body(),
        t::TEXT_SECONDARY,
        label_r[2],
        None,
    );
    let wid = WidgetId::of(&format!("{id}_{}", label));
    let val_str = format!("{value:.decimals$}");
    ui.text_input(wid, input_r, &val_str, "")
        .and_then(|s| s.trim().parse::<f32>().ok())
}

fn draw_light_card(
    ui: &mut Ui,
    theme: &Theme,
    cards: &InspectorCards,
    id: &str,
    scene: &mut Scene,
    scene_dirty: &mut bool,
) {
    draw_card_header(ui, theme, cards.light_card, theme.px(22.0), "LIGHT");

    let light = scene
        .find_object(id)
        .and_then(|o| o.light.clone())
        .unwrap_or_default();

    let label_w = theme.px(56.0);

    let kind_idx = matches!(light.kind, LightKind::Spot) as usize;
    if let Some(new_idx) = ui.tabs(cards.light_rows[0], kind_idx, &["Point", "Spot"]) {
        if let Some(obj) = scene.find_object_mut(id) {
            if let Some(l) = obj.light.as_mut() {
                l.kind = if new_idx == 1 {
                    LightKind::Spot
                } else {
                    LightKind::Point
                };
                *scene_dirty = true;
            }
        }
    }

    if let Some(v) = text_field_row(
        ui, theme, cards.light_rows[1], "Intens.", label_w, id, light.intensity, 2,
    ) {
        if v >= 0.0 {
            if let Some(l) = scene.find_object_mut(id).and_then(|o| o.light.as_mut()) {
                l.intensity = v;
                *scene_dirty = true;
            }
        }
    }

    if let Some(v) = text_field_row(
        ui, theme, cards.light_rows[2], "Range", label_w, id, light.range, 2,
    ) {
        if v > 0.0 {
            if let Some(l) = scene.find_object_mut(id).and_then(|o| o.light.as_mut()) {
                l.range = v;
                *scene_dirty = true;
            }
        }
    }

    if let Some(v) = text_field_row(
        ui, theme, cards.light_rows[3], "Cone\u{b0}", label_w, id, light.cone_angle_deg, 1,
    ) {
        if v > 0.0 && v <= 180.0 {
            if let Some(l) = scene.find_object_mut(id).and_then(|o| o.light.as_mut()) {
                l.cone_angle_deg = v;
                *scene_dirty = true;
            }
        }
    }

    ui.color_swatch(
        cards.light_rows[4],
        Color(light.color.0, light.color.1, light.color.2, 255),
    );
}

fn draw_sound_card(
    ui: &mut Ui,
    theme: &Theme,
    cards: &InspectorCards,
    id: &str,
    scene: &mut Scene,
    scene_dirty: &mut bool,
) {
    draw_card_header(ui, theme, cards.sound_card, theme.px(22.0), "SOUND");

    let sound = scene
        .find_object(id)
        .and_then(|o| o.sound.clone())
        .unwrap_or(SoundSourceDef {
            clip: String::new(),
            volume: 1.0,
            pitch: 1.0,
            min_distance: 1.0,
            max_distance: 10.0,
            looping: false,
            autoplay: false,
            directional: false,
            cone_angle_deg: 45.0,
        });

    let label_w = theme.px(56.0);

    let (label_r, input_r) = split_row(cards.sound_rows[0], label_w, theme.px(6.0));
    ui.label_styled(
        label_r[0],
        label_r[1] + (label_r[3] - theme.body()) * 0.5,
        "Clip",
        theme.body(),
        t::TEXT_SECONDARY,
        label_r[2],
        None,
    );
    let clip_wid = WidgetId::of(&format!("sound_clip_{id}"));
    if let Some(new_str) = ui.text_input(clip_wid, input_r, &sound.clip, "sound/activate.mp3") {
        if let Some(s) = scene.find_object_mut(id).and_then(|o| o.sound.as_mut()) {
            s.clip = new_str.trim().to_string();
            *scene_dirty = true;
        }
    }

    if let Some(v) = text_field_row(
        ui, theme, cards.sound_rows[1], "Volume", label_w, id, sound.volume, 2,
    ) {
        if v >= 0.0 {
            if let Some(s) = scene.find_object_mut(id).and_then(|o| o.sound.as_mut()) {
                s.volume = v;
                *scene_dirty = true;
            }
        }
    }

    if let Some(v) = text_field_row(
        ui, theme, cards.sound_rows[2], "Pitch", label_w, id, sound.pitch, 2,
    ) {
        if v > 0.0 {
            if let Some(s) = scene.find_object_mut(id).and_then(|o| o.sound.as_mut()) {
                s.pitch = v;
                *scene_dirty = true;
            }
        }
    }

    if let Some(v) = text_field_row(
        ui, theme, cards.sound_rows[3], "Min D.", label_w, id, sound.min_distance, 2,
    ) {
        if v >= 0.0 {
            if let Some(s) = scene.find_object_mut(id).and_then(|o| o.sound.as_mut()) {
                s.min_distance = v;
                *scene_dirty = true;
            }
        }
    }

    if let Some(v) = text_field_row(
        ui, theme, cards.sound_rows[4], "Max D.", label_w, id, sound.max_distance, 2,
    ) {
        if v > sound.min_distance {
            if let Some(s) = scene.find_object_mut(id).and_then(|o| o.sound.as_mut()) {
                s.max_distance = v;
                *scene_dirty = true;
            }
        }
    }

    let toggle_row = cards.sound_rows[5];
    let tw = (toggle_row[2] - theme.px(8.0) * 2.0) / 3.0;
    let toggles = [
        ("Loop", sound.looping),
        ("Auto", sound.autoplay),
        ("Dir.", sound.directional),
    ];
    for (i, (label, value)) in toggles.iter().enumerate() {
        let r = [
            toggle_row[0] + i as f32 * (tw + theme.px(8.0)),
            toggle_row[1],
            tw,
            toggle_row[3],
        ];
        if let Some(new_val) = ui.checkbox(r, *value, label) {
            if let Some(s) = scene.find_object_mut(id).and_then(|o| o.sound.as_mut()) {
                match i {
                    0 => s.looping = new_val,
                    1 => s.autoplay = new_val,
                    _ => s.directional = new_val,
                }
                *scene_dirty = true;
            }
        }
    }

    if let Some(v) = text_field_row(
        ui, theme, cards.sound_rows[6], "Cone\u{b0}", label_w, id, sound.cone_angle_deg, 1,
    ) {
        if v > 0.0 && v <= 180.0 {
            if let Some(s) = scene.find_object_mut(id).and_then(|o| o.sound.as_mut()) {
                s.cone_angle_deg = v;
                *scene_dirty = true;
            }
        }
    }
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
