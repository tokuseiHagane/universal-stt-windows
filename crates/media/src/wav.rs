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
        let scaled = (sample.clamp(-1.0, 1.0) * 32767.0).round() as i16;
        writer
            .write_sample(scaled)
            .map_err(|e| MediaError::Decode(e.to_string()))?;
    }
    writer
        .finalize()
        .map_err(|e| MediaError::Decode(e.to_string()))
}
