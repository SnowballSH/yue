# Yue

Yue (乐) is a desktop music visualizer and MP4 exporter written in Rust with Raylib.
It renders a live audio-reactive preview, lets you load custom background/logo
art, and exports a high-quality H.264/ALAC video through FFmpeg.

<img width="1392" height="864" alt="image" src="https://github.com/user-attachments/assets/98723dbb-cbe6-4b79-bbd8-90394d071de9" />

## Current Features

- Live preview window at `1280x720`
- Audio-reactive center ring and circular spectrum visualization
- Track loading for `wav`, `mp3`, `ogg`, and `flac`
- Optional background image and center logo image
- Async export progress overlay so the UI stays responsive during MP4 generation
- High-quality export pipeline:
  - logical composition at `1920x1080`
  - supersampled internal render at `3840x2160`
  - downscale with `lanczos`
  - video encoded with `libx264`
  - audio encoded with `ALAC`

## Requirements

- Rust toolchain
- FFmpeg available on `PATH`
- Raylib build/runtime prerequisites for your platform

## Run

```bash
cargo run
```

## Controls

- `Select Track`: load an audio file
- `Cycle Theme`: switch accent color
- `Load BG Image`: load a background image
- `Load Ring Image`: load the center logo image
- `Render MP4`: export `output.mp4`
- `Space`: pause/resume playback
- Slider: seek through the current track

## Export Notes

- Export currently writes to `output.mp4` in the project root.
- A temporary WAV file is created in the system temp directory during export.
- The UI remains interactive enough to display progress while export work advances frame-by-frame on the main loop.

## Development

Useful commands:

```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

## Project Layout

- `src/main.rs`: app entry point, UI loop, file picking, playback, export job lifecycle
- `src/export.rs`: offline export pipeline, FFmpeg integration, WAV decoding, export math
- `src/audio.rs`: shared audio state and RMS helper
- `src/fft.rs`: FFT processing
- `src/visuals/`: visual composition modules
  - `core_ring.rs`: center ring and logo rendering
  - `spectrum.rs`: circular spectrum points/trails
  - `mod.rs`: visualizer composition and canvas scaling

## Testing

The project has unit coverage for:

- RMS calculation
- FFT peak detection for a sine wave
- Core ring state updates
- Spectrum update behavior
- Export helper logic
- WAV sample decoding for float and int sources

## Name

`Yue` (`乐`) refers to music.
