use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use hound::{SampleFormat, WavReader};
use raylib::consts::PixelFormat;
use raylib::prelude::*;

use crate::audio;
use crate::fft::FftProcessor;
use crate::visuals::{Visualizer, VisualizerCanvas};

pub const EXPORT_WIDTH: i32 = 1920;
pub const EXPORT_HEIGHT: i32 = 1080;
const EXPORT_SUPERSAMPLE: i32 = 2;
const RENDER_WIDTH: i32 = EXPORT_WIDTH * EXPORT_SUPERSAMPLE;
const RENDER_HEIGHT: i32 = EXPORT_HEIGHT * EXPORT_SUPERSAMPLE;
const EXPORT_FPS: usize = 60;
const AUDIO_SAMPLE_RATE: usize = 48_000;
const LEAD_IN_SECS: f32 = 0.3;
const TAIL_PAD_SECS: f32 = 0.7;
const MAX_FRAMES_PER_TICK: usize = 4;
const MAX_TICK_BUDGET: Duration = Duration::from_millis(12);

pub struct ExportJob {
    theme_color: Color,
    output_path: PathBuf,
    temp_wav: PathBuf,
    render_tex: RenderTexture2D,
    visualizer: Visualizer,
    fft_processor: FftProcessor,
    smoothed_spectrum: Vec<f32>,
    samples: Vec<f32>,
    stereo_samples_per_frame: usize,
    total_frames: usize,
    next_frame: usize,
    ffmpeg: Child,
    stdin: Option<ChildStdin>,
    finalizing: bool,
    completed: bool,
}

pub enum ExportTick {
    Working,
    Finished(PathBuf),
}

impl ExportJob {
    pub fn start(
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
        input_path: &str,
        theme_color: Color,
    ) -> Result<Self, String> {
        let temp_wav = make_temp_wav_path();
        prepare_audio(input_path, &temp_wav)?;

        let samples = read_wav_samples(&temp_wav)?;
        let stereo_samples_per_frame = stereo_samples_per_frame();
        let total_frames = total_frames_for_sample_count(samples.len());

        let output_path = PathBuf::from("output.mp4");
        let render_tex = rl
            .load_render_texture(thread, RENDER_WIDTH as u32, RENDER_HEIGHT as u32)
            .map_err(|e| e.to_string())?;
        let mut ffmpeg = spawn_ffmpeg_encoder(&temp_wav, &output_path)?;
        let stdin = ffmpeg
            .stdin
            .take()
            .ok_or_else(|| "Failed to open FFmpeg stdin".to_string())?;

        Ok(Self {
            theme_color,
            output_path,
            temp_wav,
            render_tex,
            visualizer: Visualizer::new(),
            fft_processor: FftProcessor::new(audio::FFT_SIZE),
            smoothed_spectrum: vec![0.0; audio::FFT_SIZE / 2],
            samples,
            stereo_samples_per_frame,
            total_frames,
            next_frame: 0,
            ffmpeg,
            stdin: Some(stdin),
            finalizing: false,
            completed: false,
        })
    }

    pub fn progress(&self) -> f32 {
        if self.finalizing {
            1.0
        } else {
            self.next_frame as f32 / self.total_frames.max(1) as f32
        }
    }

    pub fn status_text(&self) -> String {
        if self.finalizing {
            "Finalizing MP4...".to_string()
        } else {
            format!(
                "Rendering frame {} / {}",
                self.next_frame.min(self.total_frames),
                self.total_frames
            )
        }
    }

    pub fn tick(
        &mut self,
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
        bg_tex: Option<&Texture2D>,
        logo_tex: Option<&Texture2D>,
    ) -> Result<ExportTick, String> {
        if self.completed {
            return Ok(ExportTick::Finished(self.output_path.clone()));
        }

        if self.next_frame < self.total_frames {
            let started = Instant::now();
            let mut processed = 0;

            while self.next_frame < self.total_frames
                && processed < MAX_FRAMES_PER_TICK
                && started.elapsed() < MAX_TICK_BUDGET
            {
                self.render_frame(rl, thread, bg_tex, logo_tex)?;
                processed += 1;
            }

            return Ok(ExportTick::Working);
        }

        if !self.finalizing {
            self.finalizing = true;
            self.stdin.take();
        }

        match self.ffmpeg.try_wait().map_err(|e| e.to_string())? {
            Some(status) if status.success() => {
                self.completed = true;
                let _ = fs::remove_file(&self.temp_wav);
                Ok(ExportTick::Finished(self.output_path.clone()))
            }
            Some(status) => Err(format!("FFmpeg encoding failed with status {}", status)),
            None => Ok(ExportTick::Working),
        }
    }

