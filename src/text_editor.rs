use std::path::{Path, PathBuf};
use std::sync::Arc;

use space_soup::ui2d::{Area, Item, Shape, ShapeType, Text, Span, Font, Align, Color};

use crate::theme::{self, Theme};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Pos {
    pub row: usize,
    pub col: usize,
}

impl Pos {
    fn new(row: usize, col: usize) -> Self { Self { row, col } }
}

struct Snapshot {
    lines: Vec<String>,
    cursor: Pos,
}

pub struct TextEditor {
    pub path: Option<PathBuf>,
    lines: Vec<String>,
    cursor: Pos,
    anchor: Option<Pos>,
    scroll_row: usize,
    scroll_col: usize,
    pub dirty: bool,
    clipboard: String,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    last_geom: Option<Geom>,
}

#[derive(Clone, Copy)]
struct Geom {
    rect: (f32, f32, f32, f32),
    text_x: f32,
    top_y: f32,
    char_w: f32,
    line_h: f32,
    visible_rows: usize,
}

const TAB: &str = "  ";

impl TextEditor {
    pub fn empty() -> Self {
        Self {
            path: None,
            lines: vec![String::new()],
            cursor: Pos::new(0, 0),
            anchor: None,
            scroll_row: 0,
            scroll_col: 0,
            dirty: false,
            clipboard: String::new(),
            undo: Vec::new(),
            redo: Vec::new(),
            last_geom: None,
        }
    }

    pub fn load(path: &Path) -> std::io::Result<Self> {
        let text = std::fs::read_to_string(path)?;
        let mut ed = Self::empty();
        ed.set_text(&text);
        ed.path = Some(path.to_path_buf());
        ed.dirty = false;
        Ok(ed)
    }

    pub fn save(&mut self) -> std::io::Result<()> {
        if let Some(p) = self.path.clone() {
            std::fs::write(&p, self.text())?;
            self.dirty = false;
        }
        Ok(())
    }

    pub fn set_text(&mut self, text: &str) {
        self.lines = if text.is_empty() {
            vec![String::new()]
        } else {
            text.replace('\t', TAB).split('\n')
                .map(|l| l.trim_end_matches('\r').to_string())
                .collect()
        };
        if self.lines.is_empty() { self.lines.push(String::new()); }
        self.cursor = Pos::new(0, 0);
        self.anchor = None;
        self.scroll_row = 0;
        self.scroll_col = 0;
        self.undo.clear();
        self.redo.clear();
    }

    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    pub fn cursor_line_col(&self) -> (usize, usize) {
        (self.cursor.row + 1, self.cursor.col + 1)
    }

    pub fn line_count(&self) -> usize { self.lines.len() }

    pub fn has_selection(&self) -> bool { self.selection_range().is_some() }

