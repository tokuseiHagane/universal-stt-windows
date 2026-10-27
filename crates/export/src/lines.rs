use crate::model::ExportLine;
use ustt_core::Project;
use uuid::Uuid;

pub fn lines(project: &Project, speakers: Option<&[Uuid]>) -> Vec<ExportLine> {
    project
        .segments
        .iter()
        .filter(|segment| match (speakers, segment.speaker_id) {
            (Some(allowed), Some(id)) => allowed.contains(&id),
            (Some(_), None) => false,
            (None, _) => true,
        })
        .map(|segment| ExportLine {
            start: segment.start,
            end: segment.end,
            speaker: segment
                .speaker_id
                .and_then(|id| project.speakers.iter().find(|s| s.id == id))
                .map(|s| s.name.clone())
                .unwrap_or_else(|| "Speaker".into()),
            text: segment.text.clone(),
        })
        .collect()
}
