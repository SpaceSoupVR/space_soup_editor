use agate::TextEditor;

use super::{App, EditTarget};

#[derive(Clone)]
pub(crate) enum NavRow {
    GroupHeader { group: NavGroup },
    SceneFile { file_index: usize },
    Object { object_id: String },
    EmptyHint { group: NavGroup },
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum NavGroup {
    Scenes,
    Objects,
}

impl App {
    pub(crate) fn nav_rows(&self) -> Vec<NavRow> {
        let mut rows = Vec::new();
        rows.push(NavRow::GroupHeader {
            group: NavGroup::Scenes,
        });
        if self.nav_scenes_open {
            if self.files_discovered.is_empty() {
                rows.push(NavRow::EmptyHint {
                    group: NavGroup::Scenes,
                });
            }
            for i in 0..self.files_discovered.len() {
                rows.push(NavRow::SceneFile { file_index: i });
            }
        }
        rows.push(NavRow::GroupHeader {
            group: NavGroup::Objects,
        });
        if self.nav_objects_open {
            let objects = &self.runtime.scene().objects;
            if objects.is_empty() {
                rows.push(NavRow::EmptyHint {
                    group: NavGroup::Objects,
                });
            }
            for o in objects {
                rows.push(NavRow::Object {
                    object_id: o.id.clone(),
                });
            }
        }
        rows
    }

    pub(crate) fn open_file(&mut self, idx: usize) {
        if let Some(p) = self.files_discovered.get(idx).cloned() {
            match TextEditor::load(&p) {
                Ok(ed) => {
                    self.editor = ed;
                    self.selected_file = Some(idx);
                    self.editing = Some(EditTarget::SceneFile);
                    self.editor_focused = true;
                }
                Err(e) => log::warn!("space_soup_editor: open {}: {e}", p.display()),
            }
        }
    }
}
