use crate::wav::WavData;
use std::f32::consts::PI;

#[derive(Debug, Clone, Copy)]
pub struct GeneratorConfig {
    pub sample_rate: u32,
    pub seconds: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct Tone {
    pub frequency_hz: f32,
    pub amplitude: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct WaveGenerator {
    left_pan: f32,
    right_pan: f32,
}

impl Default for WaveGenerator {
    fn default() -> Self {
        Self {
            left_pan: 0.9,
            right_pan: 0.75,
        }
    }
}

impl WaveGenerator {
    pub fn demo_wave(&self, config: GeneratorConfig) -> WavData {
        let frame_count = (config.sample_rate as f32 * config.seconds) as usize;
        let mut samples = Vec::with_capacity(frame_count * 2);

        let bass = Tone {
            frequency_hz: 110.0,
            amplitude: 0.28,
        };
        let lead = Tone {
            frequency_hz: 440.0,
            amplitude: 0.35,
        };

        for frame in 0..frame_count {
            let time = frame as f32 / config.sample_rate as f32;
            let sweep_frequency = 220.0 + 660.0 * (time / config.seconds);
            let envelope = fade_envelope(time, config.seconds);
            let value = envelope
                * (sine_sample(bass, time)
                    + sine_sample(lead, time)
                    + 0.18 * (2.0 * PI * sweep_frequency * time).sin());
            let left = clamp_to_i16(value * self.left_pan);
            let right =
                clamp_to_i16(value * self.right_pan + 0.08 * sine_sample(bass, time + 0.002));
            samples.push(left);
            samples.push(right);
        }

        WavData::new(config.sample_rate, 2, samples)
            .expect("generator produces valid stereo 16-bit PCM data")
    }
}

fn sine_sample(tone: Tone, time: f32) -> f32 {
    tone.amplitude * (2.0 * PI * tone.frequency_hz * time).sin()
}

fn fade_envelope(time: f32, seconds: f32) -> f32 {
    let fade = 0.04_f32.min(seconds / 4.0);
    if time < fade {
        time / fade
    } else if time > seconds - fade {
        (seconds - time) / fade
    } else {
        1.0
    }
    .clamp(0.0, 1.0)
}

fn clamp_to_i16(value: f32) -> i16 {
    let scaled = (value.clamp(-1.0, 1.0) * i16::MAX as f32).round();
    scaled as i16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_wave_has_expected_duration_and_channels() {
        let generator = WaveGenerator::default();
        let wav = generator.demo_wave(GeneratorConfig {
            sample_rate: 1000,
            seconds: 1.5,
        });
        assert_eq!(wav.channels, 2);
        assert_eq!(wav.frame_count(), 1500);
    }

    #[test]
    fn envelope_is_bounded() {
        assert_eq!(fade_envelope(-1.0, 2.0), 0.0);
        assert!((fade_envelope(1.0, 2.0) - 1.0).abs() < f32::EPSILON);
        assert_eq!(fade_envelope(3.0, 2.0), 0.0);
    }
}
