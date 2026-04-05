pub(crate) mod api;
pub(crate) mod internal;

use std::ffi::{c_void, CStr, CString};
use std::ptr;

use libc::{mode_t, size_t, stat, wchar_t};

use crate::ffi::{archive, archive_entry};
use crate::ffi::archive_entry_api as ffi;

fn to_cstring(value: &str) -> CString {
    CString::new(value).expect("input must not contain interior NUL bytes")
}

pub fn to_wide_null(value: &str) -> Vec<wchar_t> {
    value.encode_utf16().map(|unit| unit as wchar_t).chain([0]).collect()
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

pub struct EntryHandle {
    raw: *mut archive_entry,
}

impl EntryHandle {
    pub fn new() -> Self {
        let raw = unsafe { ffi::archive_entry_new() };
        assert!(!raw.is_null(), "archive_entry_new returned NULL");
        Self { raw }
    }

    pub fn new2(source: *mut archive) -> Self {
        let raw = unsafe { ffi::archive_entry_new2(source) };
        assert!(!raw.is_null(), "archive_entry_new2 returned NULL");
        Self { raw }
    }

    pub fn as_ptr(&self) -> *mut archive_entry {
        self.raw
    }

    pub fn into_raw(self) -> *mut archive_entry {
        let raw = self.raw;
        std::mem::forget(self);
        raw
    }

    pub unsafe fn from_raw(raw: *mut archive_entry) -> Self {
        Self { raw }
    }

    pub fn clear(&mut self) {
        unsafe {
            ffi::archive_entry_clear(self.raw);
        }
    }

    pub fn clone_entry(&self) -> Self {
        let raw = unsafe { ffi::archive_entry_clone(self.raw) };
        assert!(!raw.is_null(), "archive_entry_clone returned NULL");
        Self { raw }
    }

    pub fn set_pathname(&mut self, value: &str) {
        let value = to_cstring(value);
        unsafe {
            ffi::archive_entry_set_pathname(self.raw, value.as_ptr());
        }
    }

    pub fn set_pathname_w(&mut self, value: &str) {
        let value = to_wide_null(value);
        unsafe {
            ffi::archive_entry_copy_pathname_w(self.raw, value.as_ptr());
        }
    }

    pub fn pathname(&self) -> Option<String> {
        c_str(unsafe { ffi::archive_entry_pathname(self.raw) })
    }

    pub fn pathname_utf8(&self) -> Option<String> {
        c_str(unsafe { ffi::archive_entry_pathname_utf8(self.raw) })
    }

    pub fn set_mode(&mut self, value: mode_t) {
        unsafe {
            ffi::archive_entry_set_mode(self.raw, value);
        }
    }

    pub fn mode(&self) -> mode_t {
        unsafe { ffi::archive_entry_mode(self.raw) }
    }

    pub fn strmode(&self) -> Option<String> {
        c_str(unsafe { ffi::archive_entry_strmode(self.raw) })
    }

    pub fn set_size(&mut self, value: i64) {
        unsafe {
            ffi::archive_entry_set_size(self.raw, value);
        }
    }

    pub fn size(&self) -> i64 {
        unsafe { ffi::archive_entry_size(self.raw) }
    }

    pub fn set_uid(&mut self, value: i64) {
        unsafe {
            ffi::archive_entry_set_uid(self.raw, value);
        }
    }

    pub fn set_gid(&mut self, value: i64) {
        unsafe {
            ffi::archive_entry_set_gid(self.raw, value);
        }
    }

    pub fn uid(&self) -> i64 {
        unsafe { ffi::archive_entry_uid(self.raw) }
    }

    pub fn gid(&self) -> i64 {
        unsafe { ffi::archive_entry_gid(self.raw) }
    }

    pub fn set_uname(&mut self, value: &str) {
        let value = to_cstring(value);
        unsafe {
            ffi::archive_entry_set_uname(self.raw, value.as_ptr());
        }
    }

    pub fn set_gname(&mut self, value: &str) {
        let value = to_cstring(value);
        unsafe {
            ffi::archive_entry_set_gname(self.raw, value.as_ptr());
        }
    }

    pub fn uname(&self) -> Option<String> {
        c_str(unsafe { ffi::archive_entry_uname(self.raw) })
    }

    pub fn gname(&self) -> Option<String> {
        c_str(unsafe { ffi::archive_entry_gname(self.raw) })
    }

    pub fn set_hardlink(&mut self, value: Option<&str>) {
        let value = value.map(to_cstring);
        let ptr = value.as_ref().map_or(ptr::null(), |value| value.as_ptr());
        unsafe {
            ffi::archive_entry_set_hardlink(self.raw, ptr);
        }
    }

    pub fn hardlink(&self) -> Option<String> {
        c_str(unsafe { ffi::archive_entry_hardlink(self.raw) })
    }

    pub fn set_symlink(&mut self, value: Option<&str>) {
        let value = value.map(to_cstring);
        let ptr = value.as_ref().map_or(ptr::null(), |value| value.as_ptr());
        unsafe {
            ffi::archive_entry_set_symlink(self.raw, ptr);
        }
    }

    pub fn symlink(&self) -> Option<String> {
        c_str(unsafe { ffi::archive_entry_symlink(self.raw) })
    }

    pub fn set_mtime(&mut self, sec: i64, nsec: i64) {
        unsafe {
            ffi::archive_entry_set_mtime(self.raw, sec, nsec as _);
        }
    }

    pub fn mtime(&self) -> (i64, i64) {
        unsafe {
            (
                ffi::archive_entry_mtime(self.raw),
                ffi::archive_entry_mtime_nsec(self.raw) as i64,
            )
        }
    }

    pub fn set_atime(&mut self, sec: i64, nsec: i64) {
        unsafe {
            ffi::archive_entry_set_atime(self.raw, sec, nsec as _);
        }
    }

    pub fn atime(&self) -> (i64, i64) {
        unsafe {
            (
                ffi::archive_entry_atime(self.raw),
                ffi::archive_entry_atime_nsec(self.raw) as i64,
            )
        }
    }

    pub fn copy_stat(&mut self, st: &stat) {
        unsafe {
            ffi::archive_entry_copy_stat(self.raw, st as *const stat);
        }
    }

    pub fn stat(&self) -> &stat {
        unsafe {
            ffi::archive_entry_stat(self.raw)
                .as_ref()
                .expect("archive_entry_stat returned NULL")
        }
    }

    pub fn add_acl(
        &mut self,
        entry_type: i32,
        permset: i32,
        tag: i32,
        qual: i32,
        name: &str,
    ) -> i32 {
        let name = to_cstring(name);
        unsafe { ffi::archive_entry_acl_add_entry(self.raw, entry_type, permset, tag, qual, name.as_ptr()) }
    }

    pub fn acl_to_text(&self, flags: i32) -> Option<String> {
        let mut len = 0isize;
        let ptr = unsafe { ffi::archive_entry_acl_to_text(self.raw, &mut len, flags) };
        c_str(ptr)
    }

    pub fn add_xattr(&mut self, name: &str, value: &[u8]) {
        let name = to_cstring(name);
        unsafe {
            ffi::archive_entry_xattr_add_entry(
                self.raw,
                name.as_ptr(),
                value.as_ptr().cast::<c_void>(),
                value.len() as size_t,
            );
        }
    }

    pub fn xattrs(&self) -> Vec<(String, Vec<u8>)> {
        let mut result = Vec::new();
        unsafe {
            ffi::archive_entry_xattr_reset(self.raw);
            loop {
                let mut name = ptr::null();
                let mut value = ptr::null();
                let mut size = 0usize;
                if ffi::archive_entry_xattr_next(self.raw, &mut name, &mut value, &mut size) != 0 {
                    break;
                }
                let bytes = std::slice::from_raw_parts(value.cast::<u8>(), size).to_vec();
                result.push((c_str(name).expect("xattr name"), bytes));
            }
        }
        result
    }

    pub fn add_sparse(&mut self, offset: i64, length: i64) {
        unsafe {
            ffi::archive_entry_sparse_add_entry(self.raw, offset, length);
        }
    }

    pub fn sparse_entries(&self) -> Vec<(i64, i64)> {
        let mut result = Vec::new();
        unsafe {
            ffi::archive_entry_sparse_reset(self.raw);
            loop {
                let mut offset = 0i64;
                let mut length = 0i64;
                if ffi::archive_entry_sparse_next(self.raw, &mut offset, &mut length) != 0 {
                    break;
                }
                result.push((offset, length));
            }
        }
        result
    }
}

impl Drop for EntryHandle {
    fn drop(&mut self) {
        unsafe {
            ffi::archive_entry_free(self.raw);
        }
    }
}
