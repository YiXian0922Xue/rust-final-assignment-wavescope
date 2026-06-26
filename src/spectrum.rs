use crate::error::{AppError, AppResult};
use crate::signal::SampleProvider;
use std::f32::consts::PI;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpectrumConfig {
    pub window_size: usize,
    pub bins: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpectrumBin {
    pub index: usize,
    pub start_hz: f32,
    pub end_hz: f32,
    pub center_hz: f32,
    pub magnitude: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Spectrum {
    pub bins: Vec<SpectrumBin>,
    pub max_frequency_hz: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowKind {
    Hann,
    Rectangular,
}

#[derive(Debug, Clone, Copy)]
pub struct SpectrumStrategy {
    window: WindowKind,
}

pub trait WindowFunction {
    fn weight(&self, index: usize, len: usize) -> f32;
}

impl SpectrumConfig {
    pub fn new(window_size: usize, bins: usize) -> AppResult<Self> {
        if !window_size.is_power_of_two() || window_size < 2 {
            return Err(AppError::invalid_argument(
                "spectrum window size must be a power of two",
            ));
        }
        if bins == 0 {
            return Err(AppError::invalid_argument("spectrum bins cannot be zero"));
        }
        Ok(Self { window_size, bins })
    }
}

impl Spectrum {
    pub fn dominant_bin(&self) -> Option<&SpectrumBin> {
        self.bins
            .iter()
            .max_by(|left, right| left.magnitude.total_cmp(&right.magnitude))
    }
}

impl Default for SpectrumStrategy {
    fn default() -> Self {
        Self {
            window: WindowKind::Hann,
        }
    }
}

impl SpectrumStrategy {
    pub fn with_window(window: WindowKind) -> Self {
        Self { window }
    }

    pub fn analyze<S>(&self, source: &S, config: &SpectrumConfig) -> Spectrum
    where
        S: SampleProvider,
    {
        let mono = source
            .channel(0)
            .map(|_| mix_source_to_mono(source))
            .unwrap_or_default();
        let windowed = prepare_window(&mono, config.window_size, self.window);
        let raw = dft_magnitudes(&windowed);
        group_bins(&raw, source.sample_rate(), config.bins)
    }
}

impl WindowFunction for WindowKind {
    fn weight(&self, index: usize, len: usize) -> f32 {
        match self {
            WindowKind::Hann => {
                if len <= 1 {
                    1.0
                } else {
                    0.5 - 0.5 * (2.0 * PI * index as f32 / (len - 1) as f32).cos()
                }
            }
            WindowKind::Rectangular => 1.0,
        }
    }
}

fn mix_source_to_mono<S>(source: &S) -> Vec<f32>
where
    S: SampleProvider,
{
    let frames = source.frames();
    let channels = source.channel_count();
    let mut mono = Vec::with_capacity(frames);
    for frame_index in 0..frames {
        let sum: f32 = (0..channels)
            .filter_map(|channel| source.channel(channel))
            .map(|samples| samples[frame_index])
            .sum();
        mono.push(sum / channels as f32);
    }
    mono
}

fn prepare_window(samples: &[f32], window_size: usize, window: WindowKind) -> Vec<f32> {
    let mut prepared = vec![0.0_f32; window_size];
    let copy_len = samples.len().min(window_size);
    let start = samples.len().saturating_sub(copy_len);
    for (target_index, sample) in samples[start..].iter().take(copy_len).enumerate() {
        prepared[target_index] = sample * window.weight(target_index, window_size);
    }
    prepared
}

fn dft_magnitudes(samples: &[f32]) -> Vec<f32> {
    let half = samples.len() / 2;
    let len = samples.len() as f32;
    let mut magnitudes = Vec::with_capacity(half);

    for bin in 0..half {
        let mut real = 0.0_f32;
        let mut imag = 0.0_f32;
        for (sample_index, sample) in samples.iter().enumerate() {
            let angle = 2.0 * PI * bin as f32 * sample_index as f32 / len;
            real += sample * angle.cos();
            imag -= sample * angle.sin();
        }
        magnitudes.push((real * real + imag * imag).sqrt() / len);
    }

    magnitudes
}

fn group_bins(raw: &[f32], sample_rate: u32, target_bins: usize) -> Spectrum {
    if raw.is_empty() || target_bins == 0 {
        return Spectrum {
            bins: Vec::new(),
            max_frequency_hz: 0.0,
        };
    }

    let max_frequency_hz = sample_rate as f32 / 2.0;
    let frequency_per_raw_bin = max_frequency_hz / raw.len() as f32;
    let raw_per_target = raw.len() as f32 / target_bins as f32;
    let mut bins = Vec::with_capacity(target_bins);

    for index in 0..target_bins {
        let start_raw = (index as f32 * raw_per_target).floor() as usize;
        let end_raw = (((index + 1) as f32 * raw_per_target).ceil() as usize).min(raw.len());
        let slice = &raw[start_raw..end_raw.max(start_raw + 1).min(raw.len())];
        let magnitude = if slice.is_empty() {
            0.0
        } else {
            slice.iter().copied().fold(0.0_f32, f32::max)
        };
        let start_hz = start_raw as f32 * frequency_per_raw_bin;
        let end_hz = end_raw as f32 * frequency_per_raw_bin;
        bins.push(SpectrumBin {
            index,
            start_hz,
            end_hz,
            center_hz: (start_hz + end_hz) / 2.0,
            magnitude,
        });
    }

    Spectrum {
        bins,
        max_frequency_hz,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signal::AudioBuffer;

    #[test]
    fn hann_window_starts_and_ends_near_zero() {
        let window = WindowKind::Hann;
        assert!(window.weight(0, 8).abs() < 0.001);
        assert!(window.weight(7, 8).abs() < 0.001);
    }

    #[test]
    fn detects_approximately_440_hz_tone() {
        let sample_rate = 8_000_u32;
        let samples: Vec<f32> = (0..2048)
            .map(|index| {
                let time = index as f32 / sample_rate as f32;
                (2.0 * PI * 440.0 * time).sin()
            })
            .collect();
        let audio = AudioBuffer::new(sample_rate, vec![samples]).unwrap();
        let config = SpectrumConfig::new(2048, 64).unwrap();
        let spectrum =
            SpectrumStrategy::with_window(WindowKind::Rectangular).analyze(&audio, &config);
        let dominant = spectrum.dominant_bin().unwrap();
        assert!((dominant.center_hz - 440.0).abs() < 80.0);
    }

    #[test]
    fn spectrum_has_requested_number_of_bins() {
        let audio = AudioBuffer::new(1000, vec![vec![0.0; 128]]).unwrap();
        let config = SpectrumConfig::new(128, 16).unwrap();
        let spectrum = SpectrumStrategy::default().analyze(&audio, &config);
        assert_eq!(spectrum.bins.len(), 16);
    }
}
