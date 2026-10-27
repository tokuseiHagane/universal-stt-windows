use crate::AsrError;
use crate::AsrJson;
use libloading::{Library, Symbol};
use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::path::Path;
use std::sync::Arc;

const ABI_MIN: u32 = 10;
const KIND_ASR: i32 = 1;
const KIND_VAD: i32 = 5;

type LoadFn = unsafe extern "C" fn(*const c_char) -> *mut c_void;
type FreeFn = unsafe extern "C" fn(*mut c_void);
type FreeStringFn = unsafe extern "C" fn(*mut c_char);
type LoadErrorFn = unsafe extern "C" fn() -> *const c_char;
type LastErrorFn = unsafe extern "C" fn(*mut c_void) -> *const c_char;
type TranscribeJsonFn = unsafe extern "C" fn(*mut c_void, *const c_char, c_int) -> *mut c_char;
type VadJsonFn = unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char) -> *mut c_char;
type ModelKindFn = unsafe extern "C" fn(*const c_void) -> c_int;
type AbiFn = unsafe extern "C" fn() -> c_int;

pub struct ParakeetLib {
    library: Library,
    abi: u32,
}

impl ParakeetLib {
    pub fn open(path: &Path) -> Result<Self, AsrError> {
        let library = unsafe { Library::new(path) }
            .map_err(|e| AsrError::Lib(format!("{}: {e}", path.display())))?;
        let abi_fn: Symbol<AbiFn> = unsafe { library.get(b"parakeet_capi_abi_version\0") }
            .map_err(|e| AsrError::Lib(format!("abi_version: {e}")))?;
        let abi = unsafe { abi_fn() } as u32;
        if abi < ABI_MIN {
            return Err(AsrError::Lib(format!(
                "parakeet ABI {abi} is older than {ABI_MIN}"
            )));
        }
        let lib = Self { library, abi };
        lib.symbol::<LoadFn>(b"parakeet_capi_load\0")?;
        lib.symbol::<FreeFn>(b"parakeet_capi_free\0")?;
        lib.symbol::<FreeStringFn>(b"parakeet_capi_free_string\0")?;
        lib.symbol::<LoadErrorFn>(b"parakeet_capi_load_error\0")?;
        lib.symbol::<LastErrorFn>(b"parakeet_capi_last_error\0")?;
        lib.symbol::<TranscribeJsonFn>(b"parakeet_capi_transcribe_path_json\0")?;
        lib.symbol::<ModelKindFn>(b"parakeet_capi_model_kind\0")?;
        Ok(lib)
    }

    pub fn abi_version(&self) -> u32 {
        self.abi
    }

