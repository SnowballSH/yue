# Yue Agent Notes

This file records project-specific context for contributors and coding agents
working inside the Yue repository.

## Project Summary

Yue is a Rust/Raylib desktop music visualizer with a built-in offline MP4 export
path. The live app previews the visualizer in a `1280x720` window, while export
renders a `1920x1080` composition into a supersampled `3840x2160` buffer before
FFmpeg downsamples and encodes the final video.

## Architecture

The codebase is intentionally split into a small number of modules:

1. `src/main.rs`
   Owns the window, GUI controls, music playback, file loading, and export job
   lifecycle.

2. `src/audio.rs`
   Holds shared live-audio state and simple helpers such as `rms`.

3. `src/fft.rs`
   Contains `FftProcessor`, which converts audio buffers into spectrum data.

4. `src/export.rs`
   Owns offline export. This includes:
   - temporary WAV preparation with FFmpeg
   - WAV sample decoding with `hound`
   - frame stepping and export progress tracking
   - supersampled render target configuration
   - final FFmpeg encode settings

5. `src/visuals/`
   Contains the current visual composition:
   - `core_ring.rs`
   - `spectrum.rs`
   - `mod.rs`

## Visualizer Model

The visualizer currently has two active visual elements:

- `CoreRing`
  Maintains ring radius, floating offset, glow, and optional center logo.

- `Spectrum`
  Maintains circular point/trail data derived from FFT magnitudes.

`VisualizerCanvas` in `src/visuals/mod.rs` separates logical layout size from
physical render size. This is important: export quality uses a larger render
buffer, but visual density and composition should follow the logical layout.

## Export Invariants

If you change export behavior, keep these aligned unless you intentionally
change the output format:

- logical export size: `1920x1080`
- supersampled render size: `3840x2160`
- framerate: `60 FPS`
- temp audio sample rate: `48_000 Hz`
- final output path: `output.mp4`

The export loop is incremental on purpose. Do not regress it back to a fully
blocking one-shot render unless explicitly requested.

## External Dependencies

- FFmpeg is required at runtime for export.
- `raylib` handles rendering and audio playback.
- `hound` is used for WAV decoding in export tests and runtime.
- `rustfft` powers spectrum generation.
- `rfd` is used for native file dialogs.

## Testing Expectations

Before finishing non-trivial changes, prefer running:

```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Keep new logic testable with pure helpers when possible. `src/export.rs`
already exposes several pure helper seams through internal functions covered by
unit tests.

## Editing Guidance

- Preserve the current project name: `Yue`.
- Update docs when changing export dimensions, codecs, controls, or module layout.
- Avoid reintroducing dead visual modules. The previous standalone particle
  system is intentionally removed from the active renderer.
- Prefer small pure helpers for new math/config logic so it can be unit tested
  without a GUI context.
