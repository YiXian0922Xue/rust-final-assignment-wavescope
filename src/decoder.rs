use crate::error::{AppError, AppResult};
use crate::signal::AudioBuffer;
use crate::wav::read_wav_file;
use std::fs::File;
use std::path::Path;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{CODEC_TYPE_NULL, DecoderOptions};
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

pub fn decode_audio_file(path: &Path) -> AppResult<AudioBuffer> {
    if path
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("wav"))
        && let Ok(wav) = read_wav_file(path)
    {
        return AudioBuffer::from_wav(wav);
    }

    let file = File::open(path)?;
    let media_source = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(extension) = path.extension().and_then(|value| value.to_str()) {
        hint.with_extension(extension);
    }

    let probed = symphonia::default::get_probe().format(
        &hint,
        media_source,
        &FormatOptions::default(),
        &MetadataOptions::default(),
    )?;
    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|track| track.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| AppError::decode("no supported audio track found"))?;

    let track_id = track.id;
    let mut decoder =
        symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default())?;
    let mut sample_rate = track.codec_params.sample_rate.unwrap_or(44_100);
    let channel_count = track
        .codec_params
        .channels
        .map(|channels| channels.count())
        .unwrap_or(1);
    let mut channels = vec![Vec::<f32>::new(); channel_count];

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(symphonia::core::errors::Error::IoError(err))
                if err.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(err) => return Err(err.into()),
        };

        if packet.track_id() != track_id {
            continue;
        }

        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            Err(symphonia::core::errors::Error::DecodeError(_)) => continue,
            Err(err) => return Err(err.into()),
        };

        sample_rate = decoded.spec().rate;
        let spec_channels = decoded.spec().channels.count();
        if spec_channels != channels.len() {
            return Err(AppError::decode(
                "audio stream changed channel count while decoding",
            ));
        }

        let mut buffer = SampleBuffer::<f32>::new(decoded.capacity() as u64, *decoded.spec());
        buffer.copy_interleaved_ref(decoded);
        for frame in buffer.samples().chunks_exact(channels.len()) {
            for (index, sample) in frame.iter().enumerate() {
                channels[index].push(sample.clamp(-1.0, 1.0));
            }
        }
    }

    if channels.iter().all(Vec::is_empty) {
        return Err(AppError::decode(
            "audio file did not contain decoded samples",
        ));
    }

    AudioBuffer::new(sample_rate, channels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_reports_error() {
        let result = decode_audio_file(Path::new("missing-file-for-test.wav"));
        assert!(result.is_err());
    }
}