    fn symbol<T>(&self, name: &[u8]) -> Result<Symbol<'_, T>, AsrError> {
        unsafe { self.library.get(name) }
            .map_err(|e| AsrError::Lib(format!("{}: {e}", String::from_utf8_lossy(name))))
    }

    pub(crate) fn load_model(
        &self,
        path: &Path,
        expected_kind: i32,
    ) -> Result<*mut c_void, AsrError> {
        let path = CString::new(path.to_string_lossy().as_bytes())
            .map_err(|e| AsrError::Lib(e.to_string()))?;
        let load: Symbol<LoadFn> = self.symbol(b"parakeet_capi_load\0")?;
        let ctx = unsafe { load(path.as_ptr()) };
        if ctx.is_null() {
            return Err(AsrError::Model(self.load_error()));
        }
        let kind_fn: Symbol<ModelKindFn> = self.symbol(b"parakeet_capi_model_kind\0")?;
        let kind = unsafe { kind_fn(ctx) };
        if kind != expected_kind {
            self.free(ctx);
            return Err(AsrError::Model(format!(
                "model kind {kind} is not {expected_kind}"
            )));
        }
        Ok(ctx)
    }

    pub(crate) fn free(&self, ctx: *mut c_void) {
        if ctx.is_null() {
            return;
        }
        if let Ok(free) = self.symbol::<FreeFn>(b"parakeet_capi_free\0") {
            unsafe { free(ctx) };
        }
    }

    fn load_error(&self) -> String {
        match self.symbol::<LoadErrorFn>(b"parakeet_capi_load_error\0") {
            Ok(get) => {
                let ptr = unsafe { get() };
                if ptr.is_null() {
                    "unknown load error".into()
                } else {
                    unsafe { CStr::from_ptr(ptr) }
                        .to_string_lossy()
                        .into_owned()
                }
            }
            Err(e) => e.to_string(),
        }
    }

    fn last_error(&self, ctx: *mut c_void) -> String {
        match self.symbol::<LastErrorFn>(b"parakeet_capi_last_error\0") {
            Ok(get) => {
                let ptr = unsafe { get(ctx) };
                if ptr.is_null() {
                    "unknown parakeet error".into()
                } else {
                    unsafe { CStr::from_ptr(ptr) }
                        .to_string_lossy()
                        .into_owned()
                }
            }
            Err(e) => e.to_string(),
        }
    }

    fn take_string(&self, ptr: *mut c_char) -> Option<String> {
        if ptr.is_null() {
            return None;
        }
        let text = unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned();
        if let Ok(free) = self.symbol::<FreeStringFn>(b"parakeet_capi_free_string\0") {
            unsafe { free(ptr) };
        }
        Some(text)
    }

    pub(crate) fn transcribe_json(&self, ctx: *mut c_void, wav: &Path) -> Result<String, AsrError> {
        let path = CString::new(wav.to_string_lossy().as_bytes())
            .map_err(|e| AsrError::Lib(e.to_string()))?;
        let call: Symbol<TranscribeJsonFn> =
            self.symbol(b"parakeet_capi_transcribe_path_json\0")?;
        let out = unsafe { call(ctx, path.as_ptr(), 0) };
        self.take_string(out)
            .ok_or_else(|| AsrError::Model(self.last_error(ctx)))
    }

    pub(crate) fn vad_json(
        &self,
        ctx: *mut c_void,
        wav: &Path,
        options: &str,
    ) -> Result<String, AsrError> {
        let path = CString::new(wav.to_string_lossy().as_bytes())
            .map_err(|e| AsrError::Lib(e.to_string()))?;
        let options = CString::new(options).map_err(|e| AsrError::Lib(e.to_string()))?;
        let call: Symbol<VadJsonFn> = self.symbol(b"parakeet_capi_vad_path_json\0")?;
        let out = unsafe { call(ctx, path.as_ptr(), options.as_ptr()) };
        self.take_string(out)
            .ok_or_else(|| AsrError::Model(self.last_error(ctx)))
    }
}

/// A parakeet context, used from one worker thread at a time.
struct SendPtr(*mut c_void);
unsafe impl Send for SendPtr {}

pub struct AsrEngine {
    lib: Arc<ParakeetLib>,
    ctx: SendPtr,
}

impl AsrEngine {
    pub fn load(lib: Arc<ParakeetLib>, model: &Path) -> Result<Self, AsrError> {
        let ctx = lib.load_model(model, KIND_ASR)?;
        Ok(Self {
            lib,
            ctx: SendPtr(ctx),
        })
    }

    pub fn transcribe_wav(&mut self, wav: &Path) -> Result<AsrJson, AsrError> {
        let text = self.lib.transcribe_json(self.ctx.0, wav)?;
        Ok(serde_json::from_str(&text)?)
    }
}

impl Drop for AsrEngine {
    fn drop(&mut self) {
        self.lib.free(self.ctx.0);
    }
}

pub struct VadEngine {
    lib: Arc<ParakeetLib>,
    ctx: SendPtr,
}

impl VadEngine {
    pub fn load(lib: Arc<ParakeetLib>, model: &Path) -> Result<Self, AsrError> {
        let ctx = lib.load_model(model, KIND_VAD)?;
        Ok(Self {
            lib,
            ctx: SendPtr(ctx),
        })
    }

    pub(crate) fn raw_segments_json(&mut self, wav: &Path) -> Result<String, AsrError> {
        self.lib.vad_json(self.ctx.0, wav, "")
    }
}

impl Drop for VadEngine {
    fn drop(&mut self) {
        self.lib.free(self.ctx.0);
    }
}
