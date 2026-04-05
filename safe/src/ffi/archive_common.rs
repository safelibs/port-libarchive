use std::ffi::c_char;
use std::os::raw::{c_int, c_longlong};

use crate::ffi::archive;
use crate::generated::LIBARCHIVE_VERSION_NUMBER;

pub const ARCHIVE_VERSION_NUMBER: c_int = LIBARCHIVE_VERSION_NUMBER;

pub const ARCHIVE_FORMAT_CPIO: c_int = 0x10000;
pub const ARCHIVE_FORMAT_CPIO_POSIX: c_int = ARCHIVE_FORMAT_CPIO | 1;
pub const ARCHIVE_FORMAT_CPIO_SVR4_NOCRC: c_int = ARCHIVE_FORMAT_CPIO | 4;
pub const ARCHIVE_FORMAT_TAR: c_int = 0x30000;
pub const ARCHIVE_FORMAT_TAR_USTAR: c_int = ARCHIVE_FORMAT_TAR | 1;

pub const ARCHIVE_MATCH_NEWER: c_int = 0x0001;
pub const ARCHIVE_MATCH_OLDER: c_int = 0x0002;
pub const ARCHIVE_MATCH_EQUAL: c_int = 0x0010;
pub const ARCHIVE_MATCH_MTIME: c_int = 0x0100;
pub const ARCHIVE_MATCH_CTIME: c_int = 0x0200;

unsafe extern "C" {
    pub fn archive_bzlib_version() -> *const c_char;
    pub fn archive_clear_error(a: *mut archive);
    pub fn archive_compression(a: *mut archive) -> c_int;
    pub fn archive_compression_name(a: *mut archive) -> *const c_char;
    pub fn archive_copy_error(dest: *mut archive, src: *mut archive);
    pub fn archive_errno(a: *mut archive) -> c_int;
    pub fn archive_error_string(a: *mut archive) -> *const c_char;
    pub fn archive_file_count(a: *mut archive) -> c_int;
    pub fn archive_filter_bytes(a: *mut archive, n: c_int) -> c_longlong;
    pub fn archive_filter_code(a: *mut archive, n: c_int) -> c_int;
    pub fn archive_filter_count(a: *mut archive) -> c_int;
    pub fn archive_filter_name(a: *mut archive, n: c_int) -> *const c_char;
    pub fn archive_format(a: *mut archive) -> c_int;
    pub fn archive_format_name(a: *mut archive) -> *const c_char;
    pub fn archive_free(a: *mut archive) -> c_int;
    pub fn archive_liblz4_version() -> *const c_char;
    pub fn archive_liblzma_version() -> *const c_char;
    pub fn archive_libzstd_version() -> *const c_char;
    pub fn archive_position_compressed(a: *mut archive) -> c_longlong;
    pub fn archive_position_uncompressed(a: *mut archive) -> c_longlong;
    pub fn archive_read_disk_new() -> *mut archive;
    pub fn archive_read_finish(a: *mut archive) -> c_int;
    pub fn archive_read_free(a: *mut archive) -> c_int;
    pub fn archive_read_new() -> *mut archive;
    pub fn archive_read_open_filename(
        a: *mut archive,
        path: *const c_char,
        block_size: usize,
    ) -> c_int;
    pub fn archive_read_open_filenames(
        a: *mut archive,
        paths: *const *const c_char,
        block_size: usize,
    ) -> c_int;
    pub fn archive_set_error(a: *mut archive, error_number: c_int, fmt: *const c_char, ...);
    pub fn archive_utility_string_sort(strings: *mut *mut c_char) -> c_int;
    pub fn archive_version_details() -> *const c_char;
    pub fn archive_version_number() -> c_int;
    pub fn archive_version_string() -> *const c_char;
    pub fn archive_write_disk_new() -> *mut archive;
    pub fn archive_write_finish(a: *mut archive) -> c_int;
    pub fn archive_write_free(a: *mut archive) -> c_int;
    pub fn archive_write_new() -> *mut archive;
    pub fn archive_zlib_version() -> *const c_char;
    pub fn archive_write_close(a: *mut archive) -> c_int;
    pub fn archive_read_close(a: *mut archive) -> c_int;
    pub fn archive_read_open_filename_w(
        a: *mut archive,
        path: *const libc::wchar_t,
        block_size: usize,
    ) -> c_int;
}
