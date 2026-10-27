#[derive(Debug, Clone, PartialEq)]
pub struct LabelledTurn {
    pub start: f64,
    pub end: f64,
    pub label: i64,
}

/// A speaker's pieces that touch made one turn, so a turn cut at a piece's edge reads as one.
pub fn joined(turns: &[LabelledTurn]) -> Vec<LabelledTurn> {
    let mut sorted = turns.to_vec();
    sorted.sort_by(|a, b| {
        a.label.cmp(&b.label).then(
            a.start
                .partial_cmp(&b.start)
                .unwrap_or(std::cmp::Ordering::Equal),
        )
    });
    let mut out: Vec<LabelledTurn> = Vec::new();
    for turn in sorted {
        if let Some(last) = out.last_mut()
            && last.label == turn.label
            && turn.start <= last.end + 1e-6
        {
            last.end = last.end.max(turn.end);
        } else {
            out.push(turn);
        }
    }
    out
}

pub fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

pub fn unit(v: &[f32]) -> Vec<f32> {
    let norm = dot(v, v).sqrt() + 1e-8;
    v.iter().map(|x| x / norm).collect()
}
