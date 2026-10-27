#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Probe,
    Decode,
    Proxy,
    Diarize,
    Transcribe,
    Merge,
    Done,
}

pub const PHASE_WEIGHTS: [f32; 6] = [0.02, 0.05, 0.08, 0.20, 0.60, 0.05];

pub fn phase_start(phase: Phase) -> f32 {
    match phase {
        Phase::Probe => 0.0,
        Phase::Decode => 0.02,
        Phase::Proxy => 0.07,
        Phase::Diarize => 0.15,
        Phase::Transcribe => 0.35,
        Phase::Merge => 0.95,
        Phase::Done => 1.0,
    }
}

pub fn overall(phase: Phase, fraction: f32) -> f32 {
    let fraction = fraction.clamp(0.0, 1.0);
    (phase_start(phase) + weight(phase) * fraction).clamp(0.0, 1.0)
}

fn weight(phase: Phase) -> f32 {
    match phase {
        Phase::Probe => PHASE_WEIGHTS[0],
        Phase::Decode => PHASE_WEIGHTS[1],
        Phase::Proxy => PHASE_WEIGHTS[2],
        Phase::Diarize => PHASE_WEIGHTS[3],
        Phase::Transcribe => PHASE_WEIGHTS[4],
        Phase::Merge => PHASE_WEIGHTS[5],
        Phase::Done => 0.0,
    }
}

pub fn estimate_eta(processed_s: f64, total_s: f64, elapsed_s: f64) -> f64 {
    if processed_s <= 0.0 || total_s <= processed_s {
        return 0.0;
    }
    (total_s - processed_s) / processed_s * elapsed_s
}

#[derive(Debug, Clone, PartialEq)]
pub struct Progress {
    pub phase: Phase,
    pub fraction: f32,
    pub eta_seconds: Option<f64>,
    pub detail: String,
}

impl Progress {
    pub fn new(phase: Phase, fraction: f32, detail: impl Into<String>) -> Self {
        Self {
            phase,
            fraction: overall(phase, fraction),
            eta_seconds: None,
            detail: detail.into(),
        }
    }

    pub fn with_eta(mut self, seconds: f64) -> Self {
        self.eta_seconds = Some(seconds);
        self
    }
}
