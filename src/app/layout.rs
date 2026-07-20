use agate::theme::Theme;
use agate::{rect_from, Flow, Region};

pub(crate) use agate::{in_rect, Rect};

/// Slim strip: view modes (left), ribbon tab selector (center), save/undo (right).
pub(crate) const TITLEBAR_H: f32 = 40.0;
/// The Roblox-style contextual tool row under the title bar; its contents
/// swap with the active `RibbonTab`.
pub(crate) const RIBBON_H: f32 = 64.0;
pub(crate) const STATUSBAR_H: f32 = 28.0;
pub(crate) const NAVIGATOR_W: f32 = 256.0;
pub(crate) const INSPECTOR_W: f32 = 300.0;
pub(crate) const TAB_BAR_H: f32 = 28.0;
pub(crate) const ROW_H: f32 = 30.0;
pub(crate) const PAD: f32 = 12.0;
pub(crate) const PANEL_GAP: f32 = 0.0;
/// Inset of the whole UI from the window edges. Zero so the outer panels
/// (title bar, navigator, inspector, status bar) sit flush against the screen;
/// `PANEL_GAP` still separates panels from each other.
pub(crate) const WINDOW_MARGIN: f32 = 0.0;

pub(crate) struct Layout {
    /// The whole window, for full-screen overlays like confirm dialogs.
    pub window: Rect,
    pub titlebar: Rect,
    pub ribbon: Rect,
    pub navigator: Rect,
    pub inspector: Rect,
    pub statusbar: Rect,
    pub center: Rect,
    /// Document-tab strip above the viewport ("Scene" + open file); the
    /// grab-pose/anim-sim/preview sub-editors reuse it as their header bar.
    pub editor_tab: Rect,
    pub editor_body: Rect,
    pub seg: [Rect; 4],
    pub seg_pill: Rect,
    pub ribbon_tabs: [Rect; 3],
    pub btn_save: Rect,
    pub btn_save_scene: Rect,
    pub btn_undo: Rect,
    pub btn_redo: Rect,
}

