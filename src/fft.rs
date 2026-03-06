use rustfft::{FftPlanner, num_complex::Complex32};
use std::sync::Arc;

pub struct FftProcessor {
    fft: Arc<dyn rustfft::Fft<f32>>,
    buffer: Vec<Complex32>,
    fft_size: usize,
}

impl FftProcessor {
    pub fn new(fft_size: usize) -> Self {
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(fft_size);
        Self {
            fft,
            buffer: vec![Complex32::new(0.0, 0.0); fft_size],
            fft_size,
        }
    }

    pub fn process(&mut self, audio_in: &[f32], channels: u32) -> Vec<f32> {
        let channels = channels.max(1) as usize;
        let frames = audio_in.len() / channels;
        let sample_count = self.fft_size.min(frames);

        // Convert audio down to mono
        for i in 0..sample_count {
            let mut sum = 0.0;
            for ch in 0..channels {
                sum += audio_in[i * channels + ch];
            }
            let mono = sum / channels as f32;
            self.buffer[i] = Complex32::new(mono, 0.0);
        }

        // Zero-pad the rest
        for i in sample_count..self.fft_size {
            self.buffer[i] = Complex32::new(0.0, 0.0);
        }

        // Perform FFT
        self.fft.process(&mut self.buffer);

        // Calculate magnitudes for first half of frequencies (Nyquist limit)
        let mut spectrum = vec![0.0; self.fft_size / 2];
        for (i, magnitude) in spectrum.iter_mut().enumerate() {
            *magnitude = self.buffer[i].norm() / sample_count.max(1) as f32;
        }

        spectrum
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    #[test]
    fn test_fft_sine_wave() {
        let sample_rate = 44100.0;
        let fft_size = 1024;
        let mut processor = FftProcessor::new(fft_size);

        // Generate a pure sine wave at 440Hz (A4)
        let freq = 440.0;
        let mut buffer = vec![0.0; fft_size];
        for (i, sample) in buffer.iter_mut().enumerate() {
            let t = i as f32 / sample_rate;
            *sample = (2.0 * PI * freq * t).sin();
        }

        let spectrum = processor.process(&buffer, 1);

        // The frequency resolution is sample_rate / fft_size
        // Bin index = freq * fft_size / sample_rate
        let expected_bin = (freq * fft_size as f32 / sample_rate).round() as usize;

        // Find the bin with maximum magnitude
        let mut max_mag = 0.0;
        let mut max_bin = 0;
        for (i, &mag) in spectrum.iter().enumerate() {
            if mag > max_mag {
                max_mag = mag;
                max_bin = i;
            }
        }

        // The maximum magnitude should be very close to the expected bin
        let diff = (max_bin as i32 - expected_bin as i32).abs();
        assert!(
            diff <= 1,
            "Expected peak near bin {}, got bin {}",
            expected_bin,
            max_bin
        );
    }
}
