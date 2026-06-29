pub(crate) mod inspector;
pub(crate) mod navigator;
pub(crate) mod scene;
pub(crate) mod statusbar;
pub(crate) mod toolbar;
pub(crate) mod viewport_overlay;

use std::time::Instant;

use glam::{Quat, Vec3};
use wgpu::{CommandEncoderDescriptor, TextureViewDescriptor};

use space_soup::renderer::{GltfMesh, MeshInstance};
use space_soup_engine::{InputFrame, LocomotionInput, PlayerRig};

use agate::Theme;

use super::layout::Layout;
use super::{EditTarget, ViewMode};

impl super::App {
    pub(crate) fn redraw(&mut self) {
        let (win_w, win_h) = self.win_size();
        let nav_rows = self.nav_rows();

        let (Some(renderer), Some(surface), Some(overlay), Some(ui)) = (
            self.renderer.as_mut(), self.surface.as_ref(),
            self.overlay.as_mut(), self.ui.as_mut(),
        ) else { return };

        let now = Instant::now();
        let dt = now.duration_since(self.last_tick).as_secs_f32();
        self.last_tick = now;

        let (render_cuboids, render_meshes, scene_change) = if self.view_mode == ViewMode::Edit {
            let (c, m) = self.runtime.render_lists();
            (c, m, None)
        } else {
            self.runtime.update(dt, &InputFrame::default(), PlayerRig::new(), &LocomotionInput::default(), None)
        };

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
                            self.mesh_base_half_size.insert(path.clone(), crate::app::scene_bridge::mesh_base_half_size(&mesh));
                            self.mesh_cache.insert(path.clone(), (mesh, model_uniform));
                        }
                        Err(e) => log::warn!("space_soup_editor: failed to preload mesh '{path}': {e}"),
                    }
                }
            }
        }

        let needed: Vec<String> = self.runtime.scene().objects.iter()
            .filter_map(|o| o.mesh.as_ref().map(|m| m.path.clone()))
            .filter(|p| !self.mesh_cache.contains_key(p))
            .collect();
        for path in needed {
            let full_path = self.runtime.game_dir().join(&path);
            match GltfMesh::load(&renderer.device, &renderer.queue, renderer.mesh_texture_layout(), &full_path) {
                Ok(mesh) => {
                    let model_uniform = renderer.create_model_uniform();
                    self.mesh_base_half_size.insert(path.clone(), crate::app::scene_bridge::mesh_base_half_size(&mesh));
                    self.mesh_cache.insert(path.clone(), (mesh, model_uniform));
                }
                Err(e) => log::warn!("space_soup_editor: failed to load mesh '{path}': {e}"),
            }
        }

        let packet = self.packet.lock().unwrap().clone();
        let yaw_rot = Quat::from_rotation_y(packet.locomotion.player_yaw_deg.to_radians());
        let pl_offset = Vec3::from(packet.locomotion.player_offset);
        let to_world = move |p: Vec3, r: Quat| -> (Vec3, Quat) { (pl_offset + yaw_rot * p, yaw_rot * r) };
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
                self.camera.position = self.edit_camera.position;
                self.camera.rotation = self.edit_camera.rotation();
            }
        }

        let selected_id = self.selected_object.clone();
        let cuboids = scene::build_cuboids(
            &render_cuboids, &packet, head_pos, head_rot, to_world,
            &self.runtime.scene().objects, selected_id.as_deref(),
            self.view_mode == ViewMode::Edit,
            self.dragging_new_model.is_some(), self.ghost_preview,
        );

        let frame = match surface.get_current_texture() {
            Ok(f) => f,
            Err(e) => { log::warn!("surface error: {e}"); return; }
        };
        let view = frame.texture.create_view(&TextureViewDescriptor::default());

        for rm in &render_meshes {
            if let Some((mesh, _)) = self.mesh_cache.get_mut(&rm.path) {
                mesh.position = rm.position;
                mesh.rotation = rm.rotation;
                mesh.scale = rm.scale;
                eprintln!("mesh sync: path={} scale={:?}", rm.path, rm.scale);
            }
        }

        let mut mesh_instances: Vec<MeshInstance> = render_meshes.iter()
            .filter_map(|rm| {
                let (mesh, model) = self.mesh_cache.get(&rm.path)?;
                Some(MeshInstance { mesh, model })
            })
            .collect();

        let editor_view_mode = matches!(self.editing, Some(EditTarget::SceneFile) | Some(EditTarget::ObjectScript(_)));
        mesh_instances.extend(scene::sync_gizmo_and_collect(
            &mut self.xform_gizmo,
            &mut self.gizmo_assets,
            &self.camera,
            (win_w, win_h),
            &self.runtime.scene().objects,
            selected_id.as_deref(),
            self.view_mode,
            editor_view_mode,
            self.gizmo_dragging,
        ));

        renderer.render_with_meshes(&view, &self.camera, &cuboids, &mesh_instances);

        let ui_input = agate::UiInput {
            mouse_pos: self.mouse_pos,
            mouse_held: std::mem::take(&mut self.mouse_held),
            mouse_pressed: std::mem::take(&mut self.mouse_pressed),
            mouse_released: std::mem::take(&mut self.mouse_released),
            scroll_y: std::mem::take(&mut self.scroll_y),
            text: std::mem::take(&mut self.text_input),
            keys: std::mem::take(&mut self.named_keys),
            cmd: self.mods.super_key() || self.mods.control_key(),
            shift: self.mods.shift_key(),
            alt: self.mods.alt_key(),
            dt,
        };
        ui.begin_frame(win_w, win_h, &ui_input);

        let theme: Theme = ui.theme;
        let layout = Layout::new(win_w, win_h, &theme);

        toolbar::draw(ui, &theme, &layout, &mut self.view_mode, &mut self.edit_camera,
            self.last_world_head, &mut self.editing, self.selected_file, &self.editor);
        if toolbar::draw_save(ui, &theme, &layout, &self.editing, self.editor.dirty) {
            let _ = self.editor.save();
        }
        if toolbar::draw_save_scene(ui, &theme, &layout, self.scene_dirty) {
            if let Ok(path) = crate::app::scene_bridge::save_scene(
                self.runtime.scene(), self.runtime.game_dir(), self.runtime.scene_name(),
            ) {
                log::info!("space_soup_editor: saved scene to {}", path.display());
                self.scene_dirty = false;
            }
        }

        let clicked_nav = navigator::draw(
            ui, &theme, &layout, &nav_rows, &self.files_discovered,
            &self.runtime.scene().objects,
            &mut self.selected_file, &mut self.selected_object, &mut self.editing,
            &mut self.nav_scenes_open, &mut self.nav_objects_open, &packet,
        );

        if self.editing.is_some() {
            let title = match &self.editing {
                Some(EditTarget::SceneFile) => self.editor.file_name(),
                Some(EditTarget::ObjectScript(id)) => format!("Script: {id}"),
                None => String::new(),
            };
            draw_editor_tab(ui, &theme, &layout, &mut self.editor, &mut self.editor_focused, &title);

            if let Some(EditTarget::ObjectScript(id)) = self.editing.clone() {
                if self.editor.dirty {
                    let text = self.editor.text();
                    crate::app::scene_bridge::set_object_script(self.runtime.scene_mut(), &id, text);
                    self.scene_dirty = true;
                }
            }
        } else if self.view_mode == ViewMode::Edit {
            if let Some(new_mode) = viewport_overlay::draw(
                ui, &theme, &layout, &self.available_models, &self.dragging_new_model,
                self.gizmo_drag, self.xform_gizmo.mode,
            ) {
                self.xform_gizmo.mode = new_mode;
            }
        }

        let mut scene_dirty = self.scene_dirty;
        let mut open_script_editor: Option<String> = None;
        let game_dir_for_inspector = self.runtime.game_dir().to_path_buf();
        inspector::draw(
            ui, &theme, &layout, &self.editing, &self.editor,
            self.runtime.scene_mut(), &game_dir_for_inspector, &mut self.selected_object, &mut scene_dirty,
            &mut open_script_editor, &packet,
        );
        self.scene_dirty = scene_dirty;

        if let Some(id) = open_script_editor {
            let text = crate::app::scene_bridge::object_script(self.runtime.scene(), &id);
            self.editor.set_text(&text);
            self.editing = Some(EditTarget::ObjectScript(id));
            self.editor_focused = true;
        }

        let show_editor_for_statusbar = matches!(self.editing, Some(EditTarget::SceneFile));
        statusbar::draw(
            ui, &theme, &layout, show_editor_for_statusbar, &self.editor,
            self.runtime.scene_name(), packet.timing.fps, packet.timing.frame_count,
        );

        overlay.set_items(ui.finish());

        let mut encoder = renderer.device.create_command_encoder(
            &CommandEncoderDescriptor { label: Some("overlay_enc") });
        overlay.render(&renderer.device, &renderer.queue, &mut encoder, &view);
        renderer.queue.submit(Some(encoder.finish()));
        frame.present();

        if let Some(fi) = clicked_nav { self.open_file(fi); }

        self.mouse_held = if self.left_down { vec![agate::AMouseButton::Left] } else { vec![] };
    }
}

fn draw_editor_tab(
    ui: &mut agate::Ui, theme: &Theme, layout: &Layout,
    editor: &mut agate::TextEditor, editor_focused: &mut bool, title_override: &str,
) {
    use agate::theme as t;

    ui.fill(layout.editor_tab, t::TOOLBAR_BG);
    ui.separator(layout.editor_tab[0],
        layout.editor_tab[1] + layout.editor_tab[3] - theme.px(1.0), layout.editor_tab[2]);
    let dot = if editor.dirty { "\u{25cf}  " } else { "" };
    let title = format!("{dot}{title_override}");
    ui.label_styled(
        layout.editor_tab[0] + theme.px(super::layout::PAD),
        layout.editor_tab[1] + (layout.editor_tab[3] - theme.body()) * 0.5,
        &title, theme.body(), t::TEXT_PRIMARY, layout.editor_tab[2], None,
    );

    let focused = *editor_focused;
    let er = layout.editor_body;
    let clicked = ui.text_editor(er, editor, focused);
    if clicked { *editor_focused = true; }
}