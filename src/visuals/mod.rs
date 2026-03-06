pub mod core_ring;
pub mod spectrum;

use raylib::prelude::*;

use core_ring::CoreRing;
use spectrum::Spectrum;

#[derive(Clone, Copy)]
pub struct VisualizerCanvas {
    pub layout_w: f32,
    pub layout_h: f32,
    pub render_w: f32,
    pub render_h: f32,
}

pub struct Visualizer {
    core_ring: CoreRing,
    spectrum: Spectrum,
}

impl Default for Visualizer {
    fn default() -> Self {
        Self::new()
    }
}

impl Visualizer {
    pub fn new() -> Self {
        Self {
            core_ring: CoreRing::new(),
            spectrum: Spectrum::new(),
        }
    }

    pub fn update(
        &mut self,
        dt: f32,
        _volume: f32,
        spectrum_data: &[f32],
        canvas: VisualizerCanvas,
    ) {
        self.core_ring
            .update(dt, spectrum_data, canvas.layout_w, canvas.layout_h);
        self.spectrum.update(dt, spectrum_data);
    }

    pub fn draw(
        &self,
        d: &mut impl RaylibDraw,
        theme_color: Color,
        bg_tex: Option<&Texture2D>,
        logo_tex: Option<&Texture2D>,
        canvas: VisualizerCanvas,
    ) {
        let draw_scale = (canvas.render_w / canvas.layout_w).min(canvas.render_h / canvas.layout_h);
        let mut center = Vector2::new(canvas.render_w / 2.0, canvas.render_h / 2.0);
        center.x += self.core_ring.offset.x * draw_scale;
        center.y += self.core_ring.offset.y * draw_scale;

        if let Some(tex) = bg_tex {
            let source = Rectangle::new(0.0, 0.0, tex.width() as f32, tex.height() as f32);
            let dest = Rectangle::new(0.0, 0.0, canvas.render_w, canvas.render_h);
            let origin = Vector2::new(0.0, 0.0);
            d.draw_texture_pro(
                tex,
                source,
                dest,
                origin,
                0.0,
                Color::new(255, 255, 255, 120),
            );
        }

        self.spectrum
            .draw(d, center, self.core_ring.radius, theme_color, draw_scale);
        self.core_ring
            .draw(d, center, theme_color, logo_tex, draw_scale);
    }
}
