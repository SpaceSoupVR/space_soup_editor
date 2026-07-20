use agate::theme as t;
use agate::{Theme, Ui};

use super::super::edit_camera::EditCamera;
use super::super::layout::Layout;
use super::super::{EditTarget, RibbonTab, ViewMode};

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw(
    ui: &mut Ui,
    theme: &Theme,
    layout: &Layout,
    view_mode: &mut ViewMode,
    edit_camera: &mut EditCamera,
    last_world_head: glam::Vec3,
    editing: &mut Option<EditTarget>,
    ribbon_tab: &mut RibbonTab,
    has_selection: bool,
) {
    ui.panel_bordered_flat(layout.titlebar, t::TITLEBAR_BG);

    ui.panel(layout.seg_pill, t::CONTROL_ACTIVE);

    let seg_labels = ["Player", "First Person", "Render View", "Edit"];
    let seg_modes = [
        ViewMode::PlayerView,
        ViewMode::FirstPerson,
        ViewMode::RenderView,
        ViewMode::Edit,
    ];
    for i in 0..4 {
        let active = *view_mode == seg_modes[i] && editing.is_none();
        let (bg, fg) = if active {
            (t::CONTROL_BG, t::TEXT_PRIMARY)
        } else {
            (t::CONTROL_ACTIVE, t::TEXT_SECONDARY)
        };
        if ui.button_styled(layout.seg[i], seg_labels[i], bg, fg) {
            *view_mode = seg_modes[i];
            if *view_mode == ViewMode::Edit {
                *edit_camera = EditCamera::new(last_world_head);
            }
            *editing = None;
        }
    }

    for i in 0..3 {
        let seg = layout.seg[i];
        let inset = theme.px(6.0);
        ui.separator_v(seg[0] + seg[2], seg[1] + inset, seg[3] - inset * 2.0);
    }

    // Ribbon tab selector — which page the ribbon row below shows. The
    // Object tab stays clickable with nothing selected (the ribbon explains
    // itself there), but its label dims as a hint.
    let rtab_labels = ["Build", "Insert", "Object"];
    let rtabs = [RibbonTab::Build, RibbonTab::Insert, RibbonTab::Object];
    for i in 0..3 {
        let active = *ribbon_tab == rtabs[i];
        let dim = rtabs[i] == RibbonTab::Object && !has_selection;
        let (bg, fg) = if active {
            (t::ACCENT, t::TEXT_ON_ACCENT)
        } else if dim {
            (t::TITLEBAR_BG, t::TEXT_SECONDARY)
        } else {
            (t::TITLEBAR_BG, t::TEXT_PRIMARY)
        };
        if ui.button_styled(layout.ribbon_tabs[i], rtab_labels[i], bg, fg) {
            *ribbon_tab = rtabs[i];
        }
    }
}

/// Undo/redo pair for inspector edits. Returns `(undo_clicked, redo_clicked)`.
/// Each button dims when its stack is empty, matching the Save buttons' style.
pub(crate) fn draw_undo_redo(
    ui: &mut Ui,
    theme: &Theme,
    layout: &Layout,
    can_undo: bool,
    can_redo: bool,
) -> (bool, bool) {
    let _ = theme;
    let style = |enabled: bool| {
        if enabled {
            (t::CONTROL_BG, t::TEXT_PRIMARY)
        } else {
            (t::CONTROL_BG, t::TEXT_SECONDARY)
        }
    };
    let (u_bg, u_fg) = style(can_undo);
    let (r_bg, r_fg) = style(can_redo);
    let undo = ui.button_styled(layout.btn_undo, "Undo", u_bg, u_fg) && can_undo;
    let redo = ui.button_styled(layout.btn_redo, "Redo", r_bg, r_fg) && can_redo;
    (undo, redo)
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
    let save_bg = if save_en {
        t::CONTROL_HOVER
    } else {
        t::CONTROL_BG
    };
    let save_fg = if save_en {
        t::TEXT_PRIMARY
    } else {
        t::TEXT_SECONDARY
    };
    ui.button_styled(layout.btn_save, "Save", save_bg, save_fg) && save_en
}

pub(crate) fn draw_save_scene(
    ui: &mut Ui,
    theme: &Theme,
    layout: &Layout,
    scene_dirty: bool,
) -> bool {
    let _ = theme;
    let bg = if scene_dirty {
        t::SUCCESS
    } else {
        t::CONTROL_BG
    };
    let fg = if scene_dirty {
        t::TEXT_ON_ACCENT
    } else {
        t::TEXT_SECONDARY
    };
    ui.button_styled(layout.btn_save_scene, "Save Scene", bg, fg) && scene_dirty
}
