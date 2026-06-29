use agate::Theme;
use winit::dpi::PhysicalSize;

use super::super::App;

pub(crate) fn scale_factor_changed(app: &mut App, scale_factor: f32) {
    app.scale = scale_factor;
    if let Some(ov) = app.overlay.as_mut() { ov.set_scale_factor(app.scale); }
    if let Some(ui) = app.ui.as_mut() { ui.theme = Theme::new(app.scale); }
    app.redraw_now();
}

pub(crate) fn resized(app: &mut App, size: PhysicalSize<u32>) {
    if let (Some(sur), Some(cfg), Some(rnd), Some(ov)) = (
        app.surface.as_ref(), app.config.as_mut(),
        app.renderer.as_mut(), app.overlay.as_mut(),
    ) {
        cfg.width = size.width;
        cfg.height = size.height;
        sur.configure(&rnd.device, cfg);
        rnd.resize(size.width, size.height);
        ov.resize(size.width, size.height);
        app.camera.aspect = size.width as f32 / size.height as f32;
    }
}
