pub(crate) mod api;
pub(crate) mod internal;

use std::ffi::CString;
use std::ptr;

use crate::entry::{to_wide_null, EntryHandle};
use crate::ffi::archive;
use crate::ffi::archive_match_api as ffi;

fn to_cstring(value: &str) -> CString {
    CString::new(value).expect("input must not contain interior NUL bytes")
}

pub struct MatchHandle {
    raw: *mut archive,
}

impl MatchHandle {
    pub fn new() -> Self {
        let raw = unsafe { ffi::archive_match_new() };
        assert!(!raw.is_null(), "archive_match_new returned NULL");
        Self { raw }
    }

    pub fn as_ptr(&self) -> *mut archive {
        self.raw
    }

    pub fn set_inclusion_recursion(&mut self, enabled: bool) -> i32 {
        unsafe { ffi::archive_match_set_inclusion_recursion(self.raw, i32::from(enabled)) }
    }

    pub fn include_pattern(&mut self, pattern: &str) -> i32 {
        let pattern = to_cstring(pattern);
        unsafe { ffi::archive_match_include_pattern(self.raw, pattern.as_ptr()) }
    }

    pub fn include_pattern_w(&mut self, pattern: &str) -> i32 {
        let pattern = to_wide_null(pattern);
        unsafe { ffi::archive_match_include_pattern_w(self.raw, pattern.as_ptr()) }
    }

    pub fn exclude_pattern(&mut self, pattern: &str) -> i32 {
        let pattern = to_cstring(pattern);
        unsafe { ffi::archive_match_exclude_pattern(self.raw, pattern.as_ptr()) }
    }

    pub fn include_uid(&mut self, value: i64) -> i32 {
        unsafe { ffi::archive_match_include_uid(self.raw, value) }
    }

    pub fn include_gid(&mut self, value: i64) -> i32 {
        unsafe { ffi::archive_match_include_gid(self.raw, value) }
    }

    pub fn include_uname(&mut self, value: &str) -> i32 {
        let value = to_cstring(value);
        unsafe { ffi::archive_match_include_uname(self.raw, value.as_ptr()) }
    }

    pub fn include_gname(&mut self, value: &str) -> i32 {
        let value = to_cstring(value);
        unsafe { ffi::archive_match_include_gname(self.raw, value.as_ptr()) }
    }

    pub fn include_date(&mut self, flag: i32, value: &str) -> i32 {
        let value = to_cstring(value);
        unsafe { ffi::archive_match_include_date(self.raw, flag, value.as_ptr()) }
    }

    pub fn include_time(&mut self, flag: i32, sec: i64, nsec: i64) -> i32 {
        unsafe { ffi::archive_match_include_time(self.raw, flag, sec, nsec as _) }
    }

    pub fn excluded(&self, entry: &EntryHandle) -> i32 {
        unsafe { ffi::archive_match_excluded(self.raw, entry.as_ptr()) }
    }

    pub fn path_excluded(&self, entry: &EntryHandle) -> i32 {
        unsafe { ffi::archive_match_path_excluded(self.raw, entry.as_ptr()) }
    }

    pub fn owner_excluded(&self, entry: &EntryHandle) -> i32 {
        unsafe { ffi::archive_match_owner_excluded(self.raw, entry.as_ptr()) }
    }

    pub fn time_excluded(&self, entry: &EntryHandle) -> i32 {
        unsafe { ffi::archive_match_time_excluded(self.raw, entry.as_ptr()) }
    }

    pub fn unmatched_inclusions(&self) -> i32 {
        unsafe { ffi::archive_match_path_unmatched_inclusions(self.raw) }
    }

    pub fn unmatched_inclusions_next(&self) -> Option<String> {
        let mut unmatched = ptr::null();
        let status = unsafe { ffi::archive_match_path_unmatched_inclusions_next(self.raw, &mut unmatched) };
        if status == crate::common::error::ARCHIVE_OK {
            crate::entry::c_str(unmatched)
        } else {
            None
        }
    }
}

impl Drop for MatchHandle {
    fn drop(&mut self) {
        unsafe {
            ffi::archive_match_free(self.raw);
        }
    }
}
