use agate::theme as t;
use agate::{Theme, Ui};
use agate::TextEditor;

use super::super::edit_camera::EditCamera;
use super::super::layout::Layout;
use super::super::{EditTarget, ViewMode};

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw(
    ui: &mut Ui,
    theme: &Theme,
    layout: &Layout,
    view_mode: &mut ViewMode,
    edit_camera: &mut EditCamera,
    last_world_head: glam::Vec3,
    editing: &mut Option<EditTarget>,
    selected_file: Option<usize>,
    _editor: &TextEditor,
) {
    ui.fill(layout.toolbar, t::TITLEBAR_BG);
    ui.separator(0.0, layout.toolbar[1] + layout.toolbar[3] - theme.px(1.0), layout.toolbar[2]);

    let is_file_editor = matches!(editing, Some(EditTarget::SceneFile));

    let seg_labels = ["Player", "First Person", "Edit"];
    let seg_modes = [ViewMode::PlayerView, ViewMode::FirstPerson, ViewMode::Edit];
    for i in 0..3 {
        let active = *view_mode == seg_modes[i] && editing.is_none();
        let (bg, fg) = if active { (t::ACCENT, t::TEXT_ON_ACCENT) } else { (t::CONTROL_BG, t::TEXT_PRIMARY) };
        if ui.button_styled(layout.seg[i], seg_labels[i], bg, fg) {
            *view_mode = seg_modes[i];
            if *view_mode == ViewMode::Edit {
                *edit_camera = EditCamera::new(last_world_head);
            }
            *editing = None;
        }
    }

    let (ed_bg, ed_fg) = if is_file_editor { (t::ACCENT, t::TEXT_ON_ACCENT) } else { (t::CONTROL_BG, t::TEXT_PRIMARY) };
    if ui.button_styled(layout.btn_editor, "Editor", ed_bg, ed_fg) {
        *editing = if is_file_editor {
            None
        } else if selected_file.is_some() {
            Some(EditTarget::SceneFile)
        } else {
            None
        };
    }
}

/// Returns true if the file-editor Save was clicked and should be acted on.
pub(crate) fn draw_save(ui: &mut Ui, theme: &Theme, layout: &Layout, editing: &Option<EditTarget>, dirty: bool) -> bool {
    let _ = theme;
    let save_en = matches!(editing, Some(EditTarget::SceneFile)) && dirty;
    let save_bg = if save_en { t::CONTROL_HOVER } else { t::CONTROL_BG };
    let save_fg = if save_en { t::TEXT_PRIMARY } else { t::TEXT_SECONDARY };
    ui.button_styled(layout.btn_save, "Save", save_bg, save_fg) && save_en
}

/// Returns true if "Save Scene" was clicked and should be acted on. Enabled
/// whenever the live scene has unsaved edits, independent of which editor
/// panel (if any) is currently open — you can save scene edits without the
/// file or script editor being open at all.
pub(crate) fn draw_save_scene(ui: &mut Ui, theme: &Theme, layout: &Layout, scene_dirty: bool) -> bool {
    let _ = theme;
    let bg = if scene_dirty { t::SUCCESS } else { t::CONTROL_BG };
    let fg = if scene_dirty { t::TEXT_ON_ACCENT } else { t::TEXT_SECONDARY };
    ui.button_styled(layout.btn_save_scene, "Save Scene", bg, fg) && scene_dirty
}