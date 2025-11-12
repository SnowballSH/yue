use raylib::prelude::*;
use std::sync::{Arc, Mutex};

#[inline]
fn rms(a: &mut [f32]) -> f32 {
    if a.is_empty() {
        return 0.0;
    }
    a.iter().map(|&x| x * x).sum::<f32>() / a.len() as f32
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path =
        "/Users/snowballsh/Documents/music/thrive_project/thrive Project/SnowballSH - Journey.wav";

    let (mut rl, thread) = init().size(640, 480).title("Hello, World").build();
    let rl_audio = RaylibAudio::init_audio_device()?;
    let music = rl_audio.new_music(path)?;
    music.play_stream();

    let mut volume = Arc::new(Mutex::new(0.0f32));

    let mut volume_callback = Arc::clone(&volume);

    let mut callback = move |buffer: &mut [f32], frames: u32| {
        // println!("buffer len: {}", buffer.len());
        // println!("frames setup: {}", frames);
        // println!("{:?}", buffer);
        // for frame in buffer.iter_mut() {
        //     *frame = frame.abs();
        // }
        let mut v = volume_callback.lock().unwrap();
        *v = rms(buffer);
    };

    let _callback = attach_audio_stream_processor_to_music(&music, &mut callback);

    rl.set_window_title(&thread, "Yue");

    let tot_secs = music.get_time_length() as i32;
    let num_mins = tot_secs / 60;
    let num_secs = tot_secs % 60;

    let mut timestamp = 0.0f32;

    let mut paused = false;

    while !rl.window_should_close() {
        music.update_stream();

        if rl.is_key_pressed(KeyboardKey::KEY_SPACE) {
            if paused {
                music.resume_stream();
            } else {
                music.pause_stream();
            }
            paused = !paused;
        }

        let mut d = rl.begin_drawing(&thread);

        d.clear_background(Color::WHITE);

        // BEGIN control buttons
        let to_toggle = d.gui_button(
            Rectangle {
                x: 290.0,
                y: 400.0,
                width: 50.0,
                height: 20.0,
            },
            if paused { "Resume" } else { "Pause" },
        );
        if to_toggle {
            if paused {
                paused = false;
                music.resume_stream();
            } else {
                music.pause_stream();
                paused = true;
            }
        }
        // END control buttons

        // BEGIN slider
        timestamp = music.get_time_played();
        let old_time = timestamp;
        d.gui_slider(
            Rectangle {
                x: 50.0,
                y: 430.0,
                width: 540.0,
                height: 28.0,
            },
            &"0:00".to_string(),
            &format!("{}:{:02}", num_mins, num_secs),
            &mut timestamp,
            0.0f32,
            tot_secs as f32,
        );
        if (timestamp - old_time).abs() > 0.01 {
            music.seek_stream(timestamp);
        }
        // END slider

        let cur_volume = *volume.lock().unwrap();

        d.draw_text(&format!("Volume: {}", cur_volume), 12, 12, 20, Color::BLACK);

        let volume_bar_width = (cur_volume * 5000.0).clamp(0.0, 616.0) as i32;
        d.draw_rectangle(12, 40, volume_bar_width, 20, Color::RED);
        d.draw_rectangle_lines(12, 40, 616, 20, Color::BLACK);
    }
    music.stop_stream();

    Ok(())
}
