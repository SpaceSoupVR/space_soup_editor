mod network;
mod text_panels;
mod scene_3d;

use std::sync::Arc;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use winit::{
    application::ApplicationHandler,
    event::{WindowEvent, ElementState, MouseButton, MouseScrollDelta, TouchPhase, KeyEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{Key, NamedKey, ModifiersState},
    window::{Window, WindowId},
};
use wgpu::*;
use glam::{Vec3, Quat};

use space_soup::renderer::{Renderer, Camera, Cuboid, Color3, GltfMesh, MeshInstance};
use space_soup::renderer::mesh_pipeline::ModelUniform;
use space_soup::ui2d::{Overlay, Font, Color};
use space_soup_engine::{GameRuntime, InputFrame, PlayerRig, LocomotionInput};

use agate::{Ui, UiInput, AMouseButton, TextEditor, Theme, WidgetId};
use agate::theme as t;

use network::SharedPacket;

const OBJECT_HALF_SIZE: f32 = 0.15;
const CAMERA_FOV_Y_DEG: f32 = 60.0;

const PALETTE: [Color3; 5] = [
    Color3(90,  200, 120, 255),
    Color3(220, 200, 80,  255),
    Color3(220, 90,  90,  255),
    Color3(90,  140, 220, 255),
    Color3(190, 130, 220, 255),
];

fn game_dir() -> PathBuf { PathBuf::from("../game") }

const TOOLBAR_H:   f32 = 52.0;
const STATUSBAR_H: f32 = 28.0;
const NAVIGATOR_W: f32 = 248.0;
const INSPECTOR_W: f32 = 300.0;
const TAB_BAR_H:   f32 = 28.0;
const ROW_H:       f32 = 26.0;
const PAD:         f32 = 12.0;

type Rect = [f32; 4];

fn rect_from(x: f32, y: f32, w: f32, h: f32) -> Rect { [x, y, w, h] }

struct Layout {
    toolbar:      Rect,
    navigator:    Rect,
    inspector:    Rect,
    statusbar:    Rect,
    center:       Rect,
    editor_tab:   Rect,
    editor_body:  Rect,
    seg:          [Rect; 3],
    btn_editor:   Rect,
    btn_save:     Rect,
}

impl Layout {
    fn new(win_w: f32, win_h: f32, theme: &Theme) -> Self {
        let tb_h  = theme.px(TOOLBAR_H);
        let sb_h  = theme.px(STATUSBAR_H);
        let nav_w = theme.px(NAVIGATOR_W);
        let ins_w = theme.px(INSPECTOR_W);
        let tab_h = theme.px(TAB_BAR_H);

        let toolbar     = rect_from(0.0, 0.0, win_w, tb_h);
        let body_y      = tb_h;
        let body_h      = (win_h - tb_h - sb_h).max(0.0);
        let navigator   = rect_from(0.0, body_y, nav_w, body_h);
        let inspector   = rect_from(win_w - ins_w, body_y, ins_w, body_h);
        let center      = rect_from(nav_w, body_y, (win_w - nav_w - ins_w).max(0.0), body_h);
        let statusbar   = rect_from(0.0, win_h - sb_h, win_w, sb_h);
        let editor_tab  = rect_from(center[0], center[1], center[2], tab_h);
        let editor_body = rect_from(center[0], center[1] + tab_h, center[2], (center[3] - tab_h).max(0.0));

        let pad   = theme.px(PAD);
        let seg_h = theme.px(28.0);
        let seg_w = theme.px(108.0);
        let gap   = theme.px(2.0);
        let seg_y = (tb_h - seg_h) * 0.5;
        let seg   = [
            rect_from(pad,                   seg_y, seg_w, seg_h),
            rect_from(pad + seg_w + gap,     seg_y, seg_w, seg_h),
            rect_from(pad + 2.0*(seg_w+gap), seg_y, seg_w, seg_h),
        ];
        let bw       = theme.px(86.0);
        let btn_gap  = theme.px(10.0);
        let btn_save = rect_from(win_w - pad - bw,          seg_y, bw, seg_h);
        let btn_editor = rect_from(btn_save[0] - btn_gap - bw, seg_y, bw, seg_h);

        Self { toolbar, navigator, inspector, statusbar, center,
               editor_tab, editor_body, seg, btn_editor, btn_save }
    }

    fn nav_row(&self, theme: &Theme, i: usize) -> Rect {
        let row_h   = theme.px(ROW_H);
        let top_pad = theme.px(6.0);
        rect_from(self.navigator[0], self.navigator[1] + top_pad + i as f32 * row_h,
                  self.navigator[2], row_h)
    }

    fn palette_rects(&self, theme: &Theme) -> [Rect; 5] {
        let sw    = theme.px(40.0);
        let gap   = theme.px(12.0);
        let total = 5.0 * sw + 4.0 * gap;
        let start = self.center[0] + (self.center[2] - total) * 0.5;
        let bar_h = theme.px(64.0);
        let y     = self.center[1] + self.center[3] - bar_h + (bar_h - sw) * 0.5;
        std::array::from_fn(|i| rect_from(start + i as f32 * (sw + gap), y, sw, sw))
    }

    fn inspector_cards(&self, theme: &Theme, top_y: f32) -> InspectorCards {
        let ix      = self.inspector[0];
        let iw      = self.inspector[2];
        let pad     = theme.px(PAD);
        let cx      = ix + pad;
        let cw      = iw - pad * 2.0;
        let fh      = theme.px(24.0);
        let cg      = theme.px(10.0);
        let hh      = theme.px(22.0);
        let rp      = theme.px(6.0);

        let name_row = rect_from(cx, top_y, cw, fh);

        let pos_y    = top_y + fh + cg;
        let pos_rh   = hh + rp * 1.5 + 3.0 * fh + 2.0 * (rp * 0.5);
        let pos_card = rect_from(cx, pos_y, cw, pos_rh);
        let pos_rows: [Rect; 3] = std::array::from_fn(|i|
            rect_from(cx + rp, pos_y + hh + rp + i as f32 * (fh + rp * 0.5), cw - rp * 2.0, fh));

        let sz_y     = pos_y + pos_rh + cg;
        let sz_card  = rect_from(cx, sz_y, cw, pos_rh);
        let sz_rows: [Rect; 3] = std::array::from_fn(|i|
            rect_from(cx + rp, sz_y + hh + rp + i as f32 * (fh + rp * 0.5), cw - rp * 2.0, fh));

        let col_y    = sz_y + pos_rh + cg;
        let col_h    = hh + rp * 1.5 + fh;
        let col_card = rect_from(cx, col_y, cw, col_h);
        let col_row  = rect_from(cx + rp, col_y + hh + rp, cw - rp * 2.0, fh);

        let act_y    = col_y + col_h + cg;
        let bw       = (cw - theme.px(8.0)) * 0.5;
        let btn_dup  = rect_from(cx, act_y, bw, theme.px(28.0));
        let btn_del  = rect_from(cx + bw + theme.px(8.0), act_y, bw, theme.px(28.0));

        InspectorCards { name_row, pos_card, pos_rows, sz_card, sz_rows,
                         col_card, col_row, btn_dup, btn_del,
                         bottom_y: act_y + theme.px(28.0) }
    }
}

struct InspectorCards {
    name_row: Rect,
    pos_card: Rect, pos_rows: [Rect; 3],
    sz_card:  Rect, sz_rows:  [Rect; 3],
    col_card: Rect, col_row:  Rect,
    btn_dup:  Rect, btn_del:  Rect,
    bottom_y: f32,
}

#[derive(Clone, Copy)]
enum NavRow {
    GroupHeader { group: NavGroup },
    SceneFile   { file_index: usize },
    Object      { object_index: usize },
    EmptyHint   { group: NavGroup },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum NavGroup { Scenes, Objects }

#[derive(Clone, Copy, PartialEq, Eq)]
enum DragAxis { X, Y, Z }

#[derive(Clone, Copy, PartialEq, Eq)]
enum DragProp { Position, Size }

struct FieldDrag { prop: DragProp, axis: DragAxis }

struct EditCamera { target: Vec3, yaw: f32, pitch: f32, distance: f32 }

impl EditCamera {
    fn new(target: Vec3) -> Self { Self { target, yaw: 0.0, pitch: 0.35, distance: 4.0 } }
    fn position(&self) -> Vec3 {
        let cp = self.pitch.cos();
        Vec3::new(
            self.target.x + self.distance * cp * self.yaw.sin(),
            self.target.y + self.distance * self.pitch.sin(),
            self.target.z + self.distance * cp * self.yaw.cos(),
        )
    }
    fn rotation(&self) -> Quat {
        let look = (self.target - self.position()).normalize_or_zero();
        Quat::from_rotation_arc(Vec3::new(0.0, 0.0, -1.0), look)
    }
    fn orbit(&mut self, dx: f32, dy: f32) {
        self.yaw  -= dx * 0.006;
        self.pitch = (self.pitch + dy * 0.006).clamp(-1.45, 1.45);
    }

    fn zoom(&mut self, d: f32) {
        let factor = (1.0 - d).clamp(0.1, 10.0);
        self.distance = (self.distance * factor).clamp(0.4, 25.0);
    }
}

struct PlacedObject { name: String, position: Vec3, half_size: Vec3, color: Color3 }

#[derive(Clone, Copy, PartialEq, Eq)]
enum ViewMode { PlayerView, FirstPerson, Edit }

struct App {
    window:   Option<Arc<Window>>,
    instance: Instance,
    surface:  Option<Surface<'static>>,
    config:   Option<SurfaceConfiguration>,

    renderer: Option<Renderer>,
    overlay:  Option<Overlay>,
    camera:   Camera,

    // Agate
    ui:       Option<Ui>,

    mouse_pos:      (f32, f32),
    mouse_pressed:  Vec<AMouseButton>,
    mouse_released: Vec<AMouseButton>,
    mouse_held:     Vec<AMouseButton>,
    scroll_y:       f32,
    text_input:     String,
    named_keys:     Vec<agate::input::NamedKey>,
    mods:           ModifiersState,

    packet:     SharedPacket,
    runtime:    GameRuntime,
    last_tick:  Instant,
    start:      Instant,
    scale:      f32,

    mesh_cache: HashMap<String, (GltfMesh, ModelUniform)>,

    view_mode:       ViewMode,
    edit_camera:     EditCamera,
    last_world_head: Vec3,

    placed_objects:    Vec<PlacedObject>,
    selected_object:   Option<usize>,
    moving_object:     bool,
    dragging_new_cube: Option<Color3>,
    ghost_preview:     Option<Vec3>,
    dragging_field:    Option<FieldDrag>,
    press_in_chrome:   bool,
    last_mouse_pos:    (f32, f32),
    left_down:         bool,
    dragged:           bool,

    editor:        TextEditor,
    show_editor:   bool,
    editor_focused: bool,
    editor_drag:   bool,
    files:         Vec<PathBuf>,
    selected_file: Option<usize>,

    nav_scenes_open:  bool,
    nav_objects_open: bool,

    files_discovered: Vec<PathBuf>,
}

impl App {
    fn new(packet: SharedPacket) -> Self {
        let dir = game_dir();
        let runtime = GameRuntime::load(&dir)
            .unwrap_or_else(|e| panic!("space_soup_editor cannot load game: {e}"));
        let files = discover_json(&dir);

        Self {
            window: None,
            instance: Instance::new(&InstanceDescriptor::default()),
            surface: None,
            config: None,
            renderer: None,
            overlay: None,
            camera: Camera::new(1.0),
            ui: None,

            mouse_pos: (0.0, 0.0),
            mouse_pressed: Vec::new(),
            mouse_released: Vec::new(),
            mouse_held: Vec::new(),
            scroll_y: 0.0,
            text_input: String::new(),
            named_keys: Vec::new(),
            mods: ModifiersState::empty(),

            packet,
            runtime,
            last_tick: Instant::now(),
            start: Instant::now(),
            scale: 1.0,
            mesh_cache: HashMap::new(),

            view_mode: ViewMode::PlayerView,
            edit_camera: EditCamera::new(Vec3::new(0.0, 1.2, 0.0)),
            last_world_head: Vec3::new(0.0, 1.2, 0.0),

            placed_objects: Vec::new(),
            selected_object: None,
            moving_object: false,
            dragging_new_cube: None,
            ghost_preview: None,
            dragging_field: None,
            press_in_chrome: false,
            last_mouse_pos: (0.0, 0.0),
            left_down: false,
            dragged: false,

            editor: TextEditor::empty(),
            show_editor: false,
            editor_focused: false,
            editor_drag: false,
            files: files.clone(),
            selected_file: None,

            nav_scenes_open: true,
            nav_objects_open: true,

            files_discovered: files,
        }
    }

    fn win_size(&self) -> (f32, f32) {
        self.window.as_ref().map(|w| {
            let s = w.inner_size();
            (s.width as f32, s.height as f32)
        }).unwrap_or((0.0, 0.0))
    }

    fn redraw_now(&self) {
        if let Some(w) = &self.window { w.request_redraw(); }
    }

    fn open_file(&mut self, idx: usize) {
        if let Some(p) = self.files_discovered.get(idx).cloned() {
            match TextEditor::load(&p) {
                Ok(ed) => {
                    self.editor = ed;
                    self.selected_file = Some(idx);
                    self.show_editor = true;
                    self.editor_focused = true;
                }
                Err(e) => log::warn!("debug_viewer: open {}: {e}", p.display()),
            }
        }
    }

    fn nav_rows(&self) -> Vec<NavRow> {
        let mut rows = Vec::new();
        rows.push(NavRow::GroupHeader { group: NavGroup::Scenes });
        if self.nav_scenes_open {
            if self.files_discovered.is_empty() {
                rows.push(NavRow::EmptyHint { group: NavGroup::Scenes });
            }
            for i in 0..self.files_discovered.len() {
                rows.push(NavRow::SceneFile { file_index: i });
            }
        }
        rows.push(NavRow::GroupHeader { group: NavGroup::Objects });
        if self.nav_objects_open {
            if self.placed_objects.is_empty() {
                rows.push(NavRow::EmptyHint { group: NavGroup::Objects });
            }
            for i in 0..self.placed_objects.len() {
                rows.push(NavRow::Object { object_index: i });
            }
        }
        rows
    }

    fn screen_ray(&self, sx: f32, sy: f32, win_w: f32, win_h: f32) -> (Vec3, Vec3) {
        let ndc_x = (sx / win_w) * 2.0 - 1.0;
        let ndc_y = 1.0 - (sy / win_h) * 2.0;
        let fov_y = CAMERA_FOV_Y_DEG.to_radians();
        let tan_half = (fov_y * 0.5).tan();
        let dir_cam = Vec3::new(
            ndc_x * tan_half * self.camera.aspect,
            ndc_y * tan_half,
            -1.0,
        ).normalize();
        (self.camera.position, self.camera.rotation * dir_cam)
    }

    fn pick_object(&self, sx: f32, sy: f32, w: f32, h: f32) -> Option<usize> {
        let (o, d) = self.screen_ray(sx, sy, w, h);
        self.placed_objects.iter().enumerate()
            .filter_map(|(i, ob)| ray_aabb_hit(o, d, ob.position, ob.half_size).map(|t| (i, t)))
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .map(|(i, _)| i)
    }

    // Route keyboard events to the text editor.
    fn editor_key(&mut self, ev: &KeyEvent) {
        let cmd   = self.mods.super_key() || self.mods.control_key();
        let shift = self.mods.shift_key();
        let ed    = &mut self.editor;
        match &ev.logical_key {
            Key::Named(NamedKey::ArrowLeft)  => ed.move_left(shift),
            Key::Named(NamedKey::ArrowRight) => ed.move_right(shift),
            Key::Named(NamedKey::ArrowUp)    => ed.move_up(shift),
            Key::Named(NamedKey::ArrowDown)  => ed.move_down(shift),
            Key::Named(NamedKey::Home)       => ed.move_home(shift),
            Key::Named(NamedKey::End)        => ed.move_end(shift),
            Key::Named(NamedKey::PageUp)     => ed.page(false, shift),
            Key::Named(NamedKey::PageDown)   => ed.page(true, shift),
            Key::Named(NamedKey::Backspace)  => ed.backspace(),
            Key::Named(NamedKey::Delete)     => ed.delete_forward(),
            Key::Named(NamedKey::Enter)      => ed.newline(),
            Key::Named(NamedKey::Tab)        => ed.insert_str("  "),
            Key::Named(NamedKey::Space)      => ed.insert_char(' '),
            Key::Character(s) if cmd => match s.as_str() {
                "s"|"S" => { let _ = ed.save(); }
                "a"|"A" => ed.select_all(),
                "c"|"C" => ed.copy(),
                "x"|"X" => ed.cut(),
                "v"|"V" => ed.paste(),
                "z"|"Z" => if shift { ed.redo() } else { ed.undo() },
                _ => {}
            },
            _ => {
                if !cmd {
                    if let Some(txt) = &ev.text {
                        for ch in txt.chars() {
                            if !ch.is_control() { ed.insert_char(ch); }
                        }
                    }
                }
            }
        }
    }

    fn redraw(&mut self) {
        let (win_w, win_h) = self.win_size();
        let nav_rows = self.nav_rows();

        let (Some(renderer), Some(surface), Some(overlay), Some(ui)) = (
            self.renderer.as_mut(), self.surface.as_ref(),
            self.overlay.as_mut(), self.ui.as_mut(),
        ) else { return };

        let now = Instant::now();
        let dt  = now.duration_since(self.last_tick).as_secs_f32();
        self.last_tick = now;

        let (render_cuboids, render_meshes, scene_change) =
            self.runtime.update(dt, &InputFrame::default(), PlayerRig::new(), &LocomotionInput::default(), None);
        if let Some(next) = scene_change {
            if let Err(e) = self.runtime.load_scene(&next) {
                log::warn!("scene switch '{next}': {e}");
            } else {
                let new_paths: Vec<String> = self.runtime.scene().objects.iter()
                    .filter_map(|o| o.mesh.as_ref().map(|m| m.path.clone()))
                    .collect();
                for path in new_paths {
                    if self.mesh_cache.contains_key(&path) { continue; }
                    let full_path = self.runtime.game_dir().join(&path);
                    match GltfMesh::load(&renderer.device, &renderer.queue, renderer.mesh_texture_layout(), &full_path) {
                        Ok(mesh) => {
                            let model_uniform = renderer.create_model_uniform();
                            self.mesh_cache.insert(path, (mesh, model_uniform));
                        }
                        Err(e) => log::warn!("debug_viewer: failed to preload mesh '{path}': {e}"),
                    }
                }
            }
        }

        let packet     = self.packet.lock().unwrap().clone();
        let yaw_rot    = Quat::from_rotation_y(packet.locomotion.player_yaw_deg.to_radians());
        let pl_offset  = Vec3::from(packet.locomotion.player_offset);
        let to_world   = |p: Vec3, r: Quat| -> (Vec3, Quat) { (pl_offset + yaw_rot * p, yaw_rot * r) };
        let (head_pos, head_rot) = to_world(packet.head.position(), packet.head.rotation());
        self.last_world_head = head_pos;

        match self.view_mode {
            ViewMode::PlayerView => {
                self.camera.position = head_pos + Vec3::new(0.0, 1.2, 2.0);
                let look = (head_pos - self.camera.position).normalize_or_zero();
                self.camera.rotation = Quat::from_rotation_arc(Vec3::NEG_Z, look);
            }
            ViewMode::FirstPerson => {
                self.camera.position = head_pos;
                self.camera.rotation = head_rot;
            }
            ViewMode::Edit => {
                self.camera.position = self.edit_camera.position();
                self.camera.rotation = self.edit_camera.rotation();
            }
        }

        let mut cuboids: Vec<Cuboid> =
            render_cuboids.iter().map(scene_3d::engine_cuboid_to_render).collect();
        cuboids.extend(scene_3d::ground_grid());
        cuboids.extend(scene_3d::build_player_overlay(
            head_pos, head_rot, &packet.left_hand, &packet.right_hand, to_world,
        ));
        for obj in &self.placed_objects {
            cuboids.push(Cuboid::solid_and_wire(obj.position, obj.half_size, obj.color, Color3(255,255,255,255)));
        }
        if let Some(idx) = self.selected_object {
            if let Some(obj) = self.placed_objects.get(idx) {
                cuboids.extend(selection_gizmo(obj.position, obj.half_size));
            }
        }
        if let (Some(color), Some(pos)) = (self.dragging_new_cube, self.ghost_preview) {
            let half = Vec3::splat(OBJECT_HALF_SIZE);
            cuboids.push(Cuboid::wireframe(
                Vec3::new(pos.x, half.y, pos.z), half,
                Color3(color.0, color.1, color.2, 160),
            ));
        }

        let frame = match surface.get_current_texture() {
            Ok(f) => f,
            Err(e) => { log::warn!("surface error: {e}"); return; }
        };
        let view = frame.texture.create_view(&TextureViewDescriptor::default());

        for rm in &render_meshes {
            if let Some((mesh, _)) = self.mesh_cache.get_mut(&rm.path) {
                mesh.position = rm.position;
                mesh.rotation = rm.rotation;
                mesh.scale    = rm.scale;
            }
        }
        let mesh_instances: Vec<MeshInstance> = render_meshes.iter()
            .filter_map(|rm| {
                let (mesh, model) = self.mesh_cache.get(&rm.path)?;
                Some(MeshInstance { mesh, model })
            })
            .collect();

        renderer.render_with_meshes(&view, &self.camera, &cuboids, &mesh_instances);

        let ui_input = UiInput {
            mouse_pos:      self.mouse_pos,
            mouse_held:     std::mem::take(&mut self.mouse_held),
            mouse_pressed:  std::mem::take(&mut self.mouse_pressed),
            mouse_released: std::mem::take(&mut self.mouse_released),
            scroll_y:       std::mem::take(&mut self.scroll_y),
            text:           std::mem::take(&mut self.text_input),
            keys:           std::mem::take(&mut self.named_keys),
            cmd:   self.mods.super_key() || self.mods.control_key(),
            shift: self.mods.shift_key(),
            alt:   self.mods.alt_key(),
            dt,
        };

        if self.left_down { ui.begin_frame(win_w, win_h, &ui_input);
        } else { ui.begin_frame(win_w, win_h, &ui_input); }

        let theme  = ui.theme;
        let layout = Layout::new(win_w, win_h, &theme);

        ui.fill(layout.toolbar, t::TITLEBAR_BG);
        ui.separator(0.0, layout.toolbar[1] + layout.toolbar[3] - theme.px(1.0), win_w);

        let seg_labels = ["Player", "First Person", "Edit"];
        let seg_modes  = [ViewMode::PlayerView, ViewMode::FirstPerson, ViewMode::Edit];
        for i in 0..3 {
            let active = self.view_mode == seg_modes[i] && !self.show_editor;
            let (bg, fg) = if active {
                (t::ACCENT, t::TEXT_ON_ACCENT)
            } else {
                (t::CONTROL_BG, t::TEXT_PRIMARY)
            };
            if ui.button_styled(layout.seg[i], seg_labels[i], bg, fg) {
                self.view_mode = seg_modes[i];
                if self.view_mode == ViewMode::Edit {
                    self.edit_camera = EditCamera::new(self.last_world_head);
                }
                self.show_editor = false;
            }
        }

        let (ed_bg, ed_fg) = if self.show_editor {
            (t::ACCENT, t::TEXT_ON_ACCENT)
        } else {
            (t::CONTROL_BG, t::TEXT_PRIMARY)
        };
        if ui.button_styled(layout.btn_editor, "Editor", ed_bg, ed_fg) {
            self.show_editor = !self.show_editor;
        }
        let save_en = self.show_editor && self.editor.dirty;
        let save_bg = if save_en { t::CONTROL_HOVER } else { t::CONTROL_BG };
        let save_fg = if save_en { t::TEXT_PRIMARY } else { t::TEXT_SECONDARY };
        if ui.button_styled(layout.btn_save, "Save", save_bg, save_fg) && save_en {
            let _ = self.editor.save();
        }

        ui.fill(layout.navigator, t::SIDEBAR_BG);
        ui.separator_v(layout.navigator[0] + layout.navigator[2] - theme.px(1.0),
                       layout.navigator[1], layout.navigator[3]);

        let mut clicked_nav: Option<usize> = None;
        for (i, row) in nav_rows.iter().enumerate() {
            let r = layout.nav_row(&theme, i);
            match *row {
                NavRow::GroupHeader { group } => {
                    let open = match group {
                        NavGroup::Scenes  => self.nav_scenes_open,
                        NavGroup::Objects => self.nav_objects_open,
                    };
                    let label = match group {
                        NavGroup::Scenes  => if open { "▾  Scenes" }  else { "▸  Scenes" },
                        NavGroup::Objects => if open { "▾  Objects" } else { "▸  Objects" },
                    };
                    if ui.list_row(r, label, false) {
                        match group {
                            NavGroup::Scenes  => self.nav_scenes_open  = !open,
                            NavGroup::Objects => self.nav_objects_open = !open,
                        }
                    }
                }
                NavRow::SceneFile { file_index } => {
                    let name = self.files_discovered.get(file_index)
                        .and_then(|p| p.file_name())
                        .map(|n| format!("  ◆  {}", n.to_string_lossy()))
                        .unwrap_or_default();
                    let sel = self.selected_file == Some(file_index) && self.show_editor;
                    if ui.list_row(r, &name, sel) {
                        clicked_nav = Some(file_index);
                    }
                }
                NavRow::Object { object_index } => {
                    let name = format!("  ■  {}", self.placed_objects.get(object_index)
                        .map(|o| o.name.as_str()).unwrap_or(""));
                    let sel = self.selected_object == Some(object_index) && !self.show_editor;
                    if ui.list_row(r, &name, sel) {
                        self.show_editor = false;
                        self.selected_object = Some(object_index);
                    }
                }
                NavRow::EmptyHint { group } => {
                    let hint = match group {
                        NavGroup::Scenes  => "  No .json files found",
                        NavGroup::Objects => "  No objects placed",
                    };
                    ui.label_styled(r[0], r[1] + (r[3] - theme.small()) * 0.5,
                                    hint, theme.small(), t::TEXT_SECONDARY, r[2], None);
                }
            }
        }

        let scene_info_y = layout.nav_row(&theme, nav_rows.len())[1] + theme.px(10.0);
        let nx = layout.navigator[0];
        let nw = layout.navigator[2];
        let clip_nav = layout.navigator;
        ui.separator(nx, scene_info_y - theme.px(8.0), nw);
        ui.label_styled(nx + theme.px(PAD), scene_info_y,
            "SCENE", theme.small(), t::TEXT_SECONDARY, nw, None);
        let info = format!(
            "{}\nobjects: {}\ncuboids: {}\nmeshes:  {}",
            packet.scene.scene_name, packet.scene.object_count,
            packet.scene.render_cuboids, packet.scene.render_meshes,
        );
        ui.label_styled(
            nx + theme.px(PAD), scene_info_y + theme.px(ROW_H),
            &info, theme.small(), t::TEXT_PRIMARY, nw - theme.px(PAD),
            Some(clip_nav),
        );

        if self.show_editor {
            ui.fill(layout.editor_tab, t::TOOLBAR_BG);
            ui.separator(layout.editor_tab[0],
                         layout.editor_tab[1] + layout.editor_tab[3] - theme.px(1.0),
                         layout.editor_tab[2]);
            let dot   = if self.editor.dirty { "●  " } else { "" };
            let title = format!("{dot}{}", self.editor.file_name());
            ui.label_styled(
                layout.editor_tab[0] + theme.px(PAD),
                layout.editor_tab[1] + (layout.editor_tab[3] - theme.body()) * 0.5,
                &title, theme.body(), t::TEXT_PRIMARY, layout.editor_tab[2], None,
            );

            let focused = self.editor_focused;
            let er = layout.editor_body;
            let clicked = ui.text_editor(er, &mut self.editor, focused);
            if clicked { self.editor_focused = true; }

        } else if self.view_mode == ViewMode::Edit {
            let bar_h = theme.px(72.0);
            let cx    = layout.center[0];
            let cw    = layout.center[2];
            let bar_y = layout.center[1] + layout.center[3] - bar_h;
            ui.fill([cx, bar_y, cw, bar_h], Color(20, 20, 24, 220));
            ui.label_styled(
                cx, bar_y + theme.px(6.0),
                "Drag a cube into the scene",
                theme.small(), t::TEXT_SECONDARY, cw, None,
            );
            let palette = layout.palette_rects(&theme);
            for (i, r) in palette.iter().enumerate() {
                let c = PALETTE[i];
                ui.fill([r[0] - theme.px(2.0), r[1] - theme.px(2.0),
                         r[2] + theme.px(4.0), r[3] + theme.px(4.0)],
                         Color(225, 228, 235, 255));
                if ui.color_swatch(*r, Color(c.0, c.1, c.2, 255)) {
                    self.dragging_new_cube = Some(c);
                }
            }
        }

        ui.fill(layout.inspector, t::SIDEBAR_BG);
        ui.separator_v(layout.inspector[0], layout.inspector[1], layout.inspector[3]);
        let ix  = layout.inspector[0];
        let iy  = layout.inspector[1];
        let iw  = layout.inspector[2];
        let ih  = layout.inspector[3];
        let hdr = if self.show_editor { "EDITOR" } else { "INSPECTOR" };
        ui.label_styled(ix + theme.px(PAD), iy + theme.px(8.0),
            hdr, theme.small(), t::TEXT_SECONDARY, iw, None);
        let body_top = iy + theme.px(ROW_H + 6.0);
        let clip_ins = layout.inspector;

        if self.show_editor {
            let (ln, col) = self.editor.cursor_line_col();
            let body = format!(
                "file:   {}\nlines:  {}\nLn {}, Col {}\nmodified: {}\n\nShortcuts:\n⌘S save  ⌘Z undo\n⌘C/⌘X/⌘V\n⌘A select all",
                self.editor.file_name(), self.editor.line_count(), ln, col,
                if self.editor.dirty { "yes" } else { "no" },
            );
            ui.label_styled(ix + theme.px(PAD), body_top,
                &body, theme.small(), t::TEXT_PRIMARY, iw - theme.px(PAD*2.0), Some(clip_ins));

        } else if let Some(obj_idx) = self.selected_object.filter(|i| *i < self.placed_objects.len()) {
            let cards = layout.inspector_cards(&theme, body_top);

            let obj_name      = self.placed_objects[obj_idx].name.clone();
            let obj_position  = self.placed_objects[obj_idx].position;
            let obj_half_size = self.placed_objects[obj_idx].half_size;
            let obj_color     = self.placed_objects[obj_idx].color;

            ui.label_styled(cards.name_row[0], cards.name_row[1] + (cards.name_row[3] - theme.body()) * 0.5,
                &obj_name, theme.body(), t::TEXT_PRIMARY, cards.name_row[2], None);


            ui.card(cards.pos_card);
            ui.label_styled(cards.pos_card[0] + theme.px(PAD), cards.pos_card[1] + theme.px(7.0),
                "POSITION", theme.small(), t::TEXT_SECONDARY, cards.pos_card[2], None);
            let pos_vals  = [obj_position.x, obj_position.y, obj_position.z];
            let axes      = ["X", "Y", "Z"];
            let label_w   = theme.px(18.0);
            let field_gap = theme.px(6.0);
            for i in 0..3usize {
                let row     = cards.pos_rows[i];
                let label_r = rect_from(row[0], row[1], label_w, row[3]);
                let input_r = rect_from(row[0] + label_w + field_gap, row[1],
                                         row[2] - label_w - field_gap, row[3]);
                ui.label_styled(label_r[0], label_r[1] + (label_r[3] - theme.body()) * 0.5,
                    axes[i], theme.body(), t::TEXT_SECONDARY, label_r[2], None);

                let id      = WidgetId::of(&format!("pos_{i}_{obj_idx}"));
                let val_str = format!("{:.3}", pos_vals[i]);
                if let Some(new_str) = ui.text_input(id, input_r, &val_str, "") {
                    if let Ok(v) = new_str.trim().parse::<f32>() {
                        let obj = &mut self.placed_objects[obj_idx];
                        match i {
                            0 => obj.position.x = v,
                            1 => obj.position.y = v,
                            _ => obj.position.z = v,
                        }
                    }
                }
            }

            ui.card(cards.sz_card);
            ui.label_styled(cards.sz_card[0] + theme.px(PAD), cards.sz_card[1] + theme.px(7.0),
                "SIZE", theme.small(), t::TEXT_SECONDARY, cards.sz_card[2], None);
            let sz_vals = [obj_half_size.x * 2.0, obj_half_size.y * 2.0, obj_half_size.z * 2.0];
            for i in 0..3usize {
                let row     = cards.sz_rows[i];
                let label_r = rect_from(row[0], row[1], label_w, row[3]);
                let input_r = rect_from(row[0] + label_w + field_gap, row[1],
                                         row[2] - label_w - field_gap, row[3]);
                ui.label_styled(label_r[0], label_r[1] + (label_r[3] - theme.body()) * 0.5,
                    axes[i], theme.body(), t::TEXT_SECONDARY, label_r[2], None);

                let id      = WidgetId::of(&format!("sz_{i}_{obj_idx}"));
                let val_str = format!("{:.3}", sz_vals[i]);
                if let Some(new_str) = ui.text_input(id, input_r, &val_str, "") {
                    if let Ok(v) = new_str.trim().parse::<f32>() {
                        if v > 0.0 {
                            let obj  = &mut self.placed_objects[obj_idx];
                            let half = (v * 0.5).max(0.005);
                            match i {
                                0 => obj.half_size.x = half,
                                1 => obj.half_size.y = half,
                                _ => obj.half_size.z = half,
                            }
                        }
                    }
                }
            }

            ui.card(cards.col_card);
            ui.label_styled(cards.col_card[0] + theme.px(PAD), cards.col_card[1] + theme.px(7.0),
                "COLOR", theme.small(), t::TEXT_SECONDARY, cards.col_card[2], None);
            ui.color_swatch(cards.col_row, Color(obj_color.0, obj_color.1, obj_color.2, 255));

            if ui.button_secondary(cards.btn_dup, "Duplicate") {
                let new_obj = PlacedObject {
                    name:      format!("{} copy", obj_name),
                    position:  obj_position + Vec3::new(0.1, 0.0, 0.1),
                    half_size: obj_half_size,
                    color:     obj_color,
                };
                self.placed_objects.push(new_obj);
                self.selected_object = Some(self.placed_objects.len() - 1);
            }
            if ui.button_danger(cards.btn_del, "Delete") {
                self.placed_objects.remove(obj_idx);
                self.selected_object = None;
            }

            let hint_y = cards.bottom_y + theme.px(14.0);
            ui.label_styled(ix + theme.px(PAD), hint_y,
                "Click a field and type a value.",
                theme.small(), t::TEXT_SECONDARY, iw - theme.px(PAD*2.0), Some(clip_ins));

        } else {
            let body = text_panels::right_panel_text(&packet);
            ui.label_styled(ix + theme.px(PAD), body_top,
                &body, theme.small(), t::TEXT_PRIMARY, iw - theme.px(PAD*2.0), Some(clip_ins));
        }

        ui.fill(layout.statusbar, t::STATUSBAR_BG);
        ui.separator(0.0, layout.statusbar[1], win_w);
        let sb   = layout.statusbar;
        let sy   = sb[1] + (sb[3] - theme.small()) * 0.5;
        let left = if self.show_editor {
            self.editor.path.as_ref().map(|p| p.display().to_string())
                .unwrap_or_else(|| "untitled".into())
        } else {
            format!("Scene: {}", self.runtime.scene_name())
        };
        ui.label_styled(sb[0] + theme.px(PAD), sy,
            &left, theme.small(), t::TEXT_SECONDARY, sb[2] * 0.5, None);
        if self.show_editor {
            let (ln, col) = self.editor.cursor_line_col();
            let mid = format!("Ln {ln}, Col {col}{}",
                if self.editor.has_selection() { "  (sel)" } else { "" });

            let mid_x = sb[0] + (sb[2] - mid.len() as f32 * theme.small() * 0.6) * 0.5;
            ui.label_styled(mid_x, sy, &mid, theme.small(), t::TEXT_SECONDARY, sb[2]*0.4, None);
        }
        let fps_text = format!("{:.1} fps · frame {}", packet.timing.fps, packet.timing.frame_count);
        let fps_w = fps_text.len() as f32 * theme.small() * 0.62;
        ui.label_styled(sb[0] + sb[2] - fps_w - theme.px(PAD), sy,
            &fps_text, theme.small(), t::TEXT_SECONDARY, fps_w + theme.px(PAD), None);

        overlay.set_items(ui.finish());

        let mut encoder = renderer.device.create_command_encoder(
            &CommandEncoderDescriptor { label: Some("overlay_enc") });
        overlay.render(&renderer.device, &renderer.queue, &mut encoder, &view);
        renderer.queue.submit(Some(encoder.finish()));
        frame.present();

        if let Some(fi) = clicked_nav { self.open_file(fi); }

        self.mouse_held = if self.left_down { vec![AMouseButton::Left] } else { vec![] };
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(
            event_loop.create_window(
                winit::window::WindowAttributes::default()
                    .with_title("space_soup_editor")
                    .with_inner_size(winit::dpi::LogicalSize::new(1600u32, 900u32)),
            ).unwrap(),
        );
        self.scale  = window.scale_factor() as f32;
        self.window = Some(window.clone());

        let surface: Surface<'static> = unsafe {
            std::mem::transmute(self.instance.create_surface(window.clone()).unwrap())
        };
        let adapter = pollster::block_on(self.instance.request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        })).unwrap();
        let (device, queue) = pollster::block_on(adapter.request_device(&DeviceDescriptor {
            required_features: Features::empty(),
            required_limits: Limits::default(),
            ..Default::default()
        })).unwrap();

        let size  = window.inner_size();
        let caps  = surface.get_capabilities(&adapter);
        let fmt   = caps.formats[0];
        let cfg   = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: fmt,
            width: size.width,
            height: size.height,
            present_mode: PresentMode::Fifo,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &cfg);

        self.camera   = Camera::new(size.width as f32 / size.height as f32);
        let renderer  = Renderer::from_device(device, queue, fmt, size.width, size.height);
        let overlay   = Overlay::new(&renderer.device, fmt, size.width, size.height, self.scale);

        let font = Arc::new(load_font());
        let ui   = Ui::new(self.scale, font);


        let mesh_paths: Vec<String> = self.runtime.scene().objects.iter()
            .filter_map(|o| o.mesh.as_ref().map(|m| m.path.clone()))
            .collect();
        for path in mesh_paths {
            if self.mesh_cache.contains_key(&path) { continue; }
            let full_path = self.runtime.game_dir().join(&path);
            match GltfMesh::load(&renderer.device, &renderer.queue, renderer.mesh_texture_layout(), &full_path) {
                Ok(mesh) => {
                    let model_uniform = renderer.create_model_uniform();
                    log::info!("debug_viewer: preloaded mesh '{path}'");
                    self.mesh_cache.insert(path, (mesh, model_uniform));
                }
                Err(e) => log::warn!("debug_viewer: failed to preload mesh '{path}': {e}"),
            }
        }

        self.renderer = Some(renderer);
        self.overlay  = Some(overlay);
        self.surface  = Some(surface);
        self.config   = Some(cfg);
        self.ui       = Some(ui);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.scale = scale_factor as f32;
                if let Some(ov) = self.overlay.as_mut() { ov.set_scale_factor(self.scale); }
                if let Some(ui) = self.ui.as_mut() { ui.theme = Theme::new(self.scale); }
                self.redraw_now();
            }

            WindowEvent::Resized(size) => {
                if let (Some(sur), Some(cfg), Some(rnd), Some(ov)) = (
                    self.surface.as_ref(), self.config.as_mut(),
                    self.renderer.as_mut(), self.overlay.as_mut(),
                ) {
                    cfg.width  = size.width;
                    cfg.height = size.height;
                    sur.configure(&rnd.device, cfg);
                    rnd.resize(size.width, size.height);
                    ov.resize(size.width, size.height);
                    self.camera.aspect = size.width as f32 / size.height as f32;
                }
            }

            WindowEvent::ModifiersChanged(m) => { self.mods = m.state(); }

            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed {
                    if self.show_editor && self.editor_focused {
                        self.editor_key(&event);
                    } else {
                        let cmd = self.mods.super_key() || self.mods.control_key();
                        if !cmd {
                            if let Some(txt) = &event.text {
                                for ch in txt.chars() {
                                    if !ch.is_control() { self.text_input.push(ch); }
                                }
                            }
                        }
                        if let Some(nk) = winit_key_to_agate(&event.logical_key) {
                            self.named_keys.push(nk);
                        }
                    }
                    self.redraw_now();
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                let new = (position.x as f32, position.y as f32);
                let dx  = new.0 - self.last_mouse_pos.0;
                let dy  = new.1 - self.last_mouse_pos.1;
                self.last_mouse_pos = new;
                self.mouse_pos      = new;

                if self.left_down && self.view_mode == ViewMode::Edit && !self.show_editor {
                    if self.moving_object {
                        if let Some(idx) = self.selected_object {
                            let (win_w, win_h) = self.win_size();
                            let plane_y = self.placed_objects[idx].position.y;
                            let (o, d)  = self.screen_ray(new.0, new.1, win_w, win_h);
                            if let Some(hit) = ray_plane_intersect(o, d, plane_y) {
                                self.placed_objects[idx].position.x = hit.x;
                                self.placed_objects[idx].position.z = hit.z;
                            }
                            self.dragged = true;
                        }
                    } else if !self.press_in_chrome && (dx.abs() > 0.5 || dy.abs() > 0.5) {
                        self.dragged = true;
                        self.edit_camera.orbit(dx, dy);
                    }
                }

                if self.dragging_new_cube.is_some() {
                    let (win_w, win_h) = self.win_size();
                    let theme  = Theme::new(self.scale);
                    let layout = Layout::new(win_w, win_h, &theme);
                    let (o, d) = self.screen_ray(new.0, new.1, win_w, win_h);
                    let in_center = in_rect_t(new, layout.center)
                        && new.1 < layout.center[1] + layout.center[3] - theme.px(72.0);
                    self.ghost_preview = if in_center { ray_plane_intersect(o, d, 0.0) } else { None };
                }

                self.redraw_now();
            }

            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => {
                let (win_w, win_h) = self.win_size();
                let theme  = Theme::new(self.scale);
                let layout = Layout::new(win_w, win_h, &theme);
                let mp     = self.mouse_pos;

                match state {
                    ElementState::Pressed => {
                        self.left_down = true;
                        self.dragged   = false;
                        self.moving_object = false;
                        self.mouse_pressed.push(AMouseButton::Left);
                        self.mouse_held.push(AMouseButton::Left);
                        self.editor_focused = self.show_editor && in_rect_t(mp, layout.editor_body);

                        self.press_in_chrome = !in_rect_t(mp, layout.center);
                        if self.view_mode == ViewMode::Edit && !self.show_editor && !self.press_in_chrome {
                            let palette = layout.palette_rects(&theme);
                            if let Some((i, _)) = palette.iter().enumerate().find(|(_, r)| in_rect_t(mp, **r)) {
                                self.dragging_new_cube = Some(PALETTE[i]);
                            } else if let Some(idx) = self.pick_object(mp.0, mp.1, win_w, win_h) {
                                self.selected_object = Some(idx);
                                self.moving_object   = true;
                            }
                        }
                    }
                    ElementState::Released => {
                        self.left_down = false;
                        self.mouse_released.push(AMouseButton::Left);

                        if let Some(color) = self.dragging_new_cube.take() {
                            if let Some(pos) = self.ghost_preview.take() {
                                let half = Vec3::splat(OBJECT_HALF_SIZE);
                                let name = format!("Cube {}", self.placed_objects.len() + 1);
                                self.placed_objects.push(PlacedObject {
                                    name, position: Vec3::new(pos.x, half.y, pos.z),
                                    half_size: half, color,
                                });
                                self.selected_object = Some(self.placed_objects.len() - 1);
                            }
                            self.ghost_preview = None;
                        }

                        if self.moving_object { self.moving_object = false; }

                        if self.view_mode == ViewMode::Edit && !self.show_editor
                            && !self.dragged && in_rect_t(mp, layout.center)
                        {
                            self.selected_object = self.pick_object(mp.0, mp.1, win_w, win_h);
                        }
                        self.dragged = false;
                    }
                    _ => {}
                }
                self.redraw_now();
            }

            WindowEvent::PinchGesture { delta, phase, .. } => {
                if self.view_mode == ViewMode::Edit && !self.show_editor
                    && phase != TouchPhase::Cancelled
                {
                    self.edit_camera.zoom(delta as f32 * 1.5);
                    self.redraw_now();
                }
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let lines = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p)   => p.y as f32 * 0.05,
                };
                self.scroll_y += lines;
                if !self.show_editor && self.view_mode == ViewMode::Edit {
                    self.edit_camera.zoom(lines * 0.08);
                }
                self.redraw_now();
            }

            WindowEvent::RedrawRequested => self.redraw(),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _: &ActiveEventLoop) { self.redraw_now(); }
}