    fn render_frame(
        &mut self,
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
        bg_tex: Option<&Texture2D>,
        logo_tex: Option<&Texture2D>,
    ) -> Result<(), String> {
        let frame_idx = self.next_frame;
        let end_idx = ((frame_idx + 1) * self.stereo_samples_per_frame).min(self.samples.len());
        let fft_window_samples = audio::FFT_SIZE * 2;
        let start_idx = end_idx.saturating_sub(fft_window_samples);
        let available = end_idx.saturating_sub(start_idx);

        let mut window = vec![0.0; fft_window_samples];
        let diff = fft_window_samples.saturating_sub(available);
        window[diff..diff + available].copy_from_slice(&self.samples[start_idx..end_idx]);

        let frame_start_idx = frame_idx * self.stereo_samples_per_frame;
        let volume = audio::rms(&self.samples[frame_start_idx..end_idx]);

        let new_spectrum = self.fft_processor.process(&window, 2);
        for (smoothed, new_value) in self.smoothed_spectrum.iter_mut().zip(new_spectrum.iter()) {
            *smoothed = *smoothed * 0.85 + *new_value * 0.15;
        }

        let dt = 1.0 / EXPORT_FPS as f32;
        let canvas = VisualizerCanvas {
            layout_w: EXPORT_WIDTH as f32,
            layout_h: EXPORT_HEIGHT as f32,
            render_w: RENDER_WIDTH as f32,
            render_h: RENDER_HEIGHT as f32,
        };
        self.visualizer
            .update(dt, volume, &self.smoothed_spectrum, canvas);

        {
            let mut d = rl.begin_texture_mode(thread, &mut self.render_tex);
            d.clear_background(Color::new(10, 10, 15, 255));
            self.visualizer
                .draw(&mut d, self.theme_color, bg_tex, logo_tex, canvas);
        }

        let mut image = self.render_tex.load_image().map_err(|e| e.to_string())?;
        image.flip_vertical();
        if image.width() < RENDER_WIDTH || image.height() < RENDER_HEIGHT {
            return Err(format!(
                "Render target readback was smaller than expected: {}x{}",
                image.width(),
                image.height()
            ));
        }
        if image.width() != RENDER_WIDTH || image.height() != RENDER_HEIGHT {
            image.crop(Rectangle::new(
                0.0,
                0.0,
                RENDER_WIDTH as f32,
                RENDER_HEIGHT as f32,
            ));
        }
        image.set_format(PixelFormat::PIXELFORMAT_UNCOMPRESSED_R8G8B8A8);

        let bytes = unsafe {
            std::slice::from_raw_parts(image.data() as *const u8, image.get_pixel_data_size())
        };
        self.stdin
            .as_mut()
            .ok_or_else(|| "FFmpeg stdin closed unexpectedly".to_string())?
            .write_all(bytes)
            .map_err(|e| e.to_string())?;

        self.next_frame += 1;
        Ok(())
    }
}

impl Drop for ExportJob {
    fn drop(&mut self) {
        self.stdin.take();
        if !self.completed {
            let _ = self.ffmpeg.kill();
        }
        let _ = fs::remove_file(&self.temp_wav);
    }
}

fn prepare_audio(input_path: &str, temp_wav: &Path) -> Result<(), String> {
    let status = Command::new("ffmpeg")
        .arg("-y")
        .arg("-i")
        .arg(input_path)
        .arg("-af")
        .arg(build_audio_filter())
        .arg("-ar")
        .arg(AUDIO_SAMPLE_RATE.to_string())
        .arg("-ac")
        .arg("2")
        .arg("-c:a")
        .arg("pcm_f32le")
        .arg(temp_wav)
        .status()
        .map_err(|e| e.to_string())?;

    if status.success() {
        Ok(())
    } else {
        Err("Failed to extract and pad audio with FFmpeg".to_string())
    }
}

fn read_wav_samples(path: &Path) -> Result<Vec<f32>, String> {
    let mut reader = WavReader::open(path).map_err(|e| e.to_string())?;
    read_wav_samples_from_reader(&mut reader)
}

fn read_wav_samples_from_reader<R: std::io::Read + std::io::Seek>(
    reader: &mut WavReader<R>,
) -> Result<Vec<f32>, String> {
    let spec = reader.spec();
    if spec.sample_rate != AUDIO_SAMPLE_RATE as u32 {
        return Err(format!(
            "Unexpected sample rate in temp WAV: expected {}, got {}",
            AUDIO_SAMPLE_RATE, spec.sample_rate
        ));
    }
    if spec.channels != 2 {
        return Err(format!(
            "Unexpected channel count in temp WAV: {}",
            spec.channels
        ));
    }

    match spec.sample_format {
        SampleFormat::Float => reader
            .samples::<f32>()
            .map(|sample| sample.map_err(|e| e.to_string()))
            .collect(),
        SampleFormat::Int => reader
            .samples::<i32>()
            .map(|sample| {
                sample
                    .map(|value| value as f32 / i32::MAX as f32)
                    .map_err(|e| e.to_string())
            })
            .collect(),
    }
}

