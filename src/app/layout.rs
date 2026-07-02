use agate::theme::Theme;

pub(crate) const TOOLBAR_H: f32 = 56.0;
pub(crate) const STATUSBAR_H: f32 = 28.0;
pub(crate) const NAVIGATOR_W: f32 = 256.0;
pub(crate) const INSPECTOR_W: f32 = 300.0;
pub(crate) const TAB_BAR_H: f32 = 28.0;
pub(crate) const ROW_H: f32 = 30.0;
pub(crate) const PAD: f32 = 12.0;
pub(crate) const PANEL_GAP: f32 = 12.0;
pub(crate) const WINDOW_MARGIN: f32 = 16.0;

pub(crate) type Rect = [f32; 4];

pub(crate) fn rect_from(x: f32, y: f32, w: f32, h: f32) -> Rect {
    [x, y, w, h]
}

pub(crate) fn in_rect(p: (f32, f32), r: Rect) -> bool {
    p.0 >= r[0] && p.0 <= r[0] + r[2] && p.1 >= r[1] && p.1 <= r[1] + r[3]
}

pub(crate) struct Layout {
    pub toolbar: Rect,
    pub navigator: Rect,
    pub inspector: Rect,
    pub statusbar: Rect,
    pub center: Rect,
    pub editor_tab: Rect,
    pub editor_body: Rect,
    pub seg: [Rect; 4],
    pub seg_pill: Rect,
    pub btn_editor: Rect,
    pub btn_save: Rect,
    pub btn_save_scene: Rect,
}

impl Layout {
    pub fn new(win_w: f32, win_h: f32, theme: &Theme) -> Self {
        let tb_h = theme.px(TOOLBAR_H);
        let sb_h = theme.px(STATUSBAR_H);
        let nav_w = theme.px(NAVIGATOR_W);
        let ins_w = theme.px(INSPECTOR_W);
        let tab_h = theme.px(TAB_BAR_H);
        let margin = theme.px(WINDOW_MARGIN);
        let gap = theme.px(PANEL_GAP);

        // Floating panels: every panel is inset from the window edge by
        // `margin` and separated from its neighbors by `gap`, so the 3D
        // viewport shows through behind them instead of panels tiling
        // edge-to-edge.
        let toolbar = rect_from(margin, margin, (win_w - 2.0 * margin).max(0.0), tb_h);

        let body_y = margin + tb_h + gap;
        let body_h = (win_h - body_y - gap - sb_h - margin).max(0.0);

        let navigator = rect_from(margin, body_y, nav_w, body_h);
        let inspector = rect_from((win_w - margin - ins_w).max(0.0), body_y, ins_w, body_h);
        let center = rect_from(
            margin + nav_w + gap, body_y,
            (win_w - 2.0 * margin - nav_w - ins_w - 2.0 * gap).max(0.0), body_h,
        );

        let statusbar = rect_from(margin, win_h - margin - sb_h, (win_w - 2.0 * margin).max(0.0), sb_h);

        let editor_tab = rect_from(center[0], body_y, center[2], tab_h);
        let editor_body = rect_from(center[0], body_y + tab_h, center[2], (body_h - tab_h).max(0.0));

        let pad = theme.px(PAD);
        let seg_h = theme.px(30.0);
        let seg_w = theme.px(100.0);
        let seg_y = toolbar[1] + (tb_h - seg_h) * 0.5;
        let seg_x0 = toolbar[0] + pad;
        let seg = [
            rect_from(seg_x0, seg_y, seg_w, seg_h),
            rect_from(seg_x0 + seg_w, seg_y, seg_w, seg_h),
            rect_from(seg_x0 + 2.0 * seg_w, seg_y, seg_w, seg_h),
            rect_from(seg_x0 + 3.0 * seg_w, seg_y, seg_w, seg_h),
        ];
        // Combined pill bounding rect for all 4 segments
        let seg_pill = rect_from(seg_x0, seg_y, 4.0 * seg_w, seg_h);

        let bw = theme.px(80.0);
        let btn_gap = theme.px(8.0);
        let toolbar_right = toolbar[0] + toolbar[2];
        let btn_save = rect_from(toolbar_right - pad - bw, seg_y, bw, seg_h);
        let btn_editor = rect_from(btn_save[0] - btn_gap - bw, seg_y, bw, seg_h);
        let btn_save_scene = rect_from(btn_editor[0] - btn_gap - theme.px(110.0), seg_y, theme.px(110.0), seg_h);

        Self {
            toolbar, navigator, inspector, statusbar, center,
            editor_tab, editor_body, seg, seg_pill, btn_editor, btn_save, btn_save_scene,
        }
    }

    pub fn nav_row(&self, theme: &Theme, i: usize) -> Rect {
        let row_h = theme.px(ROW_H);
        let top_pad = theme.px(10.0);
        let inset = theme.px(8.0);
        rect_from(
            self.navigator[0] + inset,
            self.navigator[1] + top_pad + i as f32 * row_h,
            self.navigator[2] - inset * 2.0,
            row_h,
        )
    }

    pub fn model_tray(&self, theme: &Theme) -> Rect {
        let gap = theme.px(PANEL_GAP);
        let bar_h = theme.px(72.0);
        let x = self.navigator[0] + self.navigator[2] + gap;
        let w = (self.inspector[0] - x - gap).max(0.0);
        let y = self.center[1] + self.center[3] - bar_h - gap;
        rect_from(x, y, w, bar_h)
    }

