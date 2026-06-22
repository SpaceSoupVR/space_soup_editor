use space_soup::ui2d::Color;

const fn c(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color(r, g, b, a)
}

pub const TITLEBAR_BG: Color = c(0x2A, 0x2D, 0x36, 255);
pub const TOOLBAR_BG: Color = c(0x24, 0x27, 0x2F, 255);
pub const SIDEBAR_BG: Color = c(0x1A, 0x1C, 0x23, 255);
pub const SIDEBAR_SEL: Color = c(0x0B, 0x84, 0xFF, 235);
pub const SIDEBAR_SEL_INACTIVE: Color = c(0x35, 0x38, 0x42, 255);
pub const STATUSBAR_BG: Color = c(0x20, 0x22, 0x2A, 255);
pub const SEPARATOR: Color = c(0x00, 0x00, 0x00, 150);
pub const HAIRLINE: Color = c(0x3C, 0x40, 0x4A, 255);

pub const CARD_BG: Color = c(0x23, 0x26, 0x2E, 255);
pub const CARD_BG_RAISED: Color = c(0x28, 0x2B, 0x34, 255);
pub const CARD_BORDER: Color = c(0x3A, 0x3E, 0x49, 255);
pub const FIELD_BG: Color = c(0x1C, 0x1E, 0x25, 255);
pub const FIELD_BORDER: Color = c(0x40, 0x44, 0x50, 255);
pub const FIELD_BORDER_FOCUS: Color = c(0x0B, 0x84, 0xFF, 255);

pub const EDITOR_BG: Color = c(0x29, 0x2A, 0x30, 255);
pub const GUTTER_BG: Color = c(0x24, 0x25, 0x2A, 255);
pub const CURRENT_LINE: Color = c(0xFF, 0xFF, 0xFF, 14);
pub const SELECTION_BG: Color = c(0x3F, 0x5A, 0x8C, 235);
pub const CARET: Color = c(0xFF, 0xFF, 0xFF, 255);
pub const LINE_NUMBER: Color = c(0x74, 0x74, 0x78, 255);
pub const LINE_NUMBER_CUR: Color = c(0xC8, 0xC8, 0xCC, 255);
pub const SCROLLBAR: Color = c(0x8E, 0x8E, 0x93, 110);

pub const TEXT_PRIMARY: Color = c(0xE6, 0xE7, 0xEA, 255);
pub const TEXT_SECONDARY: Color = c(0x8F, 0x95, 0xA1, 255);
pub const TEXT_ON_ACCENT: Color = c(0xFF, 0xFF, 0xFF, 255);

pub const ACCENT: Color = c(0x0B, 0x84, 0xFF, 255);
pub const ACCENT_HI: Color = c(0x6F, 0xB6, 0xFF, 255);
pub const ACCENT_SOFT: Color = c(0x0B, 0x84, 0xFF, 40);
pub const CONTROL_BG: Color = c(0x32, 0x35, 0x3F, 255);
pub const CONTROL_BG_HOVER: Color = c(0x3D, 0x40, 0x4B, 255);
pub const CONTROL_BORDER: Color = c(0x4A, 0x4E, 0x5A, 255);
pub const DIRTY_DOT: Color = c(0xE8, 0xC1, 0x4A, 255);
pub const SUCCESS: Color = c(0x32, 0xD7, 0x4B, 255);
pub const WARNING: Color = c(0xF4, 0xC2, 0x3D, 255);
pub const DANGER: Color = c(0xFF, 0x5F, 0x57, 255);

pub const SYN_PLAIN: Color = c(0xDF, 0xDF, 0xE0, 255);
pub const SYN_STRING: Color = c(0xFC, 0x6A, 0x5D, 255);
pub const SYN_NUMBER: Color = c(0xD0, 0xBF, 0x69, 255);
pub const SYN_KEYWORD: Color = c(0xFC, 0x5F, 0xA3, 255);
pub const SYN_KEY: Color = c(0x9E, 0xF1, 0xDD, 255);
pub const SYN_PUNCT: Color = c(0x9A, 0x9A, 0xA0, 255);
pub const SYN_COMMENT: Color = c(0x6C, 0x79, 0x86, 255);

pub const PT_EDITOR: f32 = 12.0;
pub const PT_EDITOR_LINE: f32 = 17.0;
pub const PT_UI: f32 = 13.0;
pub const PT_UI_SMALL: f32 = 11.0;
pub const PT_TOOLBAR: f32 = 13.0;
pub const PT_PANEL_TITLE: f32 = 11.0;

pub const TOOLBAR_H: f32 = 52.0;
pub const STATUSBAR_H: f32 = 28.0;
pub const TAB_BAR_H: f32 = 28.0;
pub const NAVIGATOR_W: f32 = 248.0;
pub const INSPECTOR_W: f32 = 300.0;
pub const ROW_H: f32 = 26.0;
pub const PAD: f32 = 12.0;
pub const CORNER: f32 = 8.0;
pub const CARD_CORNER: f32 = 10.0;
pub const FIELD_H: f32 = 24.0;
pub const CARD_GAP: f32 = 10.0;

#[derive(Clone, Copy)]
pub struct Theme {
    pub scale: f32,
}

impl Theme {
    pub fn new(scale: f32) -> Self {
        Self { scale: scale.max(0.5) }
    }

    #[inline]
    pub fn px(&self, points: f32) -> f32 {
        points * self.scale
    }

    #[inline]
    pub fn font(&self, points: f32) -> f32 {
        points * self.scale
    }
}