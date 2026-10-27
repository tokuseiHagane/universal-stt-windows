use ustt_core::Segment;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimelineLayout {
    pub px_per_sec: f64,
    pub scroll_sec: f64,
    pub width_px: f64,
    pub duration: f64,
}

impl TimelineLayout {
    pub fn new(px_per_sec: f64, width_px: f64, duration: f64) -> Self {
        Self {
            px_per_sec: px_per_sec.clamp(1.0, 2000.0),
            scroll_sec: 0.0,
            width_px: width_px.max(1.0),
            duration: duration.max(0.0),
        }
    }

    pub fn x(&self, seconds: f64) -> f64 {
        (seconds.clamp(0.0, self.duration) - self.scroll_sec) * self.px_per_sec
    }

    pub fn time(&self, x: f64) -> f64 {
        (self.scroll_sec + x / self.px_per_sec).clamp(0.0, self.duration)
    }

    pub fn visible(&self) -> (f64, f64) {
        (
            self.scroll_sec,
            (self.scroll_sec + self.width_px / self.px_per_sec).min(self.duration),
        )
    }

    pub fn zoom_at(&mut self, x: f64, factor: f64) {
        let at = self.time(x);
        let px = (self.px_per_sec * factor).clamp(1.0, 2000.0);
        self.px_per_sec = px;
        self.scroll_sec = at - x / px;
        self.clamp_scroll();
    }

    pub fn scroll_by(&mut self, seconds: f64) {
        self.scroll_sec += seconds;
        self.clamp_scroll();
    }

    fn clamp_scroll(&mut self) {
        self.scroll_sec = self.scroll_sec.clamp(
            0.0,
            (self.duration - self.width_px / self.px_per_sec).max(0.0),
        );
    }
}

pub fn hit_segment(segments: &[Segment], seconds: f64) -> Option<usize> {
    segments
        .iter()
        .position(|s| seconds >= s.start && seconds <= s.end)
}
