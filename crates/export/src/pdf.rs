use crate::model::{ExportError, ExportLine};
use printpdf::{Base64OrRaw, GeneratePdfOptions, PdfDocument, PdfSaveOptions};
use std::collections::BTreeMap;
use ustt_core::Project;

const FONT: &[u8] = include_bytes!("../assets/NotoSans-Regular.ttf");

pub fn render_pdf(
    project: &Project,
    lines: &[ExportLine],
    timestamps: bool,
) -> Result<Vec<u8>, ExportError> {
    let speakers = project
        .speakers
        .iter()
        .map(|s| escape(&s.name))
        .collect::<Vec<_>>()
        .join(", ");
    let mut html = String::from(
        "<html><head><style>\
         body{font-family:'Noto';font-size:10.5pt;color:#1f2328;line-height:1.45}\
         h1{font-size:20pt;margin:0 0 6pt 0}\
         .meta{color:#666;font-size:9pt;margin-bottom:14pt}\
         .line{margin:0 0 4pt 0}\
         .who{font-weight:600}\
         </style></head><body>",
    );
    html.push_str(&format!("<h1>{}</h1>", escape(&project.title)));
    html.push_str(&format!(
        "<p class=\"meta\">{} · {} · {}</p>",
        project.modified_at.format("%Y-%m-%d"),
        duration(project.duration),
        speakers
    ));
    for line in lines {
        let prefix = if timestamps {
            format!("[{}] ", clock(line.start))
        } else {
            String::new()
        };
        html.push_str(&format!(
            "<p class=\"line\">{}<span class=\"who\">{}:</span> {}</p>",
            prefix,
            escape(&line.speaker),
            escape(&line.text)
        ));
    }
    html.push_str("</body></html>");

    let mut fonts = BTreeMap::new();
    fonts.insert("Noto".to_string(), Base64OrRaw::Raw(FONT.to_vec()));
    let options = GeneratePdfOptions {
        page_width: Some(210.0),
        page_height: Some(297.0),
        margin_top: Some(18.0),
        margin_bottom: Some(18.0),
        margin_left: Some(18.0),
        margin_right: Some(18.0),
        ..Default::default()
    };
    let mut warnings = Vec::new();
    let doc = PdfDocument::from_html(&html, &BTreeMap::new(), &fonts, &options, &mut warnings)
        .map_err(ExportError::Pdf)?;
    Ok(doc.save(&PdfSaveOptions::default(), &mut warnings))
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
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
