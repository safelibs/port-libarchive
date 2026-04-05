use std::ffi::{c_char, c_int, c_void};

use libc::{size_t, wchar_t};

use crate::common::state::{
    ArchiveCloseCallback, ArchiveOpenCallback, ArchivePassphraseCallback, ArchiveReadCallback,
    ArchiveSeekCallback, ArchiveSkipCallback, ArchiveSwitchCallback,
};
use crate::ffi::{archive, archive_entry};

unsafe extern "C" {
    pub fn archive_read_new() -> *mut archive;
    pub fn archive_read_free(a: *mut archive) -> c_int;
    pub fn archive_read_close(a: *mut archive) -> c_int;

    pub fn archive_read_support_compression_all(a: *mut archive) -> c_int;
    pub fn archive_read_support_compression_bzip2(a: *mut archive) -> c_int;
    pub fn archive_read_support_compression_compress(a: *mut archive) -> c_int;
    pub fn archive_read_support_compression_gzip(a: *mut archive) -> c_int;
    pub fn archive_read_support_compression_lzip(a: *mut archive) -> c_int;
    pub fn archive_read_support_compression_lzma(a: *mut archive) -> c_int;
    pub fn archive_read_support_compression_none(a: *mut archive) -> c_int;
    pub fn archive_read_support_compression_program(
        a: *mut archive,
        command: *const c_char,
    ) -> c_int;
    pub fn archive_read_support_compression_program_signature(
        a: *mut archive,
        command: *const c_char,
        signature: *const c_void,
        signature_len: size_t,
    ) -> c_int;
    pub fn archive_read_support_compression_rpm(a: *mut archive) -> c_int;
    pub fn archive_read_support_compression_uu(a: *mut archive) -> c_int;
    pub fn archive_read_support_compression_xz(a: *mut archive) -> c_int;

    pub fn archive_read_support_filter_all(a: *mut archive) -> c_int;
    pub fn archive_read_support_filter_by_code(a: *mut archive, filter_code: c_int) -> c_int;
    pub fn archive_read_support_filter_bzip2(a: *mut archive) -> c_int;
    pub fn archive_read_support_filter_compress(a: *mut archive) -> c_int;
    pub fn archive_read_support_filter_gzip(a: *mut archive) -> c_int;
    pub fn archive_read_support_filter_grzip(a: *mut archive) -> c_int;
    pub fn archive_read_support_filter_lrzip(a: *mut archive) -> c_int;
    pub fn archive_read_support_filter_lz4(a: *mut archive) -> c_int;
    pub fn archive_read_support_filter_lzip(a: *mut archive) -> c_int;
    pub fn archive_read_support_filter_lzma(a: *mut archive) -> c_int;
    pub fn archive_read_support_filter_lzop(a: *mut archive) -> c_int;
    pub fn archive_read_support_filter_none(a: *mut archive) -> c_int;
    pub fn archive_read_support_filter_program(
        a: *mut archive,
        command: *const c_char,
    ) -> c_int;
    pub fn archive_read_support_filter_program_signature(
        a: *mut archive,
        command: *const c_char,
        signature: *const c_void,
        signature_len: size_t,
    ) -> c_int;
    pub fn archive_read_support_filter_rpm(a: *mut archive) -> c_int;
    pub fn archive_read_support_filter_uu(a: *mut archive) -> c_int;
    pub fn archive_read_support_filter_xz(a: *mut archive) -> c_int;
    pub fn archive_read_support_filter_zstd(a: *mut archive) -> c_int;

    pub fn archive_read_support_format_7zip(a: *mut archive) -> c_int;
    pub fn archive_read_support_format_all(a: *mut archive) -> c_int;
    pub fn archive_read_support_format_ar(a: *mut archive) -> c_int;
    pub fn archive_read_support_format_by_code(a: *mut archive, format_code: c_int) -> c_int;
    pub fn archive_read_support_format_cab(a: *mut archive) -> c_int;
    pub fn archive_read_support_format_cpio(a: *mut archive) -> c_int;
    pub fn archive_read_support_format_empty(a: *mut archive) -> c_int;
    pub fn archive_read_support_format_gnutar(a: *mut archive) -> c_int;
    pub fn archive_read_support_format_iso9660(a: *mut archive) -> c_int;
    pub fn archive_read_support_format_lha(a: *mut archive) -> c_int;
    pub fn archive_read_support_format_mtree(a: *mut archive) -> c_int;
    pub fn archive_read_support_format_rar(a: *mut archive) -> c_int;
    pub fn archive_read_support_format_rar5(a: *mut archive) -> c_int;
    pub fn archive_read_support_format_raw(a: *mut archive) -> c_int;
    pub fn archive_read_support_format_tar(a: *mut archive) -> c_int;
    pub fn archive_read_support_format_warc(a: *mut archive) -> c_int;
    pub fn archive_read_support_format_xar(a: *mut archive) -> c_int;
    pub fn archive_read_support_format_zip(a: *mut archive) -> c_int;
    pub fn archive_read_support_format_zip_streamable(a: *mut archive) -> c_int;
    pub fn archive_read_support_format_zip_seekable(a: *mut archive) -> c_int;

    pub fn archive_read_set_format(a: *mut archive, format_code: c_int) -> c_int;
    pub fn archive_read_append_filter(a: *mut archive, filter_code: c_int) -> c_int;
    pub fn archive_read_append_filter_program(
        a: *mut archive,
        command: *const c_char,
    ) -> c_int;
    pub fn archive_read_append_filter_program_signature(
        a: *mut archive,
        command: *const c_char,
        signature: *const c_void,
        signature_len: size_t,
    ) -> c_int;

    pub fn archive_read_set_open_callback(
        a: *mut archive,
        callback: Option<ArchiveOpenCallback>,
    ) -> c_int;
    pub fn archive_read_set_read_callback(
        a: *mut archive,
        callback: Option<ArchiveReadCallback>,
    ) -> c_int;
    pub fn archive_read_set_seek_callback(
        a: *mut archive,
        callback: Option<ArchiveSeekCallback>,
    ) -> c_int;
    pub fn archive_read_set_skip_callback(
        a: *mut archive,
        callback: Option<ArchiveSkipCallback>,
    ) -> c_int;
    pub fn archive_read_set_close_callback(
        a: *mut archive,
        callback: Option<ArchiveCloseCallback>,
    ) -> c_int;
    pub fn archive_read_set_switch_callback(
        a: *mut archive,
        callback: Option<ArchiveSwitchCallback>,
    ) -> c_int;
    pub fn archive_read_set_callback_data(a: *mut archive, client_data: *mut c_void) -> c_int;
    pub fn archive_read_set_callback_data2(
        a: *mut archive,
        client_data: *mut c_void,
        index: u32,
    ) -> c_int;
    pub fn archive_read_add_callback_data(
        a: *mut archive,
        client_data: *mut c_void,
        index: u32,
    ) -> c_int;
    pub fn archive_read_append_callback_data(
        a: *mut archive,
        client_data: *mut c_void,
    ) -> c_int;
    pub fn archive_read_prepend_callback_data(
        a: *mut archive,
        client_data: *mut c_void,
    ) -> c_int;

    pub fn archive_read_open1(a: *mut archive) -> c_int;
    pub fn archive_read_open(
        a: *mut archive,
        client_data: *mut c_void,
        open_cb: Option<ArchiveOpenCallback>,
        read_cb: Option<ArchiveReadCallback>,
        close_cb: Option<ArchiveCloseCallback>,
    ) -> c_int;
    pub fn archive_read_open2(
        a: *mut archive,
        client_data: *mut c_void,
        open_cb: Option<ArchiveOpenCallback>,
        read_cb: Option<ArchiveReadCallback>,
        skip_cb: Option<ArchiveSkipCallback>,
        close_cb: Option<ArchiveCloseCallback>,
    ) -> c_int;
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
    pub fn archive_read_open_file(
        a: *mut archive,
        path: *const c_char,
        block_size: size_t,
    ) -> c_int;
    pub fn archive_read_open_memory(
        a: *mut archive,
        buffer: *const c_void,
        size: size_t,
    ) -> c_int;
    pub fn archive_read_open_memory2(
        a: *mut archive,
        buffer: *const c_void,
        size: size_t,
        read_size: size_t,
    ) -> c_int;
    pub fn archive_read_open_fd(a: *mut archive, fd: c_int, block_size: size_t) -> c_int;
    pub fn archive_read_open_FILE(a: *mut archive, file: *mut libc::FILE) -> c_int;

    pub fn archive_read_next_header(a: *mut archive, entry: *mut *mut archive_entry) -> c_int;
    pub fn archive_read_next_header2(a: *mut archive, entry: *mut archive_entry) -> c_int;
    pub fn archive_read_header_position(a: *mut archive) -> i64;
    pub fn archive_read_has_encrypted_entries(a: *mut archive) -> c_int;
    pub fn archive_read_format_capabilities(a: *mut archive) -> c_int;
    pub fn archive_read_data(a: *mut archive, buffer: *mut c_void, size: size_t) -> isize;
    pub fn archive_read_data_block(
        a: *mut archive,
        buffer: *mut *const c_void,
        size: *mut size_t,
        offset: *mut i64,
    ) -> c_int;
    pub fn archive_read_data_skip(a: *mut archive) -> c_int;
    pub fn archive_read_data_into_fd(a: *mut archive, fd: c_int) -> c_int;
    pub fn archive_seek_data(a: *mut archive, offset: i64, whence: c_int) -> i64;

    pub fn archive_read_set_format_option(
        a: *mut archive,
        module: *const c_char,
        option: *const c_char,
        value: *const c_char,
    ) -> c_int;
    pub fn archive_read_set_filter_option(
        a: *mut archive,
        module: *const c_char,
        option: *const c_char,
        value: *const c_char,
    ) -> c_int;
    pub fn archive_read_set_option(
        a: *mut archive,
        module: *const c_char,
        option: *const c_char,
        value: *const c_char,
    ) -> c_int;
    pub fn archive_read_set_options(a: *mut archive, options: *const c_char) -> c_int;

    pub fn archive_read_add_passphrase(a: *mut archive, passphrase: *const c_char) -> c_int;
    pub fn archive_read_set_passphrase_callback(
        a: *mut archive,
        client_data: *mut c_void,
        callback: Option<ArchivePassphraseCallback>,
    ) -> c_int;

    pub fn archive_read_extract(a: *mut archive, entry: *mut archive_entry, flags: c_int) -> c_int;
    pub fn archive_read_extract2(
        a: *mut archive,
        entry: *mut archive_entry,
        disk: *mut archive,
    ) -> c_int;
}
