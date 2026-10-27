use crate::MediaError;
use hound::{SampleFormat, WavReader, WavSpec, WavWriter};
use std::path::Path;

pub fn read_wav_mono16k(path: &Path) -> Result<Vec<f32>, MediaError> {
    let mut reader = WavReader::open(path)
        .map_err(|e| MediaError::Decode(format!("open {}: {e}", path.display())))?;
    let spec = reader.spec();
    if spec.channels != 1 || spec.sample_rate != 16_000 || spec.bits_per_sample != 16 {
        return Err(MediaError::Decode(format!(
            "expected mono 16-bit 16 kHz, got {} ch / {}-bit / {} Hz",
            spec.channels, spec.bits_per_sample, spec.sample_rate
        )));
    }
    let mut out = Vec::new();
    for sample in reader.samples::<i16>() {
        out.push(sample.map_err(|e| MediaError::Decode(e.to_string()))? as f32 / 32768.0);
    }
    Ok(out)
}

pub fn write_wav_mono16k(path: &Path, samples: &[f32]) -> Result<(), MediaError> {
    let spec = WavSpec {
        channels: 1,
        sample_rate: 16_000,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };
    let mut writer = WavWriter::create(path, spec)
        .map_err(|e| MediaError::Decode(format!("create {}: {e}", path.display())))?;
    for &sample in samples {
        let scaled = (sample.clamp(-1.0, 1.0) * 32768.0)
            .round()
            .clamp(-32768.0, 32767.0) as i16;
        writer
            .write_sample(scaled)
            .map_err(|e| MediaError::Decode(e.to_string()))?;
    }
    writer
        .finalize()
        .map_err(|e| MediaError::Decode(e.to_string()))
}

pub fn wav_mono16k_samples(path: &Path) -> Result<usize, MediaError> {
    let reader = WavReader::open(path)
        .map_err(|e| MediaError::Decode(format!("open {}: {e}", path.display())))?;
    check_spec(reader.spec())?;
    Ok(reader.len() as usize)
}

/// Reads `count` samples starting at `start` from a canonical mono 16-bit WAV
/// without loading the whole file.
pub fn read_wav_mono16k_range(
    path: &Path,
    start: usize,
    count: usize,
) -> Result<Vec<f32>, MediaError> {
    if count == 0 {
        return Ok(Vec::new());
    }
    let reader = WavReader::open(path)
        .map_err(|e| MediaError::Decode(format!("open {}: {e}", path.display())))?;
    check_spec(reader.spec())?;
    let total = reader.len() as usize;
    let start = start.min(total);
    let count = count.min(total - start);
    let offset = data_offset(path)? + start as u64 * 2;
    let mut file = std::fs::File::open(path)?;
    use std::io::{Read, Seek, SeekFrom};
    file.seek(SeekFrom::Start(offset))?;
    let mut raw = vec![0u8; count * 2];
    file.read_exact(&mut raw)?;
    Ok(raw
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| i16::from_le_bytes(*pair) as f32 / 32768.0)
        .collect())
}

fn check_spec(spec: hound::WavSpec) -> Result<(), MediaError> {
    if spec.channels != 1 || spec.sample_rate != 16_000 || spec.bits_per_sample != 16 {
        return Err(MediaError::Decode(format!(
            "expected mono 16-bit 16 kHz, got {} ch / {}-bit / {} Hz",
            spec.channels, spec.bits_per_sample, spec.sample_rate
        )));
    }
    Ok(())
}

fn data_offset(path: &Path) -> Result<u64, MediaError> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = std::fs::File::open(path)?;
    let mut header = [0u8; 12];
    file.read_exact(&mut header)?;
    if &header[0..4] != b"RIFF" || &header[8..12] != b"WAVE" {
        return Err(MediaError::Decode("not a RIFF/WAVE file".into()));
    }
    let mut pos = 12u64;
    file.seek(SeekFrom::Start(pos))?;
    loop {
        let mut chunk = [0u8; 8];
        file.read_exact(&mut chunk)?;
        let size = u32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]) as u64;
        if &chunk[0..4] == b"data" {
            return Ok(pos + 8);
        }
        pos += 8 + size + size % 2;
        file.seek(SeekFrom::Start(pos))?;
    }
}
