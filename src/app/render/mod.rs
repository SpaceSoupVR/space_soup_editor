pub(crate) mod anim_sim_panel;
pub(crate) mod confirm;
pub(crate) mod grab_pose_panel;
pub(crate) mod inspector;
pub(crate) mod navigator;
pub(crate) mod object_preview_panel;
pub(crate) mod scene;
pub(crate) mod statusbar;
pub(crate) mod toolbar;
pub(crate) mod viewport_overlay;

use std::time::Instant;

use glam::{Quat, Vec3};
use wgpu::{CommandEncoderDescriptor, TextureViewDescriptor};

use space_soup::renderer::{Color3, Cuboid, GltfMesh, MeshInstance};
use space_soup_engine::{InputFrame, LocomotionInput, PlayerFrameInput, PlayerRig};
use space_soup_protocol::PlayerId;
use std::collections::HashMap;

use agate::Theme;

use crate::transform_gizmo::GizmoMode;

use super::anim_sim_editor;
use super::grab_pose_editor;
use super::layout::Layout;
use super::object_preview;
use super::snap;
use super::{EditTarget, EditorTool, ViewMode};

/// Splits a row rect into a fixed-width label cell and the remaining input cell.
/// Shared by the inspector and the sub-editor panels.
pub(crate) fn split_row(row: [f32; 4], label_w: f32, field_gap: f32) -> ([f32; 4], [f32; 4]) {
    let label_r = [row[0], row[1], label_w, row[3]];
    let input_r = [
        row[0] + label_w + field_gap,
        row[1],
        row[2] - label_w - field_gap,
        row[3],
    ];
    (label_r, input_r)
}