    pub fn file_name(&self) -> String {
        self.path.as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "untitled".to_string())
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot { lines: self.lines.clone(), cursor: self.cursor }
    }

    fn push_undo(&mut self) {
        self.undo.push(self.snapshot());
        if self.undo.len() > 200 { self.undo.remove(0); }
        self.redo.clear();
    }

    pub fn undo(&mut self) {
        if let Some(s) = self.undo.pop() {
            self.redo.push(self.snapshot());
            self.lines = s.lines;
            self.cursor = s.cursor;
            self.anchor = None;
            self.dirty = true;
            self.ensure_visible_default();
        }
    }

    pub fn redo(&mut self) {
        if let Some(s) = self.redo.pop() {
            self.undo.push(self.snapshot());
            self.lines = s.lines;
            self.cursor = s.cursor;
            self.anchor = None;
            self.dirty = true;
            self.ensure_visible_default();
        }
    }

    fn selection_range(&self) -> Option<(Pos, Pos)> {
        let a = self.anchor?;
        let b = self.cursor;
        if a == b { return None; }
        Some(if a <= b { (a, b) } else { (b, a) })
    }

    fn clear_selection(&mut self) { self.anchor = None; }

    fn begin_selection_if_needed(&mut self, extend: bool) {
        if extend {
            if self.anchor.is_none() { self.anchor = Some(self.cursor); }
        } else {
            self.anchor = None;
        }
    }

    pub fn select_all(&mut self) {
        self.anchor = Some(Pos::new(0, 0));
        let last = self.lines.len() - 1;
        self.cursor = Pos::new(last, char_len(&self.lines[last]));
    }

    fn delete_selection(&mut self) -> bool {
        let Some((start, end)) = self.selection_range() else { return false };
        self.push_undo();
        if start.row == end.row {
            let line = &mut self.lines[start.row];
            let s = byte_idx(line, start.col);
            let e = byte_idx(line, end.col);
            line.replace_range(s..e, "");
        } else {
            let head = substr(&self.lines[start.row], 0, start.col);
            let tail = substr_from(&self.lines[end.row], end.col);
            let merged = format!("{head}{tail}");
            self.lines.splice(start.row..=end.row, std::iter::once(merged));
        }
        self.cursor = start;
        self.anchor = None;
        self.dirty = true;
        true
    }

    fn selected_text(&self) -> String {
        let Some((start, end)) = self.selection_range() else { return String::new() };
        if start.row == end.row {
            return substr(&self.lines[start.row], start.col, end.col);
        }
        let mut out = substr_from(&self.lines[start.row], start.col);
        out.push('\n');
        for r in (start.row + 1)..end.row {
            out.push_str(&self.lines[r]);
            out.push('\n');
        }
        out.push_str(&substr(&self.lines[end.row], 0, end.col));
        out
    }

    pub fn insert_str(&mut self, s: &str) {
        if s.is_empty() { return; }
        self.delete_selection();
        self.push_undo();
        let s = s.replace('\t', TAB).replace('\r', "");
        if !s.contains('\n') {
            let line = &mut self.lines[self.cursor.row];
            let b = byte_idx(line, self.cursor.col);
            line.insert_str(b, &s);
            self.cursor.col += char_len(&s);
        } else {
            let parts: Vec<&str> = s.split('\n').collect();
            let row = self.cursor.row;
            let cur = self.lines[row].clone();
            let head = substr(&cur, 0, self.cursor.col);
            let tail = substr_from(&cur, self.cursor.col);
            let mut new_lines: Vec<String> = Vec::with_capacity(parts.len());
            new_lines.push(format!("{head}{}", parts[0]));
            for p in &parts[1..parts.len() - 1] { new_lines.push(p.to_string()); }
            let last = parts[parts.len() - 1];
            let new_cursor_col = char_len(last);
            new_lines.push(format!("{last}{tail}"));
            let added = new_lines.len();
            self.lines.splice(row..=row, new_lines);
            self.cursor.row = row + added - 1;
            self.cursor.col = new_cursor_col;
        }
        self.dirty = true;
        self.ensure_visible_default();
    }

    pub fn insert_char(&mut self, ch: char) {
        let mut buf = [0u8; 4];
        self.insert_str(ch.encode_utf8(&mut buf));
    }

    pub fn newline(&mut self) {
        let indent: String = self.lines[self.cursor.row]
            .chars().take_while(|c| *c == ' ').collect();
        self.insert_str(&format!("\n{indent}"));
    }

    pub fn backspace(&mut self) {
        if self.delete_selection() { self.ensure_visible_default(); return; }
        self.push_undo();
        if self.cursor.col > 0 {
            let line = &mut self.lines[self.cursor.row];
            let prev = self.cursor.col - 1;
            let s = byte_idx(line, prev);
            let e = byte_idx(line, self.cursor.col);
            line.replace_range(s..e, "");
            self.cursor.col = prev;
        } else if self.cursor.row > 0 {
            let cur = self.lines.remove(self.cursor.row);
            let above = self.cursor.row - 1;
            let new_col = char_len(&self.lines[above]);
            self.lines[above].push_str(&cur);
            self.cursor.row = above;
            self.cursor.col = new_col;
        } else {
            self.undo.pop();
            return;
        }
        self.dirty = true;
        self.ensure_visible_default();
    }

    pub fn delete_forward(&mut self) {
        if self.delete_selection() { self.ensure_visible_default(); return; }
        self.push_undo();
        let len = char_len(&self.lines[self.cursor.row]);
        if self.cursor.col < len {
            let line = &mut self.lines[self.cursor.row];
            let s = byte_idx(line, self.cursor.col);
            let e = byte_idx(line, self.cursor.col + 1);
            line.replace_range(s..e, "");
        } else if self.cursor.row + 1 < self.lines.len() {
            let next = self.lines.remove(self.cursor.row + 1);
            self.lines[self.cursor.row].push_str(&next);
        } else {
            self.undo.pop();
            return;
        }
        self.dirty = true;
    }

    pub fn copy(&mut self) {
        let s = self.selected_text();
        if !s.is_empty() { self.clipboard = s; }
    }

    pub fn cut(&mut self) {
        let s = self.selected_text();
        if !s.is_empty() {
            self.clipboard = s;
            self.delete_selection();
            self.ensure_visible_default();
        }
    }

    pub fn paste(&mut self) {
        let s = self.clipboard.clone();
        if !s.is_empty() { self.insert_str(&s); }
    }

    pub fn move_left(&mut self, extend: bool) {
        self.begin_selection_if_needed(extend);
        if self.cursor.col > 0 {
            self.cursor.col -= 1;
        } else if self.cursor.row > 0 {
            self.cursor.row -= 1;
            self.cursor.col = char_len(&self.lines[self.cursor.row]);
        }
        if !extend { self.clear_selection(); }
        self.ensure_visible_default();
    }

    pub fn move_right(&mut self, extend: bool) {
        self.begin_selection_if_needed(extend);
        let len = char_len(&self.lines[self.cursor.row]);
        if self.cursor.col < len {
            self.cursor.col += 1;
        } else if self.cursor.row + 1 < self.lines.len() {
            self.cursor.row += 1;
            self.cursor.col = 0;
        }
        if !extend { self.clear_selection(); }
        self.ensure_visible_default();
    }

    pub fn move_up(&mut self, extend: bool) {
        self.begin_selection_if_needed(extend);
        if self.cursor.row > 0 {
            self.cursor.row -= 1;
            let len = char_len(&self.lines[self.cursor.row]);
            self.cursor.col = self.cursor.col.min(len);
        } else {
            self.cursor.col = 0;
        }
        if !extend { self.clear_selection(); }
        self.ensure_visible_default();
    }

    pub fn move_down(&mut self, extend: bool) {
        self.begin_selection_if_needed(extend);
        if self.cursor.row + 1 < self.lines.len() {
            self.cursor.row += 1;
            let len = char_len(&self.lines[self.cursor.row]);
            self.cursor.col = self.cursor.col.min(len);
        } else {
            self.cursor.col = char_len(&self.lines[self.cursor.row]);
        }
        if !extend { self.clear_selection(); }
        self.ensure_visible_default();
    }

    pub fn move_home(&mut self, extend: bool) {
        self.begin_selection_if_needed(extend);
        let line = &self.lines[self.cursor.row];
        let first_non_ws = line.chars().take_while(|c| *c == ' ').count();
        self.cursor.col = if self.cursor.col == first_non_ws { 0 } else { first_non_ws };
        if !extend { self.clear_selection(); }
        self.ensure_visible_default();
    }

    pub fn move_end(&mut self, extend: bool) {
        self.begin_selection_if_needed(extend);
        self.cursor.col = char_len(&self.lines[self.cursor.row]);
        if !extend { self.clear_selection(); }
        self.ensure_visible_default();
    }

    pub fn page(&mut self, down: bool, extend: bool) {
        let page = self.last_geom.map(|g| g.visible_rows.saturating_sub(2)).unwrap_or(20);
        self.begin_selection_if_needed(extend);
        if down {
            self.cursor.row = (self.cursor.row + page).min(self.lines.len() - 1);
        } else {
            self.cursor.row = self.cursor.row.saturating_sub(page);
        }
        let len = char_len(&self.lines[self.cursor.row]);
        self.cursor.col = self.cursor.col.min(len);
        if !extend { self.clear_selection(); }
        self.ensure_visible_default();
    }

    pub fn scroll_by(&mut self, rows: i32) {
        let max = self.lines.len().saturating_sub(1);
        let next = (self.scroll_row as i32 + rows).clamp(0, max as i32);
        self.scroll_row = next as usize;
    }

    fn ensure_visible_default(&mut self) {
        if let Some(g) = self.last_geom {
            self.ensure_visible(g.visible_rows, self.visible_cols(g));
        }
    }

    fn visible_cols(&self, g: Geom) -> usize {
        let usable = (g.rect.2 - (g.text_x - g.rect.0)).max(g.char_w);
        (usable / g.char_w).floor() as usize
    }

    fn ensure_visible(&mut self, visible_rows: usize, visible_cols: usize) {
        if visible_rows == 0 { return; }
        if self.cursor.row < self.scroll_row {
            self.scroll_row = self.cursor.row;
        } else if self.cursor.row >= self.scroll_row + visible_rows {
            self.scroll_row = self.cursor.row + 1 - visible_rows;
        }
        if visible_cols > 0 {
            if self.cursor.col < self.scroll_col {
                self.scroll_col = self.cursor.col;
            } else if self.cursor.col >= self.scroll_col + visible_cols {
                self.scroll_col = self.cursor.col + 1 - visible_cols;
            }
        }
    }

    pub fn pos_at(&self, x: f32, y: f32) -> Option<Pos> {
        let g = self.last_geom?;
        if x < g.rect.0 || x > g.rect.0 + g.rect.2 || y < g.rect.1 || y > g.rect.1 + g.rect.3 {
            return None;
        }
        let rel_row = ((y - g.top_y) / g.line_h).floor().max(0.0) as usize;
        let row = (self.scroll_row + rel_row).min(self.lines.len() - 1);
        let rel_x = (x - g.text_x).max(0.0);
        let col_f = (rel_x / g.char_w).round();
        let col = (self.scroll_col + col_f as usize).min(char_len(&self.lines[row]));
        Some(Pos::new(row, col))
    }

    pub fn click(&mut self, x: f32, y: f32, extend: bool) {
        if let Some(p) = self.pos_at(x, y) {
            self.begin_selection_if_needed(extend);
            self.cursor = p;
            if !extend { self.clear_selection(); }
            self.ensure_visible_default();
        }
    }

    pub fn drag_to(&mut self, x: f32, y: f32) {
        if let Some(p) = self.pos_at(x, y) {
            if self.anchor.is_none() { self.anchor = Some(self.cursor); }
            self.cursor = p;
            self.ensure_visible_default();
        }
    }

    pub fn build_items(
        &mut self,
        rect: (f32, f32, f32, f32),
        theme: &Theme,
        font: &Arc<Font>,
        show_caret: bool,
        focused: bool,
        items: &mut Vec<(Area, Item)>,
    ) {
        let (rx, ry, rw, rh) = rect;
        let font_px = theme.font(theme::PT_EDITOR);
        let line_h = theme.px(theme::PT_EDITOR_LINE);

        let char_w = font.metrics('0', font_px).advance_width.max(theme.px(6.0));

        let digits = digit_count(self.lines.len());
        let gutter_w = char_w * digits as f32 + theme.px(20.0);
        let text_x = rx + gutter_w + theme.px(8.0);
        let top_y = ry + theme.px(6.0);
        let visible_rows = (((rh - theme.px(12.0)) / line_h).floor() as usize).max(1);

        let geom = Geom { rect, text_x, top_y, char_w, line_h, visible_rows };
        self.last_geom = Some(geom);
        self.ensure_visible(visible_rows, self.visible_cols(geom));

        items.push(rect_item((rx, ry), (rw, rh), theme::EDITOR_BG));
        items.push(rect_item((rx, ry), (gutter_w, rh), theme::GUTTER_BG));
        items.push(rect_item((rx + gutter_w - theme.px(1.0), ry), (theme.px(1.0), rh), theme::HAIRLINE));

        let sel = self.selection_range();
        let last_row = (self.scroll_row + visible_rows).min(self.lines.len());

        for (vi, row) in (self.scroll_row..last_row).enumerate() {
            let ly = top_y + vi as f32 * line_h;

            if sel.is_none() && row == self.cursor.row {
                items.push(rect_item((rx + gutter_w, ly), (rw - gutter_w, line_h), theme::CURRENT_LINE));
            }

            if let Some((s, e)) = sel {
                if row >= s.row && row <= e.row {
                    let start_col = if row == s.row { s.col } else { 0 };
                    let end_col = if row == e.row { e.col } else { char_len(&self.lines[row]) + 1 };
                    let sx = text_x + (start_col.saturating_sub(self.scroll_col)) as f32 * char_w;
                    let ex = text_x + (end_col.saturating_sub(self.scroll_col)) as f32 * char_w;
                    let w = (ex - sx).max(char_w * 0.35);
                    items.push(rect_item((sx, ly), (w, line_h), theme::SELECTION_BG));
                }
            }

            let num = format!("{:>width$}", row + 1, width = digits);
            let num_color = if row == self.cursor.row { theme::LINE_NUMBER_CUR } else { theme::LINE_NUMBER };
            let num_span = Span::new(num, font.clone(), font_px, num_color).with_align(Align::Left);
            items.push((
                Area { offset: (rx + theme.px(8.0), ly), bounds: Some((rx, ry, rx + gutter_w, ry + rh)) },
                Item::Text(Text::new(vec![num_span], gutter_w)),
            ));

            let spans = highlight_json_line(&self.lines[row], self.scroll_col, font, font_px);
            if !spans.is_empty() {
                items.push((
                    Area { offset: (text_x, ly), bounds: Some((rx + gutter_w, ry, rx + rw, ry + rh)) },
                    Item::Text(Text::new(spans, 1.0e6)),
                ));
            }

            if show_caret && focused && row == self.cursor.row && sel.is_none() {
                let cx = text_x + (self.cursor.col.saturating_sub(self.scroll_col)) as f32 * char_w;
                items.push(rect_item((cx, ly + theme.px(1.0)), (theme.px(1.5), line_h - theme.px(2.0)), theme::CARET));
            }
        }

        if self.lines.len() > visible_rows {
            let track_h = rh - theme.px(8.0);
            let thumb_h = (track_h * visible_rows as f32 / self.lines.len() as f32).max(theme.px(24.0));
            let max_scroll = (self.lines.len() - visible_rows) as f32;
            let t = if max_scroll > 0.0 { self.scroll_row as f32 / max_scroll } else { 0.0 };
            let thumb_y = ry + theme.px(4.0) + t * (track_h - thumb_h);
            items.push((
                Area { offset: (rx + rw - theme.px(8.0), thumb_y), bounds: None },
                Item::Shape(Shape {
                    shape: ShapeType::RoundedRectangle(0.0, (theme.px(4.0), thumb_h), 0.0, theme.px(2.0)),
                    color: theme::SCROLLBAR,
                }),
            ));
        }
    }
}

