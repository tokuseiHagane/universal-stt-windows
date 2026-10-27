use crate::model::{ExportError, ExportLine};
use docx_rs::{AlignmentType, Docx, Paragraph, Run};
use std::io::Cursor;
use ustt_core::Project;

pub fn render_docx(
    project: &Project,
    lines: &[ExportLine],
    timestamps: bool,
) -> Result<Vec<u8>, ExportError> {
    let meta = format!(
        "{} · {} · {}",
        project.modified_at.format("%Y-%m-%d"),
        duration(project.duration),
        project
            .speakers
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );
    let mut doc = Docx::new()
        .default_size(22)
        .add_paragraph(
            Paragraph::new()
                .align(AlignmentType::Left)
                .add_run(Run::new().bold().add_text(&project.title)),
        )
        .add_paragraph(Paragraph::new().add_run(Run::new().add_text(meta)));
    for line in lines {
        let text = if timestamps {
            format!("[{}] {}: {}", clock(line.start), line.speaker, line.text)
        } else {
            format!("{}: {}", line.speaker, line.text)
        };
        doc = doc.add_paragraph(Paragraph::new().add_run(Run::new().add_text(text)));
    }
    let mut bytes = Vec::new();
    doc.build()
        .pack(Cursor::new(&mut bytes))
        .map_err(|e| ExportError::Docx(e.to_string()))?;
    Ok(bytes)
}

fn clock(seconds: f64) -> String {
    let total = seconds.max(0.0) as u64;
    format!(
        "{:02}:{:02}:{:02}",
        total / 3600,
        total / 60 % 60,
        total % 60
    )
}

fn duration(seconds: f64) -> String {
    let total = seconds.max(0.0) as u64;
    format!(
        "{:02}:{:02}:{:02}",
        total / 3600,
        total / 60 % 60,
        total % 60
    )
}
