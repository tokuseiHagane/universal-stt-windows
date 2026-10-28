use crate::AsrError;
use std::ffi::{CString, c_char};
use std::path::{Path, PathBuf};

pub(crate) struct CPath {
    path: CString,
    _guard: Option<tempfile::TempDir>,
}

impl CPath {
    pub(crate) fn as_ptr(&self) -> *const c_char {
        self.path.as_ptr()
    }

    #[cfg(test)]
    fn to_str(&self) -> Option<&str> {
        self.path.to_str().ok()
    }
}

/// parakeet.cpp opens VAD files through the Windows ANSI API. Prefer an
/// ASCII path relative to the portable directory, then an 8.3 alias, then a
/// temporary hard link/copy on an ASCII path.
pub(crate) fn c_path(path: &Path) -> Result<CPath, AsrError> {
    let (alias, guard) = compatible_path(path)?;
    let path = CString::new(alias.as_os_str().as_encoded_bytes())
        .map_err(|e| AsrError::Lib(e.to_string()))?;
    Ok(CPath {
        path,
        _guard: guard,
    })
}

#[cfg(any(windows, test))]
fn bridge(path: &Path, base: &Path) -> Result<(PathBuf, tempfile::TempDir), AsrError> {
    let guard = tempfile::Builder::new()
        .prefix("ustt-path-")
        .tempdir_in(base)?;
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .filter(|value| {
            !value.is_empty()
                && value.len() <= 16
                && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
        })
        .unwrap_or("bin");
    let alias = guard.path().join(format!("input.{extension}"));
    if std::fs::hard_link(path, &alias).is_err() {
        std::fs::copy(path, &alias)?;
    }
    Ok((alias, guard))
}

#[cfg(not(windows))]
fn compatible_path(path: &Path) -> Result<(PathBuf, Option<tempfile::TempDir>), AsrError> {
    Ok((path.to_path_buf(), None))
}

#[cfg(windows)]
fn compatible_path(path: &Path) -> Result<(PathBuf, Option<tempfile::TempDir>), AsrError> {
    if path.to_string_lossy().is_ascii() {
        return Ok((path.to_path_buf(), None));
    }
    if let Ok(cwd) = std::env::current_dir()
        && let Ok(relative) = path.strip_prefix(cwd)
        && relative.to_string_lossy().is_ascii()
    {
        return Ok((relative.to_path_buf(), None));
    }
    if let Some(alias) = short_path(path) {
        return Ok((alias, None));
    }

    let mut bases = vec![std::env::temp_dir(), PathBuf::from(r"C:\Users\Public")];
    if let Ok(cwd) = std::env::current_dir() {
        bases.push(cwd);
    }
    let mut last_error = None;
    for base in bases {
        let base = if base.to_string_lossy().is_ascii() {
            base
        } else if let Some(alias) = short_path(&base) {
            alias
        } else {
            continue;
        };
        match bridge(path, &base) {
            Ok((alias, guard)) => return Ok((alias, Some(guard))),
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.unwrap_or_else(|| {
        AsrError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!(
                "no ASCII-compatible temporary directory for {}",
                path.display()
            ),
        ))
    }))
}

#[cfg(windows)]
fn short_path(path: &Path) -> Option<PathBuf> {
    use std::ffi::OsString;
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use windows_sys::Win32::Storage::FileSystem::GetShortPathNameW;

    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let needed = unsafe { GetShortPathNameW(wide.as_ptr(), std::ptr::null_mut(), 0) };
    if needed == 0 {
        return None;
    }
    let mut buffer = vec![0u16; needed as usize + 1];
    let written =
        unsafe { GetShortPathNameW(wide.as_ptr(), buffer.as_mut_ptr(), buffer.len() as u32) };
    if written == 0 || written as usize >= buffer.len() {
        return None;
    }
    let alias = PathBuf::from(OsString::from_wide(&buffer[..written as usize]));
    alias.to_string_lossy().is_ascii().then_some(alias)
}

#[cfg(test)]
mod tests {
    use super::{bridge, c_path};
    use std::path::Path;

    #[test]
    fn ascii_paths_unchanged() {
        let path = Path::new("models/silero-vad-f16.gguf");
        assert_eq!(
            c_path(path).unwrap().to_str().unwrap(),
            path.to_str().unwrap()
        );
    }

    #[test]
    fn bridge_copies_non_ascii_input_to_an_ascii_path() {
        let source = tempfile::tempdir().unwrap();
        let base = tempfile::tempdir().unwrap();
        let input = source.path().join("речь.wav");
        std::fs::write(&input, b"wav-data").unwrap();

        let (alias, guard) = bridge(&input, base.path()).unwrap();
        assert!(alias.to_string_lossy().is_ascii());
        assert_eq!(std::fs::read(&alias).unwrap(), b"wav-data");
        let alias_dir = alias.parent().unwrap().to_path_buf();
        drop(guard);
        assert!(!alias_dir.exists());
    }
}
