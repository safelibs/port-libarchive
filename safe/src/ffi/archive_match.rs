use std::ffi::c_char;
use std::os::raw::{c_int, c_long};

use libc::wchar_t;

use crate::ffi::{archive, archive_entry};

unsafe extern "C" {
    pub fn archive_match_new() -> *mut archive;
    pub fn archive_match_free(a: *mut archive) -> c_int;

    pub fn archive_match_excluded(a: *mut archive, entry: *mut archive_entry) -> c_int;
    pub fn archive_match_path_excluded(a: *mut archive, entry: *mut archive_entry) -> c_int;
    pub fn archive_match_set_inclusion_recursion(a: *mut archive, enabled: c_int) -> c_int;

    pub fn archive_match_exclude_pattern(a: *mut archive, pattern: *const c_char) -> c_int;
    pub fn archive_match_exclude_pattern_w(a: *mut archive, pattern: *const wchar_t) -> c_int;
    pub fn archive_match_exclude_pattern_from_file(
        a: *mut archive,
        path: *const c_char,
        null_separator: c_int,
    ) -> c_int;
    pub fn archive_match_exclude_pattern_from_file_w(
        a: *mut archive,
        path: *const wchar_t,
        null_separator: c_int,
    ) -> c_int;

    pub fn archive_match_include_pattern(a: *mut archive, pattern: *const c_char) -> c_int;
    pub fn archive_match_include_pattern_w(a: *mut archive, pattern: *const wchar_t) -> c_int;
    pub fn archive_match_include_pattern_from_file(
        a: *mut archive,
        path: *const c_char,
        null_separator: c_int,
    ) -> c_int;
    pub fn archive_match_include_pattern_from_file_w(
        a: *mut archive,
        path: *const wchar_t,
        null_separator: c_int,
    ) -> c_int;

    pub fn archive_match_path_unmatched_inclusions(a: *mut archive) -> c_int;
    pub fn archive_match_path_unmatched_inclusions_next(
        a: *mut archive,
        unmatched: *mut *const c_char,
    ) -> c_int;
    pub fn archive_match_path_unmatched_inclusions_next_w(
        a: *mut archive,
        unmatched: *mut *const wchar_t,
    ) -> c_int;

    pub fn archive_match_time_excluded(a: *mut archive, entry: *mut archive_entry) -> c_int;
    pub fn archive_match_include_time(
        a: *mut archive,
        flag: c_int,
        sec: i64,
        nsec: c_long,
    ) -> c_int;
    pub fn archive_match_include_date(a: *mut archive, flag: c_int, date: *const c_char) -> c_int;
    pub fn archive_match_include_date_w(
        a: *mut archive,
        flag: c_int,
        date: *const wchar_t,
    ) -> c_int;
    pub fn archive_match_include_file_time(
        a: *mut archive,
        flag: c_int,
        path: *const c_char,
    ) -> c_int;
    pub fn archive_match_include_file_time_w(
        a: *mut archive,
        flag: c_int,
        path: *const wchar_t,
    ) -> c_int;
    pub fn archive_match_exclude_entry(
        a: *mut archive,
        flag: c_int,
        entry: *mut archive_entry,
    ) -> c_int;

    pub fn archive_match_owner_excluded(a: *mut archive, entry: *mut archive_entry) -> c_int;
    pub fn archive_match_include_uid(a: *mut archive, uid: i64) -> c_int;
    pub fn archive_match_include_gid(a: *mut archive, gid: i64) -> c_int;
    pub fn archive_match_include_uname(a: *mut archive, name: *const c_char) -> c_int;
    pub fn archive_match_include_uname_w(a: *mut archive, name: *const wchar_t) -> c_int;
    pub fn archive_match_include_gname(a: *mut archive, name: *const c_char) -> c_int;
    pub fn archive_match_include_gname_w(a: *mut archive, name: *const wchar_t) -> c_int;
}
