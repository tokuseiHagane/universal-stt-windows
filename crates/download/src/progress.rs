use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
pub struct DownloadProgress {
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub fraction: Option<f32>,
}

pub(crate) enum OutputLine {
    Progress(DownloadProgress),
    Title(String),
    File(PathBuf),
    Other,
}

pub(crate) fn parse_line(line: &str) -> OutputLine {
    if let Some(value) = marker(line, "USTT_PROGRESS|") {
        let fields: Vec<_> = value.split('|').collect();
        if fields.len() >= 4 {
            let downloaded_bytes = number(fields[0]).unwrap_or(0.0).max(0.0) as u64;
            let total_bytes = number(fields[1])
                .or_else(|| number(fields[2]))
                .map(|v| v as u64);
            let fraction = total_bytes
                .filter(|total| *total > 0)
                .map(|total| (downloaded_bytes as f64 / total as f64).clamp(0.0, 1.0) as f32)
                .or_else(|| number(fields[3]).map(|v| (v / 100.0).clamp(0.0, 1.0) as f32));
            return OutputLine::Progress(DownloadProgress {
                downloaded_bytes,
                total_bytes,
                fraction,
            });
        }
    }
    if let Some(value) = marker(line, "USTT_TITLE|") {
        return OutputLine::Title(value.trim().to_string());
    }
    if let Some(value) = marker(line, "USTT_FILE|") {
        return OutputLine::File(PathBuf::from(value.trim()));
    }
    OutputLine::Other
}

fn marker<'a>(line: &'a str, marker: &str) -> Option<&'a str> {
    line.find(marker).map(|index| &line[index + marker.len()..])
}

fn number(value: &str) -> Option<f64> {
    let value = value.trim().trim_end_matches('%');
    if value.eq_ignore_ascii_case("na") || value.eq_ignore_ascii_case("n/a") {
        None
    } else {
        value.parse().ok().filter(|v: &f64| v.is_finite())
    }
}