fn rect_item(offset: (f32, f32), size: (f32, f32), color: Color) -> (Area, Item) {
    (
        Area { offset, bounds: None },
        Item::Shape(Shape { shape: ShapeType::Rectangle(0.0, size, 0.0), color }),
    )
}

fn char_len(s: &str) -> usize { s.chars().count() }

fn byte_idx(s: &str, col: usize) -> usize {
    s.char_indices().nth(col).map(|(i, _)| i).unwrap_or(s.len())
}

fn substr(s: &str, from: usize, to: usize) -> String {
    s.chars().skip(from).take(to.saturating_sub(from)).collect()
}

fn substr_from(s: &str, from: usize) -> String {
    s.chars().skip(from).collect()
}

fn digit_count(n: usize) -> usize {
    let mut d = 1;
    let mut v = n;
    while v >= 10 { v /= 10; d += 1; }
    d.max(2)
}

fn highlight_json_line(line: &str, scroll_col: usize, font: &Arc<Font>, font_px: f32) -> Vec<Span> {
    if line.is_empty() { return Vec::new(); }
    let chars: Vec<char> = line.chars().collect();
    let toks = tokenize(&chars);

    let mut spans: Vec<Span> = Vec::new();
    let mut char_cursor = 0usize;

    for (i, t) in toks.iter().enumerate() {
        let text: String = chars[t.start..t.end].iter().collect();
        let tok_chars = t.end - t.start;

        let visible_text = if char_cursor + tok_chars <= scroll_col {
            char_cursor += tok_chars;
            continue;
        } else if char_cursor < scroll_col {
            let skip = scroll_col - char_cursor;
            text.chars().skip(skip).collect::<String>()
        } else {
            text.clone()
        };
        char_cursor += tok_chars;

        if visible_text.is_empty() { continue; }

        let color = match t.kind {
            Kind::Str => {
                if next_is_colon(&toks, i, &chars) { theme::SYN_KEY } else { theme::SYN_STRING }
            }
            Kind::Number => theme::SYN_NUMBER,
            Kind::Literal => theme::SYN_KEYWORD,
            Kind::Punct => theme::SYN_PUNCT,
            Kind::Comment => theme::SYN_COMMENT,
            Kind::Ws | Kind::Other => theme::SYN_PLAIN,
        };
        spans.push(Span::new(visible_text, font.clone(), font_px, color).with_align(Align::Left));
    }
    spans
}

