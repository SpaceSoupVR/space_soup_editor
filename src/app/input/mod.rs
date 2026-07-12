pub(crate) mod gesture;
pub(crate) mod keyboard;
pub(crate) mod mouse;
pub(crate) mod resize;

use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::WindowId;

use super::App;

pub(crate) fn handle_window_event(
    app: &mut App,
    event_loop: &ActiveEventLoop,
    _id: WindowId,
    event: WindowEvent,
) {
    match event {
        WindowEvent::CloseRequested => event_loop.exit(),

        WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
            resize::scale_factor_changed(app, scale_factor as f32);
        }
        WindowEvent::Resized(size) => resize::resized(app, size),

        WindowEvent::ModifiersChanged(m) => {
            app.mods = m.state();
        }

        WindowEvent::KeyboardInput { event, .. } => {
            keyboard::handle_key_event(app, &event);
            app.redraw_now();
        }

        // Dropping focus can swallow key-release events; clear held fly keys so
        // the camera doesn't drift.
        WindowEvent::Focused(false) => {
            app.fly.clear();
        }

        WindowEvent::CursorMoved { position, .. } => {
            mouse::cursor_moved(app, position);
        }
        WindowEvent::MouseInput {
            state,
            button: winit::event::MouseButton::Left,
            ..
        } => {
            mouse::left_button(app, state);
        }

        WindowEvent::PinchGesture { delta, phase, .. } => {
            gesture::pinch(app, delta, phase);
        }
        WindowEvent::MouseWheel { delta, .. } => {
            gesture::mouse_wheel(app, delta);
        }

        WindowEvent::RedrawRequested => app.redraw(),
        _ => {}
    }
}
