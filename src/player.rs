use crate::error::{AppError, AppResult};
use crate::signal::{AudioBuffer, SampleProvider};
use rodio::buffer::SamplesBuffer;
use rodio::{OutputStream, OutputStreamHandle, Sink};
use std::time::{Duration, Instant};

pub struct AudioPlayer {
    _stream: OutputStream,
    handle: OutputStreamHandle,
    sink: Option<Sink>,
    samples: Vec<f32>,
    channels: u16,
    sample_rate: u32,
    position: Duration,
    started_at: Option<Instant>,
    duration: Duration,
}

impl AudioPlayer {
    pub fn new() -> AppResult<Self> {
        let (stream, handle) = OutputStream::try_default()
            .map_err(|err| AppError::playback(format!("cannot open output device: {err}")))?;
        Ok(Self {
            _stream: stream,
            handle,
            sink: None,
            samples: Vec::new(),
            channels: 0,
            sample_rate: 0,
            position: Duration::ZERO,
            started_at: None,
            duration: Duration::ZERO,
        })
    }

    pub fn load_audio(&mut self, audio: &AudioBuffer) -> AppResult<()> {
        self.stop();
        self.channels = u16::try_from(audio.channel_count())
            .map_err(|_| AppError::playback("too many channels for playback"))?;
        self.sample_rate = audio.sample_rate();
        self.samples = audio.to_interleaved_samples();
        self.duration = Duration::from_secs_f32(audio.duration_seconds());
        Ok(())
    }

    pub fn play(&mut self) -> AppResult<()> {
        if let Some(sink) = &self.sink {
            sink.play();
            self.started_at = Some(Instant::now());
            return Ok(());
        }

        if self.samples.is_empty() || self.channels == 0 || self.sample_rate == 0 {
            return Err(AppError::playback("no audio samples loaded"));
        }

        let source = SamplesBuffer::new(self.channels, self.sample_rate, self.samples.clone());
        let sink = Sink::try_new(&self.handle)
            .map_err(|err| AppError::playback(format!("cannot create audio sink: {err}")))?;
        sink.append(source);
        sink.play();
        self.position = Duration::ZERO;
        self.started_at = Some(Instant::now());
        self.sink = Some(sink);
        Ok(())
    }

    pub fn pause(&mut self) {
        self.position = self.position();
        self.started_at = None;
        if let Some(sink) = &self.sink {
            sink.pause();
        }
    }

    pub fn stop(&mut self) {
        if let Some(sink) = self.sink.take() {
            sink.stop();
        }
        self.position = Duration::ZERO;
        self.started_at = None;
    }

    pub fn is_playing(&self) -> bool {
        self.started_at.is_some()
    }

    pub fn position(&self) -> Duration {
        let mut position = self.position;
        if let Some(started_at) = self.started_at {
            position += started_at.elapsed();
        }
        position.min(self.duration)
    }
}
