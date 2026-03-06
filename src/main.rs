pub mod audio;
pub mod export;
pub mod fft;
pub mod visuals;

use raylib::prelude::*;
use std::sync::Arc;

use audio::{AudioState, get_music, rms};
use export::{ExportJob, ExportTick};
use fft::FftProcessor;
use visuals::{Visualizer, VisualizerCanvas};

fn load_filtered_texture(
    rl: &mut RaylibHandle,
    thread: &RaylibThread,
    path: &std::path::Path,
) -> Option<Texture2D> {
    let img = Image::load_image(path.to_str().unwrap_or("")).ok()?;
    let mut tex = rl.load_texture_from_image(thread, &img).ok()?;
    tex.gen_texture_mipmaps();
    tex.set_texture_filter(thread, TextureFilter::TEXTURE_FILTER_BILINEAR);
    Some(tex)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (mut rl, thread) = raylib::init()
        .size(1280, 720)
        .title("Yue - NCS Visualizer")
        .msaa_4x()
        .build();

    rl.set_target_fps(60);
    let rl_audio = RaylibAudio::init_audio_device()?;

    let audio_state = AudioState::new();
    let mut visualizer = Visualizer::new();

    // Theming variables
    let theme_colors = [
        Color::new(0, 255, 255, 255),   // Cyan
        Color::new(255, 0, 255, 255),   // Magenta
        Color::new(255, 255, 0, 255),   // Yellow
        Color::new(50, 255, 50, 255),   // Lime
        Color::new(255, 100, 100, 255), // Pinkish Red
        Color::new(255, 255, 255, 255), // White
    ];
    let mut current_theme_idx = 0;

    let mut bg_texture: Option<Texture2D> = None;
    let mut logo_texture: Option<Texture2D> = None;

    let mut current_music: Option<Music> = None;
    let mut current_track_path: Option<String> = None;
    let mut current_track_name = String::from("No track selected");

    let mut paused = false;
    let mut tot_secs = 0;
    let mut num_mins = 0;
    let mut num_secs = 0;
    let mut export_job: Option<ExportJob> = None;
    let mut export_notice: Option<String> = None;
    let preview_canvas = VisualizerCanvas {
        layout_w: 1280.0,
        layout_h: 720.0,
        render_w: 1280.0,
        render_h: 720.0,
    };

    while !rl.window_should_close() {
        let mut finished_export = None;
        let mut export_error = None;
        if let Some(job) = export_job.as_mut() {
            match job.tick(&mut rl, &thread, bg_texture.as_ref(), logo_texture.as_ref()) {
                Ok(ExportTick::Working) => {}
                Ok(ExportTick::Finished(path)) => {
                    finished_export = Some(path);
                }
                Err(err) => {
                    export_error = Some(err);
                }
            }
        }
        if let Some(path) = finished_export {
            export_job = None;
            export_notice = Some(format!("Export completed: {}", path.display()));
        }
        if let Some(err) = export_error {
            export_job = None;
            export_notice = Some(format!("Export failed: {}", err));
        }

        let export_in_progress = export_job.is_some();

        if let Some(music) = &mut current_music {
            if !paused {
                music.update_stream();
            }

            if !export_in_progress && rl.is_key_pressed(KeyboardKey::KEY_SPACE) {
                if paused {
                    music.resume_stream();
                } else {
                    music.pause_stream();
                }
                paused = !paused;
            }
        }

        let cur_volume = *audio_state.volume.lock().unwrap();
        let spectrum_values = audio_state.spectrum.lock().unwrap().clone();

        let dt = rl.get_frame_time();
        visualizer.update(dt, cur_volume, &spectrum_values, preview_canvas);

        // GUI Interaction Flags
        let mut load_bg = false;
        let mut load_logo = false;
        let mut load_track = false;
        let mut start_render = false;

        {
            let mut d = rl.begin_drawing(&thread);
            d.clear_background(Color::new(10, 10, 15, 255));

            let current_color = theme_colors[current_theme_idx];

            visualizer.draw(
                &mut d,
                current_color,
                bg_texture.as_ref(),
                logo_texture.as_ref(),
                preview_canvas,
            );

            // Header UI
            d.draw_text(&current_track_name, 20, 20, 20, Color::WHITE);
            if let Some(message) = &export_notice {
                d.draw_text(message, 20, 48, 18, Color::new(210, 210, 210, 255));
            }

            if !export_in_progress
                && d.gui_button(
                    Rectangle {
                        x: 1140.0,
                        y: 20.0,
                        width: 120.0,
                        height: 30.0,
                    },
                    "Select Track",
                )
            {
                load_track = true;
            }

            if !export_in_progress
                && d.gui_button(
                    Rectangle {
                        x: 1140.0,
                        y: 60.0,
                        width: 120.0,
                        height: 30.0,
                    },
                    "Cycle Theme",
                )
            {
                current_theme_idx = (current_theme_idx + 1) % theme_colors.len();
            }

            if !export_in_progress
                && d.gui_button(
                    Rectangle {
                        x: 1140.0,
                        y: 100.0,
                        width: 120.0,
                        height: 30.0,
                    },
                    "Load BG Image",
                )
            {
                load_bg = true;
            }

            if !export_in_progress
                && d.gui_button(
                    Rectangle {
                        x: 1140.0,
                        y: 140.0,
                        width: 120.0,
                        height: 30.0,
                    },
                    "Load Ring Image",
                )
            {
                load_logo = true;
            }

            // Draw Render Button if music is loaded
            if current_music.is_some()
                && !export_in_progress
                && d.gui_button(
                    Rectangle {
                        x: 1140.0,
                        y: 180.0,
                        width: 120.0,
                        height: 30.0,
                    },
                    "Render MP4",
                )
            {
                start_render = true;
            }

            if let Some(music) = current_music.as_mut().filter(|_| !export_in_progress) {
                let to_toggle = d.gui_button(
                    Rectangle {
                        x: 615.0,
                        y: 620.0,
                        width: 50.0,
                        height: 20.0,
                    },
                    if paused { "Resume" } else { "Pause" },
                );

                if to_toggle {
                    if paused {
                        music.resume_stream();
                        paused = false;
                    } else {
                        music.pause_stream();
                        paused = true;
                    }
                }

                let mut timestamp = music.get_time_played();
                let old_time = timestamp;
                d.gui_slider(
                    Rectangle {
                        x: 290.0,
                        y: 650.0,
                        width: 700.0,
                        height: 28.0,
                    },
                    "0:00",
                    &format!("{}:{:02}", num_mins, num_secs),
                    &mut timestamp,
                    0.0f32,
                    tot_secs as f32,
                );
                if (timestamp - old_time).abs() > 0.01 {
                    music.seek_stream(timestamp);
                }
            }

            if let Some(job) = export_job.as_ref() {
                let mut progress = job.progress();
                let status = job.status_text();
                d.draw_rectangle(0, 0, 1280, 720, Color::new(0, 0, 0, 180));
                d.draw_rectangle_rounded(
                    Rectangle::new(310.0, 270.0, 660.0, 150.0),
                    0.08,
                    12,
                    Color::new(18, 22, 30, 245),
                );
                d.draw_text("Exporting MP4", 350, 300, 30, Color::WHITE);
                d.draw_text(&status, 350, 340, 20, Color::new(220, 220, 220, 255));
                d.gui_progress_bar(
                    Rectangle::new(350.0, 375.0, 580.0, 24.0),
                    "",
                    &format!("{:>5.1}%", progress * 100.0),
                    &mut progress,
                    0.0,
                    1.0,
                );
            }
        } // Drop RaylibDrawHandle `d` here

        if load_bg
            && let Some(path) = rfd::FileDialog::new()
                .add_filter("Images", &["png", "jpg", "jpeg"])
                .pick_file()
        {
            bg_texture = load_filtered_texture(&mut rl, &thread, &path);
        }

        if load_logo
            && let Some(path) = rfd::FileDialog::new()
                .add_filter("Images", &["png", "jpg", "jpeg"])
                .pick_file()
        {
            logo_texture = load_filtered_texture(&mut rl, &thread, &path);
        }

        if load_track
            && let Some(path) = rfd::FileDialog::new()
                .add_filter("Audio", &["wav", "mp3", "ogg", "flac"])
                .pick_file()
        {
            if let Some(music) = &mut current_music {
                music.stop_stream();
            }

            if let Ok(new_music) = get_music(&rl_audio, path.to_str().unwrap_or("")) {
                new_music.play_stream();

                let vc = Arc::clone(&audio_state.volume);
                let sc = Arc::clone(&audio_state.spectrum);
                let mut fp = FftProcessor::new(audio::FFT_SIZE);

                let cb = move |buffer: &mut [f32], channels: u32| {
                    let mut v = vc.lock().unwrap();
                    *v = rms(buffer);
                    drop(v);
                    let new_spectrum = fp.process(buffer, channels);
                    if let Ok(mut spectrum) = sc.lock() {
                        for i in 0..new_spectrum.len() {
                            spectrum[i] = spectrum[i] * 0.85 + new_spectrum[i] * 0.15;
                        }
                    }
                };

                let mut cb1 = Box::new(cb);
                let handle = attach_audio_stream_processor_to_music(&new_music, &mut *cb1);
                let b_handle = Box::new(handle);
                Box::leak(b_handle);
                Box::leak(cb1);

                tot_secs = new_music.get_time_length() as i32;
                num_mins = tot_secs / 60;
                num_secs = tot_secs % 60;
                paused = false;

                let file_name = std::path::Path::new(&path)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();
                current_track_name = format!("Playing: {}", file_name);

                current_track_path = Some(path.to_str().unwrap_or("").to_string());
                current_music = Some(new_music);
                export_notice = None;
            }
        }

        if start_render && let Some(path) = &current_track_path {
            if let Some(music) = &mut current_music
                && !paused
            {
                music.pause_stream();
                paused = true;
            }

            let current_color = theme_colors[current_theme_idx];
            export_notice = None;

            match ExportJob::start(&mut rl, &thread, path, current_color) {
                Ok(job) => {
                    export_job = Some(job);
                }
                Err(err) => {
                    export_notice = Some(format!("Export failed to start: {}", err));
                }
            }
        }
    }

    Ok(())
}
