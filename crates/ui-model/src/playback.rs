#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Playback {
    pub position: f64,
    pub duration: f64,
    pub playing: bool,
    pub rate: f64,
    pub volume: f64,
    pub muted: bool,
}

impl Default for Playback {
    fn default() -> Self {
        Self {
            position: 0.0,
            duration: 0.0,
            playing: false,
            rate: 1.0,
            volume: 1.0,
            muted: false,
        }
    }
}

impl Playback {
    pub fn seek(&mut self, seconds: f64) {
        self.position = if self.duration > 0.0 {
            seconds.clamp(0.0, self.duration)
        } else {
            0.0
        };
    }

    pub fn step(&mut self, delta: f64) {
        self.seek(self.position + delta);
    }

    pub fn set_duration(&mut self, duration: f64) {
        self.duration = duration.max(0.0);
        self.position = self.position.clamp(0.0, self.duration);
    }
}
