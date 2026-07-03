use agate::theme as t;
use agate::{Theme, Ui};
use space_soup::ui2d::Color;
use space_soup_engine::Hand;

use crate::transform_gizmo::GizmoMode;

use super::super::layout::Layout;
use super::super::{EditorTool, GizmoPart};

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw(
    ui: &mut Ui,
    theme: &Theme,
    layout: &Layout,
    available_models: &[std::path::PathBuf],
    dragging_new_model: &Option<std::path::PathBuf>,
    _gizmo_drag: Option<GizmoPart>,
    current_mode: GizmoMode,
    current_tool: EditorTool,
    current_hand: Hand,
) -> (Option<GizmoMode>, Option<EditorTool>, Option<Hand>) {
    let mode_rects = layout.mode_button_rects(theme);
    let modes = [GizmoMode::Translate, GizmoMode::Rotate, GizmoMode::Scale];
    let mode_labels = ["Move", "Rotate", "Scale"];

    // Draw pill background behind all 3 mode buttons (Xcode-style recessed trough)
    let first_r = mode_rects[0];
    let last_r = mode_rects[2];
    let pill_w = last_r[0] + last_r[2] - first_r[0];
    let pill_r = [first_r[0], first_r[1], pill_w, first_r[3]];
    ui.panel(pill_r, t::CONTROL_ACTIVE);

    let mut clicked_mode = None;
    for i in 0..3 {
        let active = current_mode == modes[i];
        let (bg, fg) = if active {
            (t::CONTROL_BG, t::TEXT_PRIMARY)
        } else {
            (t::CONTROL_ACTIVE, t::TEXT_SECONDARY)
        };
        if ui.button_styled(mode_rects[i], mode_labels[i], bg, fg) {
            clicked_mode = Some(modes[i]);
        }
    }

    // Thin vertical separator lines between mode buttons
    for i in 0..2 {
        let r = mode_rects[i];
        let inset = theme.px(6.0);
        ui.separator_v(r[0] + r[2], r[1] + inset, r[3] - inset * 2.0);
    }

    let tool_rects = layout.tool_button_rects(theme);
    let tools = [EditorTool::Select, EditorTool::Rigging, EditorTool::Snap];
    let tool_labels = ["Select", "Rigging", "Snap"];

    let tfirst = tool_rects[0];
    let tlast = tool_rects[2];
    let tpill_w = tlast[0] + tlast[2] - tfirst[0];
    ui.panel([tfirst[0], tfirst[1], tpill_w, tfirst[3]], t::CONTROL_ACTIVE);

    let mut clicked_tool = None;
    for i in 0..3 {
        let active = current_tool == tools[i];
        let (bg, fg) = if active {
            (t::ACCENT, t::TEXT_ON_ACCENT)
        } else {
            (t::CONTROL_ACTIVE, t::TEXT_SECONDARY)
        };
        if ui.button_styled(tool_rects[i], tool_labels[i], bg, fg) {
            clicked_tool = Some(tools[i]);
        }
    }
    for i in 0..2 {
        let r = tool_rects[i];
        let inset = theme.px(6.0);
        ui.separator_v(r[0] + r[2], r[1] + inset, r[3] - inset * 2.0);
    }

    let mut clicked_hand = None;
    if current_tool == EditorTool::Rigging || current_tool == EditorTool::Snap {
        let hand_rects = layout.hand_toggle_rects(theme);
        let hands = [Hand::Left, Hand::Right];
        let hand_labels = ["L", "R"];
        let hfirst = hand_rects[0];
        let hlast = hand_rects[1];
        ui.panel([hfirst[0], hfirst[1], hlast[0] + hlast[2] - hfirst[0], hfirst[3]], t::CONTROL_ACTIVE);
        for i in 0..2 {
            let active = current_hand == hands[i];
            let (bg, fg) = if active {
                (t::ACCENT, t::TEXT_ON_ACCENT)
            } else {
                (t::CONTROL_ACTIVE, t::TEXT_SECONDARY)
            };
            if ui.button_styled(hand_rects[i], hand_labels[i], bg, fg) {
                clicked_hand = Some(hands[i]);
            }
        }
    }

    let tray = layout.model_tray(theme);
    ui.panel_bordered(tray, Color(20, 20, 24, 230));

    let cx = tray[0] + theme.px(12.0);
    let cw = tray[2] - theme.px(24.0);

    if available_models.is_empty() {
        ui.label_styled(
            cx, tray[1] + theme.px(6.0),
            "No models found in game/models/",
            theme.small(), t::TEXT_SECONDARY, cw, Some(tray),
        );
    } else {
        ui.label_styled(
            cx, tray[1] + theme.px(6.0),
            "Drag a model into the scene",
            theme.small(), t::TEXT_SECONDARY, cw, Some(tray),
        );
        let model_rects = layout.model_rects(theme, available_models.len());
        for (i, r) in model_rects.iter().enumerate() {
            let path = &available_models[i];
            let label = path.file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| format!("model {i}"));
            let active = dragging_new_model.as_deref() == Some(path.as_path());
            let bg = if active { t::ACCENT } else { Color(40, 40, 48, 255) };
            let fg = if active { t::TEXT_ON_ACCENT } else { t::TEXT_PRIMARY };
            ui.panel(*r, bg);
            ui.label_styled(
                r[0] + theme.px(10.0), r[1] + (r[3] - theme.small()) * 0.5,
                &label, theme.small(), fg, r[2] - theme.px(20.0), Some(*r),
            );
        }
    }

    (clicked_mode, clicked_tool, clicked_hand)
}