fn in_rect_t(p: (f32, f32), r: Rect) -> bool {
    p.0 >= r[0] && p.0 <= r[0]+r[2] && p.1 >= r[1] && p.1 <= r[1]+r[3]
}

fn winit_key_to_agate(key: &Key) -> Option<agate::input::NamedKey> {
    use agate::input::NamedKey as A;
    match key {
        Key::Named(NamedKey::ArrowLeft)  => Some(A::ArrowLeft),
        Key::Named(NamedKey::ArrowRight) => Some(A::ArrowRight),
        Key::Named(NamedKey::ArrowUp)    => Some(A::ArrowUp),
        Key::Named(NamedKey::ArrowDown)  => Some(A::ArrowDown),
        Key::Named(NamedKey::Home)       => Some(A::Home),
        Key::Named(NamedKey::End)        => Some(A::End),
        Key::Named(NamedKey::PageUp)     => Some(A::PageUp),
        Key::Named(NamedKey::PageDown)   => Some(A::PageDown),
        Key::Named(NamedKey::Backspace)  => Some(A::Backspace),
        Key::Named(NamedKey::Delete)     => Some(A::Delete),
        Key::Named(NamedKey::Enter)      => Some(A::Enter),
        Key::Named(NamedKey::Tab)        => Some(A::Tab),
        Key::Named(NamedKey::Escape)     => Some(A::Escape),
        _ => None,
    }
}

