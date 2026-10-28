#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhaseView {
    Download,
    Probe,
    Decode,
    Proxy,
    Diarize,
    Transcribe,
    Merge,
    Done,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProgressView {
    pub phase: PhaseView,
    pub fraction: f32,
    pub phase_fraction: f32,
    pub eta_seconds: Option<f64>,
    pub phase_eta_seconds: Option<f64>,
    pub detail: String,
}

impl ProgressView {
    pub fn new(phase: PhaseView, fraction: f32) -> Self {
        Self {
            phase,
            fraction: fraction.clamp(0.0, 1.0),
            phase_fraction: fraction.clamp(0.0, 1.0),
            eta_seconds: None,
            phase_eta_seconds: None,
            detail: String::new(),
        }
    }
}
