#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Download,
    Probe,
    Decode,
    Proxy,
    Diarize,
    Transcribe,
    Merge,
    Done,
}

pub const PHASE_WEIGHTS: [f32; 6] = [0.02, 0.05, 0.08, 0.20, 0.60, 0.05];
pub const DOWNLOAD_WEIGHT: f32 = 0.25;

pub fn phase_start(phase: Phase) -> f32 {
    match phase {
        Phase::Download => 0.0,
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
        Phase::Download => DOWNLOAD_WEIGHT,
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

pub fn with_overall_eta(progress: Progress, elapsed_s: f64) -> Progress {
    if progress.eta_seconds.is_some() || progress.fraction <= 0.0 || progress.fraction >= 1.0 {
        return progress;
    }
    let eta = estimate_eta(progress.fraction as f64, 1.0, elapsed_s);
    if eta.is_finite() {
        progress.with_eta(eta.max(0.0))
    } else {
        progress
    }
}

pub fn with_phase_eta(progress: Progress, elapsed_s: f64) -> Progress {
    if progress.phase_eta_seconds.is_some()
        || progress.phase_fraction <= 0.0
        || progress.phase_fraction >= 1.0
    {
        return progress;
    }
    let eta = estimate_eta(progress.phase_fraction as f64, 1.0, elapsed_s);
    if eta.is_finite() {
        Progress {
            phase_eta_seconds: Some(eta.max(0.0)),
            ..progress
        }
    } else {
        progress
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Progress {
    pub phase: Phase,
    pub fraction: f32,
    pub phase_fraction: f32,
    pub eta_seconds: Option<f64>,
    pub phase_eta_seconds: Option<f64>,
    pub detail: String,
}

impl Progress {
    pub fn new(phase: Phase, fraction: f32, detail: impl Into<String>) -> Self {
        Self {
            phase,
            fraction: overall(phase, fraction),
            phase_fraction: fraction.clamp(0.0, 1.0),
            eta_seconds: None,
            phase_eta_seconds: None,
            detail: detail.into(),
        }
    }

    pub fn new_scaled(
        phase: Phase,
        fraction: f32,
        detail: impl Into<String>,
        download_weight: f32,
    ) -> Self {
        let mut progress = Self::new(phase, fraction, detail);
        let download_weight = download_weight.clamp(0.0, 1.0);
        if download_weight > 0.0 && phase != Phase::Done {
            progress.fraction = if phase == Phase::Download {
                progress.phase_fraction * download_weight
            } else {
                download_weight + progress.fraction * (1.0 - download_weight)
            };
        }
        progress
    }

    pub fn with_eta(mut self, seconds: f64) -> Self {
        self.eta_seconds = Some(seconds);
        self
    }
}
