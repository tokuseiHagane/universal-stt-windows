use crate::DiarizeError;
use crate::banks::parse_banks;
use crate::dft::Dft;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrontendGeometry {
    pub sample_rate: u32,
    pub frame: usize,
    pub shift: usize,
    pub n_fft: usize,
    pub mels: usize,
    pub preemphasis: f32,
    pub scale: f32,
    pub log_floor: f32,
}

impl FrontendGeometry {
    pub fn bins(&self) -> usize {
        self.n_fft / 2 + 1
    }
}

pub struct Frontend {
    geometry: FrontendGeometry,
    /// `[bins * mels]`, row-major, so one frame's power is one row.
    banks_transposed: Vec<f32>,
    window: Vec<f32>,
    dft: Dft,
}

impl Frontend {
    pub fn new(geometry: FrontendGeometry, banks: &[u8]) -> Result<Self, DiarizeError> {
        let (banks, mels, bins) = parse_banks(banks)?;
        if mels != geometry.mels || bins != geometry.bins() {
            return Err(DiarizeError::Model(format!(
                "mel bank table is {mels} x {bins}, geometry says {} x {}",
                geometry.mels,
                geometry.bins()
            )));
        }

        let mut banks_transposed = vec![0.0f32; bins * mels];
        for m in 0..mels {
            for b in 0..bins {
                banks_transposed[b * mels + m] = banks[m * bins + b];
            }
        }

        let window = (0..geometry.frame)
            .map(|n| {
                (0.54
                    - 0.46
                        * (2.0 * std::f64::consts::PI * n as f64 / (geometry.frame - 1) as f64)
                            .cos()) as f32
            })
            .collect();

        Ok(Self {
            geometry,
            banks_transposed,
            window,
            dft: Dft::new(geometry.n_fft)?,
        })
    }

    pub fn geometry(&self) -> FrontendGeometry {
        self.geometry
    }

    pub fn frames(&self, samples: usize) -> usize {
        if samples < self.geometry.frame {
            0
        } else {
            1 + (samples - self.geometry.frame) / self.geometry.shift
        }
    }

    /// Mean of each mel over the whole recording; the caller reuses it for any span.
    pub fn means(&self, samples: &[f32]) -> (Vec<f32>, usize) {
        self.means_pieces(samples, 6000)
    }

    pub fn means_pieces(&self, samples: &[f32], piece_frames: usize) -> (Vec<f32>, usize) {
        let total = self.frames(samples.len());
        let mels = self.geometry.mels;
        let mut sums = vec![0.0f64; mels];
        let mut piece = 0;
        while piece < total {
            let count = piece_frames.min(total - piece);
            let mel = self.log_mel(samples, piece, count);
            for (m, sum) in sums.iter_mut().enumerate() {
                let mut acc = 0.0f64;
                for f in 0..count {
                    acc += mel[f * mels + m] as f64;
                }
                *sum += acc;
            }
            piece += count;
        }
        if total == 0 {
            return (vec![0.0; mels], 0);
        }
        let means = sums
            .into_iter()
            .map(|s| (s / total as f64) as f32)
            .collect();
        (means, total)
    }

    /// Log-mel for `range`, `[range.count * mels]`, means removed per mel.
    pub fn features(
        &self,
        samples: &[f32],
        range: std::ops::Range<usize>,
        means: &[f32],
    ) -> Vec<f32> {
        let mels = self.geometry.mels;
        let mut out = self.log_mel(samples, range.start, range.len());
        for f in 0..range.len() {
            for m in 0..mels {
                out[f * mels + m] -= means[m];
            }
        }
        out
    }

    /// Raw (not mean-removed) log-mel for frames `first..first+frames`.
    pub fn mel(&self, samples: &[f32], first: usize, frames: usize) -> Vec<f32> {
        self.log_mel(samples, first, frames)
    }

    /// Both passes over a recording short enough to hold whole.
    pub fn features_all(&self, samples: &[f32]) -> (Vec<f32>, usize) {
        let (means, total) = self.means(samples);
        (self.features(samples, 0..total, &means), total)
    }

    fn log_mel(&self, samples: &[f32], first_frame: usize, frames: usize) -> Vec<f32> {
        let mels = self.geometry.mels;
        if frames == 0 {
            return Vec::new();
        }
        let power = self.power_spectra(samples, first_frame, frames);
        let bins = self.geometry.bins();
        let mut mel = vec![0.0f32; frames * mels];
        for f in 0..frames {
            let row = &power[f * bins..(f + 1) * bins];
            let acc = &mut mel[f * mels..(f + 1) * mels];
            for (b, &p) in row.iter().enumerate() {
                let bank_row = &self.banks_transposed[b * mels..(b + 1) * mels];
                for (m, &weight) in bank_row.iter().enumerate() {
                    acc[m] += p * weight;
                }
            }
            for value in acc.iter_mut() {
                *value = value.max(self.geometry.log_floor).ln();
            }
        }
        mel
    }

    fn power_spectra(&self, samples: &[f32], first_frame: usize, frames: usize) -> Vec<f32> {
        let g = self.geometry;
        let bins = g.bins();
        let mut power = vec![0.0f32; frames * bins];
        let mut frame = vec![0.0f32; g.frame];
        let mut re = vec![0.0f32; g.n_fft];

        for f in 0..frames {
            let start = (first_frame + f) * g.shift;
            for (i, slot) in frame.iter_mut().enumerate() {
                *slot = samples[start + i] * g.scale;
            }

            let sum: f64 = frame.iter().map(|&v| v as f64).sum();
            let mean = (sum / g.frame as f64) as f32;
            for value in frame.iter_mut() {
                *value -= mean;
            }

            // From the end, so each sample still sees its unmodified predecessor.
            for i in (1..g.frame).rev() {
                frame[i] -= g.preemphasis * frame[i - 1];
            }
            frame[0] -= g.preemphasis * frame[0];

            for i in 0..g.frame {
                re[i] = frame[i] * self.window[i];
            }
            for value in re.iter_mut().skip(g.frame) {
                *value = 0.0;
            }

            let (out_re, out_im) = self.dft.transform(&re);
            for k in 0..bins {
                power[f * bins + k] = out_re[k] * out_re[k] + out_im[k] * out_im[k];
            }
        }
        power
    }
}
