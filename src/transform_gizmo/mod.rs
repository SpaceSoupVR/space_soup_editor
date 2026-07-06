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
    // Not currently wired to any UI toggle (the grab-pose editor's was removed), but kept for a
    // possible future re-enable rather than dropping the space concept from the gizmo entirely.
    #[allow(dead_code)]
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
    Translate {
        anchor_offset: Vec3,
    },
    Rotate {
        axis: Axis,
        start_rot: Quat,
        start_angle: f32,
        plane_u: Vec3,
        plane_v: Vec3,
    },
    Scale {
        axis: Axis,
        start_scale: Vec3,
        start_mouse: Vec2,
    },
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

    pub fn set_position(&mut self, p: Vec3) {
        self.position = p;
    }
    pub fn set_rotation(&mut self, r: Quat) {
        self.rotation = r;
    }
    pub fn set_scale(&mut self, s: Vec3) {
        self.scale = s;
    }
    pub fn get_position(&self) -> Vec3 {
        self.position
    }
    pub fn get_rotation(&self) -> Quat {
        self.rotation
    }
    pub fn get_scale(&self) -> Vec3 {
        self.scale
    }

    #[allow(dead_code)]
    pub fn current_drag_angle_degrees(&self) -> Option<f32> {
        self.current_angle_deg
    }

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
    fn default() -> Self {
        Self::new()
    }
}
