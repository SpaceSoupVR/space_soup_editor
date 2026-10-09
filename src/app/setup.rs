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
        let attrs = winit::window::WindowAttributes::default()
            .with_title("Space Soup")
            .with_inner_size(winit::dpi::LogicalSize::new(1600u32, 900u32))
            .with_theme(Some(winit::window::Theme::Dark));
        let window = Arc::new(event_loop.create_window(attrs).unwrap());

        // Native Apple chrome from the render layer: unified titlebar strip
        // (traffic lights, drag, zoom) with our content extending under it.
        // The UI is opaque and docked, so no vibrancy backdrop.
        #[cfg(target_os = "macos")]
        if !space_soup::renderer::configure_macos_window(
            window.as_ref(),
            &space_soup::renderer::MacosWindowChrome {
                // Pin the appearance so titlebar text and controls stay dark
                // even on a light system.
                appearance: space_soup::renderer::WindowAppearance::Dark,
                vibrancy: None,
                ..Default::default()
            },
        ) {
            log::warn!("space_soup_editor: macOS window chrome unavailable");
        }
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
        // A non-opaque compositing mode lets the vibrancy layer show through
        // wherever the surface alpha is below 1 (the translucent clear + UI).
        let alpha_mode = caps
            .alpha_modes
            .iter()
            .copied()
            .find(|m| {
                matches!(
                    m,
                    CompositeAlphaMode::PostMultiplied | CompositeAlphaMode::PreMultiplied
                )
            })
            .unwrap_or(caps.alpha_modes[0]);
        let cfg = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: fmt,
            width: size.width,
            height: size.height,
            present_mode: PresentMode::Fifo,
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &cfg);

        self.camera = Camera::new(size.width as f32 / size.height as f32);
        let mut renderer = Renderer::from_device(device, queue, fmt, size.width, size.height);
        // Neutral dark viewport backdrop, matching the docked chrome.
        renderer.clear_color = wgpu::Color {
            r: 0.016,
            g: 0.017,
            b: 0.022,
            a: 1.0,
        };
        let overlay = Overlay::new(&renderer.device, fmt, size.width, size.height, self.scale);

        let font = Arc::new(load_font());
        let ui = Ui::new(self.scale, font);

        crate::app::scene_bridge::ensure_scene_meshes_cached(
            &renderer,
            &mut self.mesh_cache,
            &mut self.mesh_base_half_size,
            self.runtime.game_dir(),
            self.runtime.scene(),
        );

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
