use crate::decoder::decode_audio_file;
use crate::eq::{EqSettings, apply_three_band_eq};
use crate::error::{AppError, AppResult};
use crate::fft_spectrum::{
    FftSpectrum, FftSpectrumAnalyzer, MAX_DB, MAX_FREQ_HZ, MIN_DB, smooth_spectrum,
};
use crate::player::AudioPlayer;
use crate::signal::{AudioBuffer, SampleProvider};
use eframe::egui;
use egui_plot::{Line, Plot, PlotPoints};
use std::path::{Path, PathBuf};
use std::time::Duration;

const GUI_WINDOW_SIZE: usize = 4096;
const SPECTRUM_SMOOTHING: f32 = 0.72;

pub fn run_gui(initial_path: Option<PathBuf>) -> AppResult<()> {
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "WaveScope",
        native_options,
        Box::new(|_cc| Box::new(WaveScopeGui::new(initial_path))),
    )
    .map_err(|err| AppError::gui(err.to_string()))
}

struct WaveScopeGui {
    path_input: String,
    original_audio: Option<AudioBuffer>,
    processed_audio: Option<AudioBuffer>,
    player: Option<AudioPlayer>,
    spectrum: Option<FftSpectrum>,
    analyzer: FftSpectrumAnalyzer,
    eq_settings: EqSettings,
    status: String,
}

impl WaveScopeGui {
    fn new(initial_path: Option<PathBuf>) -> Self {
        let mut app = Self {
            path_input: initial_path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_default(),
            original_audio: None,
            processed_audio: None,
            player: AudioPlayer::new().ok(),
            spectrum: None,
            analyzer: FftSpectrumAnalyzer::new(GUI_WINDOW_SIZE),
            eq_settings: EqSettings::default(),
            status: String::from("Enter an audio path, then click Load."),
        };

        if let Some(path) = initial_path {
            app.load_path(&path);
        }
        app
    }

    fn load_current_path(&mut self) {
        let path = PathBuf::from(self.path_input.trim());
        self.load_path(&path);
    }

    fn load_path(&mut self, path: &Path) {
        match decode_audio_file(path) {
            Ok(audio) => {
                self.status = format!(
                    "Loaded {} ({:.2}s, {} Hz, {} channel(s))",
                    path.display(),
                    audio.duration_seconds(),
                    audio.sample_rate(),
                    audio.channel_count()
                );
                self.original_audio = Some(audio);
                self.rebuild_processed_audio();
            }
            Err(err) => {
                self.status = err.to_string();
                self.original_audio = None;
                self.processed_audio = None;
                self.spectrum = None;
                if let Some(player) = &mut self.player {
                    player.stop();
                }
            }
        }
    }

    fn rebuild_processed_audio(&mut self) {
        let Some(original) = &self.original_audio else {
            return;
        };

        match apply_three_band_eq(original, self.eq_settings) {
            Ok(processed) => {
                if let Some(player) = &mut self.player
                    && let Err(err) = player.load_audio(&processed)
                {
                    self.status = err.to_string();
                }
                self.spectrum = Some(self.spectrum_at(&processed, self.current_seconds()));
                self.processed_audio = Some(processed);
            }
            Err(err) => self.status = err.to_string(),
        }
    }

    fn play(&mut self) {
        match &mut self.player {
            Some(player) => {
                if let Err(err) = player.play() {
                    self.status = err.to_string();
                }
            }
            None => self.status = String::from("No audio output device is available."),
        }
    }

    fn pause(&mut self) {
        if let Some(player) = &mut self.player {
            player.pause();
        }
    }

    fn stop(&mut self) {
        if let Some(player) = &mut self.player {
            player.stop();
            if let Some(audio) = &self.processed_audio {
                self.spectrum = Some(self.spectrum_at(audio, 0.0));
            }
        }
    }

    fn current_seconds(&self) -> f32 {
        self.player
            .as_ref()
            .map(|player| player.position().as_secs_f32())
            .unwrap_or_default()
    }

    fn refresh_synced_spectrum(&mut self) {
        if let Some(audio) = &self.processed_audio {
            let current = self.spectrum_at(audio, self.current_seconds());
            self.spectrum = Some(match &self.spectrum {
                Some(previous) => smooth_spectrum(&previous.points, &current, SPECTRUM_SMOOTHING),
                None => current,
            });
        }
    }

