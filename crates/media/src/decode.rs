use crate::MediaError;
use crate::tools::Tools;
use std::path::Path;
use std::process::Command;

pub fn decode_wav16k(tools: &Tools, source: &Path, dest: &Path) -> Result<(), MediaError> {
    if let Some(parent) = dest.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }

    let output = Command::new(&tools.ffmpeg)
        .args(["-y", "-i"])
        .arg(source)
        .args(["-vn", "-ac", "1", "-ar", "16000", "-c:a", "pcm_s16le"])
        .arg(dest)
        .output()?;

    if !output.status.success() {
        return Err(MediaError::Decode(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }
    Ok(())
}
