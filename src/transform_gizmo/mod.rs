//! Unity-style transform gizmo: translate (arrows + plane handles), rotate
//! (rings), and scale (cube handles) — built from real cone/cylinder/torus/
//! cube geometry, not the engine's axis-aligned `Cuboid` primitive.
//!
//! ## Why this looks unusual
//! `space_soup`'s public API has exactly two ways to put a 3D shape on
//! screen: `Cuboid` (boxes only) and `GltfMesh::load(path)` (reads an actual
//! .gltf/.glb file off disk). There is no "give me a raw triangle list"
//! entry point. To get true cone/cylinder/torus geometry without touching
//! the renderer crate's internals, this module:
//!   1. Generates the gizmo's geometry procedurally (`geometry.rs`).
//!   2. Encodes each part as a minimal, valid, in-memory GLB (`glb.rs`).
//!   3. Writes that GLB to a small on-disk cache and loads it back via the
//!      existing `GltfMesh::load`, exactly like any other model asset
//!      (`assets.rs`).
//!
//! ## Data flow fix (rotate/scale used to be dead ends)
//! `drag()` (in `drag.rs`) only ever mutated this struct's own
//! `rotation`/`scale` fields — nothing read them back onto the actual
//! `PlacedObject` being edited, and the host app's per-frame sync never set
//! them in the other direction either, so a Rotate/Scale drag looked like
//! it did nothing. That's now fixed on the *caller* side
//! (`app::render::scene::sync_gizmo_and_collect` syncs in, `app::input::mouse`
//! writes back out); this module's job is unchanged — it just needed the
//! caller to actually use `get_rotation()`/`get_scale()`/`set_rotation()`/
//! `set_scale()`, which already existed.
//!
//! ## Known gaps (require renderer-crate changes not available here)
//! - **Always-on-top**: gizmo meshes go through the normal depth-tested mesh
//!   pipeline, so scene geometry in front of the gizmo will occlude it.
//! - **Orthographic cameras**: `Camera` in this engine is perspective-only
//!   today. `screen_scale()` (`render.rs`) is written so swapping in an
//!   orthographic branch later is a one-line change, but there's nothing to
//!   branch on yet.

mod assets;
mod colors;
mod drag;
mod geometry;
mod glb;
mod math;
mod picking;
mod render;

pub(crate) use assets::GizmoAssets;

use glam::{Quat, Vec2, Vec3};

use self::colors::ColorState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum GizmoMode {
    Translate,
    Rotate,
    Scale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum GizmoSpace {
    Local,
    World,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Axis {
    X,
    Y,
    Z,
    XY,
    XZ,
    YZ,
    XYZ,
}

enum DragState {
    Translate { anchor_offset: Vec3 },
    Rotate { axis: Axis, start_rot: Quat, start_angle: f32, plane_u: Vec3, plane_v: Vec3 },
    Scale { axis: Axis, start_scale: Vec3, start_mouse: Vec2 },
}

pub(crate) struct TransformGizmo {
    pub mode: GizmoMode,
    pub space: GizmoSpace,
    pub selected_axis: Option<Axis>,
    pub hovered_axis: Option<Axis>,

    position: Vec3,
    rotation: Quat,
    scale: Vec3,

    drag: Option<DragState>,
    current_angle_deg: Option<f32>,
}

impl TransformGizmo {
    pub fn new() -> Self {
        Self {
            mode: GizmoMode::Translate,
            space: GizmoSpace::World,
            selected_axis: None,
            hovered_axis: None,
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
            drag: None,
            current_angle_deg: None,
        }
    }

    // --- Transform operations ------------------------------------

    pub fn set_position(&mut self, p: Vec3) { self.position = p; }
    pub fn set_rotation(&mut self, r: Quat) { self.rotation = r; }
    pub fn set_scale(&mut self, s: Vec3) { self.scale = s; }
    pub fn get_position(&self) -> Vec3 { self.position }
    pub fn get_rotation(&self) -> Quat { self.rotation }
    pub fn get_scale(&self) -> Vec3 { self.scale }

    /// Angle (degrees) accumulated so far in an in-progress rotate drag —
    /// for the UI to display, e.g. in a future on-screen readout.
    #[allow(dead_code)]
    pub fn current_drag_angle_degrees(&self) -> Option<f32> { self.current_angle_deg }

    #[allow(dead_code)]
    pub fn rotate(&mut self, axis: Axis, angle: f32) {
        let dir = match axis {
            Axis::X => self.basis() * Vec3::X,
            Axis::Y => self.basis() * Vec3::Y,
            Axis::Z => self.basis() * Vec3::Z,
            _ => return,
        };
        self.rotation = Quat::from_axis_angle(dir, angle) * self.rotation;
    }

    #[allow(dead_code)]
    pub fn scale_axis(&mut self, axis: Axis, amount: f32) {
        match axis {
            Axis::X => self.scale.x *= amount,
            Axis::Y => self.scale.y *= amount,
            Axis::Z => self.scale.z *= amount,
            Axis::XYZ => self.scale *= amount,
            _ => {}
        }
    }

    // --- Space helpers -------------------------------------------------------

    /// World-space basis the handles are drawn/dragged along: the object's
    /// own rotation in Local space, or identity in World space.
    fn basis(&self) -> Quat {
        match self.space {
            GizmoSpace::World => Quat::IDENTITY,
            GizmoSpace::Local => self.rotation,
        }
    }

    fn axis_dir(&self, axis: Axis) -> Vec3 {
        match axis {
            Axis::X => self.basis() * Vec3::X,
            Axis::Y => self.basis() * Vec3::Y,
            Axis::Z => self.basis() * Vec3::Z,
            _ => Vec3::ZERO,
        }
    }

    fn state_for(&self, axis: Axis) -> ColorState {
        if self.selected_axis == Some(axis) {
            ColorState::Selected
        } else if self.hovered_axis == Some(axis) {
            ColorState::Hover
        } else {
            ColorState::Normal
        }
    }
}

impl Default for TransformGizmo {
    fn default() -> Self { Self::new() }
}
