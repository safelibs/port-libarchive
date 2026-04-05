use std::ffi::{c_char, CStr, CString};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static UNIQUE_ID: AtomicU64 = AtomicU64::new(0);

pub fn c_str(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        None
    } else {
        Some(
            unsafe { CStr::from_ptr(ptr) }
                .to_string_lossy()
                .into_owned(),
        )
    }
}

pub struct CStringArray {
    _storage: Vec<CString>,
    pointers: Vec<*mut c_char>,
}

impl CStringArray {
    pub fn new(values: &[&str]) -> Self {
        let storage: Vec<CString> = values
            .iter()
            .map(|value| CString::new(*value).expect("value must not contain NUL"))
            .collect();
        let mut pointers: Vec<*mut c_char> = storage
            .iter()
            .map(|value| value.as_ptr() as *mut c_char)
            .collect();
        pointers.push(std::ptr::null_mut());
        Self {
            _storage: storage,
            pointers,
        }
    }

    pub fn as_mut_ptr(&mut self) -> *mut *mut c_char {
        self.pointers.as_mut_ptr()
    }

    pub fn strings(&self) -> Vec<String> {
        self.pointers
            .iter()
            .take_while(|ptr| !ptr.is_null())
            .map(|ptr| c_str(*ptr as *const c_char).expect("pointer should contain UTF-8"))
            .collect()
    }
}

pub fn temp_path(stem: &str) -> PathBuf {
    let unique = UNIQUE_ID.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("libarchive-safe-{stem}-{nanos}-{unique}"))
}

pub fn write_temp_file(stem: &str, contents: &[u8]) -> PathBuf {
    let path = temp_path(stem);
    fs::write(&path, contents).expect("failed to write temporary file");
    path
}