impl Layout {
    pub fn new(win_w: f32, win_h: f32, theme: &Theme) -> Self {
        let tb_h = theme.px(TITLEBAR_H);
        let rb_h = theme.px(RIBBON_H);
        let sb_h = theme.px(STATUSBAR_H);
        let nav_w = theme.px(NAVIGATOR_W);
        let ins_w = theme.px(INSPECTOR_W);
        let tab_h = theme.px(TAB_BAR_H);
        let margin = theme.px(WINDOW_MARGIN);
        let gap = theme.px(PANEL_GAP);

        let window = Region::new(rect_from(0.0, 0.0, win_w, win_h)).inset(margin);
        let (titlebar, rest) = window.split_top_gap(tb_h, gap);
        let (ribbon, rest) = rest.split_top_gap(rb_h, gap);
        let (statusbar, rest) = rest.split_bottom_gap(sb_h, gap);
        let (navigator, rest) = rest.split_left_gap(nav_w, gap);
        let (inspector, center_region) = rest.split_right_gap(ins_w, gap);
        let center = center_region.rect();

        let (editor_tab, editor_body_region) = Region::new(center).split_top(tab_h);
        let editor_body = editor_body_region.rect();

        let pad = theme.px(PAD);
        let seg_h = theme.px(28.0);
        let seg_w = theme.px(96.0);
        let seg_y = titlebar[1] + (tb_h - seg_h) * 0.5;
        let mut seg_flow = Flow::row(titlebar[0] + pad, seg_y, seg_h, 0.0);
        let seg = [
            seg_flow.take(seg_w),
            seg_flow.take(seg_w),
            seg_flow.take(seg_w),
            seg_flow.take(seg_w),
        ];
        let seg_pill = rect_from(seg[0][0], seg_y, seg_w * 4.0, seg_h);

        // Ribbon tab selector, centered in the title bar like Roblox's
        // Home/Model/... strip but driving the ribbon row below.
        let rtab_w = theme.px(88.0);
        let rtabs_x0 = titlebar[0] + (titlebar[2] - rtab_w * 3.0) * 0.5;
        let mut rtab_flow = Flow::row(rtabs_x0, seg_y, seg_h, 0.0);
        let ribbon_tabs = [
            rtab_flow.take(rtab_w),
            rtab_flow.take(rtab_w),
            rtab_flow.take(rtab_w),
        ];

        let bw = theme.px(80.0);
        let btn_gap = theme.px(8.0);
        let titlebar_right = titlebar[0] + titlebar[2];
        let mut right_flow = Flow::row_from_right(titlebar_right - pad, seg_y, seg_h, btn_gap);
        let btn_save = right_flow.take(bw);
        let btn_save_scene = right_flow.take(theme.px(110.0));
        let btn_redo = right_flow.take(theme.px(64.0));
        let btn_undo = right_flow.take(theme.px(64.0));

        // seg_pill (left-anchored), ribbon tabs (centered) and the save
        // buttons (right-anchored) are three independent `Flow`s over one
        // title bar — not a single split chain — so an undersized window
        // can't be ruled out structurally; catch collisions explicitly
        // instead of letting the buttons silently draw on top of each other.
        let mut guard = agate::OverlapGuard::new();
        guard.claim("titlebar.seg_pill", seg_pill);
        guard.claim_group("titlebar.ribbon_tabs", &ribbon_tabs);
        guard.claim("titlebar.btn_save_scene", btn_save_scene);
        guard.claim("titlebar.btn_save", btn_save);
        guard.claim("titlebar.btn_redo", btn_redo);
        guard.claim("titlebar.btn_undo", btn_undo);

        Self {
            window: rect_from(0.0, 0.0, win_w, win_h),
            titlebar,
            ribbon,
            navigator,
            inspector,
            statusbar,
            center,
            editor_tab,
            editor_body,
            seg,
            seg_pill,
            ribbon_tabs,
            btn_save,
            btn_save_scene,
            btn_undo,
            btn_redo,
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

    const RIBBON_CHIP_W: f32 = 110.0;
    const RIBBON_CHIP_H: f32 = 38.0;
    const RIBBON_CHIP_GAP: f32 = 10.0;
    /// Y of the ribbon's control row (buttons/chips); the strip below it
    /// holds the small group captions.
    fn ribbon_row_y(&self, theme: &Theme) -> f32 {
        self.ribbon[1] + theme.px(8.0)
    }

    /// Clip/content area for the Insert tab's horizontally scrolling chips.
    pub fn ribbon_chip_area(&self, theme: &Theme) -> Rect {
        let pad = theme.px(PAD);
        rect_from(
            self.ribbon[0] + pad,
            self.ribbon_row_y(theme),
            self.ribbon[2] - pad * 2.0,
            theme.px(Self::RIBBON_CHIP_H),
        )
    }

    pub fn ribbon_chip_rects(&self, theme: &Theme, count: usize, scroll_x: f32) -> Vec<Rect> {
        let area = self.ribbon_chip_area(theme);
        let cw = theme.px(Self::RIBBON_CHIP_W);
        let gap = theme.px(Self::RIBBON_CHIP_GAP);
        (0..count)
            .map(|i| rect_from(area[0] + i as f32 * (cw + gap) - scroll_x, area[1], cw, area[3]))
            .collect()
    }

    pub fn ribbon_chip_max_scroll(&self, theme: &Theme, count: usize) -> f32 {
        let area = self.ribbon_chip_area(theme);
        let cw = theme.px(Self::RIBBON_CHIP_W);
        let gap = theme.px(Self::RIBBON_CHIP_GAP);
        let content_w = count as f32 * cw + (count.max(1) - 1) as f32 * gap;
        (content_w - area[2]).max(0.0)
    }

    pub fn gizmo_rects(&self, theme: &Theme) -> [Rect; 3] {
        let size = theme.px(34.0);
        let gap = theme.px(8.0);
        let pad = theme.px(14.0);
        let x = self.inspector[0] - pad - size;
        let y0 = self.center[1] + pad;
        std::array::from_fn(|i| rect_from(x, y0 + i as f32 * (size + gap), size, size))
    }

    pub fn tool_button_rects(&self, theme: &Theme) -> [Rect; 3] {
        let w = theme.px(72.0);
        let h = theme.px(32.0);
        let gap = theme.px(6.0);
        let x0 = self.ribbon[0] + theme.px(PAD);
        let mut flow = Flow::row(x0, self.ribbon_row_y(theme), h, gap);
        [flow.take(w), flow.take(w), flow.take(w)]
    }

    pub fn mode_button_rects(&self, theme: &Theme) -> [Rect; 3] {
        let tools = self.tool_button_rects(theme);
        let w = theme.px(72.0);
        let h = theme.px(32.0);
        let gap = theme.px(6.0);
        let x0 = tools[2][0] + tools[2][2] + theme.px(28.0);
        let mut flow = Flow::row(x0, tools[0][1], h, gap);
        [flow.take(w), flow.take(w), flow.take(w)]
    }

    pub fn hand_toggle_rects(&self, theme: &Theme) -> [Rect; 2] {
        let modes = self.mode_button_rects(theme);
        let w = theme.px(36.0);
        let gap = theme.px(6.0);
        let x0 = modes[2][0] + modes[2][2] + theme.px(28.0);
        let mut flow = Flow::row(x0, modes[0][1], modes[0][3], gap);
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

        // Object actions (Voxelize, Duplicate, Delete, ...) live in the
        // ribbon's Object tab now — the inspector is pure properties.
        let bottom_y = col_flow.take(0.0)[1];

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
            bottom_y,
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
    pub bottom_y: f32,
}
