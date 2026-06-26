# WaveScope

WaveScope is a small Rust audio spectrum analyzer for course project use. It supports both command-line analysis and a simple native GUI. The GUI can show a waveform, a real-time 0-20000 Hz FFT spectrum in dB, synchronized playback, and a simple fixed three-band EQ.

The project can be opened directly in RustRover and built with Cargo. It uses a few common Rust crates for decoding, playback, GUI, plotting, and FFT, but it does not require installing a separate audio workstation or native DSP SDK.

## Features

- Decode common audio formats with Symphonia: WAV, MP3, FLAC, OGG/Vorbis, MP4/M4A, AAC, ALAC, AIFF, CAF, and others supported by enabled Symphonia features
- Prefer the self-written WAV parser for simple 16-bit PCM WAV files
- Command-line analysis with duration, sample rate, channel stats, waveform preview, text spectrum, and optional CSV export
- Built-in demo WAV generator
- Native GUI built with eframe/egui
- Waveform display with a playback cursor
- FFT spectrum display from 0 Hz to 20000 Hz, with dB amplitude
- Synchronized spectrum refresh during playback
- Simple fixed EQ controls:
  - Low band around 120 Hz
  - Mid band around 1 kHz
  - High band around 8 kHz
  - Each band supports -12 dB to +12 dB
- Unit tests for argument parsing, WAV I/O, signal helpers, FFT spectrum detection, EQ processing, and rendering helpers

## Requirements

- Rust stable toolchain
- Cargo
- RustRover or another Cargo-compatible editor

Dependencies are downloaded by Cargo. The project includes `.cargo/config.toml`, which uses the `rsproxy.cn` sparse registry mirror to make dependency downloads more stable in domestic network environments.

## Quick Start

Download dependencies:

```powershell
cargo fetch
```

Show help:

```powershell
cargo run -- help
```

Generate a demo audio file:

```powershell
cargo run -- generate demo.wav
```

Analyze an audio file in the terminal:

```powershell
cargo run -- analyze demo.wav
```

Export terminal spectrum data to CSV:

```powershell
cargo run -- analyze demo.wav --bins 64 --window 2048 --window-kind hann --csv spectrum.csv
```

Open the GUI:

```powershell
cargo run -- gui
```

Open the GUI and load an audio file immediately:

```powershell
cargo run -- gui demo.wav
```

## Commands

```text
cargo run -- analyze <input-audio> [--bins 48] [--window 2048] [--window-kind hann] [--csv spectrum.csv]
cargo run -- generate <output.wav> [--seconds 3.0] [--rate 44100]
cargo run -- gui [input-audio]
```

Arguments:

- `analyze <input-audio>`: analyze an audio file in the terminal
- `--bins`: number of text spectrum bars, from 8 to 120, default 48
- `--window`: command-line analysis window size, power of two from 128 to 16384, default 2048
- `--window-kind`: `hann` or `rectangular`, default `hann`
- `--csv`: export terminal spectrum bins to CSV
- `generate <output.wav>`: create a demo WAV file
- `--seconds`: demo duration from 0.1 to 30.0 seconds, default 3.0
- `--rate`: demo sample rate from 8000 to 192000, default 44100
- `gui [input-audio]`: open the native GUI, optionally loading an audio file

## Project Structure

```text
src/
  main.rs          Program entry point
  app.rs           CLI parsing and command dispatch
  decoder.rs       Multi-format decoding with Symphonia
  eq.rs            Three-band biquad EQ
  error.rs         Shared error type and AppResult alias
  fft_spectrum.rs  FFT spectrum analysis for GUI display
  generator.rs     Demo WAV generator
  gui.rs           eframe/egui native interface
  player.rs        rodio playback from processed samples
  render.rs        Terminal output and CSV export
  signal.rs        Audio buffer, stats, helpers
  spectrum.rs      Text-mode DFT spectrum analysis
  wav.rs           Self-written WAV reader/writer
.cargo/
  config.toml      Cargo registry mirror config
```

## Checks

```powershell
cargo fmt -- --check
cargo test
cargo clippy -- -D warnings
```

## Implementation Notes

- `AudioBuffer` is the common internal representation for decoded audio.
- `decoder.rs` uses Symphonia for general audio decoding and falls back to the self-written WAV path for simple PCM WAV files.
- `fft_spectrum.rs` uses `rustfft` to compute a denser spectrum curve for the GUI.
- `eq.rs` implements three fixed peaking filters using biquad coefficients.
- `player.rs` plays the processed in-memory samples, so EQ changes affect playback.
- The CLI keeps the original text spectrum design for simple terminal demonstrations.

## Limitations

- The GUI uses a path text box instead of a file picker to keep dependencies small.
- Playback synchronization is project-level synchronization based on elapsed playback time, not sample-accurate DAW synchronization.
- EQ is intentionally simple and fixed-band; it is not a full parametric EQ like Pro-Q.
- FFT display is designed for visualization and course demonstration, not professional metering calibration.
