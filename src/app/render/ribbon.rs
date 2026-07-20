//! The contextual ribbon row under the title bar. Its contents swap with the
//! active `RibbonTab`: Build (tools + gizmo modes), Insert (drag-out chips for
//! lights/sounds/models), Object (actions on the selected object). The clicks
//! are returned to the caller rather than applied here so scene-mutating
//! actions can run inside the undo-capture window in `render::redraw`.

use std::path::PathBuf;

use agate::theme as t;
use agate::{Flow, Theme, Ui};
use space_soup_engine::Hand;

use crate::transform_gizmo::GizmoMode;

use super::super::layout::{Layout, PAD};
use super::super::{EditorTool, NewObjectSource, RibbonTab, PRIMITIVE_PALETTE_COUNT};

/// Facts about the selected object the Object tab needs to pick its labels.
pub(crate) struct SelectionInfo {
    pub id: String,
    pub has_mesh: bool,
    pub has_sound: bool,
    pub has_script: bool,
}

/// One frame's clicks on the Object tab, applied by the caller.
#[derive(Default)]
pub(crate) struct ObjectActions {
    pub voxelize: bool,
    pub sound_preview: bool,
    pub script: bool,
    pub grab_pose: bool,
    pub anim_sim: bool,
    pub preview: bool,
    pub teleport: bool,
    pub duplicate: bool,
    pub delete: bool,
}

