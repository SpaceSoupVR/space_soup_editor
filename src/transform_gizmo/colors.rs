use space_soup::renderer::Color3;

use super::Axis;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ColorState {
    Normal,
    Hover,
    Selected,
}

pub(crate) fn axis_base_color(axis: Axis) -> Color3 {
    match axis {
        Axis::X => Color3(225, 40, 40, 255),
        Axis::Y => Color3(80, 220, 60, 255),
        Axis::Z => Color3(50, 120, 230, 255),

        Axis::XY => Color3(50, 120, 230, 160),
        Axis::XZ => Color3(80, 220, 60, 160),
        Axis::YZ => Color3(225, 40, 40, 160),
        Axis::XYZ => Color3(245, 245, 245, 235),
    }
}

pub(crate) fn color_for(axis: Axis, state: ColorState) -> Color3 {
    match state {
        ColorState::Hover | ColorState::Selected => Color3(255, 220, 40, 255),
        ColorState::Normal => axis_base_color(axis),
    }
}
