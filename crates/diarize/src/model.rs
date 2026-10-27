use crate::DiarizeError;
use crate::Frontend;
use crate::FrontendGeometry;
use crate::cluster;
use crate::decision::{Decision, FrameClock, Stretch, sigmoid};
use crate::embedding::{Embedding, Pool};
use crate::linker::{LabelledTurn, joined};
use crate::logits::compute_logits;
use crate::meta::Meta;
use crate::pieces;
use crate::session::{Sessions, Tensor};
use crate::voices;
use std::path::Path;
use ustt_core::{Diarization, Turn};

pub struct WhoModel {
    meta: Meta,
    frontend: Frontend,
    voice_frontend: Option<Frontend>,
    sessions: Sessions,
    embedding: Option<Embedding>,
}

struct Lane {
    turns: Vec<LabelledTurn>,
    solo: Vec<f32>,
    offset: usize,
}

impl WhoModel {
    pub fn load(dir: impl AsRef<Path>) -> Result<Self, DiarizeError> {
        let dir = dir.as_ref();
        let meta = Meta::load(dir)?;
        meta.validate()?;

        let banks = std::fs::read(dir.join(&meta.frontend.banks))?;
        let frontend = Frontend::new(meta.geometry(), &banks)?;

        let voice_frontend = if meta.voice.compiled.is_empty() {
            None
        } else {
            let banks = std::fs::read(dir.join(&meta.voice.frontend.banks))?;
            let geometry = FrontendGeometry {
                sample_rate: meta.audio.sample_rate,
                frame: meta.voice.frontend.frame_samples,
                shift: meta.voice.frontend.shift_samples,
                n_fft: meta.voice.frontend.n_fft,
                mels: meta.voice.frontend.n_mels,
                preemphasis: meta.voice.frontend.preemphasis,
                scale: meta.voice.frontend.scale,
                log_floor: meta.voice.frontend.log_floor,
            };
            Some(Frontend::new(geometry, &banks)?)
        };

        let embedding = if meta.voice.embedding.weights.is_empty() {
            None
        } else {
            let bytes = std::fs::read(dir.join(&meta.voice.embedding.weights))?;
            Some(Embedding::from_sidecar(&bytes)?)
        };

        let sessions = Sessions::load(dir, &meta)?;
        Ok(Self {
            meta,
            frontend,
            voice_frontend,
            sessions,
            embedding,
        })
    }

