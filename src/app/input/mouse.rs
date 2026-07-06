use glam::{Vec2, Vec3};
use winit::dpi::PhysicalPosition;
use winit::event::ElementState;

use agate::{AMouseButton, Theme};

use crate::transform_gizmo::{Axis, GizmoMode};

use super::super::grab_pose_editor;
use super::super::layout::{in_rect, Layout};
use super::super::picking::ray_plane_intersect;
use super::super::render::scene::GIZMO_ANCHOR_MARGIN;
use super::super::scene_bridge::{mesh_base_half_size, new_object, unique_id};
use super::super::snap;
use super::super::{App, EditorTool, GizmoPart, ViewMode};

fn has_gizmo_target(app: &App) -> bool {
    match app.tool {
        EditorTool::Snap => app.snap_selected_joint.is_some(),
        _ => app.selected_object.is_some(),
    }
}

pub(crate) fn cursor_moved(app: &mut App, position: PhysicalPosition<f64>) {
    let new = (position.x as f32, position.y as f32);
    let dx = new.0 - app.last_mouse_pos.0;
    let dy = new.1 - app.last_mouse_pos.1;
    app.last_mouse_pos = new;
    app.mouse_pos = new;

    if app.grab_pose_editor.is_some() {
        grab_pose_cursor_moved(app, new, dx, dy);
        return;
    }

    let mouse_vec2 = Vec2::new(new.0, new.1);
    let viewport = app.win_size();

    let (win_w, win_h) = viewport;
    let theme = Theme::new(app.scale);
    let layout = Layout::new(win_w, win_h, &theme);
    let over_viewport = in_rect(new, layout.center)
        && !in_rect(new, layout.navigator)
        && !in_rect(new, layout.inspector);

    if app.view_mode == ViewMode::Edit
        && app.editing.is_none()
        && has_gizmo_target(app)
        && !app.gizmo_dragging
        && over_viewport
    {
        app.xform_gizmo.hovered_axis =
            app.xform_gizmo
                .raycast_gizmo(mouse_vec2, &app.camera, viewport);
    } else {
        app.xform_gizmo.hovered_axis = None;
    }

    if app.left_down && app.view_mode == ViewMode::Edit && app.editing.is_none() {
        if app.gizmo_dragging {
            app.xform_gizmo.drag(mouse_vec2, &app.camera, viewport);
            if app.tool == EditorTool::Snap {
                snap::apply_gizmo_drag_to_joint(app);
            } else {
                apply_gizmo_drag_to_selected_object(app);
            }
            app.dragged = true;
        } else if let Some(part) = app.gizmo_drag {
            match part {
                GizmoPart::Orbit => app.edit_camera.look(dx, dy),
                GizmoPart::Pan => app.edit_camera.pan(dx, dy),
                GizmoPart::Zoom => app.edit_camera.dolly(-dy * 0.004),
            }
            app.dragged = true;
        } else if app.moving_object {
            if let Some(id) = app.selected_object.clone() {
                let plane_y = app
                    .runtime
                    .scene()
                    .find_object(&id)
                    .map(|o| o.cuboid.position.y)
                    .unwrap_or(0.0);
                let (o, d) = app.screen_ray(new.0, new.1, win_w, win_h);
                if let Some(hit) = ray_plane_intersect(o, d, plane_y) {
                    let target = hit + app.move_anchor_offset;
                    if let Some(obj) = app.runtime.scene_mut().find_object_mut(&id) {
                        obj.cuboid.position.x = target.x;
                        obj.cuboid.position.z = target.z;
                        app.scene_dirty = true;
                    }
                }
                app.dragged = true;
            }
        } else if !app.press_in_chrome && (dx.abs() > 0.5 || dy.abs() > 0.5) {
            app.dragged = true;
            app.edit_camera.look(dx, dy);
        }
    }

    if app.dragging_new_model.is_some() {
        let (o, d) = app.screen_ray(new.0, new.1, win_w, win_h);
        let in_center = over_viewport && new.1 < layout.model_tray(&theme)[1];
        app.ghost_preview = if in_center {
            ray_plane_intersect(o, d, 0.0)
        } else {
            None
        };
    }

    app.redraw_now();
}

