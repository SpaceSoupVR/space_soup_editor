use agate::theme as t;
use agate::{Theme, Ui};

use super::super::layout::{Layout, PAD};
use agate::TextEditor;

pub(crate) fn draw(
    ui: &mut Ui,
    theme: &Theme,
    layout: &Layout,
    show_editor: bool,
    editor: &TextEditor,
    scene_name: &str,
    fps: f32,
    frame_count: u64,
    tool_hint: Option<&str>,
) {
    ui.fill(layout.statusbar, t::STATUSBAR_BG);
    ui.separator(0.0, layout.statusbar[1], layout.statusbar[2] + layout.statusbar[0]);
    // Thin accent bar on the left edge (Xcode-style)
    ui.fill([layout.statusbar[0], layout.statusbar[1], theme.px(3.0), layout.statusbar[3]], t::ACCENT);
    let sb = layout.statusbar;
    let sy = sb[1] + (sb[3] - theme.small()) * 0.5;

    let left = if show_editor {
        editor.path.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "untitled".into())
    } else {
        match tool_hint {
            Some(hint) => format!("Scene: {scene_name}  \u{b7}  {hint}"),
            None => format!("Scene: {scene_name}"),
        }
    };
    ui.label_styled(sb[0] + theme.px(PAD), sy, &left, theme.small(), t::TEXT_SECONDARY, sb[2] * 0.5, None);

    if show_editor {
        let (ln, col) = editor.cursor_line_col();
        let mid = format!("Ln {ln}, Col {col}{}", if editor.has_selection() { "  (sel)" } else { "" });
        let mid_x = sb[0] + (sb[2] - mid.len() as f32 * theme.small() * 0.6) * 0.5;
        ui.label_styled(mid_x, sy, &mid, theme.small(), t::TEXT_SECONDARY, sb[2] * 0.4, None);
    }

    let fps_text = format!("{fps:.1} fps \u{b7} frame {frame_count}");
    let fps_w = fps_text.len() as f32 * theme.small() * 0.62;
    ui.label_styled(sb[0] + sb[2] - fps_w - theme.px(PAD), sy,
        &fps_text, theme.small(), t::TEXT_SECONDARY, fps_w + theme.px(PAD), None);
}