pub(crate) struct RibbonResult {
    pub mode: Option<GizmoMode>,
    pub tool: Option<EditorTool>,
    pub hand: Option<Hand>,
    pub actions: ObjectActions,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw(
    ui: &mut Ui,
    theme: &Theme,
    layout: &Layout,
    tab: RibbonTab,
    current_mode: GizmoMode,
    current_tool: EditorTool,
    current_hand: Hand,
    available_models: &[PathBuf],
    dragging_new_object: &Option<NewObjectSource>,
    chip_scroll: &mut f32,
    selection: Option<SelectionInfo>,
) -> RibbonResult {
    ui.panel_bordered_flat(layout.ribbon, t::SIDEBAR_BG);

    let mut out = RibbonResult {
        mode: None,
        tool: None,
        hand: None,
        actions: ObjectActions::default(),
    };

    match tab {
        RibbonTab::Build => draw_build(ui, theme, layout, current_mode, current_tool, current_hand, &mut out),
        RibbonTab::Insert => draw_insert(ui, theme, layout, available_models, dragging_new_object, chip_scroll),
        RibbonTab::Object => draw_object(ui, theme, layout, selection, &mut out.actions),
    }

    out
}

/// Small caption under a button group, Roblox-ribbon style.
fn caption(ui: &mut Ui, theme: &Theme, group: &[agate::Rect], text: &str) {
    let first = group[0];
    let last = group[group.len() - 1];
    let w = last[0] + last[2] - first[0];
    let y = first[1] + first[3] + theme.px(4.0);
    ui.label_styled(
        first[0],
        y,
        text,
        theme.small(),
        t::TEXT_SECONDARY,
        w,
        Some(layout_ribbon_clip(theme, first, w)),
    );
}

/// Clip rect wide enough for a centered caption; keeps labels inside the ribbon.
fn layout_ribbon_clip(theme: &Theme, first: agate::Rect, w: f32) -> agate::Rect {
    [first[0], first[1], w, first[3] + theme.px(20.0)]
}

fn draw_build(
    ui: &mut Ui,
    theme: &Theme,
    layout: &Layout,
    current_mode: GizmoMode,
    current_tool: EditorTool,
    current_hand: Hand,
    out: &mut RibbonResult,
) {
    let tool_rects = layout.tool_button_rects(theme);
    let tools = [EditorTool::Select, EditorTool::Rigging, EditorTool::Snap];
    let tool_labels = ["Select", "Rigging", "Snap"];
    let tool_tooltips = [
        "Select and move objects",
        "Rig hands: pick an object then a hand to attach it to",
        "Snap: adjust finger-curl grip points",
    ];
    let tfirst = tool_rects[0];
    let tlast = tool_rects[2];
    ui.panel(
        [tfirst[0], tfirst[1], tlast[0] + tlast[2] - tfirst[0], tfirst[3]],
        t::CONTROL_ACTIVE,
    );
    for i in 0..3 {
        let active = current_tool == tools[i];
        let (bg, fg) = if active {
            (t::ACCENT, t::TEXT_ON_ACCENT)
        } else {
            (t::CONTROL_ACTIVE, t::TEXT_SECONDARY)
        };
        if ui.button_styled(tool_rects[i], tool_labels[i], bg, fg) {
            out.tool = Some(tools[i]);
        }
        ui.tooltip(tool_rects[i], tool_tooltips[i]);
    }
    for i in 0..2 {
        let r = tool_rects[i];
        let inset = theme.px(6.0);
        ui.separator_v(r[0] + r[2], r[1] + inset, r[3] - inset * 2.0);
    }
    caption(ui, theme, &tool_rects, "Tool");

    let mode_rects = layout.mode_button_rects(theme);
    let modes = [GizmoMode::Translate, GizmoMode::Rotate, GizmoMode::Scale];
    let mode_labels = ["Move", "Rotate", "Scale"];
    let mode_tooltips = ["Move (translate)", "Rotate", "Scale"];
    let mfirst = mode_rects[0];
    let mlast = mode_rects[2];
    ui.panel(
        [mfirst[0], mfirst[1], mlast[0] + mlast[2] - mfirst[0], mfirst[3]],
        t::CONTROL_ACTIVE,
    );
    for i in 0..3 {
        let active = current_mode == modes[i];
        let (bg, fg) = if active {
            (t::CONTROL_BG, t::TEXT_PRIMARY)
        } else {
            (t::CONTROL_ACTIVE, t::TEXT_SECONDARY)
        };
        if ui.button_styled(mode_rects[i], mode_labels[i], bg, fg) {
            out.mode = Some(modes[i]);
        }
        ui.tooltip(mode_rects[i], mode_tooltips[i]);
    }
    for i in 0..2 {
        let r = mode_rects[i];
        let inset = theme.px(6.0);
        ui.separator_v(r[0] + r[2], r[1] + inset, r[3] - inset * 2.0);
    }
    caption(ui, theme, &mode_rects, "Gizmo");

    // Hand toggle only matters while rigging/snapping, same as before.
    if current_tool == EditorTool::Rigging || current_tool == EditorTool::Snap {
        let hand_rects = layout.hand_toggle_rects(theme);
        let hands = [Hand::Left, Hand::Right];
        let hand_labels = ["L", "R"];
        let hand_tooltips = ["Left hand", "Right hand"];
        let hfirst = hand_rects[0];
        let hlast = hand_rects[1];
        ui.panel(
            [hfirst[0], hfirst[1], hlast[0] + hlast[2] - hfirst[0], hfirst[3]],
            t::CONTROL_ACTIVE,
        );
        for i in 0..2 {
            let active = current_hand == hands[i];
            let (bg, fg) = if active {
                (t::ACCENT, t::TEXT_ON_ACCENT)
            } else {
                (t::CONTROL_ACTIVE, t::TEXT_SECONDARY)
            };
            if ui.button_styled(hand_rects[i], hand_labels[i], bg, fg) {
                out.hand = Some(hands[i]);
            }
            ui.tooltip(hand_rects[i], hand_tooltips[i]);
        }
        caption(ui, theme, &hand_rects, "Hand");
    }
}

fn draw_insert(
    ui: &mut Ui,
    theme: &Theme,
    layout: &Layout,
    available_models: &[PathBuf],
    dragging_new_object: &Option<NewObjectSource>,
    chip_scroll: &mut f32,
) {
    let area = layout.ribbon_chip_area(theme);
    let total = PRIMITIVE_PALETTE_COUNT + available_models.len();
    let max_scroll = layout.ribbon_chip_max_scroll(theme, total);
    *chip_scroll = chip_scroll.clamp(0.0, max_scroll);

    let chip_rects = layout.ribbon_chip_rects(theme, total, *chip_scroll);
    for (i, r) in chip_rects.iter().enumerate() {
        if r[0] + r[2] < area[0] || r[0] > area[0] + area[2] {
            continue;
        }
        let (label, active) = if i < PRIMITIVE_PALETTE_COUNT {
            let source = if i == 0 {
                NewObjectSource::Light
            } else {
                NewObjectSource::Sound
            };
            let label = if i == 0 { "\u{1F4A1} Light" } else { "\u{1F50A} Sound" };
            (label.to_string(), dragging_new_object.as_ref() == Some(&source))
        } else {
            let path = &available_models[i - PRIMITIVE_PALETTE_COUNT];
            let label = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| format!("model {i}"));
            let active = matches!(
                dragging_new_object,
                Some(NewObjectSource::Model(p)) if p.as_path() == path.as_path()
            );
            (label, active)
        };

        let hovered = ui.is_hovered(*r);
        let (bg, fg) = if active {
            (t::ACCENT, t::TEXT_ON_ACCENT)
        } else if hovered {
            (t::CONTROL_HOVER, t::TEXT_PRIMARY)
        } else {
            (t::CONTROL_BG, t::TEXT_PRIMARY)
        };
        ui.panel_clipped(*r, bg, Some(area));
        ui.label_styled(
            r[0] + theme.px(10.0),
            r[1] + (r[3] - theme.small()) * 0.5,
            &label,
            theme.small(),
            fg,
            r[2] - theme.px(20.0),
            Some(area),
        );
    }

