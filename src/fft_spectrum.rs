use rustfft::FftPlanner;
use rustfft::num_complex::Complex;
use std::f32::consts::PI;

pub const MIN_FREQ_HZ: f32 = 0.0;
pub const MAX_FREQ_HZ: f32 = 20_000.0;
pub const MIN_DB: f32 = -90.0;
pub const MAX_DB: f32 = 12.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FftSpectrumPoint {
    pub frequency_hz: f32,
    pub db: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FftSpectrum {
    pub points: Vec<FftSpectrumPoint>,
    pub peak_frequency_hz: f32,
    pub peak_db: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct FftSpectrumAnalyzer {
    window_size: usize,
}

impl FftSpectrumAnalyzer {
    pub fn new(window_size: usize) -> Self {
        Self {
            window_size: window_size.next_power_of_two().max(128),
        }
    }

    pub fn analyze(&self, samples: &[f32], sample_rate: u32) -> FftSpectrum {
        if sample_rate == 0 {
            return empty_spectrum();
        }

        let mut buffer = vec![Complex::new(0.0_f32, 0.0_f32); self.window_size];
        let copy_len = samples.len().min(self.window_size);
        let source_start = samples.len().saturating_sub(copy_len);

        for (index, sample) in samples[source_start..].iter().take(copy_len).enumerate() {
            let window = hann_weight(index, self.window_size);
            buffer[index].re = sample * window;
        }

        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(self.window_size);
        fft.process(&mut buffer);

        let max_bin = self.window_size / 2;
        let hz_per_bin = sample_rate as f32 / self.window_size as f32;
        let mut points = Vec::new();
        let mut peak_frequency_hz = 0.0;
        let mut peak_db = MIN_DB;

        for (bin, value) in buffer.iter().take(max_bin).enumerate() {
            let frequency_hz = bin as f32 * hz_per_bin;
            if !(MIN_FREQ_HZ..=MAX_FREQ_HZ).contains(&frequency_hz) {
                continue;
            }
            let magnitude = value.norm() / self.window_size as f32;
            let db = amplitude_to_db(magnitude).clamp(MIN_DB, MAX_DB);
            if db > peak_db {
                peak_db = db;
                peak_frequency_hz = frequency_hz;
            }
            points.push(FftSpectrumPoint { frequency_hz, db });
        }

        FftSpectrum {
            points,
            peak_frequency_hz,
            peak_db,
        }
    }
}

pub fn smooth_spectrum(
    previous: &[FftSpectrumPoint],
    current: &FftSpectrum,
    amount: f32,
) -> FftSpectrum {
    if previous.len() != current.points.len() {
        return current.clone();
    }
    let keep = amount.clamp(0.0, 0.98);
    let add = 1.0 - keep;
    let points: Vec<FftSpectrumPoint> = previous
        .iter()
        .zip(&current.points)
        .map(|(old, new)| FftSpectrumPoint {
            frequency_hz: new.frequency_hz,
            db: old.db * keep + new.db * add,
        })
        .collect();

    let peak = points
        .iter()
        .max_by(|left, right| left.db.total_cmp(&right.db))
        .copied();

    FftSpectrum {
        points,
        peak_frequency_hz: peak.map(|point| point.frequency_hz).unwrap_or_default(),
        peak_db: peak.map(|point| point.db).unwrap_or(MIN_DB),
    }
}

fn hann_weight(index: usize, len: usize) -> f32 {
    if len <= 1 {
        1.0
    } else {
        0.5 - 0.5 * (2.0 * PI * index as f32 / (len - 1) as f32).cos()
    }
}

fn amplitude_to_db(amplitude: f32) -> f32 {
    20.0 * amplitude.max(1.0e-8).log10() + 12.0
}

fn empty_spectrum() -> FftSpectrum {
    FftSpectrum {
        points: Vec::new(),
        peak_frequency_hz: 0.0,
        peak_db: MIN_DB,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fft_detects_tone_frequency() {
        let sample_rate = 44_100_u32;
        let samples: Vec<f32> = (0..4096)
            .map(|index| {
                let time = index as f32 / sample_rate as f32;
                (2.0 * PI * 1000.0 * time).sin()
            })
            .collect();
        let spectrum = FftSpectrumAnalyzer::new(4096).analyze(&samples, sample_rate);
        assert!((spectrum.peak_frequency_hz - 1000.0).abs() < 40.0);
    }

    #[test]
    fn smoothing_keeps_point_count() {
        let current = FftSpectrum {
            points: vec![FftSpectrumPoint {
                frequency_hz: 100.0,
                db: -10.0,
            }],
            peak_frequency_hz: 100.0,
            peak_db: -10.0,
        };
        let smoothed = smooth_spectrum(
            &[FftSpectrumPoint {
                frequency_hz: 100.0,
                db: -50.0,
            }],
            &current,
            0.5,
        );
        assert_eq!(smoothed.points.len(), 1);
        assert!((smoothed.points[0].db + 30.0).abs() < 0.01);
    }
}
