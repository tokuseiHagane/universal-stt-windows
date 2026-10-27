/// Fixed eight-colour palette for speakers; cycles after eight.
pub const SPEAKER_COLORS: [&str; 8] = [
    "#4F8EF7", "#F2A65A", "#5BC0A8", "#E5678B", "#9B7EDE", "#D9C04A", "#57A9D6", "#C77DFF",
];

pub fn color_for_index(index: usize) -> &'static str {
    SPEAKER_COLORS[index % SPEAKER_COLORS.len()]
}
