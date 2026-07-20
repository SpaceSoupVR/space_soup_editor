//! The few controls that stay anchored to the 3D viewport itself. The tool
//! and gizmo-mode pills and the model tray moved into the ribbon; what's
//! left is the camera-navigation cluster (orbit / pan / zoom), which belongs
//! next to the scene it drives.

use agate::theme as t;
use agate::{OverlapGuard, Theme, Ui};

use super::super::layout::Layout;
use super::super::GizmoPart;

pub(crate) fn draw(ui: &mut Ui, theme: &Theme, layout: &Layout, gizmo_drag: Option<GizmoPart>) {
    // Anchored off the inspector edge rather than split from a shared
    // parent, so the guard is the explicit backstop against overlap.
    let mut guard = OverlapGuard::new();

    let gizmo_nav_rects = layout.gizmo_rects(theme);
    guard.claim_group("viewport.gizmo_buttons", &gizmo_nav_rects);
    let gizmo_nav_parts = [GizmoPart::Orbit, GizmoPart::Pan, GizmoPart::Zoom];
    let gizmo_nav_labels = ["O", "P", "Z"];
    let gizmo_nav_tooltips = [
        "Orbit camera (drag)",
        "Pan camera (drag)",
        "Zoom camera (drag)",
    ];
    for i in 0..3 {
        let active = gizmo_drag == Some(gizmo_nav_parts[i]);
        let (bg, fg) = if active {
            (t::ACCENT, t::TEXT_ON_ACCENT)
        } else {
            (t::CONTROL_BG, t::TEXT_SECONDARY)
        };
        ui.button_styled(gizmo_nav_rects[i], gizmo_nav_labels[i], bg, fg);
        ui.tooltip(gizmo_nav_rects[i], gizmo_nav_tooltips[i]);
    }
}