    pub fn diarize(
        &mut self,
        samples: &[f32],
        sample_rate: u32,
        progress: &mut dyn FnMut(f32),
    ) -> Result<Diarization, DiarizeError> {
        let audio = resample_linear(samples, sample_rate, self.meta.audio.sample_rate);
        let seconds = audio.len() as f64 / self.meta.audio.sample_rate as f64;
        let (means, mel_frames) = self.frontend.means(&audio);
        if mel_frames == 0 {
            return Err(DiarizeError::Model(
                "audio is shorter than one frame".into(),
            ));
        }
        let decision = Decision::from_meta(&self.meta.decision);
        let clock = FrameClock {
            step: self.meta.frames.step_s,
            first_centre: self.meta.frames.first_centre_s,
        };
        let longest = self
            .meta
            .windows
            .last()
            .map(|w| w.frames)
            .ok_or_else(|| DiarizeError::Model("no windows".into()))?;
        let pieces = pieces::cut(mel_frames, longest);

        if pieces.len() == 1 {
            let logits = compute_logits(
                &mut self.sessions,
                &self.meta,
                &self.frontend,
                &audio,
                pieces[0].clone(),
                &means,
            )?;
            let stretches = decision.stretches(
                &sigmoid(&logits.presence),
                &sigmoid(&logits.masks),
                pieces[0].len(),
            );
            progress(1.0);
            return Ok(Self::finish(
                stretches
                    .into_iter()
                    .map(|stretch| labelled(stretch, &clock, 0.0, stretch.query as i64))
                    .collect(),
                seconds,
            ));
        }

        let mut lanes: Vec<Lane> = Vec::new();
        let piece_count = pieces.len();
        for (index, piece) in pieces.into_iter().enumerate() {
            let logits = compute_logits(
                &mut self.sessions,
                &self.meta,
                &self.frontend,
                &audio,
                piece.clone(),
                &means,
            )?;
            let masks = sigmoid(&logits.masks);
            let stretches = decision.stretches(&sigmoid(&logits.presence), &masks, piece.len());
            let offset = piece.start as f64 * self.meta.frames.step_s;

            let mut decided: Vec<usize> = stretches.iter().map(|s| s.query).collect();
            decided.sort_unstable();
            decided.dedup();
            let mut on = vec![vec![false; piece.len()]; decided.len()];
            for (row, &query) in decided.iter().enumerate() {
                for f in 0..piece.len() {
                    on[row][f] = masks[query * piece.len() + f] > decision.mask_threshold;
                }
            }
            for (row, &query) in decided.iter().enumerate() {
                let mut solo = vec![0.0f32; piece.len()];
                for f in 0..piece.len() {
                    if on[row][f] {
                        let alone = on
                            .iter()
                            .enumerate()
                            .all(|(other, row_on)| other == row || !row_on[f]);
                        if alone {
                            solo[f] = 1.0;
                        }
                    }
                }
                let turns: Vec<LabelledTurn> = stretches
                    .iter()
                    .filter(|s| s.query == query)
                    .map(|&s| {
                        let label = (index * self.meta.model.queries + query) as i64;
                        labelled(s, &clock, offset, label)
                    })
                    .collect();
                lanes.push(Lane {
                    turns,
                    solo,
                    offset: piece.start,
                });
            }
            progress(0.6 * (index + 1) as f32 / piece_count as f32);
        }

        if self.voice_frontend.is_none() || self.embedding.is_none() {
            progress(1.0);
            return Ok(Self::finish(
                lanes.into_iter().flat_map(|l| l.turns).collect(),
                seconds,
            ));
        }
        let labels = Self::join_voices(
            &mut self.sessions,
            &self.meta,
            &lanes,
            &audio,
            mel_frames,
            self.voice_frontend.as_ref().unwrap(),
            self.embedding.as_ref().unwrap(),
            progress,
        )?;
        let mut turns = Vec::new();
        for (lane, label) in lanes.into_iter().zip(labels) {
            turns.extend(
                lane.turns
                    .into_iter()
                    .map(|turn| LabelledTurn { label, ..turn }),
            );
        }
        progress(1.0);
        Ok(Self::finish(turns, seconds))
    }

