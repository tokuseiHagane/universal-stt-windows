use crate::i18n::Lang;
use chrono::{DateTime, Utc};

pub fn timecode(seconds: f64) -> String {
    let total = seconds.max(0.0).floor() as u64;
    format!(
        "{:02}:{:02}:{:02}",
        total / 3600,
        total / 60 % 60,
        total % 60
    )
}

pub fn duration_label(seconds: f64) -> String {
    timecode(seconds)
}

pub fn date_label(at: &DateTime<Utc>, lang: Lang) -> String {
    match lang {
        Lang::Ru => at.format("%d.%m.%Y").to_string(),
        Lang::En => at.format("%Y-%m-%d").to_string(),
    }
}
