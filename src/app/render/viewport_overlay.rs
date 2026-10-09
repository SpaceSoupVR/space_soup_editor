use agate::theme as t;
use agate::{OverlapGuard, Theme, Ui};
use space_soup_engine::Hand;

use crate::transform_gizmo::GizmoMode;

use super::super::layout::Layout;
use super::super::{EditorTool, GizmoPart, NewObjectSource, PRIMITIVE_PALETTE_COUNT};

/// Union of a left-to-right run of equally tall rects.
fn union_row(rects: &[[f32; 4]]) -> [f32; 4] {
    let first = rects[0];
    let last = rects[rects.len() - 1];
    [first[0], first[1], last[0] + last[2] - first[0], first[3]]
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw(
    ui: &mut Ui,
    theme: &Theme,
    layout: &Layout,
    available_models: &[std::path::PathBuf],
    dragging_new_object: &Option<NewObjectSource>,
    model_scroll_y: &mut f32,
    gizmo_drag: Option<GizmoPart>,
    current_mode: GizmoMode,
    current_tool: EditorTool,
    current_hand: Hand,
) -> (Option<GizmoMode>, Option<EditorTool>, Option<Hand>) {
    // These button groups all float over the shared 3D viewport, independently
    // anchored off the navigator/inspector/center edges — not splits of one
    // shared parent, so `Region`/`Flow` can't make them structurally safe.
    // This is the explicit backstop for that case.
    let mut guard = OverlapGuard::new();

    // Transform mode — floating segmented control.
    let mode_rects = layout.mode_button_rects(theme);
    guard.claim_group("viewport.mode_buttons", &mode_rects);
    let modes = [GizmoMode::Translate, GizmoMode::Rotate, GizmoMode::Scale];
    let mode_pill = union_row(&mode_rects);
    let mode_idx = modes.iter().position(|m| *m == current_mode).unwrap();
    let clicked_mode = ui
        .segmented(mode_pill, mode_idx, &["Move", "Rotate", "Scale"])
        .map(|i| modes[i]);
    let mode_tooltips = ["Move (translate)", "Rotate", "Scale"];
    for (r, tip) in mode_rects.iter().zip(mode_tooltips) {
        ui.tooltip(*r, tip);
    }

    // Editing tool — a second segmented group below.
    let tool_rects = layout.tool_button_rects(theme);
    guard.claim_group("viewport.tool_buttons", &tool_rects);
    let tools = [EditorTool::Select, EditorTool::Rigging, EditorTool::Snap];
    let tool_pill = union_row(&tool_rects);
    let tool_idx = tools.iter().position(|t| *t == current_tool).unwrap();
    let clicked_tool = ui
        .segmented(tool_pill, tool_idx, &["Select", "Rigging", "Snap"])
        .map(|i| tools[i]);
    let tool_tooltips = [
        "Select and move objects",
        "Rig hands: pick an object then a hand to attach it to",
        "Snap: adjust finger-curl grip points",
    ];
    for (r, tip) in tool_rects.iter().zip(tool_tooltips) {
        ui.tooltip(*r, tip);
    }

    // Hand picker, only while a hand-based tool is active.
    let mut clicked_hand = None;
    if current_tool == EditorTool::Rigging || current_tool == EditorTool::Snap {
        let hand_rects = layout.hand_toggle_rects(theme);
        guard.claim_group("viewport.hand_toggle", &hand_rects);
        let hands = [Hand::Left, Hand::Right];
        let hand_pill = union_row(&hand_rects);
        let hand_idx = hands.iter().position(|h| *h == current_hand).unwrap();
        clicked_hand = ui.segmented(hand_pill, hand_idx, &["L", "R"]).map(|i| hands[i]);
        ui.tooltip(hand_rects[0], "Left hand");
        ui.tooltip(hand_rects[1], "Right hand");
    }

    // Camera controls, hugging the inspector edge.
    let gizmo_nav_rects = layout.gizmo_rects(theme);
    guard.claim_group("viewport.gizmo_buttons", &gizmo_nav_rects);
    let gizmo_nav_parts = [GizmoPart::Orbit, GizmoPart::Pan, GizmoPart::Zoom];
    let gizmo_nav_labels = ["O", "P", "Z"];
    let gizmo_nav_tooltips = [
        "Orbit camera (drag)",
        "Pan camera (drag)",
        "Zoom camera (drag)",
    ];
    for i in 0..3 {
        let active = gizmo_drag == Some(gizmo_nav_parts[i]);
        let (bg, fg) = if active {
            (t::ACCENT, t::TEXT_ON_ACCENT)
        } else {
            (t::TOOLBAR_BG, t::TEXT_SECONDARY)
        };
        ui.button_styled(gizmo_nav_rects[i], gizmo_nav_labels[i], bg, fg);
        ui.tooltip(gizmo_nav_rects[i], gizmo_nav_tooltips[i]);
    }

    // Library tray: drag a light, sound, or model into the scene.
    let tray = layout.model_tray(theme);
    guard.claim("viewport.model_tray", tray);
    ui.panel_docked(tray, t::TOOLBAR_BG, agate::DockEdge::Top);

    let cx = tray[0] + theme.px(12.0);
    let cw = tray[2] - theme.px(24.0);

    ui.label_styled(
        cx,
        tray[1] + theme.px(8.0),
        "LIBRARY",
        theme.small(),
        t::TEXT_SECONDARY,
        theme.px(80.0),
        Some(tray),
    );
    ui.label_styled(
        cx + theme.px(64.0),
        tray[1] + theme.px(8.0),
        "drag into the scene",
        theme.small(),
        t::TEXT_DISABLED,
        cw - theme.px(64.0),
        Some(tray),
    );

    let total = PRIMITIVE_PALETTE_COUNT + available_models.len();
    let list = layout.model_list_area(theme);
    let max_scroll = layout.model_max_scroll(theme, total);
    *model_scroll_y = model_scroll_y.clamp(0.0, max_scroll);

    let chip_rects = layout.model_rects(theme, total, *model_scroll_y);
    for (i, r) in chip_rects.iter().enumerate() {
        if r[1] + r[3] < list[1] || r[1] > list[1] + list[3] {
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
        ui.panel_clipped(*r, bg, Some(list));
        ui.label_styled(
            r[0] + theme.px(10.0),
            r[1] + (r[3] - theme.small()) * 0.5,
            &label,
            theme.small(),
            fg,
            r[2] - theme.px(20.0),
            Some(list),
        );
    }

    if max_scroll > 0.0 {
        let track_h = list[3];
        let thumb_h = (track_h * list[3] / (list[3] + max_scroll)).max(theme.px(24.0));
        let frac = (*model_scroll_y / max_scroll).clamp(0.0, 1.0);
        let thumb_y = list[1] + frac * (track_h - thumb_h);
        let thumb_x = tray[0] + tray[2] - theme.px(8.0);
        ui.panel([thumb_x, thumb_y, theme.px(4.0), thumb_h], t::SCROLLBAR);
    }

    (clicked_mode, clicked_tool, clicked_hand)
}
