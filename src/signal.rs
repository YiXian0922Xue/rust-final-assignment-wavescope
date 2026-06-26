use crate::error::{AppError, AppResult};
use crate::spectrum::Spectrum;
use crate::wav::WavData;

#[derive(Debug, Clone)]
pub struct AudioBuffer {
    sample_rate: u32,
    channels: usize,
    frames: Vec<Vec<f32>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChannelStats {
    pub channel_index: usize,
    pub rms: f32,
    pub peak: f32,
    pub mean: f32,
    pub crest_factor: f32,
    pub zero_crossings: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AudioAnalysis {
    pub sample_rate: u32,
    pub channels: usize,
    pub frames: usize,
    pub duration_seconds: f32,
    pub channel_stats: Vec<ChannelStats>,
    pub dominant_frequency_hz: f32,
    pub dominant_magnitude: f32,
}

pub trait SampleProvider {
    fn sample_rate(&self) -> u32;
    fn channel_count(&self) -> usize;
    fn frames(&self) -> usize;
    fn channel(&self, index: usize) -> Option<&[f32]>;

    fn duration_seconds(&self) -> f32 {
        self.frames() as f32 / self.sample_rate() as f32
    }
}

impl AudioBuffer {
    pub fn new(sample_rate: u32, frames: Vec<Vec<f32>>) -> AppResult<Self> {
        if sample_rate == 0 {
            return Err(AppError::invalid_argument("sample rate cannot be zero"));
        }
        if frames.is_empty() {
            return Err(AppError::invalid_argument("audio must contain a channel"));
        }
        let expected = frames[0].len();
        if frames.iter().any(|channel| channel.len() != expected) {
            return Err(AppError::invalid_argument(
                "all channels must contain the same number of frames",
            ));
        }
        Ok(Self {
            sample_rate,
            channels: frames.len(),
            frames,
        })
    }

    pub fn from_wav(data: WavData) -> AppResult<Self> {
        let channels = data.channels as usize;
        let mut separated = vec![Vec::with_capacity(data.frame_count()); channels];
        for frame in data.samples.chunks_exact(channels) {
            for (channel_index, sample) in frame.iter().enumerate() {
                separated[channel_index].push(*sample as f32 / i16::MAX as f32);
            }
        }
        Self::new(data.sample_rate, separated)
    }

    pub fn mono_mix(&self) -> Vec<f32> {
        let frame_count = self.frames();
        let mut mix = Vec::with_capacity(frame_count);
        for frame_index in 0..frame_count {
            let sum: f32 = self
                .frames
                .iter()
                .map(|channel| channel[frame_index])
                .sum::<f32>();
            mix.push(sum / self.channels as f32);
        }
        mix
    }

    pub fn to_interleaved_samples(&self) -> Vec<f32> {
        let mut samples = Vec::with_capacity(self.frames() * self.channels);
        for frame_index in 0..self.frames() {
            for channel in &self.frames {
                samples.push(channel[frame_index].clamp(-1.0, 1.0));
            }
        }
        samples
    }

    pub fn mono_window_at(&self, center_frame: usize, window_size: usize) -> Vec<f32> {
        if window_size == 0 {
            return Vec::new();
        }

        let mono = self.mono_mix();
        if mono.is_empty() {
            return vec![0.0; window_size];
        }

        let half = window_size / 2;
        let start = center_frame.saturating_sub(half);
        let mut window = vec![0.0; window_size];
        for (target, sample) in window.iter_mut().enumerate() {
            let source = start + target;
            if let Some(value) = mono.get(source) {
                *sample = *value;
            }
        }
        window
    }

    pub fn analyze(&self, spectrum: &Spectrum) -> AudioAnalysis {
        let channel_stats = self
            .frames
            .iter()
            .enumerate()
            .map(|(index, samples)| channel_stats(index, samples))
            .collect();
        let dominant = spectrum.dominant_bin();

        AudioAnalysis {
            sample_rate: self.sample_rate,
            channels: self.channels,
            frames: self.frames(),
            duration_seconds: self.duration_seconds(),
            channel_stats,
            dominant_frequency_hz: dominant.map(|bin| bin.center_hz).unwrap_or_default(),
            dominant_magnitude: dominant.map(|bin| bin.magnitude).unwrap_or_default(),
        }
    }

    pub fn waveform_preview(&self, points: usize) -> Vec<f32> {
        if points == 0 || self.frames() == 0 {
            return Vec::new();
        }

        let mono = normalize_peak(&self.mono_mix());
        let smoothing_window = (mono.len() / points).clamp(1, 256);
        let smoothed = moving_average(&mono, smoothing_window);
        let bucket_size = smoothed.len().div_ceil(points).max(1);

        smoothed
            .chunks(bucket_size)
            .take(points)
            .map(|chunk| {
                chunk
                    .iter()
                    .copied()
                    .max_by(|left, right| left.abs().total_cmp(&right.abs()))
                    .unwrap_or_default()
            })
            .collect()
    }
}

impl SampleProvider for AudioBuffer {
    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn channel_count(&self) -> usize {
        self.channels
    }

    fn frames(&self) -> usize {
        self.frames.first().map_or(0, Vec::len)
    }

    fn channel(&self, index: usize) -> Option<&[f32]> {
        self.frames.get(index).map(Vec::as_slice)
    }
}

pub fn normalize_peak(samples: &[f32]) -> Vec<f32> {
    let peak = samples
        .iter()
        .copied()
        .map(f32::abs)
        .fold(0.0_f32, f32::max);
    if peak <= f32::EPSILON {
        return samples.to_vec();
    }
    samples.iter().map(|sample| sample / peak).collect()
}

pub fn moving_average<T>(samples: &[T], window: usize) -> Vec<f32>
where
    T: Copy + Into<f32>,
{
    if window == 0 || samples.is_empty() {
        return Vec::new();
    }

    let mut result = Vec::with_capacity(samples.len());
    let mut sum = 0.0_f32;
    for (index, sample) in samples.iter().enumerate() {
        sum += (*sample).into();
        if index >= window {
            sum -= samples[index - window].into();
        }
        let divisor = (index + 1).min(window) as f32;
        result.push(sum / divisor);
    }
    result
}

fn channel_stats(channel_index: usize, samples: &[f32]) -> ChannelStats {
    if samples.is_empty() {
        return ChannelStats {
            channel_index,
            rms: 0.0,
            peak: 0.0,
            mean: 0.0,
            crest_factor: 0.0,
            zero_crossings: 0,
        };
    }

    let mut square_sum = 0.0_f32;
    let mut peak = 0.0_f32;
    let mut sum = 0.0_f32;
    let mut zero_crossings = 0_usize;
    let mut previous = samples[0];

    for sample in samples {
        square_sum += sample * sample;
        peak = peak.max(sample.abs());
        sum += sample;
        if signs_differ(previous, *sample) {
            zero_crossings += 1;
        }
        previous = *sample;
    }

    let rms = (square_sum / samples.len() as f32).sqrt();
    let crest_factor = if rms <= f32::EPSILON { 0.0 } else { peak / rms };

    ChannelStats {
        channel_index,
        rms,
        peak,
        mean: sum / samples.len() as f32,
        crest_factor,
        zero_crossings,
    }
}

fn signs_differ(left: f32, right: f32) -> bool {
    (left < 0.0 && right >= 0.0) || (left >= 0.0 && right < 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mono_mix_averages_channels() {
        let audio = AudioBuffer::new(4, vec![vec![1.0, 0.0], vec![0.0, -1.0]]).unwrap();
        assert_eq!(audio.mono_mix(), vec![0.5, -0.5]);
    }

    #[test]
    fn moving_average_handles_generic_input() {
        let values = [1_i16, 3, 5, 7];
        assert_eq!(moving_average(&values, 2), vec![1.0, 2.0, 4.0, 6.0]);
    }

    #[test]
    fn channel_stats_counts_zero_crossings() {
        let audio = AudioBuffer::new(10, vec![vec![-1.0, -0.5, 0.5, 1.0, -0.2]]).unwrap();
        let config = crate::spectrum::SpectrumConfig::new(4, 4).unwrap();
        let spectrum = crate::spectrum::SpectrumStrategy::default().analyze(&audio, &config);
        let analysis = audio.analyze(&spectrum);
        assert_eq!(analysis.channel_stats[0].zero_crossings, 2);
    }

    #[test]
    fn waveform_preview_limits_point_count() {
        let audio = AudioBuffer::new(10, vec![vec![0.0, 1.0, 0.0, -1.0, 0.0]]).unwrap();
        let preview = audio.waveform_preview(3);
        assert!(preview.len() <= 3);
        assert!(!preview.is_empty());
    }

    #[test]
    fn mono_window_zero_pads_out_of_range() {
        let audio = AudioBuffer::new(10, vec![vec![1.0, 2.0]]).unwrap();
        assert_eq!(audio.mono_window_at(0, 4), vec![1.0, 2.0, 0.0, 0.0]);
    }

    #[test]
    fn interleaves_channel_samples() {
        let audio = AudioBuffer::new(10, vec![vec![0.1, 0.2], vec![0.3, 0.4]]).unwrap();
        assert_eq!(audio.to_interleaved_samples(), vec![0.1, 0.3, 0.2, 0.4]);
    }
}