fn next_is_colon(toks: &[Tok], from: usize, chars: &[char]) -> bool {
    for t in &toks[from + 1..] {
        match t.kind {
            Kind::Ws => continue,
            Kind::Punct => return chars.get(t.start) == Some(&':'),
            _ => return false,
        }
    }
    false
}

#[derive(Clone, Copy, PartialEq)]
enum Kind { Str, Number, Literal, Punct, Ws, Comment, Other }

struct Tok { start: usize, end: usize, kind: Kind }

fn tokenize(chars: &[char]) -> Vec<Tok> {
    let mut out = Vec::new();
    let mut i = 0;
    let n = chars.len();
    while i < n {
        let c = chars[i];
        if c == ' ' || c == '\t' {
            let start = i;
            while i < n && (chars[i] == ' ' || chars[i] == '\t') { i += 1; }
            out.push(Tok { start, end: i, kind: Kind::Ws });
        } else if c == '"' {
            let start = i;
            i += 1;
            while i < n {
                if chars[i] == '\\' { i += 2; continue; }
                if chars[i] == '"' { i += 1; break; }
                i += 1;
            }
            out.push(Tok { start, end: i.min(n), kind: Kind::Str });
        } else if c == '/' && i + 1 < n && chars[i + 1] == '/' {
            out.push(Tok { start: i, end: n, kind: Kind::Comment });
            i = n;
        } else if c.is_ascii_digit() || (c == '-' && i + 1 < n && chars[i + 1].is_ascii_digit()) {
            let start = i;
            i += 1;
            while i < n && (chars[i].is_ascii_digit() || matches!(chars[i], '.' | 'e' | 'E' | '+' | '-')) {
                i += 1;
            }
            out.push(Tok { start, end: i, kind: Kind::Number });
        } else if c.is_ascii_alphabetic() {
            let start = i;
            while i < n && chars[i].is_ascii_alphabetic() { i += 1; }
            let word: String = chars[start..i].iter().collect();
            let kind = if matches!(word.as_str(), "true" | "false" | "null") { Kind::Literal } else { Kind::Other };
            out.push(Tok { start, end: i, kind });
        } else if matches!(c, '{' | '}' | '[' | ']' | ':' | ',') {
            out.push(Tok { start: i, end: i + 1, kind: Kind::Punct });
            i += 1;
        } else {
            out.push(Tok { start: i, end: i + 1, kind: Kind::Other });
            i += 1;
        }
    }
    out
}