//! Shared "unsaved changes" dialog for the grab pose and animation sub-editors.
//! Drawn as a full-window modal; the caller must skip its other interactive
//! widgets while this is up (immediate-mode widgets underneath would still take
//! the click).

use agate::theme as t;
use agate::{Color, Theme, Ui};

use super::super::layout::Layout;

pub(crate) enum ExitChoice {
    /// "No" — discard everything since the last save and close.
    Exit,
    /// "Yes" — save, then close.
    SaveExit,
    /// The corner "×" — stay in the editor.
    Return,
}

pub(crate) fn draw_exit_confirm(ui: &mut Ui, theme: &Theme, layout: &Layout) -> Option<ExitChoice> {
    // Dim everything behind the dialog.
    ui.fill(layout.window, Color(0, 0, 0, 140));

    let w = theme.px(360.0);
    let h = theme.px(120.0);
    let x = layout.window[0] + (layout.window[2] - w) * 0.5;
    let y = layout.window[1] + (layout.window[3] - h) * 0.4;
    let card = [x, y, w, h];
    ui.panel_bordered(card, t::SURFACE_RAISED);

    let pad = theme.px(16.0);

    // "×" close button in the top-right corner cancels the exit.
    let close = theme.px(24.0);
    let close_r = [x + w - pad * 0.5 - close, y + pad * 0.5, close, close];
    if ui.button_secondary(close_r, "\u{00d7}") {
        return Some(ExitChoice::Return);
    }

    ui.label_styled(
        x + pad,
        y + pad,
        "Save changes before exiting?",
        theme.body(),
        t::TEXT_PRIMARY,
        w - pad * 2.0 - close,
        Some(card),
    );

    let bh = theme.px(30.0);
    let gap = theme.px(8.0);
    let bw = (w - pad * 2.0 - gap) / 2.0;
    let by = y + h - pad - bh;
    let yes_r = [x + pad, by, bw, bh];
    let no_r = [x + pad + bw + gap, by, bw, bh];

    if ui.button_success(yes_r, "Yes") {
        return Some(ExitChoice::SaveExit);
    }
    if ui.button_danger(no_r, "No") {
        return Some(ExitChoice::Exit);
    }
    None
}