    pub fn model_rects(&self, theme: &Theme, count: usize) -> Vec<Rect> {
        let tray = self.model_tray(theme);
        let mw = theme.px(110.0);
        let mh = theme.px(40.0);
        let gap = theme.px(12.0);
        let n = count.max(1);
        let total = count as f32 * mw + (n - 1) as f32 * gap;
        let start = tray[0] + (tray[2] - total) * 0.5;
        let y = tray[1] + (tray[3] - mh) * 0.5;
        (0..count).map(|i| rect_from(start + i as f32 * (mw + gap), y, mw, mh)).collect()
    }

    pub fn gizmo_rects(&self, theme: &Theme) -> [Rect; 3] {
        let size = theme.px(34.0);
        let gap = theme.px(8.0);
        let pad = theme.px(14.0);
        let x = self.inspector[0] - pad - size;
        let y0 = self.center[1] + pad;
        std::array::from_fn(|i| rect_from(x, y0 + i as f32 * (size + gap), size, size))
    }

    pub fn mode_button_rects(&self, theme: &Theme) -> [Rect; 3] {
        let w = theme.px(64.0);
        let h = theme.px(32.0);
        let gap = theme.px(6.0);
        let pad = theme.px(14.0);
        let x0 = self.navigator[0] + self.navigator[2] + theme.px(PANEL_GAP) + pad;
        let y = self.center[1] + pad;
        std::array::from_fn(|i| rect_from(x0 + i as f32 * (w + gap), y, w, h))
    }

    pub fn tool_button_rects(&self, theme: &Theme) -> [Rect; 3] {
        let w = theme.px(64.0);
        let h = theme.px(32.0);
        let gap = theme.px(6.0);
        let pad = theme.px(14.0);
        let row_gap = theme.px(8.0);
        let x0 = self.navigator[0] + self.navigator[2] + theme.px(PANEL_GAP) + pad;
        let y = self.center[1] + pad + h + row_gap;
        std::array::from_fn(|i| rect_from(x0 + i as f32 * (w + gap), y, w, h))
    }

    pub fn inspector_cards(&self, theme: &Theme, top_y: f32) -> InspectorCards {
        let ix = self.inspector[0];
        let iw = self.inspector[2];
        let pad = theme.px(PAD);
        let cx = ix + pad;
        let cw = iw - pad * 2.0;
        let fh = theme.px(26.0);
        let cg = theme.px(12.0);
        let hh = theme.px(22.0);
        let rp = theme.px(8.0);

        let name_row = rect_from(cx, top_y, cw, fh);

        let pos_y = top_y + fh + cg;
        let pos_rh = hh + rp * 1.5 + 3.0 * fh + 2.0 * (rp * 0.5);
        let pos_card = rect_from(cx, pos_y, cw, pos_rh);
        let pos_rows: [Rect; 3] = std::array::from_fn(|i| {
            rect_from(cx + rp, pos_y + hh + rp + i as f32 * (fh + rp * 0.5), cw - rp * 2.0, fh)
        });

        let sz_y = pos_y + pos_rh + cg;
        let sz_card = rect_from(cx, sz_y, cw, pos_rh);
        let sz_rows: [Rect; 3] = std::array::from_fn(|i| {
            rect_from(cx + rp, sz_y + hh + rp + i as f32 * (fh + rp * 0.5), cw - rp * 2.0, fh)
        });

        let rot_y = sz_y + pos_rh + cg;
        let rot_card = rect_from(cx, rot_y, cw, pos_rh);
        let rot_rows: [Rect; 3] = std::array::from_fn(|i| {
            rect_from(cx + rp, rot_y + hh + rp + i as f32 * (fh + rp * 0.5), cw - rp * 2.0, fh)
        });

        let col_y = rot_y + pos_rh + cg;
        let col_h = hh + rp * 1.5 + fh;
        let col_card = rect_from(cx, col_y, cw, col_h);
        let col_row = rect_from(cx + rp, col_y + hh + rp, cw - rp * 2.0, fh);

        let voxelize_y = col_y + col_h + cg;
        let btn_voxelize = rect_from(cx, voxelize_y, cw, theme.px(30.0));

        let script_y = voxelize_y + theme.px(30.0) + cg;
        let btn_script = rect_from(cx, script_y, cw, theme.px(30.0));

        let act_y = script_y + theme.px(30.0) + cg;
        let bw = (cw - theme.px(8.0)) * 0.5;
        let btn_dup = rect_from(cx, act_y, bw, theme.px(30.0));
        let btn_del = rect_from(cx + bw + theme.px(8.0), act_y, bw, theme.px(30.0));

        InspectorCards {
            name_row,
            pos_card, pos_rows,
            sz_card, sz_rows,
            rot_card, rot_rows,
            col_card, col_row,
            btn_voxelize,
            btn_script,
            btn_dup, btn_del,
            bottom_y: act_y + theme.px(30.0),
        }
    }
}

pub(crate) struct InspectorCards {
    pub name_row: Rect,
    pub pos_card: Rect, pub pos_rows: [Rect; 3],
    pub sz_card: Rect, pub sz_rows: [Rect; 3],
    pub rot_card: Rect, pub rot_rows: [Rect; 3],
    pub col_card: Rect, pub col_row: Rect,
    pub btn_voxelize: Rect,
    pub btn_script: Rect,
    pub btn_dup: Rect, pub btn_del: Rect,
    pub bottom_y: f32,
}
