use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Rule {
    Product,
    #[default]
    Mask,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Decision {
    pub presence_threshold: f32,
    pub mask_threshold: f32,
    pub rule: Rule,
    pub pool: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stretch {
    pub query: usize,
    pub first: usize,
    pub end: usize,
}

impl Decision {
    pub fn from_meta(decision: &crate::meta::Decision) -> Self {
        Self {
            presence_threshold: decision.presence_threshold,
            mask_threshold: decision.mask_threshold,
            rule: match decision.rule.as_str() {
                "product" => Rule::Product,
                _ => Rule::Mask,
            },
            pool: decision.pool.max(1),
        }
    }

    pub fn stretches(&self, presence: &[f32], masks: &[f32], frames: usize) -> Vec<Stretch> {
        let mut out = Vec::new();
        for (query, &probability) in presence.iter().enumerate() {
            if probability <= self.presence_threshold {
                continue;
            }
            let row = &masks[query * frames..(query + 1) * frames];
            let mut activity: Vec<f32> = if self.rule == Rule::Product {
                row.iter().map(|m| m * probability).collect()
            } else {
                row.to_vec()
            };
            if self.pool > 1 {
                let mut start = 0;
                while start < frames {
                    let end = (start + self.pool).min(frames);
                    let mean = activity[start..end].iter().sum::<f32>() / (end - start) as f32;
                    activity[start..end].fill(mean);
                    start = end;
                }
            }
            let mut first: Option<usize> = None;
            for (frame, &value) in activity.iter().enumerate() {
                let on = value > self.mask_threshold;
                if on && first.is_none() {
                    first = Some(frame);
                }
                if !on && let Some(a) = first {
                    out.push(Stretch {
                        query,
                        first: a,
                        end: frame,
                    });
                    first = None;
                }
            }
            if let Some(a) = first {
                out.push(Stretch {
                    query,
                    first: a,
                    end: frames,
                });
            }
        }
        out
    }
}

pub fn sigmoid(values: &[f32]) -> Vec<f32> {
    values.iter().map(|&z| 1.0 / (1.0 + (-z).exp())).collect()
}

#[derive(Debug, Clone, Copy)]
pub struct FrameClock {
    pub step: f64,
    pub first_centre: f64,
}

impl FrameClock {
    pub fn centre(&self, frame: usize) -> f64 {
        self.first_centre + self.step * frame as f64
    }

    pub fn bounds(&self, stretch: Stretch) -> (f64, f64) {
        (
            (self.centre(stretch.first) - self.step / 2.0).max(0.0),
            self.centre(stretch.end - 1) + self.step / 2.0,
        )
    }
}