fn apply_gizmo_drag_to_selected_object(app: &mut App) {
    let Some(id) = app.selected_object.clone() else {
        return;
    };
    let mode = app.xform_gizmo.mode;
    let gizmo_pos = app.xform_gizmo.get_position();
    let gizmo_rot = app.xform_gizmo.get_rotation();
    let gizmo_scale = app.xform_gizmo.get_scale();

    let mesh_path = app
        .runtime
        .scene()
        .find_object(&id)
        .and_then(|o| o.mesh.as_ref().map(|m| m.path.clone()));

    let base_half: Option<Vec3> = match &mesh_path {
        Some(path) => {
            if let Some(&cached) = app.mesh_base_half_size.get(path) {
                Some(cached)
            } else if let Some((gltf, _)) = app.mesh_cache.get(path) {
                let computed = mesh_base_half_size(gltf);
                app.mesh_base_half_size.insert(path.clone(), computed);
                Some(computed)
            } else {
                None
            }
        }
        None => None,
    };

    let Some(obj) = app.runtime.scene_mut().find_object_mut(&id) else {
        return;
    };
    match mode {
        GizmoMode::Translate => {
            let clearance = obj.cuboid.half_size.y + GIZMO_ANCHOR_MARGIN;
            obj.cuboid.position = gizmo_pos - Vec3::new(0.0, clearance, 0.0);
        }
        GizmoMode::Rotate => {
            obj.cuboid.rotation = gizmo_rot;
        }
        GizmoMode::Scale => {
            let new_half = Vec3::new(
                gizmo_scale.x.max(0.005),
                gizmo_scale.y.max(0.005),
                gizmo_scale.z.max(0.005),
            );
            obj.cuboid.half_size = new_half;

            if let (Some(mesh), Some(base)) = (obj.mesh.as_mut(), base_half) {
                mesh.scale = Vec3::new(
                    new_half.x / base.x.max(0.0001),
                    new_half.y / base.y.max(0.0001),
                    new_half.z / base.z.max(0.0001),
                );
            }
        }
    }
    app.scene_dirty = true;
}

fn grab_pose_cursor_moved(app: &mut App, _new: (f32, f32), dx: f32, dy: f32) {
    let preview_mode = app
        .grab_pose_editor
        .as_ref()
        .map(|s| s.preview_mode)
        .unwrap_or(false);

    if app.left_down {
        if preview_mode {
            if !app.press_in_chrome {
                grab_pose_editor::preview_drag(app, dx, dy);
                app.dragged = true;
            }
        } else if !app.press_in_chrome && (dx.abs() > 0.5 || dy.abs() > 0.5) {
            app.dragged = true;
            if let Some(state) = app.grab_pose_editor.as_mut() {
                state.orbit.orbit(dx, dy);
            }
        }
    }

    app.redraw_now();
}

fn grab_pose_left_button(app: &mut App, state: ElementState) {
    let (win_w, win_h) = app.win_size();
    let theme = Theme::new(app.scale);
    let layout = Layout::new(win_w, win_h, &theme);
    let mp = app.mouse_pos;

    match state {
        ElementState::Pressed => {
            app.left_down = true;
            app.dragged = false;
            app.gizmo_drag = None;
            app.gizmo_dragging = false;
            app.mouse_pressed.push(AMouseButton::Left);
            app.mouse_held.push(AMouseButton::Left);

            app.press_in_chrome = !in_rect(mp, layout.grab_pose_viewport())
                || in_rect(mp, layout.inspector)
                || in_rect(mp, layout.editor_tab);
        }
        ElementState::Released => {
            app.left_down = false;
            app.mouse_released.push(AMouseButton::Left);
            app.dragged = false;
        }
    }
    app.redraw_now();
}