    ui.label_styled(
        area[0],
        area[1] + area[3] + theme.px(4.0),
        "Drag a chip into the scene \u{00b7} scroll for more",
        theme.small(),
        t::TEXT_SECONDARY,
        area[2],
        Some(layout.ribbon),
    );

    if max_scroll > 0.0 {
        let track_w = area[2];
        let thumb_w = (track_w * area[2] / (area[2] + max_scroll)).max(theme.px(24.0));
        let frac = (*chip_scroll / max_scroll).clamp(0.0, 1.0);
        let thumb_x = area[0] + frac * (track_w - thumb_w);
        let thumb_y = layout.ribbon[1] + layout.ribbon[3] - theme.px(5.0);
        ui.panel([thumb_x, thumb_y, thumb_w, theme.px(3.0)], t::SCROLLBAR);
    }
}

fn draw_object(
    ui: &mut Ui,
    theme: &Theme,
    layout: &Layout,
    selection: Option<SelectionInfo>,
    actions: &mut ObjectActions,
) {
    let pad = theme.px(PAD);
    let x0 = layout.ribbon[0] + pad;
    let Some(sel) = selection else {
        ui.label_styled(
            x0,
            layout.ribbon[1] + (layout.ribbon[3] - theme.body()) * 0.5,
            "Select an object in the scene or Navigator to use these actions.",
            theme.body(),
            t::TEXT_SECONDARY,
            layout.ribbon[2] - pad * 2.0,
            Some(layout.ribbon),
        );
        return;
    };

    let h = theme.px(32.0);
    let y = layout.ribbon[1] + theme.px(8.0);
    let mut flow = Flow::row(x0, y, h, theme.px(8.0));

    // Same dual-use first slot as the old inspector: Voxelize for meshes,
    // sound preview for sound emitters (never both).
    let mut rects: Vec<agate::Rect> = Vec::new();
    if sel.has_mesh {
        let r = flow.take(theme.px(90.0));
        if ui.button_secondary(r, "Voxelize") {
            actions.voxelize = true;
        }
        rects.push(r);
    } else if sel.has_sound {
        let r = flow.take(theme.px(110.0));
        if ui.button_secondary(r, "Play Preview") {
            actions.sound_preview = true;
        }
        rects.push(r);
    }

    let script_label = if sel.has_script { "Edit Script" } else { "Add Script" };
    let r = flow.take(theme.px(100.0));
    if ui.button_secondary(r, script_label) {
        actions.script = true;
    }
    rects.push(r);

    let r = flow.take(theme.px(125.0));
    if ui.button_secondary(r, "Edit Grab Pose") {
        actions.grab_pose = true;
    }
    rects.push(r);

    let r = flow.take(theme.px(160.0));
    if ui.button_secondary(r, "Simulate Animations") {
        actions.anim_sim = true;
    }
    rects.push(r);

    let r = flow.take(theme.px(85.0));
    if ui.button_secondary(r, "Preview") {
        actions.preview = true;
    }
    rects.push(r);

    let r = flow.take(theme.px(90.0));
    if ui.button_secondary(r, "Teleport") {
        actions.teleport = true;
    }
    ui.tooltip(r, "Jump the editor camera to this object");
    rects.push(r);

    let r = flow.take(theme.px(95.0));
    if ui.button_secondary(r, "Duplicate") {
        actions.duplicate = true;
    }
    rects.push(r);

    let r = flow.take(theme.px(80.0));
    if ui.button_danger(r, "Delete") {
        actions.delete = true;
    }
    rects.push(r);

    caption(ui, theme, &rects, &format!("Selected: {}", sel.id));
}
