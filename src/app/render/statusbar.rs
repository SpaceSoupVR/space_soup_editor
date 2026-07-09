use agate::theme as t;
use agate::{Region, Theme, Ui};

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
    ui.panel_bordered(layout.statusbar, t::STATUSBAR_BG);
    let sb = layout.statusbar;
    let sy = sb[1] + (sb[3] - theme.small()) * 0.5;

    let fps_text = format!("{fps:.1} fps \u{b7} frame {frame_count}");
    let fps_w = fps_text.len() as f32 * theme.small() * 0.62 + theme.px(PAD);

    // Left/mid/right zones are a true 3-way split of one row, so the middle
    // (Ln/Col) zone can never overlap the left or fps zones regardless of
    // text length or window width.
    let (left_r, rest) = Region::new(sb).split_left(sb[2] * 0.5);
    let (fps_r, mid_r) = rest.split_right(fps_w);

    let left = if show_editor {
        editor
            .path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "untitled".into())
    } else {
        match tool_hint {
            Some(hint) => format!("Scene: {scene_name}  \u{b7}  {hint}"),
            None => format!("Scene: {scene_name}"),
        }
    };
    ui.label_styled(
        left_r[0] + theme.px(PAD),
        sy,
        &left,
        theme.small(),
        t::TEXT_SECONDARY,
        left_r[2] - theme.px(PAD),
        Some(left_r),
    );

    if show_editor {
        let mid_r = mid_r.rect();
        let (ln, col) = editor.cursor_line_col();
        let mid = format!(
            "Ln {ln}, Col {col}{}",
            if editor.has_selection() {
                "  (sel)"
            } else {
                ""
            }
        );
        let mid_x = mid_r[0] + (mid_r[2] - mid.len() as f32 * theme.small() * 0.6) * 0.5;
        ui.label_styled(
            mid_x.max(mid_r[0]),
            sy,
            &mid,
            theme.small(),
            t::TEXT_SECONDARY,
            mid_r[2],
            Some(mid_r),
        );
    }

    ui.label_styled(
        fps_r[0],
        sy,
        &fps_text,
        theme.small(),
        t::TEXT_SECONDARY,
        fps_r[2],
        Some(fps_r),
    );
}