    #[allow(clippy::too_many_arguments)]
    fn join_voices(
        sessions: &mut Sessions,
        meta: &Meta,
        lanes: &[Lane],
        audio: &[f32],
        mel_frames: usize,
        voice_frontend: &Frontend,
        embedding: &Embedding,
        progress: &mut dyn FnMut(f32),
    ) -> Result<Vec<i64>, DiarizeError> {
        let per_column = meta.voice.mel_frames_per_column;
        let (voice_means, voice_frames) = voice_frontend.means(audio);
        let used = mel_frames.min(voice_frames) / per_column * per_column;
        let columns = used / per_column;
        let mels = voice_frontend.geometry().mels;
        let minimum = (meta.join.min_solo_s / meta.frames.step_s).round() as usize;

        let masks: Vec<Option<Vec<bool>>> = lanes
            .iter()
            .map(|lane| {
                let frames: f32 = lane.solo.iter().sum();
                (frames as usize >= minimum)
                    .then(|| voices::column_mask(&lane.solo, lane.offset, columns, per_column))
            })
            .collect();
        let mut pools: Vec<Option<Pool>> = masks
            .iter()
            .map(|mask| mask.as_ref().map(|_| embedding.pool()))
            .collect();

        let width = meta.voice.mel_frames;
        let margin = meta.voice.margin_mel_frames;
        let windows = voices::windows(used, width, margin, per_column);
        let window_count = windows.len();
        for (index, window) in windows.into_iter().enumerate() {
            let mut input = vec![0.0f32; width * mels];
            let available = width.min(used.saturating_sub(window.start));
            if available > 0 {
                let features = voice_frontend.features(
                    audio,
                    window.start..window.start + available,
                    &voice_means,
                );
                input[..features.len()].copy_from_slice(&features);
            }
            let outputs = sessions.run(
                &meta.voice.compiled,
                vec![(
                    meta.voice.inputs[0].as_str(),
                    Tensor::new(input, vec![1, width, mels]),
                )],
                &[meta.voice.outputs[0].as_str()],
            )?;
            let columns_dim = embedding.columns;
            if outputs[0].data.len() != (width / per_column) * columns_dim {
                return Err(DiarizeError::Model(format!(
                    "voice graph returned {} values, want {}",
                    outputs[0].data.len(),
                    (width / per_column) * columns_dim
                )));
            }
            for row in window.keep_low..window.keep_high {
                let column = window.place_low + (row - window.keep_low);
                if column >= columns {
                    continue;
                }
                for lane in 0..lanes.len() {
                    if masks[lane].as_ref().is_some_and(|m| m[column])
                        && let Some(pool) = pools[lane].as_mut()
                    {
                        pool.add(&outputs[0].data, row * columns_dim);
                    }
                }
            }
            progress(0.6 + 0.4 * (index + 1) as f32 / window_count.max(1) as f32);
        }

        let mut voices = Vec::new();
        let mut voiced = Vec::new();
        for (lane, pool) in pools.iter().enumerate() {
            if let Some(unit) = pool.as_ref().and_then(|p| embedding.embed(p)) {
                voices.push(unit);
                voiced.push(lane);
            }
        }
        let mut labels = vec![-1i64; lanes.len()];
        for (lane, label) in voiced
            .iter()
            .zip(cluster::average_linkage(&voices, meta.join.cosine))
        {
            labels[*lane] = label as i64;
        }
        if meta.join.voiceless == "fold" && !voiced.is_empty() {
            let candidates: Vec<Vec<LabelledTurn>> = voiced
                .iter()
                .map(|&lane| lanes[lane].turns.clone())
                .collect();
            for lane in 0..lanes.len() {
                if labels[lane] < 0 {
                    let nearest = voices::nearest(&lanes[lane].turns, &candidates, &voiced);
                    labels[lane] = labels[nearest];
                }
            }
            return Ok(voices::first_appearance(&labels)
                .into_iter()
                .map(|v| v as i64)
                .collect());
        }

        let mut next = labels.iter().copied().max().unwrap_or(-1) + 1;
        for label in &mut labels {
            if *label < 0 {
                *label = next;
                next += 1;
            }
        }
        Ok(labels)
    }

    fn finish(turns: Vec<LabelledTurn>, duration: f64) -> Diarization {
        let mut turns = joined(&turns);
        turns.sort_by(|a, b| {
            a.start
                .partial_cmp(&b.start)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let raw: Vec<i64> = turns.iter().map(|t| t.label).collect();
        let numbered = voices::first_appearance(&raw);
        let speaker_count = numbered.iter().copied().max().map_or(0, |m| m + 1) as u32;
        Diarization {
            turns: turns
                .into_iter()
                .zip(numbered)
                .map(|(turn, label)| Turn {
                    start: turn.start,
                    end: turn.end,
                    speaker_index: label as u32,
                })
                .collect(),
            speaker_count,
            duration,
        }
    }
}

fn labelled(stretch: Stretch, clock: &FrameClock, offset: f64, label: i64) -> LabelledTurn {
    let (start, end) = clock.bounds(stretch);
    LabelledTurn {
        start: start + offset,
        end: end + offset,
        label,
    }
}

fn resample_linear(samples: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == 0 || to == 0 || from == to || samples.is_empty() {
        return samples.to_vec();
    }
    let ratio = from as f64 / to as f64;
    let count = (samples.len() as f64 * to as f64 / from as f64).round() as usize;
    (0..count)
        .map(|i| {
            let pos = i as f64 * ratio;
            let index = pos.floor() as usize;
            let frac = pos - index as f64;
            let a = samples[index.min(samples.len() - 1)];
            let b = samples[(index + 1).min(samples.len() - 1)];
            a + (b - a) * frac as f32
        })
        .collect()
}