    fn spectrum_at(&self, audio: &AudioBuffer, seconds: f32) -> FftSpectrum {
        let center_frame = (seconds * audio.sample_rate() as f32) as usize;
        let window = audio.mono_window_at(center_frame, GUI_WINDOW_SIZE);
        self.analyzer.analyze(&window, audio.sample_rate())
    }

    fn draw_controls(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Audio file");
            ui.text_edit_singleline(&mut self.path_input);
            if ui.button("Load").clicked() {
                self.load_current_path();
            }
        });

        ui.horizontal(|ui| {
            if ui.button("Play").clicked() {
                self.play();
            }
            if ui.button("Pause").clicked() {
                self.pause();
            }
            if ui.button("Stop").clicked() {
                self.stop();
            }
        });
    }

    fn draw_eq(&mut self, ui: &mut egui::Ui) {
        let before = self.eq_settings;
        ui.horizontal(|ui| {
            ui.label("EQ");
            ui.add(
                egui::Slider::new(&mut self.eq_settings.low_gain_db, -12.0..=12.0)
                    .text("Low 120 Hz"),
            );
            ui.add(
                egui::Slider::new(&mut self.eq_settings.mid_gain_db, -12.0..=12.0)
                    .text("Mid 1 kHz"),
            );
            ui.add(
                egui::Slider::new(&mut self.eq_settings.high_gain_db, -12.0..=12.0)
                    .text("High 8 kHz"),
            );
            if ui.button("Flat").clicked() {
                self.eq_settings = EqSettings::default();
            }
        });
        if before != self.eq_settings {
            self.rebuild_processed_audio();
        }
    }

    fn draw_summary(&self, ui: &mut egui::Ui) {
        ui.label(&self.status);
        if let Some(audio) = &self.processed_audio {
            ui.horizontal(|ui| {
                ui.label(format!(
                    "Time: {:.2}s / {:.2}s",
                    self.current_seconds(),
                    audio.duration_seconds()
                ));
                ui.label(format!("Frames: {}", audio.frames()));
                ui.label(format!("Channels: {}", audio.channel_count()));
            });
        }
    }

    fn draw_waveform(&self, ui: &mut egui::Ui) {
        let Some(audio) = &self.processed_audio else {
            return;
        };
        let mono = audio.mono_mix();
        let step = (mono.len() / 1600).max(1);
        let points = PlotPoints::from_iter(
            mono.iter()
                .enumerate()
                .step_by(step)
                .map(|(index, sample)| [index as f64 / audio.sample_rate() as f64, *sample as f64]),
        );
        Plot::new("waveform")
            .height(180.0)
            .allow_scroll(false)
            .allow_zoom(false)
            .show(ui, |plot_ui| {
                plot_ui.line(Line::new(points));
                let position = self.current_seconds() as f64;
                let playhead = PlotPoints::from_iter([[position, -1.0], [position, 1.0]]);
                plot_ui.line(Line::new(playhead).color(egui::Color32::LIGHT_RED));
            });
    }

    fn draw_spectrum(&self, ui: &mut egui::Ui) {
        let Some(spectrum) = &self.spectrum else {
            return;
        };
        let points = PlotPoints::from_iter(
            spectrum
                .points
                .iter()
                .map(|point| [point.frequency_hz as f64, point.db as f64]),
        );
        Plot::new("spectrum")
            .height(240.0)
            .include_x(0.0)
            .include_x(MAX_FREQ_HZ as f64)
            .include_y(MIN_DB as f64)
            .include_y(MAX_DB as f64)
            .allow_scroll(false)
            .allow_zoom(false)
            .show(ui, |plot_ui| {
                plot_ui.line(Line::new(points));
            });
    }
}

impl eframe::App for WaveScopeGui {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let playing = self
            .player
            .as_ref()
            .map(AudioPlayer::is_playing)
            .unwrap_or(false);
        if playing {
            self.refresh_synced_spectrum();
            ctx.request_repaint_after(Duration::from_millis(50));
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("WaveScope");
            self.draw_controls(ui);
            self.draw_eq(ui);
            ui.separator();
            self.draw_summary(ui);
            ui.separator();
            ui.label("Waveform");
            self.draw_waveform(ui);
            ui.label("Synchronized spectrum");
            self.draw_spectrum(ui);
        });
    }
}