fn spawn_ffmpeg_encoder(temp_wav: &Path, output_path: &Path) -> Result<Child, String> {
    Command::new("ffmpeg")
        .arg("-y")
        .arg("-f")
        .arg("rawvideo")
        .arg("-vcodec")
        .arg("rawvideo")
        .arg("-s")
        .arg(format!("{}x{}", RENDER_WIDTH, RENDER_HEIGHT))
        .arg("-pix_fmt")
        .arg("rgba")
        .arg("-r")
        .arg(EXPORT_FPS.to_string())
        .arg("-i")
        .arg("-")
        .arg("-i")
        .arg(temp_wav)
        .arg("-c:v")
        .arg("libx264")
        .arg("-preset")
        .arg("slower")
        .arg("-crf")
        .arg("14")
        .arg("-vf")
        .arg(export_scale_filter())
        .arg("-c:a")
        .arg("alac")
        .arg("-movflags")
        .arg("+faststart")
        .arg("-pix_fmt")
        .arg("yuv420p")
        .arg(output_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())
}

fn make_temp_wav_path() -> PathBuf {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    std::env::temp_dir().join(format!("yue_export_{}_{}.wav", std::process::id(), ts))
}

fn samples_per_frame() -> usize {
    AUDIO_SAMPLE_RATE.div_ceil(EXPORT_FPS)
}

fn stereo_samples_per_frame() -> usize {
    samples_per_frame() * 2
}

fn total_frames_for_sample_count(sample_count: usize) -> usize {
    sample_count.div_ceil(stereo_samples_per_frame()).max(1)
}

fn build_audio_filter() -> String {
    let adelay_ms = (LEAD_IN_SECS * 1000.0).round() as usize;
    format!("adelay={0}|{0},apad=pad_dur={1}", adelay_ms, TAIL_PAD_SECS)
}

fn export_scale_filter() -> String {
    format!("scale={}:{}:flags=lanczos", EXPORT_WIDTH, EXPORT_HEIGHT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hound::{SampleFormat, WavSpec, WavWriter};

    #[test]
    fn test_total_frames_for_empty_and_partial_audio() {
        assert_eq!(total_frames_for_sample_count(0), 1);
        assert_eq!(total_frames_for_sample_count(stereo_samples_per_frame()), 1);
        assert_eq!(
            total_frames_for_sample_count(stereo_samples_per_frame() + 1),
            2
        );
    }

    #[test]
    fn test_build_audio_filter() {
        assert_eq!(build_audio_filter(), "adelay=300|300,apad=pad_dur=0.7");
    }

    #[test]
    fn test_export_scale_filter() {
        assert_eq!(export_scale_filter(), "scale=1920:1080:flags=lanczos");
    }

    #[test]
    fn test_read_wav_samples_float() {
        let path = temp_test_wav_path("float");
        write_float_wav(&path, &[0.25, -0.25, 0.5, -0.5]);

        let samples = read_wav_samples(&path).expect("float wav should load");
        assert_eq!(samples, vec![0.25, -0.25, 0.5, -0.5]);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_read_wav_samples_int() {
        let path = temp_test_wav_path("int");
        write_int_wav(&path, &[0, i32::MAX / 2, -i32::MAX / 2, i32::MAX]);

        let samples = read_wav_samples(&path).expect("int wav should load");
        assert!((samples[0] - 0.0).abs() < 1e-6);
        assert!((samples[1] - 0.5).abs() < 1e-6);
        assert!((samples[2] + 0.5).abs() < 1e-6);
        assert!((samples[3] - 1.0).abs() < 1e-6);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_read_wav_samples_rejects_wrong_sample_rate() {
        let path = temp_test_wav_path("bad_rate");
        let spec = WavSpec {
            channels: 2,
            sample_rate: 44_100,
            bits_per_sample: 32,
            sample_format: SampleFormat::Float,
        };
        let mut writer = WavWriter::create(&path, spec).expect("create wav");
        writer.write_sample(0.0f32).expect("write sample");
        writer.write_sample(0.0f32).expect("write sample");
        writer.finalize().expect("finalize wav");

        let err = read_wav_samples(&path).expect_err("wrong sample rate should fail");
        assert!(err.contains("Unexpected sample rate"));

        let _ = fs::remove_file(path);
    }

    fn temp_test_wav_path(prefix: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "yue_export_test_{}_{}_{}.wav",
            prefix,
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ))
    }

    fn write_float_wav(path: &Path, samples: &[f32]) {
        let spec = WavSpec {
            channels: 2,
            sample_rate: AUDIO_SAMPLE_RATE as u32,
            bits_per_sample: 32,
            sample_format: SampleFormat::Float,
        };
        let mut writer = WavWriter::create(path, spec).expect("create float wav");
        for sample in samples {
            writer.write_sample(*sample).expect("write sample");
        }
        writer.finalize().expect("finalize float wav");
    }

    fn write_int_wav(path: &Path, samples: &[i32]) {
        let spec = WavSpec {
            channels: 2,
            sample_rate: AUDIO_SAMPLE_RATE as u32,
            bits_per_sample: 32,
            sample_format: SampleFormat::Int,
        };
        let mut writer = WavWriter::create(path, spec).expect("create int wav");
        for sample in samples {
            writer.write_sample(*sample).expect("write sample");
        }
        writer.finalize().expect("finalize int wav");
    }
}
