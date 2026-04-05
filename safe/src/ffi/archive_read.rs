use std::ffi::{c_char, c_int, c_void};

use libc::{size_t, wchar_t};

use crate::ffi::{archive, archive_entry};

unsafe extern "C" {
    pub fn archive_read_new() -> *mut archive;
    pub fn archive_read_free(a: *mut archive) -> c_int;
    pub fn archive_read_close(a: *mut archive) -> c_int;

    pub fn archive_read_support_filter_all(a: *mut archive) -> c_int;
    pub fn archive_read_support_filter_none(a: *mut archive) -> c_int;
    pub fn archive_read_support_filter_bzip2(a: *mut archive) -> c_int;
    pub fn archive_read_support_filter_compress(a: *mut archive) -> c_int;
    pub fn archive_read_support_filter_gzip(a: *mut archive) -> c_int;
    pub fn archive_read_support_filter_grzip(a: *mut archive) -> c_int;
    pub fn archive_read_support_filter_lrzip(a: *mut archive) -> c_int;
    pub fn archive_read_support_filter_lz4(a: *mut archive) -> c_int;
    pub fn archive_read_support_filter_lzip(a: *mut archive) -> c_int;
    pub fn archive_read_support_filter_lzma(a: *mut archive) -> c_int;
    pub fn archive_read_support_filter_lzop(a: *mut archive) -> c_int;
    pub fn archive_read_support_filter_xz(a: *mut archive) -> c_int;
    pub fn archive_read_support_filter_zstd(a: *mut archive) -> c_int;

    pub fn archive_read_support_format_all(a: *mut archive) -> c_int;
    pub fn archive_read_support_format_empty(a: *mut archive) -> c_int;
    pub fn archive_read_support_format_raw(a: *mut archive) -> c_int;

    pub fn archive_read_open_memory(a: *mut archive, buff: *const c_void, size: size_t) -> c_int;
    pub fn archive_read_open_filename(
        a: *mut archive,
        path: *const c_char,
        block_size: size_t,
    ) -> c_int;
    pub fn archive_read_open_filenames(
        a: *mut archive,
        paths: *const *const c_char,
        block_size: size_t,
    ) -> c_int;
    pub fn archive_read_open_filename_w(
        a: *mut archive,
        path: *const wchar_t,
        block_size: size_t,
    ) -> c_int;

    pub fn archive_read_next_header(a: *mut archive, entry: *mut *mut archive_entry) -> c_int;
    pub fn archive_read_next_header2(a: *mut archive, entry: *mut archive_entry) -> c_int;
    pub fn archive_read_data(a: *mut archive, buff: *mut c_void, size: size_t) -> isize;
    pub fn archive_read_data_block(
        a: *mut archive,
        buff: *mut *const c_void,
        size: *mut size_t,
        offset: *mut i64,
    ) -> c_int;
    pub fn archive_read_extract(a: *mut archive, entry: *mut archive_entry, flags: c_int) -> c_int;
    pub fn archive_read_extract2(
        a: *mut archive,
        entry: *mut archive_entry,
        disk: *mut archive,
    ) -> c_int;
}