impl super::App {
    pub(crate) fn redraw(&mut self) {
        if self.grab_pose_editor.is_some() {
            self.redraw_grab_pose();
            return;
        }
        if self.anim_sim_editor.is_some() {
            self.redraw_anim_sim();
            return;
        }
        if self.object_preview.is_some() {
            self.redraw_object_preview();
            return;
        }

        let (win_w, win_h) = self.win_size();
        let nav_rows = self.nav_rows();

        let (Some(renderer), Some(surface), Some(overlay), Some(ui)) = (
            self.renderer.as_mut(),
            self.surface.as_ref(),
            self.overlay.as_mut(),
            self.ui.as_mut(),
        ) else {
            return;
        };

        let now = Instant::now();
        let dt = now.duration_since(self.last_tick).as_secs_f32();
        self.last_tick = now;

        let (render_cuboids, render_meshes, render_lights, scene_change) =
            if self.view_mode == ViewMode::Edit {
                let (c, m, l) = self.runtime.render_lists();
                (c, m, l, None)
            } else {
                // Editor's "Play mode" preview is single-player; the engine
                // is otherwise multi-player-capable now, so it just supplies
                // one entry under the fixed local-player id.
                let mut inputs = HashMap::new();
                inputs.insert(
                    PlayerId::local(),
                    PlayerFrameInput {
                        rig: PlayerRig::new(),
                        input: InputFrame::default(),
                        locomotion_input: LocomotionInput::default(),
                        teleport_target: None,
                    },
                );
                self.runtime.update(dt, &inputs)
            };

        if let Some(next) = scene_change {
            if let Err(e) = self.runtime.load_scene(&next) {
                log::warn!("scene switch '{next}': {e}");
            } else {
                let new_paths: Vec<String> = self
                    .runtime
                    .scene()
                    .objects
                    .iter()
                    .filter_map(|o| o.mesh.as_ref().map(|m| m.path.clone()))
                    .collect();
                for path in new_paths {
                    if self.mesh_cache.contains_key(&path) {
                        continue;
                    }
                    let full_path = self.runtime.game_dir().join(&path);
                    match GltfMesh::load(
                        &renderer.device,
                        &renderer.queue,
                        renderer.mesh_texture_layout(),
                        &full_path,
                    ) {
                        Ok(mesh) => {
                            let model_uniform = renderer.create_model_uniform();
                            self.mesh_base_half_size.insert(
                                path.clone(),
                                crate::app::scene_bridge::mesh_base_half_size(&mesh),
                            );
                            self.mesh_cache.insert(path.clone(), (mesh, model_uniform));
                        }
                        Err(e) => {
                            log::warn!("space_soup_editor: failed to preload mesh '{path}': {e}")
                        }
                    }
                }
            }
        }

        let needed: Vec<String> = self
            .runtime
            .scene()
            .objects
            .iter()
            .filter_map(|o| o.mesh.as_ref().map(|m| m.path.clone()))
            .filter(|p| !self.mesh_cache.contains_key(p))
            .collect();
        for path in needed {
            let full_path = self.runtime.game_dir().join(&path);
            match GltfMesh::load(
                &renderer.device,
                &renderer.queue,
                renderer.mesh_texture_layout(),
                &full_path,
            ) {
                Ok(mesh) => {
                    let model_uniform = renderer.create_model_uniform();
                    self.mesh_base_half_size.insert(
                        path.clone(),
                        crate::app::scene_bridge::mesh_base_half_size(&mesh),
                    );
                    self.mesh_cache.insert(path.clone(), (mesh, model_uniform));
                }
                Err(e) => log::warn!("space_soup_editor: failed to load mesh '{path}': {e}"),
            }
        }

        let game_dir_for_snap = self.runtime.game_dir().to_path_buf();
        snap::update_preview(
            renderer,
            &mut self.mesh_cache,
            &game_dir_for_snap,
            self.runtime.scene_mut(),
            &mut self.scene_dirty,
            self.tool,
            self.selected_object.as_deref(),
            self.snap_hand,
            &mut self.snap_selected_joint,
            &mut self.snap_joint_frame,
        );

        let packet = self.packet.lock().unwrap().clone();
        let yaw_rot = Quat::from_rotation_y(packet.locomotion.player_yaw_deg.to_radians());
        let pl_offset = Vec3::from(packet.locomotion.player_offset);
        let to_world =
            move |p: Vec3, r: Quat| -> (Vec3, Quat) { (pl_offset + yaw_rot * p, yaw_rot * r) };
        let (head_pos, head_rot) = to_world(packet.head.position(), packet.head.rotation());
        self.last_world_head = head_pos;

        match self.view_mode {
            ViewMode::PlayerView => {
                self.camera.position = head_pos + Vec3::new(0.0, 1.2, 2.0);
                let look = (head_pos - self.camera.position).normalize_or_zero();
                self.camera.rotation = Quat::from_rotation_arc(Vec3::NEG_Z, look);
            }
            ViewMode::FirstPerson | ViewMode::RenderView => {
                let forward = head_rot * Vec3::NEG_Z;
                self.camera.position = head_pos + forward * 0.15;
                self.camera.rotation = head_rot;
            }
            ViewMode::Edit => {
                // WASDQE free-look fly (keys are gated in keyboard::fly_active;
                // Shift = faster). Harmless when nothing is held.
                self.edit_camera.fly(&self.fly, dt, self.mods.shift_key());
                self.camera.position = self.edit_camera.position;
                self.camera.rotation = self.edit_camera.rotation();
            }
        }

        const MAX_RENDER_DIST: f32 = 40.0;
        let (render_cuboids, render_meshes) = if self.view_mode == ViewMode::RenderView {
            (
                render_cuboids
                    .into_iter()
                    .filter(|rc| rc.position.distance(head_pos) < MAX_RENDER_DIST)
                    .collect::<Vec<_>>(),
                render_meshes
                    .into_iter()
                    .filter(|rm| rm.position.distance(head_pos) < MAX_RENDER_DIST)
                    .collect::<Vec<_>>(),
            )
        } else {
            (render_cuboids, render_meshes)
        };

        let selected_id = self.selected_object.clone();
        let mut cuboids = scene::build_cuboids(
            &render_cuboids,
            &packet,
            head_pos,
            head_rot,
            to_world,
            &self.runtime.scene().objects,
            selected_id.as_deref(),
            self.view_mode == ViewMode::Edit,
            self.dragging_new_object.is_some(),
            self.ghost_preview,
        );

        if self.tool == EditorTool::Snap {
            for (i, joint) in self.snap_joint_frame.iter().enumerate() {
                let selected = self.snap_selected_joint == Some(i);
                let (color, half) = if selected {
                    (Color3(255, 220, 40, 255), Vec3::splat(0.012))
                } else {
                    (Color3(80, 200, 255, 220), Vec3::splat(0.008))
                };
                cuboids.push(Cuboid::solid(joint.current_pos, half, color));
            }
        }

        let frame = match surface.get_current_texture() {
            Ok(f) => f,
            Err(e) => {
                log::warn!("surface error: {e}");
                return;
            }
        };
        let view = frame.texture.create_view(&TextureViewDescriptor::default());

        for rm in &render_meshes {
            if let Some((mesh, _)) = self.mesh_cache.get_mut(&rm.path) {
                mesh.position = rm.position;
                mesh.rotation = rm.rotation;
                mesh.scale = rm.scale;
            }
        }

        let mut mesh_instances: Vec<MeshInstance> = render_meshes
            .iter()
            .filter_map(|rm| {
                let (mesh, model) = self.mesh_cache.get(&rm.path)?;
                Some(MeshInstance { mesh, model })
            })
            .collect();

        if self.tool == EditorTool::Snap && !self.snap_joint_frame.is_empty() {
            let hand_path = snap::hand_glb_path(self.snap_hand);
            if let Some((mesh, model)) = self.mesh_cache.get(hand_path) {
                mesh_instances.push(MeshInstance { mesh, model });
            }
        }

        mesh_instances.extend(
            self.debug_meshes
                .iter()
                .map(|(mesh, model)| MeshInstance { mesh, model }),
        );

        let editor_view_mode = matches!(
            self.editing,
            Some(EditTarget::SceneFile) | Some(EditTarget::ObjectScript(_))
        );
        if self.tool == EditorTool::Snap {
            let joint_pos = self
                .snap_selected_joint
                .and_then(|i| self.snap_joint_frame.get(i))
                .map(|j| j.current_pos);
            mesh_instances.extend(snap::collect_joint_gizmo_instances(
                &mut self.xform_gizmo,
                &mut self.gizmo_assets,
                &self.camera,
                (win_w, win_h),
                joint_pos,
                self.gizmo_dragging,
            ));
        } else {
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
        }

        if self.view_mode == ViewMode::Edit && !editor_view_mode {
            mesh_instances.extend(scene::collect_icon_instances(
                &self.icon_assets,
                &mut self.icon_mesh_cache,
                renderer,
                &self.camera,
                &self.runtime.scene().objects,
            ));
        }

        let lights: Vec<space_soup::renderer::Light> =
            render_lights.iter().map(crate::app::scene_bridge::to_render_light).collect();
        renderer.render_with_lights(&view, &self.camera, &cuboids, &mesh_instances, &lights);

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

        if self.view_mode == ViewMode::Edit {
            ui.card_border(layout.center);
        }

        toolbar::draw(
            ui,
            &theme,
            &layout,
            &mut self.view_mode,
            &mut self.edit_camera,
            self.last_world_head,
            &mut self.editing,
            self.selected_file,
            &self.editor,
        );
        if toolbar::draw_save(ui, &theme, &layout, &self.editing, self.editor.dirty) {
            let _ = self.editor.save();
        }
        if toolbar::draw_save_scene(ui, &theme, &layout, self.scene_dirty) {
            if let Ok(path) = crate::app::scene_bridge::save_scene(
                self.runtime.scene(),
                self.runtime.game_dir(),
                self.runtime.scene_name(),
            ) {
                log::info!("space_soup_editor: saved scene to {}", path.display());
                self.scene_dirty = false;
            }
        }

        let clicked_nav = navigator::draw(
            ui,
            &theme,
            &layout,
            &nav_rows,
            &self.files_discovered,
            &self.runtime.scene().objects,
            &mut self.selected_file,
            &mut self.selected_object,
            &mut self.editing,
            &mut self.nav_scenes_open,
            &mut self.nav_objects_open,
            &packet,
        );

        if self.editing.is_some() {
            let title = match &self.editing {
                Some(EditTarget::SceneFile) => self.editor.file_name(),
                Some(EditTarget::ObjectScript(id)) => format!("Script: {id}"),
                None => String::new(),
            };
            draw_editor_tab(
                ui,
                &theme,
                &layout,
                &mut self.editor,
                &mut self.editor_focused,
                &title,
            );

            if let Some(EditTarget::ObjectScript(id)) = self.editing.clone() {
                if self.editor.dirty {
                    let text = self.editor.text();
                    crate::app::scene_bridge::set_object_script(
                        self.runtime.scene_mut(),
                        &id,
                        text,
                    );
                    self.scene_dirty = true;
                }
            }
        } else if self.view_mode == ViewMode::Edit {
            let (new_mode, new_tool, new_hand) = viewport_overlay::draw(
                ui,
                &theme,
                &layout,
                &self.available_models,
                &self.dragging_new_object,
                &mut self.model_scroll_y,
                self.gizmo_drag,
                self.xform_gizmo.mode,
                self.tool,
                self.snap_hand,
            );
            if let Some(new_mode) = new_mode {
                self.xform_gizmo.mode = new_mode;
            }
            if let Some(new_tool) = new_tool {
                if new_tool != self.tool {
                    self.rig_selection.clear();
                    self.snap_selected_joint = None;
                    self.tool = new_tool;
                }
            }
            if let Some(new_hand) = new_hand {
                self.snap_hand = new_hand;
            }
        }

        let mut scene_dirty = self.scene_dirty;
        let mut open_script_editor: Option<String> = None;
        let mut open_grab_pose_editor: Option<String> = None;
        let mut open_anim_sim_editor: Option<String> = None;
        let mut open_object_preview: Option<String> = None;
        let mut preview_sound: Option<(String, f32, f32)> = None;
        let game_dir_for_inspector = self.runtime.game_dir().to_path_buf();
        inspector::draw(
            ui,
            &theme,
            &layout,
            &self.editing,
            &self.editor,
            self.runtime.scene_mut(),
            &game_dir_for_inspector,
            &mut self.selected_object,
            &mut scene_dirty,
            &mut open_script_editor,
            &mut open_grab_pose_editor,
            &mut open_anim_sim_editor,
            &mut open_object_preview,
            &mut preview_sound,
            &mut self.inspector_content_height,
            self.view_mode == ViewMode::Edit,
            &mut self.inspector_rot_edit,
            &packet,
        );
        self.scene_dirty = scene_dirty;
        if let Some((clip, volume, pitch)) = preview_sound {
            self.runtime.preview_sound(&clip, volume, pitch);
        }

        if let Some(id) = open_script_editor {
            let text = crate::app::scene_bridge::object_script(self.runtime.scene(), &id);
            self.editor.set_text(&text);
            self.editing = Some(EditTarget::ObjectScript(id));
            self.editor_focused = true;
        }

        let show_editor_for_statusbar = matches!(self.editing, Some(EditTarget::SceneFile));
        let tool_hint = match self.tool {
            EditorTool::Select => None,
            EditorTool::Rigging => Some(match self.rig_selection.len() {
                0 => "Rigging: click a grabbable object".to_string(),
                1 => "Rigging: click a hand reference object (or press G to seed without one)"
                    .to_string(),
                _ => "Rigging: seeding grip pose\u{2026}".to_string(),
            }),
            EditorTool::Snap => Some(match (&self.selected_object, self.snap_selected_joint) {
                (None, _) => "Snap: select an object with a grip pose".to_string(),
                (Some(_), None) => "Snap: click a finger-joint marker to drag its curl".to_string(),
                (Some(_), Some(i)) => self
                    .snap_joint_frame
                    .get(i)
                    .map(|j| format!("Snap: dragging '{}'", j.name))
                    .unwrap_or_default(),
            }),
        };
        statusbar::draw(
            ui,
            &theme,
            &layout,
            show_editor_for_statusbar,
            &self.editor,
            self.runtime.scene_name(),
            packet.timing.fps,
            packet.timing.frame_count,
            tool_hint.as_deref(),
        );

        overlay.set_items(ui.finish());

        let mut encoder = renderer
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("overlay_enc"),
            });
        overlay.render(&renderer.device, &renderer.queue, &mut encoder, &view);
        renderer.queue.submit(Some(encoder.finish()));
        frame.present();

        if let Some(fi) = clicked_nav {
            self.open_file(fi);
        }
        if let Some(id) = open_grab_pose_editor {
            grab_pose_editor::open(self, id);
        }
        if let Some(id) = open_anim_sim_editor {
            anim_sim_editor::open(self, id);
        }
        if let Some(id) = open_object_preview {
            object_preview::open(self, id);
        }

        self.mouse_held = if self.left_down {
            vec![agate::AMouseButton::Left]
        } else {
            vec![]
        };
    }

    fn redraw_grab_pose(&mut self) {
        let (win_w, win_h) = self.win_size();

        let (Some(renderer), Some(surface), Some(overlay), Some(ui)) = (
            self.renderer.as_mut(),
            self.surface.as_ref(),
            self.overlay.as_mut(),
            self.ui.as_mut(),
        ) else {
            return;
        };

        let now = Instant::now();
        self.last_tick = now;

        let game_dir = self.runtime.game_dir().to_path_buf();
        grab_pose_editor::ensure_hand_meshes_loaded(renderer, &mut self.mesh_cache, &game_dir);

        let object_id = self.grab_pose_editor.as_ref().map(|s| s.object_id.clone());
        let object_mesh_path = object_id
            .as_ref()
            .and_then(|id| self.runtime.scene().find_object(id))
            .and_then(|o| o.mesh.as_ref().map(|m| m.path.clone()));
        if let Some(path) = &object_mesh_path {
            if !self.mesh_cache.contains_key(path) {
                let full_path = game_dir.join(path);
                match GltfMesh::load(
                    &renderer.device,
                    &renderer.queue,
                    renderer.mesh_texture_layout(),
                    &full_path,
                ) {
                    Ok(mesh) => {
                        let model_uniform = renderer.create_model_uniform();
                        self.mesh_cache.insert(path.clone(), (mesh, model_uniform));
                    }
                    Err(e) => log::warn!(
                        "space_soup_editor: grab pose editor failed to load '{path}': {e}"
                    ),
                }
            }
        }

        let Some(state) = self.grab_pose_editor.as_ref() else {
            return;
        };
        self.camera.position = state.orbit.eye_position();
        self.camera.rotation = state.orbit.rotation();

        grab_pose_editor::update_transforms(
            state,
            self.runtime.scene(),
            &mut self.mesh_cache,
            &renderer.queue,
        );
        let cuboids =
            grab_pose_editor::collect_cuboids(state, self.runtime.scene());
        let mesh_instances =
            grab_pose_editor::collect_mesh_instances(state, self.runtime.scene(), &self.mesh_cache);

        let frame = match surface.get_current_texture() {
            Ok(f) => f,
            Err(e) => {
                log::warn!("surface error: {e}");
                return;
            }
        };
        let view = frame.texture.create_view(&TextureViewDescriptor::default());

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
            dt: 0.0,
        };
        ui.begin_frame(win_w, win_h, &ui_input);

        let theme: Theme = ui.theme;
        let layout = Layout::new(win_w, win_h, &theme);
        ui.card_border(layout.grab_pose_viewport());

        let scene_ref = self.runtime.scene();
        let Some(state) = self.grab_pose_editor.as_mut() else {
            return;
        };
        let actions = grab_pose_panel::draw(
            ui,
            &theme,
            &layout,
            state,
            scene_ref,
        );

        overlay.set_items(ui.finish());

        let mut encoder = renderer
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("overlay_enc"),
            });
        overlay.render(&renderer.device, &renderer.queue, &mut encoder, &view);
        renderer.queue.submit(Some(encoder.finish()));
        frame.present();

        self.mouse_held = if self.left_down {
            vec![agate::AMouseButton::Left]
        } else {
            vec![]
        };

        if actions.save {
            grab_pose_editor::save(self);
        }
        if actions.request_exit {
            grab_pose_editor::request_exit(self);
        }
        if actions.exit_discard {
            grab_pose_editor::exit_discard(self);
            return;
        }
        if actions.exit_save {
            grab_pose_editor::exit_save(self);
            return;
        }
        if actions.cancel_exit {
            grab_pose_editor::cancel_exit(self);
        }
        if actions.recenter {
            grab_pose_editor::recenter_view(self);
        }
        if let Some(snap) = actions.set_pos_snap {
            grab_pose_editor::set_pos_snap(self, snap);
        }
        if let Some(snap) = actions.set_rot_snap {
            grab_pose_editor::set_rot_snap(self, snap);
        }
        if let Some(step) = actions.set_pos_snap_step {
            grab_pose_editor::set_pos_snap_step(self, step);
        }
        if let Some(step) = actions.set_rot_snap_step {
            grab_pose_editor::set_rot_snap_step(self, step);
        }
        if actions.reset {
            grab_pose_editor::reset_active_point(self);
        }
        if actions.undo {
            grab_pose_editor::undo(self);
        }
        if actions.redo {
            grab_pose_editor::redo(self);
        }
        let field_dragging = actions.field_edit.is_some()
            || actions.finger_curl_edit.is_some()
            || actions.set_pos_snap_step.is_some()
            || actions.set_rot_snap_step.is_some();
        if let Some((field, value)) = actions.field_edit {
            grab_pose_editor::apply_field_edit(self, field, value);
        }
        if let Some((group_idx, value)) = actions.finger_curl_edit {
            grab_pose_editor::apply_finger_curl(self, group_idx, value);
        }
        if !field_dragging {
            // No field drag this frame — end coalescing so the next drag is its
            // own undo entry.
            if let Some(state) = self.grab_pose_editor.as_mut() {
                state.end_coalesce();
            }
        }
        if let Some(i) = actions.select_point {
            grab_pose_editor::select_point(self, i);
        }
        if let Some(hand) = actions.add_point {
            grab_pose_editor::add_point(self, hand);
        }
        if actions.delete_point {
            grab_pose_editor::delete_active_point(self);
        }
        if let Some(name) = actions.rename_point {
            grab_pose_editor::rename_active_point(self, name);
        }
        if let Some(kind) = actions.set_kind {
            grab_pose_editor::set_active_point_kind(self, kind);
        }
        if let Some(view) = actions.set_view {
            grab_pose_editor::set_hand_view(self, view);
        }
    }

    fn redraw_anim_sim(&mut self) {
        let (win_w, win_h) = self.win_size();

        let (Some(renderer), Some(surface), Some(overlay), Some(ui)) = (
            self.renderer.as_mut(),
            self.surface.as_ref(),
            self.overlay.as_mut(),
            self.ui.as_mut(),
        ) else {
            return;
        };

        // Real dt so preview playback advances (about_to_wait drives redraws).
        let now = Instant::now();
        let dt = now.duration_since(self.last_tick).as_secs_f32();
        self.last_tick = now;

        // Load the object's mesh if it isn't cached yet.
        let game_dir = self.runtime.game_dir().to_path_buf();
        let object_mesh_path = self
            .anim_sim_editor
            .as_ref()
            .and_then(|s| self.runtime.scene().find_object(&s.object_id))
            .and_then(|o| o.mesh.as_ref().map(|m| m.path.clone()));
        if let Some(path) = &object_mesh_path {
            if !self.mesh_cache.contains_key(path) {
                let full_path = game_dir.join(path);
                match GltfMesh::load(
                    &renderer.device,
                    &renderer.queue,
                    renderer.mesh_texture_layout(),
                    &full_path,
                ) {
                    Ok(mesh) => {
                        let model_uniform = renderer.create_model_uniform();
                        self.mesh_cache.insert(path.clone(), (mesh, model_uniform));
                    }
                    Err(e) => log::warn!(
                        "space_soup_editor: anim sim editor failed to load '{path}': {e}"
                    ),
                }
            }
        }

        let Some(state) = self.anim_sim_editor.as_ref() else {
            return;
        };
        self.camera.position = state.orbit.eye_position();
        self.camera.rotation = state.orbit.rotation();

        anim_sim_editor::update_transforms(state, self.runtime.scene(), &mut self.mesh_cache);
        let cuboids = anim_sim_editor::collect_cuboids(state, self.runtime.scene());
        let mesh_instances =
            anim_sim_editor::collect_mesh_instances(state, self.runtime.scene(), &self.mesh_cache);

        let frame = match surface.get_current_texture() {
            Ok(f) => f,
            Err(e) => {
                log::warn!("surface error: {e}");
                return;
            }
        };
        let view = frame.texture.create_view(&TextureViewDescriptor::default());

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
        ui.card_border(layout.anim_sim_viewport());

        let has_anim_clipboard = self.anim_clipboard.is_some();
        let has_key_clipboard = self.keyframe_clipboard.is_some();
        let scene_ref = self.runtime.scene();
        let Some(state) = self.anim_sim_editor.as_mut() else {
            return;
        };
        let actions = anim_sim_panel::draw(
            ui,
            &theme,
            &layout,
            state,
            scene_ref,
            has_anim_clipboard,
            has_key_clipboard,
        );

        overlay.set_items(ui.finish());

        let mut encoder = renderer
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("overlay_enc"),
            });
        overlay.render(&renderer.device, &renderer.queue, &mut encoder, &view);
        renderer.queue.submit(Some(encoder.finish()));
        frame.present();

        self.mouse_held = if self.left_down {
            vec![agate::AMouseButton::Left]
        } else {
            vec![]
        };

        // Advance the preview after drawing (uses this frame's dt).
        anim_sim_editor::update_playback(self, dt);

        self.apply_anim_sim_actions(actions);
    }

    fn apply_anim_sim_actions(&mut self, actions: anim_sim_panel::AnimSimPanelActions) {
        use space_soup_engine::BINDING_BUTTONS;

        if actions.save {
            anim_sim_editor::save(self);
        }
        if actions.request_exit {
            anim_sim_editor::request_exit(self);
        }
        if actions.exit_discard {
            anim_sim_editor::exit_discard(self);
            return;
        }
        if actions.exit_save {
            anim_sim_editor::exit_save(self);
            return;
        }
        if actions.cancel_exit {
            anim_sim_editor::cancel_exit(self);
        }
        if actions.recenter {
            anim_sim_editor::recenter_view(self);
        }
        if let Some(step) = actions.set_snap_step {
            anim_sim_editor::set_snap_step(self, step);
        }
        if let Some(speed) = actions.set_speed {
            anim_sim_editor::set_speed(self, speed);
        }
        if actions.undo {
            anim_sim_editor::undo(self);
        }
        if actions.redo {
            anim_sim_editor::redo(self);
        }

        if let Some(i) = actions.select_anim {
            anim_sim_editor::select_anim(self, i);
        }
        if actions.add_anim {
            anim_sim_editor::add_anim(self);
        }
        if actions.delete_anim {
            anim_sim_editor::delete_anim(self);
        }
        if let Some(name) = actions.rename_anim {
            anim_sim_editor::rename_anim(self, name);
        }
        if let Some(v) = actions.set_looping {
            anim_sim_editor::set_looping(self, v);
        }
        if let Some(e) = actions.set_easing {
            anim_sim_editor::set_easing(self, e);
        }
        if actions.copy_anim {
            anim_sim_editor::copy_anim(self);
        }
        if actions.paste_anim {
            anim_sim_editor::paste_anim(self);
        }

        if actions.play {
            anim_sim_editor::play(self);
        }
        if actions.pause {
            anim_sim_editor::pause(self);
        }
        if actions.stop {
            anim_sim_editor::stop(self);
        }
        if let Some(v) = actions.seek {
            anim_sim_editor::seek(self, v);
        }

        if let Some(i) = actions.select_key {
            anim_sim_editor::select_key(self, i);
        }
        if actions.add_key {
            anim_sim_editor::add_key_at_playhead(self);
        }
        if actions.capture_pose {
            anim_sim_editor::capture_pose_key(self);
        }
        if actions.delete_key {
            anim_sim_editor::delete_key(self);
        }
        if actions.copy_key {
            anim_sim_editor::copy_key(self);
        }
        if actions.paste_key {
            anim_sim_editor::paste_key(self);
        }
        if let Some((field, value)) = actions.key_field_edit {
            anim_sim_editor::edit_key_field(self, field, value);
        } else if let Some(state) = self.anim_sim_editor.as_mut() {
            // No field drag this frame — end coalescing so the next drag is its
            // own undo entry.
            state.end_coalesce();
        }
        if let Some(ch) = actions.toggle_channel {
            anim_sim_editor::toggle_key_channel(self, ch);
        }

        if actions.add_binding {
            anim_sim_editor::add_binding(self);
        }
        if let Some(i) = actions.remove_binding {
            anim_sim_editor::remove_binding(self, i);
        }
        if let Some(i) = actions.cycle_binding_button {
            anim_sim_editor::edit_binding(self, i, |b| {
                let cur = BINDING_BUTTONS
                    .iter()
                    .position(|id| *id == b.button)
                    .unwrap_or(0);
                b.button = BINDING_BUTTONS[(cur + 1) % BINDING_BUTTONS.len()].to_string();
            });
        }
        if let Some(i) = actions.cycle_binding_anim {
            let names: Vec<String> = self
                .anim_sim_editor
                .as_ref()
                .and_then(|s| self.runtime.scene().find_object(&s.object_id))
                .map(|o| o.animations.iter().map(|a| a.name.clone()).collect())
                .unwrap_or_default();
            if !names.is_empty() {
                anim_sim_editor::edit_binding(self, i, |b| {
                    let cur = names.iter().position(|n| *n == b.animation);
                    let next = match cur {
                        Some(c) => (c + 1) % names.len(),
                        None => 0,
                    };
                    b.animation = names[next].clone();
                });
            }
        }
        if let Some((i, mode)) = actions.binding_mode {
            anim_sim_editor::edit_binding(self, i, |b| b.play_mode = mode);
        }
        if let Some((i, scope)) = actions.binding_scope {
            anim_sim_editor::edit_binding(self, i, |b| b.scope = scope);
        }
    }

    fn redraw_object_preview(&mut self) {
        let (win_w, win_h) = self.win_size();

        let (Some(renderer), Some(surface), Some(overlay), Some(ui)) = (
            self.renderer.as_mut(),
            self.surface.as_ref(),
            self.overlay.as_mut(),
            self.ui.as_mut(),
        ) else {
            return;
        };

        let now = Instant::now();
        self.last_tick = now;

        let game_dir = self.runtime.game_dir().to_path_buf();
        let object_id = self.object_preview.as_ref().map(|s| s.object_id.clone());
        let object_mesh_path = object_id
            .as_ref()
            .and_then(|id| self.runtime.scene().find_object(id))
            .and_then(|o| o.mesh.as_ref().map(|m| m.path.clone()));
        if let Some(path) = &object_mesh_path {
            if !self.mesh_cache.contains_key(path) {
                let full_path = game_dir.join(path);
                match GltfMesh::load(
                    &renderer.device,
                    &renderer.queue,
                    renderer.mesh_texture_layout(),
                    &full_path,
                ) {
                    Ok(mesh) => {
                        let model_uniform = renderer.create_model_uniform();
                        self.mesh_cache.insert(path.clone(), (mesh, model_uniform));
                    }
                    Err(e) => {
                        log::warn!("space_soup_editor: object preview failed to load '{path}': {e}")
                    }
                }
            }
        }

        let Some(state) = self.object_preview.as_ref() else {
            return;
        };
        if self.runtime.scene().find_object(&state.object_id).is_none() {
            object_preview::close(self);
            return;
        }
        self.camera.position = state.orbit.eye_position();
        self.camera.rotation = state.orbit.rotation();

        object_preview::update_transforms(state, self.runtime.scene(), &mut self.mesh_cache);
        let mut cuboids = object_preview::collect_cuboid(state, self.runtime.scene());
        cuboids.extend(object_preview::collect_skeleton_cuboids(
            state,
            self.runtime.scene(),
            &self.mesh_cache,
        ));
        let mut mesh_instances =
            object_preview::collect_mesh_instances(state, self.runtime.scene(), &self.mesh_cache);

        let object_id = state.object_id.clone();
        mesh_instances.extend(scene::sync_gizmo_and_collect(
            &mut self.xform_gizmo,
            &mut self.gizmo_assets,
            &self.camera,
            (win_w, win_h),
            &self.runtime.scene().objects,
            Some(&object_id),
            ViewMode::Edit,
            false,
            self.gizmo_dragging,
        ));

        let frame = match surface.get_current_texture() {
            Ok(f) => f,
            Err(e) => {
                log::warn!("surface error: {e}");
                return;
            }
        };
        let view = frame.texture.create_view(&TextureViewDescriptor::default());

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
            dt: 0.0,
        };
        ui.begin_frame(win_w, win_h, &ui_input);

        let theme: Theme = ui.theme;
        let layout = Layout::new(win_w, win_h, &theme);
        ui.card_border(layout.grab_pose_viewport());

        let Some(state) = self.object_preview.as_mut() else {
            return;
        };
        let (done_clicked, new_mode) =
            draw_preview_header(ui, &theme, &layout, state, self.xform_gizmo.mode);
        if let Some(mode) = new_mode {
            self.xform_gizmo.mode = mode;
        }

        let mut scene_dirty = self.scene_dirty;
        let game_dir_for_panel = self.runtime.game_dir().to_path_buf();
        let Some(state) = self.object_preview.as_mut() else {
            return;
        };
        let panel_actions = object_preview_panel::draw(
            ui,
            &theme,
            &layout,
            state,
            self.runtime.scene_mut(),
            &game_dir_for_panel,
            &mut scene_dirty,
        );
        self.scene_dirty = scene_dirty;
        let current_id = self
            .object_preview
            .as_ref()
            .map(|s| s.object_id.clone())
            .unwrap_or(object_id);
        self.selected_object = Some(current_id.clone());

        if panel_actions.open_script_editor {
            let text = crate::app::scene_bridge::object_script(self.runtime.scene(), &current_id);
            self.editor.set_text(&text);
            self.editing = Some(EditTarget::ObjectScript(current_id.clone()));
            self.editor_focused = true;
        }
        if panel_actions.deleted {
            self.runtime
                .scene_mut()
                .objects
                .retain(|o| o.id != current_id);
            self.selected_object = None;
            self.scene_dirty = true;
        }

        overlay.set_items(ui.finish());

        let mut encoder = renderer
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("overlay_enc"),
            });
        overlay.render(&renderer.device, &renderer.queue, &mut encoder, &view);
        renderer.queue.submit(Some(encoder.finish()));
        frame.present();

        self.mouse_held = if self.left_down {
            vec![agate::AMouseButton::Left]
        } else {
            vec![]
        };

        if done_clicked
            || panel_actions.open_script_editor
            || panel_actions.deleted
            || !self.runtime.scene().objects.iter().any(|o| o.id == current_id)
        {
            object_preview::close(self);
        }
        if panel_actions.open_grab_pose_editor {
            object_preview::close(self);
            grab_pose_editor::open(self, current_id);
        }
    }
}

