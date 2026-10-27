use crate::MediaError;
use hound::WavReader;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Peak {
    pub min: f32,
    pub max: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Waveform {
    pub bucket_ms: u32,
    pub peaks: Vec<Peak>,
}

impl Waveform {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(16 + self.peaks.len() * 8);
        out.extend_from_slice(b"USTTW1");
        out.extend_from_slice(&self.bucket_ms.to_le_bytes());
        out.extend_from_slice(&(self.peaks.len() as u64).to_le_bytes());
        for peak in &self.peaks {
            out.extend_from_slice(&peak.min.to_le_bytes());
            out.extend_from_slice(&peak.max.to_le_bytes());
        }
        out
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, MediaError> {
        if bytes.len() < 18 || &bytes[0..6] != b"USTTW1" {
            return Err(MediaError::Waveform("bad magic or truncated header".into()));
        }
        let bucket_ms = u32::from_le_bytes(bytes[6..10].try_into().unwrap());
        let count = u64::from_le_bytes(bytes[10..18].try_into().unwrap()) as usize;
        let expected = 18 + count.saturating_mul(8);
        if bytes.len() != expected {
            return Err(MediaError::Waveform(format!(
                "expected {expected} bytes, got {}",
                bytes.len()
            )));
        }
        let mut peaks = Vec::with_capacity(count);
        for start in (18..bytes.len()).step_by(8) {
            let chunk = &bytes[start..start + 8];
            peaks.push(Peak {
                min: f32::from_le_bytes(chunk[0..4].try_into().unwrap()),
                max: f32::from_le_bytes(chunk[4..8].try_into().unwrap()),
            });
        }
        Ok(Self { bucket_ms, peaks })
    }
}

pub struct WaveformBuilder {
    bucket_ms: u32,
    samples_per_bucket: u32,
    pending: u32,
    min: f32,
    max: f32,
    peaks: Vec<Peak>,
}

impl WaveformBuilder {
    pub fn new(sample_rate: u32, bucket_ms: u32) -> Self {
        let samples_per_bucket = (sample_rate as u64 * bucket_ms as u64 / 1000).max(1) as u32;
        Self {
            bucket_ms,
            samples_per_bucket,
            pending: 0,
            min: f32::INFINITY,
            max: f32::NEG_INFINITY,
            peaks: Vec::new(),
        }
    }

    pub fn push(&mut self, sample: f32) {
        if sample.is_finite() {
            self.min = self.min.min(sample);
            self.max = self.max.max(sample);
        }
        self.pending += 1;
        if self.pending >= self.samples_per_bucket {
            self.flush();
        }
    }

    pub fn finish(mut self) -> Waveform {
        if self.pending > 0 {
            self.flush();
        }
        Waveform {
            bucket_ms: self.bucket_ms,
            peaks: self.peaks,
        }
    }

    fn flush(&mut self) {
        let (min, max) = if self.min.is_finite() && self.max.is_finite() {
            (self.min, self.max)
        } else {
            (0.0, 0.0)
        };
        self.peaks.push(Peak { min, max });
        self.pending = 0;
        self.min = f32::INFINITY;
        self.max = f32::NEG_INFINITY;
    }
}

pub fn waveform_from_samples(samples: &[f32], sample_rate: u32, bucket_ms: u32) -> Waveform {
    let mut builder = WaveformBuilder::new(sample_rate, bucket_ms);
    for &sample in samples {
        builder.push(sample);
    }
    builder.finish()
}

pub fn waveform_from_wav(path: &Path, bucket_ms: u32) -> Result<Waveform, MediaError> {
    let mut reader = WavReader::open(path)
        .map_err(|e| MediaError::Decode(format!("open {}: {e}", path.display())))?;
    let sample_rate = reader.spec().sample_rate;
    let mut builder = WaveformBuilder::new(sample_rate, bucket_ms);
    for sample in reader.samples::<i16>() {
        builder.push(sample.map_err(|e| MediaError::Decode(e.to_string()))? as f32 / 32768.0);
    }
    Ok(builder.finish())
}
