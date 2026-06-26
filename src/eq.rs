use crate::error::AppResult;
use crate::signal::{AudioBuffer, SampleProvider};
use std::f32::consts::PI;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EqSettings {
    pub low_gain_db: f32,
    pub mid_gain_db: f32,
    pub high_gain_db: f32,
}

impl Default for EqSettings {
    fn default() -> Self {
        Self {
            low_gain_db: 0.0,
            mid_gain_db: 0.0,
            high_gain_db: 0.0,
        }
    }
}

impl EqSettings {
    pub fn is_flat(self) -> bool {
        self.low_gain_db.abs() < 0.05
            && self.mid_gain_db.abs() < 0.05
            && self.high_gain_db.abs() < 0.05
    }
}

pub fn apply_three_band_eq(audio: &AudioBuffer, settings: EqSettings) -> AppResult<AudioBuffer> {
    if settings.is_flat() {
        return Ok(audio.clone());
    }

    let sample_rate = audio.sample_rate() as f32;
    let mut channels = Vec::with_capacity(audio.channel_count());
    for channel_index in 0..audio.channel_count() {
        let mut samples = audio.channel(channel_index).unwrap_or_default().to_vec();
        apply_peak_filter(&mut samples, sample_rate, 120.0, 0.70, settings.low_gain_db);
        apply_peak_filter(
            &mut samples,
            sample_rate,
            1_000.0,
            0.90,
            settings.mid_gain_db,
        );
        apply_peak_filter(
            &mut samples,
            sample_rate,
            8_000.0,
            0.70,
            settings.high_gain_db,
        );
        soft_limit(&mut samples);
        channels.push(samples);
    }

    AudioBuffer::new(audio.sample_rate(), channels)
}

fn apply_peak_filter(
    samples: &mut [f32],
    sample_rate: f32,
    frequency_hz: f32,
    q: f32,
    gain_db: f32,
) {
    if gain_db.abs() < 0.05 || samples.is_empty() {
        return;
    }

    let coefficients = BiquadCoefficients::peaking_eq(sample_rate, frequency_hz, q, gain_db);
    let mut state = BiquadState::default();
    for sample in samples {
        *sample = state.process(*sample, coefficients);
    }
}

fn soft_limit(samples: &mut [f32]) {
    for sample in samples {
        *sample = sample.clamp(-1.2, 1.2).tanh();
    }
}

#[derive(Debug, Clone, Copy)]
struct BiquadCoefficients {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
}

impl BiquadCoefficients {
    fn peaking_eq(sample_rate: f32, frequency_hz: f32, q: f32, gain_db: f32) -> Self {
        let frequency_hz = frequency_hz.clamp(10.0, sample_rate * 0.45);
        let a = 10.0_f32.powf(gain_db / 40.0);
        let omega = 2.0 * PI * frequency_hz / sample_rate;
        let sin = omega.sin();
        let cos = omega.cos();
        let alpha = sin / (2.0 * q.max(0.1));

        let b0 = 1.0 + alpha * a;
        let b1 = -2.0 * cos;
        let b2 = 1.0 - alpha * a;
        let a0 = 1.0 + alpha / a;
        let a1 = -2.0 * cos;
        let a2 = 1.0 - alpha / a;

        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct BiquadState {
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl BiquadState {
    fn process(&mut self, input: f32, coefficients: BiquadCoefficients) -> f32 {
        let output =
            coefficients.b0 * input + coefficients.b1 * self.x1 + coefficients.b2 * self.x2
                - coefficients.a1 * self.y1
                - coefficients.a2 * self.y2;

        self.x2 = self.x1;
        self.x1 = input;
        self.y2 = self.y1;
        self.y1 = output;
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_eq_keeps_samples() {
        let audio = AudioBuffer::new(44_100, vec![vec![0.1, -0.2, 0.3]]).unwrap();
        let processed = apply_three_band_eq(&audio, EqSettings::default()).unwrap();
        assert_eq!(processed.channel(0).unwrap(), audio.channel(0).unwrap());
    }

    #[test]
    fn boosted_eq_changes_samples() {
        let audio = AudioBuffer::new(44_100, vec![vec![0.1; 256]]).unwrap();
        let processed = apply_three_band_eq(
            &audio,
            EqSettings {
                low_gain_db: 6.0,
                mid_gain_db: 0.0,
                high_gain_db: 0.0,
            },
        )
        .unwrap();
        assert_ne!(processed.channel(0).unwrap(), audio.channel(0).unwrap());
    }
}