pub(crate) fn left_button(app: &mut App, state: ElementState) {
    if app.grab_pose_editor.is_some() {
        grab_pose_left_button(app, state);
        return;
    }

    let (win_w, win_h) = app.win_size();
    let theme = Theme::new(app.scale);
    let layout = Layout::new(win_w, win_h, &theme);
    let mp = app.mouse_pos;

    match state {
        ElementState::Pressed => {
            app.left_down = true;
            app.dragged = false;
            app.moving_object = false;
            app.gizmo_drag = None;
            app.gizmo_dragging = false;
            app.mouse_pressed.push(AMouseButton::Left);
            app.mouse_held.push(AMouseButton::Left);
            app.editor_focused = app.editing.is_some() && in_rect(mp, layout.editor_body);

            app.press_in_chrome = !in_rect(mp, layout.center)
                || in_rect(mp, layout.navigator)
                || in_rect(mp, layout.inspector);

            if app.view_mode == ViewMode::Edit && app.editing.is_none() {
                let mp2 = Vec2::new(mp.0, mp.1);

                let model_list_area = layout.model_list_area(&theme);
                let model_rects =
                    layout.model_rects(&theme, app.available_models.len(), app.model_scroll_y);
                let clicked_chip = model_rects
                    .iter()
                    .position(|r| in_rect(mp, *r) && in_rect(mp, model_list_area));

                if let Some(i) = clicked_chip {
                    app.dragging_new_model = Some(app.available_models[i].clone());
                    app.dragged = true;
                } else if app.dragging_new_model.is_some() && !app.press_in_chrome {
                    if let Some(path) = app.dragging_new_model.take() {
                        if let Some(pos) = app.ghost_preview.take() {
                            place_dropped_model(app, &path, pos);
                        } else {
                            app.ghost_preview = None;
                        }
                    }
                    app.dragged = true;
                } else {
                    let xform_hit = if !app.press_in_chrome && has_gizmo_target(app) {
                        app.xform_gizmo
                            .raycast_gizmo(mp2, &app.camera, (win_w, win_h))
                    } else {
                        None
                    };

                    let marker_hit = if app.tool == EditorTool::Snap
                        && !app.press_in_chrome
                        && xform_hit.is_none()
                    {
                        let (o, d) = app.screen_ray(mp.0, mp.1, win_w, win_h);
                        snap::pick_joint_marker(&app.snap_joint_frame, o, d)
                    } else {
                        None
                    };

                    let gizmo = layout.gizmo_rects(&theme);
                    let gizmo_parts = [GizmoPart::Orbit, GizmoPart::Pan, GizmoPart::Zoom];
                    if let Some(axis) = xform_hit {
                        app.xform_gizmo
                            .begin_drag(axis, mp2, &app.camera, (win_w, win_h));
                        app.gizmo_dragging = true;
                        app.dragged = true;
                    } else if let Some(idx) = marker_hit {
                        app.snap_selected_joint = Some(idx);
                        if let Some(joint) = app.snap_joint_frame.get(idx) {
                            app.xform_gizmo.set_position(joint.current_pos);

                            app.xform_gizmo
                                .begin_drag(Axis::XYZ, mp2, &app.camera, (win_w, win_h));
                            app.gizmo_dragging = true;
                        }
                        app.dragged = true;
                    } else if let Some((i, _)) =
                        gizmo.iter().enumerate().find(|(_, r)| in_rect(mp, **r))
                    {
                        app.gizmo_drag = Some(gizmo_parts[i]);
                        app.dragged = true;
                    } else if !app.press_in_chrome {
                        match app.tool {
                            EditorTool::Rigging => {
                                if let Some(id) = app.pick_object(mp.0, mp.1, win_w, win_h) {
                                    if let Some(pos) =
                                        app.rig_selection.iter().position(|s| s == &id)
                                    {
                                        app.rig_selection.remove(pos);
                                    } else {
                                        app.rig_selection.push(id);
                                        if app.rig_selection.len() == 2 {
                                            let object = app.rig_selection[0].clone();
                                            let hand = app.rig_selection[1].clone();
                                            snap::seed_grip_pose(
                                                app.runtime.scene_mut(),
                                                &object,
                                                app.snap_hand,
                                                Some(&hand),
                                            );
                                            app.scene_dirty = true;
                                            app.rig_selection.clear();
                                            app.selected_object = Some(object);
                                        }
                                    }
                                    app.dragged = true;
                                }
                            }
                            EditorTool::Snap => {
                                app.selected_object = app.pick_object(mp.0, mp.1, win_w, win_h);
                                app.snap_selected_joint = None;
                                app.dragged = true;
                            }
                            EditorTool::Select => {
                                if let Some(id) = app.pick_object(mp.0, mp.1, win_w, win_h) {
                                    let now = std::time::Instant::now();
                                    let is_double_click = app.last_clicked_object.as_deref()
                                        == Some(id.as_str())
                                        && app
                                            .last_click_time
                                            .map(|t| now.duration_since(t).as_millis() < 400)
                                            .unwrap_or(false);

                                    if is_double_click {
                                        let plane_y = app
                                            .runtime
                                            .scene()
                                            .find_object(&id)
                                            .map(|o| o.cuboid.position.y)
                                            .unwrap_or(0.0);
                                        let (o, d) = app.screen_ray(mp.0, mp.1, win_w, win_h);
                                        let offset = ray_plane_intersect(o, d, plane_y)
                                            .and_then(|hit| {
                                                app.runtime
                                                    .scene()
                                                    .find_object(&id)
                                                    .map(|obj| obj.cuboid.position - hit)
                                            })
                                            .unwrap_or(Vec3::ZERO);
                                        app.move_anchor_offset = offset;
                                        app.moving_object = true;
                                    }

                                    app.last_click_time = Some(now);
                                    app.last_clicked_object = Some(id.clone());
                                    app.selected_object = Some(id);
                                } else {
                                    app.last_click_time = None;
                                    app.last_clicked_object = None;
                                }
                            }
                        }
                    }
                }
            }
        }
        ElementState::Released => {
            app.left_down = false;
            app.gizmo_drag = None;
            if app.gizmo_dragging {
                app.xform_gizmo.end_drag();
                app.gizmo_dragging = false;
            }
            app.mouse_released.push(AMouseButton::Left);

            let was_moving = app.moving_object;
            if app.moving_object {
                app.moving_object = false;
            }

            if app.view_mode == ViewMode::Edit
                && app.editing.is_none()
                && app.tool == EditorTool::Select
                && !app.dragged
                && !was_moving
                && in_rect(mp, layout.center)
                && !app.press_in_chrome
            {
                app.selected_object = app.pick_object(mp.0, mp.1, win_w, win_h);
            }
            app.dragged = false;
        }
    }
    app.redraw_now();
}

fn place_dropped_model(app: &mut App, model_path: &std::path::Path, pos: Vec3) {
    let game_dir = app.runtime.game_dir().to_path_buf();
    let rel = model_path.strip_prefix(&game_dir).unwrap_or(model_path);
    let rel_str = rel.to_string_lossy().replace('\\', "/");

    let base = model_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "model".to_string());

    let scene = app.runtime.scene_mut();
    let id = unique_id(scene, &base);
    let half = Vec3::splat(0.25);
    let obj = new_object(
        id.clone(),
        Vec3::new(pos.x, half.y, pos.z),
        half,
        Some(rel_str),
    );
    scene.objects.push(obj);
    app.selected_object = Some(id);
    app.scene_dirty = true;
}
