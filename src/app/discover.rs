use std::path::{Path, PathBuf};

use winit::keyboard::{Key, NamedKey};

use space_soup::ui2d::Font;

pub(crate) fn game_dir() -> PathBuf {
    PathBuf::from("../game")
}

pub(crate) fn discover_json(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut push = |p: PathBuf| {
        if p.extension().is_some_and(|e| e == "json") {
            out.push(p);
        }
    };
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_file() {
                push(p);
            } else if p.is_dir() {
                if let Ok(rd2) = std::fs::read_dir(&p) {
                    for e2 in rd2.flatten() {
                        push(e2.path());
                    }
                }
            }
        }
    }
    out.sort();
    out
}

pub(crate) fn discover_models(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let models_dir = dir.join("models");
    if let Ok(rd) = std::fs::read_dir(&models_dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_file() {
                if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                    if ext.eq_ignore_ascii_case("glb") || ext.eq_ignore_ascii_case("gltf") {
                        out.push(p);
                    }
                }
            }
        }
    }
    out.sort();
    out
}

pub(crate) fn load_font() -> Font {
    const PATH: &str = "font.ttf";
    match std::fs::read(PATH) {
        Ok(bytes) => Font::new(&bytes),
        Err(e) => panic!("space_soup_editor: could not read '{PATH}': {e}"),
    }
}

pub(crate) fn winit_key_to_agate(key: &Key) -> Option<agate::input::NamedKey> {
    use agate::input::NamedKey as A;
    match key {
        Key::Named(NamedKey::ArrowLeft) => Some(A::ArrowLeft),
        Key::Named(NamedKey::ArrowRight) => Some(A::ArrowRight),
        Key::Named(NamedKey::ArrowUp) => Some(A::ArrowUp),
        Key::Named(NamedKey::ArrowDown) => Some(A::ArrowDown),
        Key::Named(NamedKey::Home) => Some(A::Home),
        Key::Named(NamedKey::End) => Some(A::End),
        Key::Named(NamedKey::PageUp) => Some(A::PageUp),
        Key::Named(NamedKey::PageDown) => Some(A::PageDown),
        Key::Named(NamedKey::Backspace) => Some(A::Backspace),
        Key::Named(NamedKey::Delete) => Some(A::Delete),
        Key::Named(NamedKey::Enter) => Some(A::Enter),
        Key::Named(NamedKey::Tab) => Some(A::Tab),
        Key::Named(NamedKey::Escape) => Some(A::Escape),
        _ => None,
    }
}