fn ray_aabb_hit(origin: Vec3, dir: Vec3, center: Vec3, half: Vec3) -> Option<f32> {
    let min = center - half;
    let max = center + half;
    let mut t_min = f32::NEG_INFINITY;
    let mut t_max = f32::INFINITY;
    for (o, d, lo, hi) in [
        (origin.x, dir.x, min.x, max.x),
        (origin.y, dir.y, min.y, max.y),
        (origin.z, dir.z, min.z, max.z),
    ] {
        if d.abs() < 1e-8 {
            if o < lo || o > hi { return None; }
        } else {
            let (mut t1, mut t2) = ((lo-o)/d, (hi-o)/d);
            if t1 > t2 { std::mem::swap(&mut t1, &mut t2); }
            t_min = t_min.max(t1);
            t_max = t_max.min(t2);
            if t_min > t_max { return None; }
        }
    }
    if t_max < 0.0 { None } else { Some(t_min.max(0.0)) }
}

fn ray_plane_intersect(origin: Vec3, dir: Vec3, plane_y: f32) -> Option<Vec3> {
    if dir.y.abs() < 1e-5 { return None; }
    let t = (plane_y - origin.y) / dir.y;
    if t < 0.0 { return None; }
    Some(origin + dir * t)
}

fn selection_gizmo(center: Vec3, half_size: Vec3) -> Vec<Cuboid> {
    const N: usize = 24;
    let mut out = Vec::with_capacity(N + 2);
    let radius  = half_size.x.max(half_size.z) * 1.8 + 0.12;
    let seg_len = (2.0 * std::f32::consts::PI * radius / N as f32) * 0.6;
    let gizmo   = Color3(80, 220, 255, 255);
    for i in 0..N {
        let a = i as f32 / N as f32 * std::f32::consts::PI * 2.0;
        let x = center.x + radius * a.cos();
        let z = center.z + radius * a.sin();
        let mut c = Cuboid::solid(Vec3::new(x, 0.01, z), Vec3::new(seg_len*0.5, 0.004, 0.012), gizmo);
        c.rotation = Quat::from_rotation_y(-a);
        out.push(c);
    }
    let arm = radius + 0.3;
    out.push(Cuboid::solid(Vec3::new(center.x, 0.01, center.z), Vec3::new(arm, 0.003, 0.01), Color3(80,220,255,200)));
    out.push(Cuboid::solid(Vec3::new(center.x, 0.01, center.z), Vec3::new(0.01, 0.003, arm), Color3(80,220,255,200)));
    out
}

fn discover_json(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut push = |p: PathBuf| { if p.extension().map_or(false, |e| e == "json") { out.push(p); } };
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_file() { push(p); }
            else if p.is_dir() {
                if let Ok(rd2) = std::fs::read_dir(&p) {
                    for e2 in rd2.flatten() { push(e2.path()); }
                }
            }
        }
    }
    out.sort();
    out
}

fn load_font() -> Font {
    const PATH: &str = "font.ttf";
    match std::fs::read(PATH) {
        Ok(bytes) => Font::new(&bytes),
        Err(e)    => panic!("debug_viewer: could not read '{PATH}': {e}"),
    }
}

fn main() {
    env_logger::init();
    let packet     = network::spawn_listener("0.0.0.0:7778");
    let event_loop = EventLoop::new().unwrap();
    let mut app    = App::new(packet);
    event_loop.run_app(&mut app).unwrap();
}