fn draw_preview_header(
    ui: &mut agate::Ui,
    theme: &Theme,
    layout: &Layout,
    state: &object_preview::ObjectPreviewState,
    current_mode: GizmoMode,
) -> (bool, Option<GizmoMode>) {
    use agate::theme as t;

    let bar = layout.editor_tab;
    ui.fill(bar, t::TOOLBAR_BG);
    ui.separator(bar[0], bar[1] + bar[3] - theme.px(1.0), bar[2]);

    let btn_h = theme.px(24.0);
    let btn_y = bar[1] + (bar[3] - btn_h) * 0.5;
    let mut right_flow = agate::Flow::row_from_right(
        bar[0] + bar[2] - theme.px(super::layout::PAD),
        btn_y,
        btn_h,
        theme.px(6.0),
    );
    let done_r = right_flow.take(theme.px(70.0));
    let scale_r = right_flow.take(theme.px(56.0));
    let rotate_r = right_flow.take(theme.px(56.0));
    let move_r = right_flow.take(theme.px(56.0));

    let title = format!("Preview \u{2014} {}", state.object_id);
    ui.label_styled(
        bar[0] + theme.px(super::layout::PAD),
        bar[1] + (bar[3] - theme.body()) * 0.5,
        &title,
        theme.body(),
        t::TEXT_PRIMARY,
        (move_r[0] - bar[0] - theme.px(super::layout::PAD)).max(0.0),
        Some(bar),
    );

    let mode_rects = [move_r, rotate_r, scale_r];
    let modes = [GizmoMode::Translate, GizmoMode::Rotate, GizmoMode::Scale];
    let labels = ["Move", "Rotate", "Scale"];
    let mut clicked_mode = None;
    for i in 0..3 {
        let active = current_mode == modes[i];
        let (bg, fg) = if active {
            (t::ACCENT, t::TEXT_ON_ACCENT)
        } else {
            (t::CONTROL_BG, t::TEXT_SECONDARY)
        };
        if ui.button_styled(mode_rects[i], labels[i], bg, fg) {
            clicked_mode = Some(modes[i]);
        }
    }

    let done_clicked = ui.button_secondary(done_r, "Done");
    (done_clicked, clicked_mode)
}

fn draw_editor_tab(
    ui: &mut agate::Ui,
    theme: &Theme,
    layout: &Layout,
    editor: &mut agate::TextEditor,
    editor_focused: &mut bool,
    title_override: &str,
) {
    use agate::theme as t;

    ui.fill(layout.editor_tab, t::TOOLBAR_BG);
    ui.separator(
        layout.editor_tab[0],
        layout.editor_tab[1] + layout.editor_tab[3] - theme.px(1.0),
        layout.editor_tab[2],
    );
    let dot = if editor.dirty { "\u{25cf}  " } else { "" };
    let title = format!("{dot}{title_override}");
    ui.label_styled(
        layout.editor_tab[0] + theme.px(super::layout::PAD),
        layout.editor_tab[1] + (layout.editor_tab[3] - theme.body()) * 0.5,
        &title,
        theme.body(),
        t::TEXT_PRIMARY,
        layout.editor_tab[2],
        Some(layout.editor_tab),
    );

    let focused = *editor_focused;
    let er = layout.editor_body;
    let clicked = ui.text_editor(er, editor, focused);
    if clicked {
        *editor_focused = true;
    }
}
