use ustt_core::Project;
use uuid::Uuid;

pub fn rename_speaker(project: &mut Project, id: Uuid, name: &str) -> bool {
    let name = name.trim();
    if name.is_empty() {
        return false;
    }
    match project.speakers.iter_mut().find(|s| s.id == id) {
        Some(speaker) if speaker.name != name => {
            speaker.name = name.to_string();
            true
        }
        _ => false,
    }
}

pub fn set_segment_text(project: &mut Project, id: Uuid, text: &str) -> bool {
    let text = text.trim();
    if text.is_empty() {
        return false;
    }
    match project.segments.iter_mut().find(|s| s.id == id) {
        Some(segment) if segment.text != text => {
            segment.text = text.to_string();
            segment.words.clear();
            true
        }
        _ => false,
    }
}

pub fn speaker_time(project: &Project, id: Uuid) -> f64 {
    project
        .segments
        .iter()
        .filter(|s| s.speaker_id == Some(id))
        .map(|s| (s.end - s.start).max(0.0))
        .sum()
}
