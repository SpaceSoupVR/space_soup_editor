use std::sync::Arc;

use wgpu::*;
use winit::event_loop::ActiveEventLoop;

use space_soup::renderer::{Camera, GltfMesh, Renderer};
use space_soup::ui2d::Overlay;

use agate::Ui;

use crate::transform_gizmo::GizmoAssets;

use super::discover::load_font;
use super::App;

impl App {
    pub(crate) fn resumed_setup(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(
            event_loop
                .create_window(
                    winit::window::WindowAttributes::default()
                        .with_title("space_soup_editor")
                        .with_inner_size(winit::dpi::LogicalSize::new(1600u32, 900u32)),
                )
                .unwrap(),
        );
        self.scale = window.scale_factor() as f32;
        self.window = Some(window.clone());

        let surface: Surface<'static> =
            unsafe { std::mem::transmute(self.instance.create_surface(window.clone()).unwrap()) };
        let adapter = pollster::block_on(self.instance.request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .unwrap();
        let (device, queue) = pollster::block_on(adapter.request_device(&DeviceDescriptor {
            required_features: Features::empty(),
            required_limits: Limits::default(),
            ..Default::default()
        }))
        .unwrap();

        let size = window.inner_size();
        let caps = surface.get_capabilities(&adapter);
        let fmt = caps.formats[0];
        let cfg = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: fmt,
            width: size.width,
            height: size.height,
            present_mode: PresentMode::Fifo,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &cfg);

        self.camera = Camera::new(size.width as f32 / size.height as f32);
        let renderer = Renderer::from_device(device, queue, fmt, size.width, size.height);
        let overlay = Overlay::new(&renderer.device, fmt, size.width, size.height, self.scale);

        let font = Arc::new(load_font());
        let ui = Ui::new(self.scale, font);

        let mesh_paths: Vec<String> = self
            .runtime
            .scene()
            .objects
            .iter()
            .filter_map(|o| o.mesh.as_ref().map(|m| m.path.clone()))
            .collect();
        for path in mesh_paths {
            if self.mesh_cache.contains_key(&path) {
                continue;
            }
            let full_path = self.runtime.game_dir().join(&path);
            match GltfMesh::load(
                &renderer.device,
                &renderer.queue,
                renderer.mesh_texture_layout(),
                &full_path,
            ) {
                Ok(mesh) => {
                    let model_uniform = renderer.create_model_uniform();
                    log::info!("space_soup_editor: preloaded mesh '{path}'");
                    self.mesh_cache.insert(path, (mesh, model_uniform));
                }
                Err(e) => log::warn!("space_soup_editor: failed to preload mesh '{path}': {e}"),
            }
        }

        self.gizmo_assets = Some(GizmoAssets::load(&renderer));
        self.icon_assets = Some(renderer.create_icon_assets());

        let hand_path = self.runtime.game_dir().join("models/hand.glb");
        for (i, x) in [-0.5_f32, 0.5_f32].into_iter().enumerate() {
            match GltfMesh::load_static_bind_pose(
                &renderer.device,
                &renderer.queue,
                renderer.mesh_texture_layout(),
                &hand_path,
            ) {
                Ok(mut mesh) => {
                    mesh.position = glam::Vec3::new(x, 1.2, -1.0);
                    let model_uniform = renderer.create_model_uniform();
                    log::info!("space_soup_editor: loaded debug hand.glb display #{i} (bind pose, unskinned)");
                    self.debug_meshes.push((mesh, model_uniform));
                }
                Err(e) => log::warn!("space_soup_editor: failed to load debug hand.glb: {e}"),
            }
        }

        self.renderer = Some(renderer);
        self.overlay = Some(overlay);
        self.surface = Some(surface);
        self.config = Some(cfg);
        self.ui = Some(ui);
    }
}
