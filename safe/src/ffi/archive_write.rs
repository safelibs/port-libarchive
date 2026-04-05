use std::ffi::{c_char, c_int, c_void};

use libc::size_t;

use crate::common::state::{
    ArchiveCloseCallback, ArchiveFreeCallback, ArchiveOpenCallback, ArchiveWriteCallback,
};
use crate::ffi::{archive, archive_entry};

unsafe extern "C" {
    pub fn archive_write_new() -> *mut archive;
    pub fn archive_write_free(a: *mut archive) -> c_int;
    pub fn archive_write_close(a: *mut archive) -> c_int;

    pub fn archive_write_set_bytes_per_block(a: *mut archive, bytes: c_int) -> c_int;
    pub fn archive_write_get_bytes_per_block(a: *mut archive) -> c_int;
    pub fn archive_write_set_bytes_in_last_block(a: *mut archive, bytes: c_int) -> c_int;
    pub fn archive_write_get_bytes_in_last_block(a: *mut archive) -> c_int;
    pub fn archive_write_set_skip_file(a: *mut archive, dev: i64, ino: i64) -> c_int;

    pub fn archive_write_add_filter(a: *mut archive, filter_code: c_int) -> c_int;
    pub fn archive_write_add_filter_by_name(a: *mut archive, name: *const c_char) -> c_int;
    pub fn archive_write_add_filter_b64encode(a: *mut archive) -> c_int;
    pub fn archive_write_add_filter_bzip2(a: *mut archive) -> c_int;
    pub fn archive_write_add_filter_compress(a: *mut archive) -> c_int;
    pub fn archive_write_add_filter_grzip(a: *mut archive) -> c_int;
    pub fn archive_write_add_filter_gzip(a: *mut archive) -> c_int;
    pub fn archive_write_add_filter_lrzip(a: *mut archive) -> c_int;
    pub fn archive_write_add_filter_lz4(a: *mut archive) -> c_int;
    pub fn archive_write_add_filter_lzip(a: *mut archive) -> c_int;
    pub fn archive_write_add_filter_lzma(a: *mut archive) -> c_int;
    pub fn archive_write_add_filter_lzop(a: *mut archive) -> c_int;
    pub fn archive_write_add_filter_none(a: *mut archive) -> c_int;
    pub fn archive_write_add_filter_program(a: *mut archive, command: *const c_char) -> c_int;
    pub fn archive_write_add_filter_uuencode(a: *mut archive) -> c_int;
    pub fn archive_write_add_filter_xz(a: *mut archive) -> c_int;
    pub fn archive_write_add_filter_zstd(a: *mut archive) -> c_int;

    pub fn archive_write_set_format(a: *mut archive, format_code: c_int) -> c_int;
    pub fn archive_write_set_format_by_name(a: *mut archive, name: *const c_char) -> c_int;
    pub fn archive_write_set_format_ar_bsd(a: *mut archive) -> c_int;
    pub fn archive_write_set_format_ar_svr4(a: *mut archive) -> c_int;
    pub fn archive_write_set_format_cpio(a: *mut archive) -> c_int;
    pub fn archive_write_set_format_cpio_bin(a: *mut archive) -> c_int;
    pub fn archive_write_set_format_cpio_newc(a: *mut archive) -> c_int;
    pub fn archive_write_set_format_cpio_odc(a: *mut archive) -> c_int;
    pub fn archive_write_set_format_cpio_pwb(a: *mut archive) -> c_int;
    pub fn archive_write_set_format_gnutar(a: *mut archive) -> c_int;
    pub fn archive_write_set_format_pax(a: *mut archive) -> c_int;
    pub fn archive_write_set_format_pax_restricted(a: *mut archive) -> c_int;
    pub fn archive_write_set_format_raw(a: *mut archive) -> c_int;
    pub fn archive_write_set_format_shar(a: *mut archive) -> c_int;
    pub fn archive_write_set_format_shar_dump(a: *mut archive) -> c_int;
    pub fn archive_write_set_format_ustar(a: *mut archive) -> c_int;
    pub fn archive_write_set_format_v7tar(a: *mut archive) -> c_int;
    pub fn archive_write_set_format_filter_by_ext(
        a: *mut archive,
        filename: *const c_char,
    ) -> c_int;
    pub fn archive_write_set_format_filter_by_ext_def(
        a: *mut archive,
        filename: *const c_char,
        default_ext: *const c_char,
    ) -> c_int;

    pub fn archive_write_open(
        a: *mut archive,
        client_data: *mut c_void,
        open_cb: Option<ArchiveOpenCallback>,
        write_cb: Option<ArchiveWriteCallback>,
        close_cb: Option<ArchiveCloseCallback>,
    ) -> c_int;
    pub fn archive_write_open2(
        a: *mut archive,
        client_data: *mut c_void,
        open_cb: Option<ArchiveOpenCallback>,
        write_cb: Option<ArchiveWriteCallback>,
        close_cb: Option<ArchiveCloseCallback>,
        free_cb: Option<ArchiveFreeCallback>,
    ) -> c_int;
    pub fn archive_write_open_filename(a: *mut archive, path: *const c_char) -> c_int;
    pub fn archive_write_open_memory(
        a: *mut archive,
        buffer: *mut c_void,
        size: size_t,
        used: *mut size_t,
    ) -> c_int;

    pub fn archive_write_header(a: *mut archive, entry: *mut archive_entry) -> c_int;
    pub fn archive_write_data(a: *mut archive, buff: *const c_void, size: size_t) -> isize;
    pub fn archive_write_data_block(
        a: *mut archive,
        buff: *const c_void,
        size: size_t,
        offset: i64,
    ) -> isize;
    pub fn archive_write_finish_entry(a: *mut archive) -> c_int;
    pub fn archive_write_fail(a: *mut archive) -> c_int;

    pub fn archive_write_set_filter_option(
        a: *mut archive,
        module: *const c_char,
        option: *const c_char,
        value: *const c_char,
    ) -> c_int;
    pub fn archive_write_set_format_option(
        a: *mut archive,
        module: *const c_char,
        option: *const c_char,
        value: *const c_char,
    ) -> c_int;
    pub fn archive_write_set_option(
        a: *mut archive,
        module: *const c_char,
        option: *const c_char,
        value: *const c_char,
    ) -> c_int;
    pub fn archive_write_set_options(a: *mut archive, options: *const c_char) -> c_int;
    pub fn archive_write_set_passphrase(a: *mut archive, passphrase: *const c_char) -> c_int;
}
