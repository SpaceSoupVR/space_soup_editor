mod app;
mod network;
mod scene_3d;
mod text_panels;
mod transform_gizmo;

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::WindowId;

use app::App;

/// Half-extent (meters) of the placeholder cube shown while dragging a new
/// model from the tray, and the default size given to a freshly-dropped
/// model before the user resizes it in the Inspector.
pub(crate) const OBJECT_HALF_SIZE: f32 = 0.2;

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.resumed_setup(event_loop);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        app::input::handle_window_event(self, event_loop, id, event);
    }

    fn about_to_wait(&mut self, _: &ActiveEventLoop) {
        self.redraw_now();
    }
}

fn main() {
    env_logger::init();
    let packet = network::spawn_listener("0.0.0.0:7778");
    let event_loop = EventLoop::new().unwrap();
    let mut app = App::new(packet);
    event_loop.run_app(&mut app).unwrap();
}
