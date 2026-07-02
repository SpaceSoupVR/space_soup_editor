use agate::theme as t;
use agate::{Theme, Ui, WidgetId};
use space_soup_engine::{DebugPacket, GameObject};

use super::super::layout::{Layout, PAD, ROW_H};
use super::super::nav::{NavGroup, NavRow};
use super::super::EditTarget;

const FOOTER_H: f32 = 90.0;

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw(
    ui: &mut Ui,
    theme: &Theme,
    layout: &Layout,
    nav_rows: &[NavRow],
    files_discovered: &[std::path::PathBuf],
    objects: &[GameObject],
    selected_file: &mut Option<usize>,
    selected_object: &mut Option<String>,
    editing: &mut Option<EditTarget>,
    nav_scenes_open: &mut bool,
    nav_objects_open: &mut bool,
    packet: &DebugPacket,
) -> Option<usize> {
    // Flat Xcode-style sidebar — flush fill, no floating card border
    ui.fill(layout.navigator, t::SIDEBAR_BG);

    let nav = layout.navigator;
    let footer_h = theme.px(FOOTER_H);
    let rows_area = [nav[0], nav[1], nav[2], (nav[3] - footer_h).max(0.0)];
    let footer_y = nav[1] + nav[3] - footer_h;

    let row_h = theme.px(ROW_H);
    let top_pad = theme.px(10.0);
    let inset = theme.px(8.0);
    let content_h = top_pad + nav_rows.len() as f32 * row_h + theme.px(8.0);

    let scroll_id = WidgetId::of("nav_scroll");
    let (_, scroll_y) = ui.scroll_area(scroll_id, rows_area, content_h);

    let is_file_editor = matches!(editing, Some(EditTarget::SceneFile));

    let mut clicked_nav: Option<usize> = None;
    for (i, row) in nav_rows.iter().enumerate() {
        let row_y = rows_area[1] + top_pad + i as f32 * row_h - scroll_y;
        // Cull rows with no overlap at all with the visible area — pure
        // performance, not correctness; clipping below handles correctness.
        if row_y + row_h < rows_area[1] || row_y > rows_area[1] + rows_area[3] {
            continue;
        }
        let r = [rows_area[0] + inset, row_y, rows_area[2] - inset * 2.0, row_h];

        match row {
            NavRow::GroupHeader { group } => {
                let open = match group {
                    NavGroup::Scenes => *nav_scenes_open,
                    NavGroup::Objects => *nav_objects_open,
                };
                let label = match group {
                    NavGroup::Scenes => "Scenes",
                    NavGroup::Objects => "Objects",
                };
                // Flat section header: slightly raised SURFACE strip + disclosure chevron
                ui.fill(r, t::SURFACE);
                let new_open = ui.disclosure(r, open, label);
                match group {
                    NavGroup::Scenes => *nav_scenes_open = new_open,
                    NavGroup::Objects => *nav_objects_open = new_open,
                }
            }
            NavRow::SceneFile { file_index } => {
                let name = files_discovered.get(*file_index)
                    .and_then(|p| p.file_name())
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                let sel = *selected_file == Some(*file_index) && is_file_editor;
                // Indent scene file rows by 20px
                let ir = [r[0] + theme.px(20.0), r[1], r[2] - theme.px(20.0), r[3]];
                if ui.list_row_clipped(ir, &name, sel, Some(rows_area)) {
                    clicked_nav = Some(*file_index);
                }
            }
            NavRow::Object { object_id } => {
                let sel = selected_object.as_deref() == Some(object_id.as_str()) && editing.is_none();
                // Indent object rows by 28px (deeper than scene files)
                let ir = [r[0] + theme.px(28.0), r[1], r[2] - theme.px(28.0), r[3]];
                if ui.list_row_clipped(ir, object_id, sel, Some(rows_area)) {
                    *editing = None;
                    *selected_object = Some(object_id.clone());
                }
            }
            NavRow::EmptyHint { group } => {
                let hint = match group {
                    NavGroup::Scenes => "No .json files found",
                    NavGroup::Objects => "No objects placed",
                };
                ui.label_styled(r[0] + theme.px(28.0), r[1] + (r[3] - theme.small()) * 0.5,
                    hint, theme.small(), t::TEXT_DISABLED, r[2], Some(rows_area));
            }
        }
    }

    ui.end_scroll_area(scroll_id, rows_area, content_h);

    let nx = nav[0];
    let nw = nav[2];
    let clip_nav = [nx, footer_y, nw, footer_h];
    // No separator — flat Xcode style footer flows directly from the list
    ui.label_styled(nx + theme.px(PAD), footer_y + theme.px(8.0),
        "SCENE", theme.small(), t::TEXT_SECONDARY, nw, None);
    let info = format!(
        "{}\nobjects: {}\ncuboids: {}\nmeshes:  {}",
        packet.scene.scene_name, objects.len(),
        packet.scene.render_cuboids, packet.scene.render_meshes,
    );
    ui.label_styled(
        nx + theme.px(PAD), footer_y + theme.px(8.0 + ROW_H),
        &info, theme.small(), t::TEXT_PRIMARY, nw - theme.px(PAD * 2.0),
        Some(clip_nav),
    );

    clicked_nav
}
