use raylib::prelude::*;

const BASE_RADIUS_RATIO: f32 = 0.19;
const MAX_BASS_PULSE_RATIO: f32 = 0.28;
const GLOW_THICKNESS_RATIO: f32 = 0.12;
const EDGE_THICKNESS_RATIO: f32 = 0.12;
const FLOAT_AMPLITUDE_RATIO: f32 = 0.045;
const FLOAT_BASS_BOOST_RATIO: f32 = 0.03;
const FLOAT_SPEED_BASE: f32 = 0.3;
const FLOAT_SPEED_BASS_SCALE: f32 = 0.08;

pub struct CoreRing {
    pub radius: f32,
    pub offset: Vector2,
    base_radius: f32,
    smoothed_bass: f32,
    kick_pulse: f32,
    time: f32,
}

impl Default for CoreRing {
    fn default() -> Self {
        Self::new()
    }
}

impl CoreRing {
    pub fn new() -> Self {
        let base_radius = 720.0 * BASE_RADIUS_RATIO;
        Self {
            radius: base_radius,
            offset: Vector2::new(0.0, 0.0),
            base_radius,
            smoothed_bass: 0.0,
            kick_pulse: 0.0,
            time: 0.0,
        }
    }

    pub fn update(&mut self, dt: f32, spectrum: &[f32], screen_w: f32, screen_h: f32) {
        self.time += dt;
        self.base_radius = screen_w.min(screen_h) * BASE_RADIUS_RATIO;
        let max_bass_pulse = self.base_radius * MAX_BASS_PULSE_RATIO;

        // Extract bass (low frequencies). Assume first few bins.
        let bass_bins = 5.min(spectrum.len()); // tighter band for kicks
        let bass = if bass_bins > 0 {
            let sum: f32 = spectrum[..bass_bins].iter().sum();
            sum / bass_bins as f32
        } else {
            0.0
        };

        // Smooth the bass reaction for general floating
        self.smoothed_bass = self.smoothed_bass * 0.9 + bass * 10.0 * 0.1;

        // Kick detection: If bass spikes suddenly above the smoothed average
        let mut kick_amount = 0.0;
        if bass * 10.0 > self.smoothed_bass + 1.0 {
            // threshold
            kick_amount = (bass * 10.0 - self.smoothed_bass) * self.base_radius * 0.035;
        }

        self.kick_pulse = (self.kick_pulse + kick_amount).min(max_bass_pulse);
        // Fast decay for the kick pulse
        self.kick_pulse *= 0.85;

        // The exact radius is base + general floating + sharp kick
        self.radius =
            self.base_radius + self.smoothed_bass.min(self.base_radius * 0.08) + self.kick_pulse;

        // Calculate organic floating offset (Lissajous curve combination)
        // Reduced amplitude based on user feedback to be more subtle
        let float_amplitude = self.base_radius * FLOAT_AMPLITUDE_RATIO
            + self.smoothed_bass.min(self.base_radius * 0.12) * FLOAT_BASS_BOOST_RATIO;
        let float_speed = FLOAT_SPEED_BASE + self.smoothed_bass.min(3.0) * FLOAT_SPEED_BASS_SCALE;

        self.offset.x = (self.time * 0.7 * float_speed).sin() * float_amplitude
            + (self.time * 1.3 * float_speed).cos() * (float_amplitude * 0.5);
        self.offset.y = (self.time * 0.9 * float_speed).cos() * float_amplitude
            + (self.time * 1.1 * float_speed).sin() * (float_amplitude * 0.5);
    }

    pub fn draw(
        &self,
        d: &mut impl RaylibDraw,
        center: Vector2,
        theme_color: Color,
        logo_tex: Option<&Texture2D>,
        draw_scale: f32,
    ) {
        let radius = self.radius * draw_scale;
        let edge_thickness =
            (self.base_radius * draw_scale * EDGE_THICKNESS_RATIO).max(16.0 * draw_scale);
        let glow_thickness =
            (self.base_radius * draw_scale * GLOW_THICKNESS_RATIO).max(18.0 * draw_scale);

        // Draw outer pulsing glow (using a slightly thicker ring for anti-aliasing look)
        let mut glow_color = theme_color;
        glow_color.a = 50; // translucent glow

        d.draw_ring(
            center,
            radius - edge_thickness * 0.2,
            radius + glow_thickness,
            0.0,
            360.0,
            160,
            glow_color,
        );

        // Draw the core circle edge smoothly using a high-segment ring
        d.draw_ring(
            center,
            radius - edge_thickness,
            radius,
            0.0,
            360.0,
            320,
            theme_color,
        );

        // If a logo is provided, draw it centered and scaled within the ring
        if let Some(tex) = logo_tex {
            let tex_w = tex.width() as f32;
            let tex_h = tex.height() as f32;

            // We want it to fit nicely inside the ring, minus some padding
            let fit_radius = radius - edge_thickness * 1.15;
            let scale = (fit_radius * 2.0) / f32::max(tex_w, tex_h);

            let dest_w = tex_w * scale;
            let dest_h = tex_h * scale;

            let source = Rectangle::new(0.0, 0.0, tex_w, tex_h);
            let dest = Rectangle::new(center.x, center.y, dest_w, dest_h);
            let origin = Vector2::new(dest_w / 2.0, dest_h / 2.0); // center anchor

            d.draw_texture_pro(tex, source, dest, origin, 0.0, Color::WHITE);
        } else {
            // Otherwise draw a solid dark center core
            d.draw_circle_v(center, radius - edge_thickness, Color::new(20, 20, 20, 255));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_corering_initialization() {
        let ring = CoreRing::new();
        assert_eq!(ring.radius, 720.0 * BASE_RADIUS_RATIO);
    }

    #[test]
    fn test_corering_update() {
        let mut ring = CoreRing::new();
        let spectrum = vec![1.0; 10]; // High bass spectrum

        // Use an unused variable marker to satisfy the unused warning.
        ring.update(0.16, &spectrum, 1280.0, 720.0);
        assert!(
            ring.radius > 720.0 * BASE_RADIUS_RATIO,
            "Radius should increase with bass"
        );
    }
}
