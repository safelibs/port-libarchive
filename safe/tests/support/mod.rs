#![allow(dead_code)]

pub mod fixtures;
pub mod ported;
pub mod upstream;

use std::ffi::{c_char, CStr, CString};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

static UNIQUE_ID: AtomicU64 = AtomicU64::new(0);
static CWD_LOCK: Mutex<()> = Mutex::new(());

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

pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    pub fn new(stem: &str) -> Self {
        let path = temp_path(stem);
        fs::create_dir_all(&path).expect("failed to create temporary directory");
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

pub struct CurrentDirGuard {
    original: PathBuf,
    _cwd_lock: MutexGuard<'static, ()>,
}

impl Drop for CurrentDirGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.original);
    }
}

pub fn pushd(path: &Path) -> CurrentDirGuard {
    let cwd_lock = CWD_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let original = std::env::current_dir().expect("current dir");
    std::env::set_current_dir(path).expect("failed to change current dir");
    CurrentDirGuard {
        original,
        _cwd_lock: cwd_lock,
    }
}

pub fn write_temp_file(stem: &str, contents: &[u8]) -> PathBuf {
    let path = temp_path(stem);
    fs::write(&path, contents).expect("failed to write temporary file");
    path
}

pub fn make_dir(path: &Path) {
    fs::create_dir_all(path).expect("failed to create directory");
}

pub fn write_file(path: &Path, contents: &[u8]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("failed to create parent directory");
    }
    fs::write(path, contents).expect("failed to write file");
}

pub fn read_file(path: &Path) -> Vec<u8> {
    fs::read(path).expect("failed to read file")
}

#[cfg(unix)]
pub fn symlink(path: &Path, target: &Path) {
    std::os::unix::fs::symlink(target, path).expect("failed to create symlink");
}
