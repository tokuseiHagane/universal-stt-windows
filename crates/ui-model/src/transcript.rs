use ustt_core::Project;

pub const ROW_HEIGHT: f64 = 64.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VirtualRows {
    pub first: usize,
    pub last: usize,
    pub offset_px: f64,
    pub total_px: f64,
}

pub fn virtual_rows(scroll_top: f64, viewport_h: f64, row_h: f64, count: usize) -> VirtualRows {
    if row_h <= 0.0 || count == 0 {
        return VirtualRows {
            first: 0,
            last: 0,
            offset_px: 0.0,
            total_px: 0.0,
        };
    }
    let scroll_top = scroll_top.max(0.0);
    let first = ((scroll_top / row_h).floor() as usize).min(count);
    let last = (((scroll_top + viewport_h.max(0.0)) / row_h).ceil() as usize).min(count);
    VirtualRows {
        first,
        last: last.max(first),
        offset_px: first as f64 * row_h,
        total_px: count as f64 * row_h,
    }
}

pub fn active_segment(project: &Project, seconds: f64) -> Option<usize> {
    project
        .segments
        .iter()
        .position(|s| seconds >= s.start && seconds < s.end)
}
