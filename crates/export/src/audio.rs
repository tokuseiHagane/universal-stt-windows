use crate::model::ExportError;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use ustt_core::{AudioFormat, Project};
use ustt_media::Tools;
use uuid::Uuid;

pub const PAUSE_S: f64 = 0.4;

#[derive(Debug, Clone, PartialEq)]
pub struct AudioTrack {
    pub name: String,
    pub ranges: Vec<(f64, f64)>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct AudioPlan {
    pub tracks: Vec<AudioTrack>,
}

pub fn plan_speakers(project: &Project, speakers: Option<&[Uuid]>) -> AudioPlan {
    let mut tracks = Vec::new();
    for speaker in &project.speakers {
        if speakers.is_some_and(|allowed| !allowed.contains(&speaker.id)) {
            continue;
        }
        let mut ranges: Vec<(f64, f64)> = project
            .segments
            .iter()
            .filter(|s| s.speaker_id == Some(speaker.id))
            .map(|s| (s.start, s.end))
            .collect();
        merge_ranges(&mut ranges);
        if ranges.is_empty() {
            continue;
        }
        tracks.push(AudioTrack {
            name: speaker.name.clone(),
            ranges,
        });
    }
    AudioPlan { tracks }
}

pub fn plan_segments(project: &Project, ids: &[Uuid]) -> AudioPlan {
    let mut ranges: Vec<(f64, f64)> = project
        .segments
        .iter()
        .filter(|s| ids.contains(&s.id))
        .map(|s| (s.start, s.end))
        .collect();
    merge_ranges(&mut ranges);
    if ranges.is_empty() {
        return AudioPlan::default();
    }
    AudioPlan {
        tracks: vec![AudioTrack {
            name: "selection".into(),
            ranges,
        }],
    }
}

fn merge_ranges(ranges: &mut Vec<(f64, f64)>) {
    ranges.retain(|(start, end)| end > start);
    ranges.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut merged: Vec<(f64, f64)> = Vec::new();
    for (start, end) in ranges.drain(..) {
        match merged.last_mut() {
            Some(last) if start - last.1 <= PAUSE_S => last.1 = last.1.max(end),
            _ => merged.push((start, end)),
        }
    }
    *ranges = merged;
}

pub fn build_filter_script(track: &AudioTrack, _wav: &Path) -> String {
    graph_for(&track.ranges, true)
}

pub fn track_filename(prefix: &str, index: usize, name: &str, extension: &str) -> String {
    format!(
        "{prefix}-{:02}-{}.{extension}",
        index + 1,
        ustt_core::slugify(name)
    )
}

fn unique_path(path: std::path::PathBuf) -> PathBuf {
    if !path.exists() {
        return path;
    }
    let stem = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let extension = path
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    for n in 2..10_000 {
        let candidate = path.with_file_name(format!("{stem}-{n}.{extension}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    path
}

/// One filter graph for a bounded batch of ranges. Batching keeps the
/// `-filter_complex` argument under the Windows command-line limit.
const BATCH: usize = 120;

fn graph_for(ranges: &[(f64, f64)], pad_last: bool) -> String {
    let mut script = String::new();
    for (index, (start, end)) in ranges.iter().enumerate() {
        let pad = if index + 1 == ranges.len() && !pad_last {
            String::new()
        } else {
            format!(",apad=pad_dur={PAUSE_S}")
        };
        script.push_str(&format!(
            "[0:a]atrim=start={start:.3}:end={end:.3},asetpts=PTS-STARTPTS{pad}[s{index}];"
        ));
    }
    for index in 0..ranges.len() {
        script.push_str(&format!("[s{index}]"));
    }
    script.push_str(&format!("concat=n={}:v=0:a=1[out]", ranges.len()));
    script
}

#[allow(clippy::too_many_arguments)]
pub fn extract(
    tools: &Tools,
    wav: &Path,
    plan: &AudioPlan,
    prefix: &str,
    format: AudioFormat,
    out_dir: &Path,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(f32, &str),
) -> Result<Vec<PathBuf>, ExportError> {
    if !wav.is_file() {
        return Err(ExportError::MissingMedia);
    }
    if plan.tracks.is_empty() {
        return Err(ExportError::Audio(
            "nothing selected for audio export".into(),
        ));
    }
    std::fs::create_dir_all(out_dir)?;
    let temp = tempfile::Builder::new()
        .prefix(".ustt-export-")
        .tempdir_in(out_dir)?;
    let extension = if matches!(format, AudioFormat::Wav) {
        "wav"
    } else {
        "m4a"
    };
    let mut outputs = Vec::new();
    for (index, track) in plan.tracks.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            cleanup(&outputs);
            return Err(ExportError::Cancelled);
        }
        let mut batches = Vec::new();
        for (batch_index, chunk) in track.ranges.chunks(BATCH).enumerate() {
            if cancel.load(Ordering::Relaxed) {
                cleanup(&outputs);
                return Err(ExportError::Cancelled);
            }
            let batch_path = temp.path().join(format!("track-{index}-{batch_index}.wav"));
            let is_last_batch = batch_index + 1 == track.ranges.chunks(BATCH).len();
            let graph = graph_for(chunk, !is_last_batch);
            let mut command = Command::new(&tools.ffmpeg);
            command
                .args(["-y", "-v", "error", "-i"])
                .arg(wav)
                .arg("-filter_complex")
                .arg(&graph)
                .args([
                    "-map",
                    "[out]",
                    "-c:a",
                    "pcm_s16le",
                    "-ar",
                    "16000",
                    "-ac",
                    "1",
                ])
                .arg(&batch_path);
            run_child(&mut command, cancel).inspect_err(|_| cleanup(&outputs))?;
            batches.push(batch_path);
        }
        let temp_out = temp.path().join(format!("track-{index}.wav"));
        if batches.len() == 1 {
            std::fs::rename(&batches[0], &temp_out)?;
        } else {
            let list = temp.path().join(format!("track-{index}.txt"));
            let mut body = String::new();
            for path in &batches {
                body.push_str(&format!(
                    "file '{}'\n",
                    path.file_name().unwrap().to_string_lossy()
                ));
            }
            std::fs::write(&list, body)?;
            let mut command = Command::new(&tools.ffmpeg);
            command
                .args(["-y", "-v", "error", "-f", "concat", "-safe", "0", "-i"])
                .arg(&list)
                .args(["-c:a", "pcm_s16le", "-ar", "16000", "-ac", "1"])
                .arg(&temp_out);
            run_child(&mut command, cancel).inspect_err(|_| cleanup(&outputs))?;
        }
        let temp_out = if matches!(format, AudioFormat::Wav) {
            temp_out
        } else {
            let encoded = temp.path().join(format!("track-{index}.m4a"));
            let mut command = Command::new(&tools.ffmpeg);
            command
                .args(["-y", "-v", "error", "-i"])
                .arg(&temp_out)
                .args(["-c:a", "aac", "-b:a", "192k"])
                .arg(&encoded);
            run_child(&mut command, cancel).inspect_err(|_| cleanup(&outputs))?;
            encoded
        };
        let final_path =
            unique_path(out_dir.join(track_filename(prefix, index, &track.name, extension)));
        std::fs::rename(&temp_out, &final_path)?;
        outputs.push(final_path);
        progress(
            (index + 1) as f32 / plan.tracks.len() as f32,
            &format!("exported {}", track.name),
        );
    }
    Ok(outputs)
}

fn cleanup(files: &[PathBuf]) {
    for file in files {
        let _ = std::fs::remove_file(file);
    }
}

fn run_child(command: &mut Command, cancel: &AtomicBool) -> Result<(), ExportError> {
    command.stdout(Stdio::null()).stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|e| ExportError::Audio(e.to_string()))?;
    loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(ExportError::Cancelled);
        }
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(_)) => {
                let output = child.wait_with_output().ok();
                let message = output
                    .map(|o| String::from_utf8_lossy(&o.stderr).trim().to_string())
                    .unwrap_or_default();
                return Err(ExportError::Audio(message));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
            Err(e) => return Err(ExportError::Audio(e.to_string())),
        }
    }
}
