use raylib::prelude::*;
use std::f32::consts::PI;

const NUM_POINTS: usize = 120; // More points for a smoother circle
const MAX_BAR_HEIGHT: f32 = 180.0;
const BASE_RING_RADIUS: f32 = 720.0 * 0.19;
const SPECTRUM_RADIUS_OFFSET_RATIO: f32 = 0.075;

pub struct Spectrum {
    points: Vec<f32>,
}

impl Default for Spectrum {
    fn default() -> Self {
        Self::new()
    }
}

impl Spectrum {
    pub fn new() -> Self {
        Self {
            points: vec![0.0; NUM_POINTS],
        }
    }

    pub fn update(&mut self, _dt: f32, spectrum: &[f32]) {
        if spectrum.is_empty() {
            return;
        }

        // We want a logarithmic scale, skipping the extreme highs
        // that just look like flat noise. Let's capture up to bin 300 (out of 512).
        let max_bin = 300.min(spectrum.len());
        let min_freq_log = 1.0_f32.ln();
        let max_freq_log = (max_bin as f32).ln();
        let log_range = max_freq_log - min_freq_log;

        // The visual wave covers 360 degrees, but we mirror the left/right audio channels
        // For simplicity, we'll map the frequency spectrum from 0 to 180 degrees,
        // and mirror it for the other 180 degrees.

        for i in 0..NUM_POINTS {
            // Find which bins this point covers logarithmically
            let ratio_start = i as f32 / NUM_POINTS as f32;
            let ratio_end = (i + 1) as f32 / NUM_POINTS as f32;

            let log_start = min_freq_log + ratio_start * log_range;
            let log_end = min_freq_log + ratio_end * log_range;

            let bin_start = log_start.exp().floor() as usize;
            let bin_end = log_end.exp().ceil() as usize;

            let start = bin_start.clamp(0, spectrum.len() - 1);
            let end = bin_end.clamp(start + 1, spectrum.len());

            let mut sum = 0.0;
            for &val in &spectrum[start..end] {
                sum += val;
            }
            let avg = sum / (end - start) as f32;

            // Apply a slight EQ curve to boost highs visually
            let eq_boost = 1.0 + (i as f32 / NUM_POINTS as f32) * 2.0;
            let height = avg * 2400.0 * eq_boost;

            let final_height = height.min(MAX_BAR_HEIGHT);

            // Smoothly interpolate towards the new height
            self.points[i] = self.points[i] * 0.7 + final_height * 0.3;
        }

        // To fix the sharp edge at the connection point (360 degrees back to 0),
        // we'll forcefully cross-fade the last few points into the first few points
        let blend_width = 15;
        for i in 0..blend_width {
            let start_i = i;
            let end_i = NUM_POINTS - 1 - i;

            let start_val = self.points[start_i];
            let end_val = self.points[end_i];

            // Linear blend closer to the seam
            let blend_factor = (blend_width - i) as f32 / blend_width as f32; // 1.0 at seam, 0.0 at edge

            // Bring them towards their average
            let avg = (start_val + end_val) * 0.5;
            self.points[start_i] = start_val * (1.0 - blend_factor) + avg * blend_factor;
            self.points[end_i] = end_val * (1.0 - blend_factor) + avg * blend_factor;
        }
    }

    pub fn draw(
        &self,
        d: &mut impl RaylibDraw,
        center: Vector2,
        core_radius: f32,
        theme_color: Color,
        draw_scale: f32,
    ) {
        let scale = (core_radius / BASE_RING_RADIUS).max(1.0);
        let render_core_radius = core_radius * draw_scale;
        let radius = render_core_radius + render_core_radius * SPECTRUM_RADIUS_OFFSET_RATIO;
        let height_scale = 1.0 + (scale - 1.0) * 0.7;
        let angle_step = 2.0 * PI / NUM_POINTS as f32;

        for i in 0..NUM_POINTS {
            let angle = i as f32 * angle_step - PI / 2.0;
            let h = self.points[i];
            let rendered_height = h * height_scale;

            let point_radius = radius + rendered_height * draw_scale;

            let p_outer = Vector2::new(
                center.x + angle.cos() * point_radius,
                center.y + angle.sin() * point_radius,
            );

            // Draw filler trail dots below the main peak to make it look dense
            // The number of trail dots depends on the height (max 6)
            let trail_count = (rendered_height / 15.0).clamp(0.0, 6.0) as usize;
            for t in 1..=trail_count {
                // Determine proportion along the line from core (0.0) up to peak (1.0)
                let proportion = t as f32 / (trail_count as f32 + 1.0);
                let trail_radius = radius + (rendered_height * proportion) * draw_scale;

                let p_trail = Vector2::new(
                    center.x + angle.cos() * trail_radius,
                    center.y + angle.sin() * trail_radius,
                );

                // Trail dots are smaller and dimmer
                let trail_size = (1.0 + (h / MAX_BAR_HEIGHT) * 1.5) * draw_scale;
                let trail_alpha = 100 + ((h / MAX_BAR_HEIGHT) * 80.0) as u8;

                let mut trail_color = theme_color;
                trail_color.a = trail_alpha;

                d.draw_circle_v(p_trail, trail_size, trail_color);
            }

            // Dot size scales slightly with its height/energy
            let dot_size = (2.0 + (h / MAX_BAR_HEIGHT) * 4.0) * draw_scale;

            // Generate glowing peak dot using theme color but higher opacity
            let peak_alpha = 150 + ((h / MAX_BAR_HEIGHT) * 105.0) as u8;
            let mut peak_color = theme_color;
            peak_color.a = peak_alpha;

            // Draw the main floating particle dot for the frequency bin
            d.draw_circle_v(p_outer, dot_size, peak_color);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spectrum_initialization() {
        let spectrum = Spectrum::new();
        assert_eq!(spectrum.points.len(), NUM_POINTS);
        assert!(spectrum.points.iter().all(|&b| b == 0.0));
    }

    #[test]
    fn test_spectrum_update() {
        let mut spectrum = Spectrum::new();
        let audio_data = vec![0.5; 512]; // Mock FFT data

        spectrum.update(0.16, &audio_data);
        // All low/mid bars should have some height now
        assert!(spectrum.points[0] > 0.0, "Bars should react to spectrum");
    }
}
