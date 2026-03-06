use raylib::prelude::*;
use std::sync::{Arc, Mutex};

pub const FFT_SIZE: usize = 1024;

pub struct AudioState {
    pub volume: Arc<Mutex<f32>>,
    pub spectrum: Arc<Mutex<Vec<f32>>>,
}

impl Default for AudioState {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioState {
    pub fn new() -> Self {
        Self {
            volume: Arc::new(Mutex::new(0.0)),
            spectrum: Arc::new(Mutex::new(vec![0.0; FFT_SIZE / 2])),
        }
    }
}

pub fn get_music<'a>(rl_audio: &'a RaylibAudio, path: &str) -> Result<Music<'a>, String> {
    rl_audio.new_music(path).map_err(|e| e.to_string())
}

#[inline]
pub fn rms(a: &[f32]) -> f32 {
    if a.is_empty() {
        return 0.0;
    }
    a.iter().map(|&x| x * x).sum::<f32>() / a.len() as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rms_empty() {
        let empty: [f32; 0] = [];
        assert_eq!(rms(&empty), 0.0);
    }

    #[test]
    fn test_rms_constant() {
        let constant = [2.0, 2.0, 2.0, 2.0];
        assert_eq!(rms(&constant), 4.0);
    }

    #[test]
    fn test_rms_alternating() {
        let alt = [-1.0, 1.0, -1.0, 1.0];
        assert_eq!(rms(&alt), 1.0);
    }
}
