use agate::theme as t;
use agate::{Theme, Ui};

use super::super::layout::{Layout, PAD};
use super::super::EditTarget;

pub(crate) fn draw(
    ui: &mut Ui,
    theme: &Theme,
    layout: &Layout,
    editing: &mut Option<EditTarget>,
    selected_file: Option<usize>,
    scene_name: &str,
    scene_dirty: bool,
) {
    super::header_backdrop(ui, theme, layout);

    // Scene identity on the left, like a document title.
    let title = if scene_dirty {
        format!("{scene_name} \u{2022} edited")
    } else {
        scene_name.to_string()
    };
    let ty = layout.toolbar[1] + (layout.toolbar[3] - theme.body()) * 0.5;
    ui.label_styled(
        layout.toolbar[0] + theme.px(PAD + 4.0),
        ty,
        &title,
        theme.body(),
        t::TEXT_SECONDARY,
        (layout.btn_save_scene[0] - layout.toolbar[0] - theme.px(PAD * 2.0)).max(0.0),
        Some(layout.toolbar),
    );

    let is_file_editor = matches!(editing, Some(EditTarget::SceneFile));

    let (ed_bg, ed_fg) = if is_file_editor {
        (t::ACCENT, t::TEXT_ON_ACCENT)
    } else {
        (t::CONTROL_BG, t::TEXT_PRIMARY)
    };
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

pub(crate) fn draw_save(
    ui: &mut Ui,
    theme: &Theme,
    layout: &Layout,
    editing: &Option<EditTarget>,
    dirty: bool,
) -> bool {
    let _ = theme;
    let save_en = matches!(editing, Some(EditTarget::SceneFile)) && dirty;
    if save_en {
        ui.button_styled(layout.btn_save, "Save", t::ACCENT, t::TEXT_ON_ACCENT)
    } else {
        ui.button_disabled(layout.btn_save, "Save", t::CONTROL_BG, t::TEXT_DISABLED);
        false
    }
}

pub(crate) fn draw_save_scene(
    ui: &mut Ui,
    theme: &Theme,
    layout: &Layout,
    scene_dirty: bool,
) -> bool {
    let _ = theme;
    if scene_dirty {
        ui.button_styled(layout.btn_save_scene, "Save Scene", t::ACCENT, t::TEXT_ON_ACCENT)
    } else {
        ui.button_disabled(
            layout.btn_save_scene,
            "Save Scene",
            t::CONTROL_BG,
            t::TEXT_DISABLED,
        );
        false
    }
}
