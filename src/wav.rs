use crate::error::{AppError, AppResult};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

const RIFF: &[u8; 4] = b"RIFF";
const WAVE: &[u8; 4] = b"WAVE";
const FMT: &[u8; 4] = b"fmt ";
const DATA: &[u8; 4] = b"data";
const PCM_FORMAT: u16 = 1;
const PCM_16_BITS: u16 = 16;

#[derive(Debug, Clone, PartialEq)]
pub struct WavData {
    pub sample_rate: u32,
    pub channels: u16,
    pub samples: Vec<i16>,
}

impl WavData {
    pub fn new(sample_rate: u32, channels: u16, samples: Vec<i16>) -> AppResult<Self> {
        if sample_rate == 0 {
            return Err(AppError::invalid_wav("sample rate cannot be zero"));
        }
        if channels == 0 {
            return Err(AppError::invalid_wav("channel count cannot be zero"));
        }
        if !samples.len().is_multiple_of(channels as usize) {
            return Err(AppError::invalid_wav(
                "sample count is not divisible by channel count",
            ));
        }
        Ok(Self {
            sample_rate,
            channels,
            samples,
        })
    }

    pub fn frame_count(&self) -> usize {
        self.samples.len() / self.channels as usize
    }

    pub fn duration_seconds(&self) -> f32 {
        self.frame_count() as f32 / self.sample_rate as f32
    }
}

#[derive(Debug, Clone, Copy)]
struct FormatChunk {
    audio_format: u16,
    channels: u16,
    sample_rate: u32,
    byte_rate: u32,
    block_align: u16,
    bits_per_sample: u16,
}

pub fn read_wav_file(path: &Path) -> AppResult<WavData> {
    let mut file = File::open(path)?;
    read_wav(&mut file)
}

pub fn write_wav_file(path: &Path, data: &WavData) -> AppResult<()> {
    let mut file = File::create(path)?;
    write_wav(&mut file, data)
}

pub fn read_wav<R>(reader: &mut R) -> AppResult<WavData>
where
    R: Read + Seek,
{
    let mut riff = [0_u8; 4];
    reader.read_exact(&mut riff)?;
    if &riff != RIFF {
        return Err(AppError::invalid_wav("missing RIFF header"));
    }

    let _file_size = read_u32_le(reader)?;
    let mut wave = [0_u8; 4];
    reader.read_exact(&mut wave)?;
    if &wave != WAVE {
        return Err(AppError::invalid_wav("missing WAVE marker"));
    }

    let mut format = None;
    let mut data = None;

    loop {
        let mut chunk_id = [0_u8; 4];
        match reader.read_exact(&mut chunk_id) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(err) => return Err(err.into()),
        }

        let chunk_size = read_u32_le(reader)?;
        match &chunk_id {
            FMT => format = Some(read_format_chunk(reader, chunk_size)?),
            DATA => data = Some(read_data_chunk(reader, chunk_size)?),
            _ => skip_chunk(reader, chunk_size)?,
        }

        if chunk_size % 2 == 1 {
            reader.seek(SeekFrom::Current(1))?;
        }
    }

    let format = format.ok_or_else(|| AppError::invalid_wav("missing fmt chunk"))?;
    validate_format(format)?;
    let samples = data.ok_or_else(|| AppError::invalid_wav("missing data chunk"))?;
    WavData::new(format.sample_rate, format.channels, samples)
}

pub fn write_wav<W>(writer: &mut W, data: &WavData) -> AppResult<()>
where
    W: Write,
{
    let data_bytes = checked_data_bytes(data.samples.len())?;
    let fmt_chunk_size = 16_u32;
    let riff_size = 4_u32
        .checked_add(8 + fmt_chunk_size)
        .and_then(|size| size.checked_add(8 + data_bytes))
        .ok_or_else(|| AppError::invalid_wav("WAV file is too large"))?;
    let byte_rate = data.sample_rate * data.channels as u32 * PCM_16_BITS as u32 / 8;
    let block_align = data.channels * PCM_16_BITS / 8;

    writer.write_all(RIFF)?;
    write_u32_le(writer, riff_size)?;
    writer.write_all(WAVE)?;

    writer.write_all(FMT)?;
    write_u32_le(writer, fmt_chunk_size)?;
    write_u16_le(writer, PCM_FORMAT)?;
    write_u16_le(writer, data.channels)?;
    write_u32_le(writer, data.sample_rate)?;
    write_u32_le(writer, byte_rate)?;
    write_u16_le(writer, block_align)?;
    write_u16_le(writer, PCM_16_BITS)?;

    writer.write_all(DATA)?;
    write_u32_le(writer, data_bytes)?;
    for sample in &data.samples {
        write_i16_le(writer, *sample)?;
    }
    Ok(())
}

