use crate::AsrError;
use std::ffi::CString;
use std::path::{Path, PathBuf};

/// parakeet.cpp opens VAD files through the Windows ANSI API. Prefer an
/// ASCII path relative to the portable directory, then an 8.3 alias.
pub(crate) fn c_path(path: &Path) -> Result<CString, AsrError> {
    let alias = compatible_path(path);
    CString::new(alias.to_string_lossy().as_bytes()).map_err(|e| AsrError::Lib(e.to_string()))
}

#[cfg(not(windows))]
fn compatible_path(path: &Path) -> PathBuf {
    path.to_path_buf()
}

#[cfg(windows)]
fn compatible_path(path: &Path) -> PathBuf {
    if path.to_string_lossy().is_ascii() {
        return path.to_path_buf();
    }
    if let Ok(cwd) = std::env::current_dir()
        && let Ok(relative) = path.strip_prefix(cwd)
        && relative.to_string_lossy().is_ascii()
    {
        return relative.to_path_buf();
    }
    short_path(path).unwrap_or_else(|| path.to_path_buf())
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
    use super::c_path;
    use std::path::Path;

    #[test]
    fn ascii_paths_unchanged() {
        let path = Path::new("models/silero-vad-f16.gguf");
        assert_eq!(
            c_path(path).unwrap().to_str().unwrap(),
            path.to_str().unwrap()
        );
    }
}
