use agate::theme::Theme;
use agate::{rect_from, Flow, Region};

pub(crate) use agate::{in_rect, Rect};

pub(crate) const TOOLBAR_H: f32 = 56.0;
pub(crate) const STATUSBAR_H: f32 = 28.0;
pub(crate) const NAVIGATOR_W: f32 = 256.0;
pub(crate) const INSPECTOR_W: f32 = 300.0;
pub(crate) const TAB_BAR_H: f32 = 28.0;
pub(crate) const ROW_H: f32 = 30.0;
pub(crate) const PAD: f32 = 12.0;
pub(crate) const PANEL_GAP: f32 = 12.0;
pub(crate) const WINDOW_MARGIN: f32 = 16.0;

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

        let window = Region::new(rect_from(0.0, 0.0, win_w, win_h)).inset(margin);
        let (toolbar, rest) = window.split_top_gap(tb_h, gap);
        let (statusbar, rest) = rest.split_bottom_gap(sb_h, gap);
        let (navigator, rest) = rest.split_left_gap(nav_w, gap);
        let (inspector, center_region) = rest.split_right_gap(ins_w, gap);
        let center = center_region.rect();

        let (editor_tab, editor_body_region) = Region::new(center).split_top(tab_h);
        let editor_body = editor_body_region.rect();

        let pad = theme.px(PAD);
        let seg_h = theme.px(30.0);
        let seg_w = theme.px(100.0);
        let seg_y = toolbar[1] + (tb_h - seg_h) * 0.5;
        let mut seg_flow = Flow::row(toolbar[0] + pad, seg_y, seg_h, 0.0);
        let seg = [
            seg_flow.take(seg_w),
            seg_flow.take(seg_w),
            seg_flow.take(seg_w),
            seg_flow.take(seg_w),
        ];
        let seg_pill = rect_from(seg[0][0], seg_y, seg_w * 4.0, seg_h);

        let bw = theme.px(80.0);
        let btn_gap = theme.px(8.0);
        let toolbar_right = toolbar[0] + toolbar[2];
        let mut right_flow = Flow::row_from_right(toolbar_right - pad, seg_y, seg_h, btn_gap);
        let btn_save = right_flow.take(bw);
        let btn_editor = right_flow.take(bw);
        let btn_save_scene = right_flow.take(theme.px(110.0));

        // seg_pill (left-anchored) and the save/editor/save-scene buttons
        // (right-anchored) are two independent `Flow`s over one toolbar —
        // not a single split chain — so an undersized toolbar can't be
        // ruled out structurally; catch it explicitly instead of letting
        // the buttons silently draw on top of each other.
        let mut guard = agate::OverlapGuard::new();
        guard.claim("toolbar.seg_pill", seg_pill);
        guard.claim("toolbar.btn_save_scene", btn_save_scene);
        guard.claim("toolbar.btn_editor", btn_editor);
        guard.claim("toolbar.btn_save", btn_save);

        Self {
            toolbar,
            navigator,
            inspector,
            statusbar,
            center,
            editor_tab,
            editor_body,
            seg,
            seg_pill,
            btn_editor,
            btn_save,
            btn_save_scene,
        }
    }

    pub fn grab_pose_viewport(&self) -> Rect {
        rect_from(
            self.navigator[0],
            self.center[1],
            self.center[0] + self.center[2] - self.navigator[0],
            self.center[3],
        )
    }

    pub const MODEL_TRAY_H: f32 = 120.0;
    const MODEL_LIST_TOP_PAD: f32 = 26.0;
    const MODEL_CHIP_W: f32 = 110.0;
    const MODEL_CHIP_H: f32 = 40.0;
    const MODEL_CHIP_GAP: f32 = 12.0;
    const MODEL_PAD_X: f32 = 12.0;

    pub fn model_tray(&self, theme: &Theme) -> Rect {
        let gap = theme.px(PANEL_GAP);
        let bar_h = theme.px(Self::MODEL_TRAY_H);
        let (tray, _) = Region::new(self.center).split_bottom_gap(bar_h, gap);
        tray
    }

    pub fn model_list_area(&self, theme: &Theme) -> Rect {
        let tray = self.model_tray(theme);
        let top_pad = theme.px(Self::MODEL_LIST_TOP_PAD);
        let bottom_pad = theme.px(8.0);
        let (_, rest) = Region::new(tray).split_top(top_pad);
        let (_, list) = rest.split_bottom(bottom_pad);
        list.rect()
    }

    pub fn model_columns(&self, theme: &Theme) -> usize {
        let tray = self.model_tray(theme);
        let mw = theme.px(Self::MODEL_CHIP_W);
        let gap = theme.px(Self::MODEL_CHIP_GAP);
        let pad_x = theme.px(Self::MODEL_PAD_X);
        let usable = (tray[2] - pad_x * 2.0 + gap).max(mw);
        ((usable / (mw + gap)).floor() as usize).max(1)
    }

    pub fn model_rects(&self, theme: &Theme, count: usize, scroll_y: f32) -> Vec<Rect> {
        let tray = self.model_tray(theme);
        let list = self.model_list_area(theme);
        let mw = theme.px(Self::MODEL_CHIP_W);
        let mh = theme.px(Self::MODEL_CHIP_H);
        let gap = theme.px(Self::MODEL_CHIP_GAP);
        let pad_x = theme.px(Self::MODEL_PAD_X);
        let cols = self.model_columns(theme);
        (0..count)
            .map(|i| {
                let col = i % cols;
                let row = i / cols;
                let x = tray[0] + pad_x + col as f32 * (mw + gap);
                let y = list[1] + row as f32 * (mh + gap) - scroll_y;
                rect_from(x, y, mw, mh)
            })
            .collect()
    }

    pub fn model_content_height(&self, theme: &Theme, count: usize) -> f32 {
        let cols = self.model_columns(theme);
        let rows = count.max(1).div_ceil(cols) as f32;
        let mh = theme.px(Self::MODEL_CHIP_H);
        let gap = theme.px(Self::MODEL_CHIP_GAP);
        rows * mh + (rows - 1.0).max(0.0) * gap
    }

    pub fn model_max_scroll(&self, theme: &Theme, count: usize) -> f32 {
        let list = self.model_list_area(theme);
        (self.model_content_height(theme, count) - list[3]).max(0.0)
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
        let mut flow = Flow::row(x0, y, h, gap);
        [flow.take(w), flow.take(w), flow.take(w)]
    }

    pub fn tool_button_rects(&self, theme: &Theme) -> [Rect; 3] {
        let w = theme.px(64.0);
        let h = theme.px(32.0);
        let gap = theme.px(6.0);
        let pad = theme.px(14.0);
        let row_gap = theme.px(8.0);
        let x0 = self.navigator[0] + self.navigator[2] + theme.px(PANEL_GAP) + pad;
        let y = self.center[1] + pad + h + row_gap;
        let mut flow = Flow::row(x0, y, h, gap);
        [flow.take(w), flow.take(w), flow.take(w)]
    }

    pub fn hand_toggle_rects(&self, theme: &Theme) -> [Rect; 2] {
        let tools = self.tool_button_rects(theme);
        let w = theme.px(32.0);
        let h = tools[2][3];
        let gap = theme.px(6.0);
        let x0 = tools[2][0] + tools[2][2] + theme.px(16.0);
        let mut flow = Flow::row(x0, tools[2][1], h, gap);
        [flow.take(w), flow.take(w)]
    }

    #[allow(clippy::too_many_arguments)]
    pub fn inspector_cards(
        &self,
        theme: &Theme,
        top_y: f32,
        has_light: bool,
        has_sound: bool,
    ) -> InspectorCards {
        let ix = self.inspector[0];
        let iw = self.inspector[2];
        let pad = theme.px(PAD);
        let cx = ix + pad;
        let cw = iw - pad * 2.0;
        let fh = theme.px(26.0);
        let cg = theme.px(12.0);
        let hh = theme.px(22.0);
        let rp = theme.px(8.0);

        let card_h = |rows: f32| hh + rp * 1.5 + rows * fh + (rows - 1.0).max(0.0) * (rp * 0.5);
        let rows_at = |y: f32, n: usize, cx: f32, cw: f32| -> Vec<Rect> {
            let mut flow = Flow::column(cx + rp, y + hh + rp, cw - rp * 2.0, rp * 0.5);
            (0..n).map(|_| flow.take(fh)).collect()
        };

        let mut guard = agate::OverlapGuard::new();
        let mut col_flow = Flow::column(cx, top_y, cw, cg);

        let name_row = col_flow.take(fh);
        guard.claim("inspector.name_row", name_row);

        let pos_rh = card_h(3.0);
        let pos_card = guard.claim("inspector.pos_card", col_flow.take(pos_rh));
        let pos_rows: [Rect; 3] = std::array::from_fn(|i| rows_at(pos_card[1], 3, cx, cw)[i]);

        // SIZE / LIGHT / SOUND share this slot — exactly one is drawn per
        // object, depending on what it has attached.
        let light_h = card_h(5.0); // kind, intensity, range, cone, color swatch
        let sound_h = card_h(7.0); // clip, volume, pitch, min/max dist, toggles, cone
        let active_idx = if has_light {
            1
        } else if has_sound {
            2
        } else {
            0
        };
        let active_rect = col_flow.take_variant(&[pos_rh, light_h, sound_h], active_idx);
        guard.claim("inspector.active_card", active_rect);
        let sz_y = active_rect[1];
        let sz_card = rect_from(cx, sz_y, cw, pos_rh);
        let sz_rows: [Rect; 3] = std::array::from_fn(|i| rows_at(sz_y, 3, cx, cw)[i]);
        let light_card = rect_from(cx, sz_y, cw, light_h);
        let light_rows: [Rect; 5] = std::array::from_fn(|i| rows_at(sz_y, 5, cx, cw)[i]);
        let sound_card = rect_from(cx, sz_y, cw, sound_h);
        let sound_rows: [Rect; 7] = std::array::from_fn(|i| rows_at(sz_y, 7, cx, cw)[i]);

        let rot_card = guard.claim("inspector.rot_card", col_flow.take(pos_rh));
        let rot_rows: [Rect; 3] = std::array::from_fn(|i| rows_at(rot_card[1], 3, cx, cw)[i]);

        // Cuboid color is meaningless for light/sound markers (they draw no
        // solid body) — the light card has its own color swatch instead.
        let has_col_card = !has_light && !has_sound;
        let col_h = hh + rp * 1.5 + fh;
        let col_card = if has_col_card {
            guard.claim("inspector.col_card", col_flow.take(col_h))
        } else {
            rect_from(cx, rot_card[1] + pos_rh + cg, cw, col_h)
        };
        let col_row = rect_from(cx + rp, col_card[1] + hh + rp, cw - rp * 2.0, fh);

        let btn_h = theme.px(30.0);
        let btn_voxelize = guard.claim("inspector.btn_voxelize", col_flow.take(btn_h));
        let btn_script = guard.claim("inspector.btn_script", col_flow.take(btn_h));
        let btn_grab_pose = guard.claim("inspector.btn_grab_pose", col_flow.take(btn_h));
        let btn_anim_sim = guard.claim("inspector.btn_anim_sim", col_flow.take(btn_h));
        let btn_preview = guard.claim("inspector.btn_preview", col_flow.take(btn_h));

        let act_y = btn_preview[1] + btn_h + cg;
        let bw = (cw - theme.px(8.0)) * 0.5;
        let mut act_flow = Flow::row(cx, act_y, btn_h, theme.px(8.0));
        let btn_dup = guard.claim("inspector.btn_dup", act_flow.take(bw));
        let btn_del = guard.claim("inspector.btn_del", act_flow.take(bw));

        InspectorCards {
            name_row,
            pos_card,
            pos_rows,
            sz_card,
            sz_rows,
            rot_card,
            rot_rows,
            col_card,
            col_row,
            light_card,
            light_rows,
            sound_card,
            sound_rows,
            btn_voxelize,
            btn_script,
            btn_grab_pose,
            btn_anim_sim,
            btn_preview,
            btn_dup,
            btn_del,
            bottom_y: act_y + btn_h,
        }
    }

    /// The anim-sim editor reuses the same full-width 3D viewport as the
    /// grab pose editor (navigator through center, inspector stays a panel).
    pub fn anim_sim_viewport(&self) -> Rect {
        self.grab_pose_viewport()
    }
}

pub(crate) struct InspectorCards {
    pub name_row: Rect,
    pub pos_card: Rect,
    pub pos_rows: [Rect; 3],
    pub sz_card: Rect,
    pub sz_rows: [Rect; 3],
    pub rot_card: Rect,
    pub rot_rows: [Rect; 3],
    pub col_card: Rect,
    pub col_row: Rect,
    pub light_card: Rect,
    pub light_rows: [Rect; 5],
    pub sound_card: Rect,
    pub sound_rows: [Rect; 7],
    pub btn_voxelize: Rect,
    pub btn_script: Rect,
    pub btn_grab_pose: Rect,
    pub btn_anim_sim: Rect,
    pub btn_preview: Rect,
    pub btn_dup: Rect,
    pub btn_del: Rect,
    pub bottom_y: f32,
}