fn validate_format(format: FormatChunk) -> AppResult<()> {
    if format.audio_format != PCM_FORMAT {
        return Err(AppError::unsupported_wav(
            "only uncompressed PCM WAV files are supported",
        ));
    }
    if format.bits_per_sample != PCM_16_BITS {
        return Err(AppError::unsupported_wav(
            "only 16-bit PCM samples are supported",
        ));
    }
    if format.channels == 0 {
        return Err(AppError::invalid_wav("channel count cannot be zero"));
    }
    let expected_block_align = format.channels * format.bits_per_sample / 8;
    if format.block_align != expected_block_align {
        return Err(AppError::invalid_wav("block align does not match format"));
    }
    let expected_byte_rate = format.sample_rate * format.block_align as u32;
    if format.byte_rate != expected_byte_rate {
        return Err(AppError::invalid_wav("byte rate does not match format"));
    }
    Ok(())
}

fn read_format_chunk<R>(reader: &mut R, chunk_size: u32) -> AppResult<FormatChunk>
where
    R: Read + Seek,
{
    if chunk_size < 16 {
        return Err(AppError::invalid_wav("fmt chunk is too short"));
    }

    let format = FormatChunk {
        audio_format: read_u16_le(reader)?,
        channels: read_u16_le(reader)?,
        sample_rate: read_u32_le(reader)?,
        byte_rate: read_u32_le(reader)?,
        block_align: read_u16_le(reader)?,
        bits_per_sample: read_u16_le(reader)?,
    };

    if chunk_size > 16 {
        skip_chunk(reader, chunk_size - 16)?;
    }
    Ok(format)
}

fn read_data_chunk<R>(reader: &mut R, chunk_size: u32) -> AppResult<Vec<i16>>
where
    R: Read,
{
    if !chunk_size.is_multiple_of(2) {
        return Err(AppError::invalid_wav(
            "16-bit PCM data chunk has an odd byte length",
        ));
    }

    let sample_count = (chunk_size / 2) as usize;
    let mut samples = Vec::with_capacity(sample_count);
    for _ in 0..sample_count {
        samples.push(read_i16_le(reader)?);
    }
    Ok(samples)
}

fn skip_chunk<R>(reader: &mut R, chunk_size: u32) -> AppResult<()>
where
    R: Seek,
{
    reader.seek(SeekFrom::Current(chunk_size as i64))?;
    Ok(())
}

fn checked_data_bytes(sample_count: usize) -> AppResult<u32> {
    sample_count
        .checked_mul(2)
        .and_then(|bytes| u32::try_from(bytes).ok())
        .ok_or_else(|| AppError::invalid_wav("WAV data is too large"))
}

fn read_u16_le<R: Read>(reader: &mut R) -> AppResult<u16> {
    let mut bytes = [0_u8; 2];
    reader.read_exact(&mut bytes)?;
    Ok(u16::from_le_bytes(bytes))
}

fn read_u32_le<R: Read>(reader: &mut R) -> AppResult<u32> {
    let mut bytes = [0_u8; 4];
    reader.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

fn read_i16_le<R: Read>(reader: &mut R) -> AppResult<i16> {
    let mut bytes = [0_u8; 2];
    reader.read_exact(&mut bytes)?;
    Ok(i16::from_le_bytes(bytes))
}

fn write_u16_le<W: Write>(writer: &mut W, value: u16) -> AppResult<()> {
    writer.write_all(&value.to_le_bytes())?;
    Ok(())
}

fn write_u32_le<W: Write>(writer: &mut W, value: u32) -> AppResult<()> {
    writer.write_all(&value.to_le_bytes())?;
    Ok(())
}

fn write_i16_le<W: Write>(writer: &mut W, value: i16) -> AppResult<()> {
    writer.write_all(&value.to_le_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn round_trip_wav_data() {
        let original = WavData::new(44_100, 2, vec![0, 10, -10, 20, 30, -30]).unwrap();
        let mut bytes = Vec::new();
        write_wav(&mut bytes, &original).unwrap();
        let mut cursor = Cursor::new(bytes);
        let decoded = read_wav(&mut cursor).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn reject_non_riff_file() {
        let mut cursor = Cursor::new(vec![0_u8; 16]);
        let result = read_wav(&mut cursor);
        assert!(matches!(result, Err(AppError::InvalidWav(_))));
    }

    #[test]
    fn duration_uses_frames_not_samples() {
        let data = WavData::new(10, 2, vec![0, 1, 2, 3, 4, 5]).unwrap();
        assert_eq!(data.frame_count(), 3);
        assert!((data.duration_seconds() - 0.3).abs() < f32::EPSILON);
    }
}
