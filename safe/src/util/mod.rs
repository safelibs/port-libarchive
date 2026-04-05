use std::ffi::{CStr, CString};

use crate::ffi::archive;
use crate::ffi::archive_common as ffi;

pub fn c_string(value: &str) -> CString {
    CString::new(value).expect("input must not contain interior NUL bytes")
}

pub fn c_str(ptr: *const i8) -> Option<String> {
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

pub struct ArchiveHandle {
    raw: *mut archive,
    free_fn: unsafe extern "C" fn(*mut archive) -> i32,
}

impl ArchiveHandle {
    pub fn reader() -> Self {
        Self::new(unsafe { ffi::archive_read_new() }, ffi::archive_read_free)
    }

    pub fn writer() -> Self {
        Self::new(unsafe { ffi::archive_write_new() }, ffi::archive_write_free)
    }

    pub fn read_disk() -> Self {
        Self::new(
            unsafe { ffi::archive_read_disk_new() },
            ffi::archive_read_free,
        )
    }

    pub fn write_disk() -> Self {
        Self::new(
            unsafe { ffi::archive_write_disk_new() },
            ffi::archive_write_free,
        )
    }

    fn new(raw: *mut archive, free_fn: unsafe extern "C" fn(*mut archive) -> i32) -> Self {
        assert!(!raw.is_null(), "archive constructor returned NULL");
        Self { raw, free_fn }
    }

    pub fn as_ptr(&self) -> *mut archive {
        self.raw
    }

    pub fn errno(&self) -> i32 {
        unsafe { ffi::archive_errno(self.raw) }
    }

    pub fn error_string(&self) -> Option<String> {
        c_str(unsafe { ffi::archive_error_string(self.raw) })
    }
}

impl Drop for ArchiveHandle {
    fn drop(&mut self) {
        unsafe {
            (self.free_fn)(self.raw);
        }
    }
}
