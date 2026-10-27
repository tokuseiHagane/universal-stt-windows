use crate::model::{ExportLine, TextOutput};

pub fn render_text(lines: &[ExportLine], format: TextOutput, timestamps: bool) -> String {
    match format {
        TextOutput::Txt => render_txt(lines, timestamps),
        TextOutput::Markdown => render_markdown(lines, timestamps),
        TextOutput::Srt => render_cues(lines, ',', true),
        TextOutput::Vtt => format!("WEBVTT\n\n{}", render_cues(lines, '.', false)),
        TextOutput::Docx | TextOutput::Pdf => String::new(),
    }
}

fn stamp(seconds: f64, separator: char) -> String {
    let total_ms = (seconds.max(0.0) * 1000.0).round() as u64;
    let h = total_ms / 3_600_000;
    let m = total_ms / 60_000 % 60;
    let s = total_ms / 1000 % 60;
    let ms = total_ms % 1000;
    format!("{h:02}:{m:02}:{s:02}{separator}{ms:03}")
}

fn short_stamp(seconds: f64) -> String {
    stamp(seconds, ':').chars().take(8).collect()
}

fn line_text(line: &ExportLine, timestamps: bool) -> String {
    if timestamps {
        format!(
            "[{}] {}: {}",
            short_stamp(line.start),
            line.speaker,
            line.text
        )
    } else {
        format!("{}: {}", line.speaker, line.text)
    }
}

fn render_txt(lines: &[ExportLine], timestamps: bool) -> String {
    let mut out = String::new();
    for line in lines {
        out.push_str(&line_text(line, timestamps));
        out.push('\n');
    }
    out
}

fn render_markdown(lines: &[ExportLine], timestamps: bool) -> String {
    let mut out = String::new();
    let mut current = "";
    for line in lines {
        if line.speaker != current {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&format!("**{}**\n\n", line.speaker));
            current = &line.speaker;
        }
        out.push_str(&line_text(line, timestamps));
        out.push_str("\n\n");
    }
    out
}

fn render_cues(lines: &[ExportLine], separator: char, numbered: bool) -> String {
    let mut out = String::new();
    for (index, line) in lines.iter().enumerate() {
        if numbered {
            out.push_str(&format!("{}\n", index + 1));
        }
        out.push_str(&format!(
            "{} --> {}\n{}: {}\n\n",
            stamp(line.start, separator),
            stamp(line.end, separator),
            line.speaker,
            line.text
        ));
    }
    out
}
