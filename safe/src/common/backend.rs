use std::ffi::{c_char, c_int, c_long, c_uchar, c_uint, c_ulong, c_void};

use libc::{dev_t, mode_t, size_t, wchar_t};

#[repr(C)]
pub struct BackendArchive {
    _private: [u8; 0],
}

#[repr(C)]
pub struct BackendEntry {
    _private: [u8; 0],
}

pub type LaInt64 = i64;
pub type LaSsize = isize;

pub type BackendOpenCallback =
    Option<unsafe extern "C" fn(*mut BackendArchive, *mut c_void) -> c_int>;
pub type BackendReadCallback =
    Option<unsafe extern "C" fn(*mut BackendArchive, *mut c_void, *mut *const c_void) -> LaSsize>;
pub type BackendSkipCallback =
    Option<unsafe extern "C" fn(*mut BackendArchive, *mut c_void, LaInt64) -> LaInt64>;
pub type BackendSeekCallback =
    Option<unsafe extern "C" fn(*mut BackendArchive, *mut c_void, LaInt64, c_int) -> LaInt64>;
pub type BackendSwitchCallback =
    Option<unsafe extern "C" fn(*mut BackendArchive, *mut c_void, *mut c_void) -> c_int>;
pub type BackendPassphraseCallback =
    Option<unsafe extern "C" fn(*mut BackendArchive, *mut c_void) -> *const c_char>;
pub type BackendWriteCallback = Option<
    unsafe extern "C" fn(*mut BackendArchive, *mut c_void, *const c_void, size_t) -> LaSsize,
>;
pub type BackendCloseCallback =
    Option<unsafe extern "C" fn(*mut BackendArchive, *mut c_void) -> c_int>;
pub type BackendFreeCallback =
    Option<unsafe extern "C" fn(*mut BackendArchive, *mut c_void) -> c_int>;

pub type BackendReadDiskLookupCallback =
    Option<unsafe extern "C" fn(*mut c_void, LaInt64) -> *const c_char>;
pub type BackendReadDiskCleanupCallback = Option<unsafe extern "C" fn(*mut c_void)>;

pub type BackendWriteDiskLookupCallback =
    Option<unsafe extern "C" fn(*mut c_void, *const c_char, LaInt64) -> LaInt64>;
pub type BackendWriteDiskCleanupCallback = Option<unsafe extern "C" fn(*mut c_void)>;

pub struct Api {
    pub(crate) _library: *mut c_void,
    pub archive_clear_error: unsafe extern "C" fn(*mut BackendArchive),
    pub archive_errno: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_error_string: unsafe extern "C" fn(*mut BackendArchive) -> *const c_char,
    pub archive_file_count: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_filter_bytes: unsafe extern "C" fn(*mut BackendArchive, c_int) -> LaInt64,
    pub archive_filter_code: unsafe extern "C" fn(*mut BackendArchive, c_int) -> c_int,
    pub archive_filter_count: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_filter_name: unsafe extern "C" fn(*mut BackendArchive, c_int) -> *const c_char,
    pub archive_format: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_format_name: unsafe extern "C" fn(*mut BackendArchive) -> *const c_char,
    pub archive_position_compressed: unsafe extern "C" fn(*mut BackendArchive) -> LaInt64,
    pub archive_position_uncompressed: unsafe extern "C" fn(*mut BackendArchive) -> LaInt64,
    pub archive_version_details: unsafe extern "C" fn() -> *const c_char,
    pub archive_bzlib_version: unsafe extern "C" fn() -> *const c_char,
    pub archive_liblz4_version: unsafe extern "C" fn() -> *const c_char,
    pub archive_liblzma_version: unsafe extern "C" fn() -> *const c_char,
    pub archive_libzstd_version: unsafe extern "C" fn() -> *const c_char,
    pub archive_zlib_version: unsafe extern "C" fn() -> *const c_char,

    pub archive_match_new: unsafe extern "C" fn() -> *mut BackendArchive,
    pub archive_match_free: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_match_set_inclusion_recursion:
        unsafe extern "C" fn(*mut BackendArchive, c_int) -> c_int,
    pub archive_match_exclude_pattern:
        unsafe extern "C" fn(*mut BackendArchive, *const c_char) -> c_int,
    pub archive_match_include_pattern:
        unsafe extern "C" fn(*mut BackendArchive, *const c_char) -> c_int,
    pub archive_match_include_time:
        unsafe extern "C" fn(*mut BackendArchive, c_int, LaInt64, c_long) -> c_int,
    pub archive_match_include_uid: unsafe extern "C" fn(*mut BackendArchive, LaInt64) -> c_int,
    pub archive_match_include_gid: unsafe extern "C" fn(*mut BackendArchive, LaInt64) -> c_int,
    pub archive_match_include_uname:
        unsafe extern "C" fn(*mut BackendArchive, *const c_char) -> c_int,
    pub archive_match_include_gname:
        unsafe extern "C" fn(*mut BackendArchive, *const c_char) -> c_int,

    pub archive_entry_new: unsafe extern "C" fn() -> *mut BackendEntry,
    pub archive_entry_free: unsafe extern "C" fn(*mut BackendEntry),
    pub archive_entry_clear: unsafe extern "C" fn(*mut BackendEntry) -> *mut BackendEntry,
    pub archive_entry_pathname: unsafe extern "C" fn(*mut BackendEntry) -> *const c_char,
    pub archive_entry_sourcepath: unsafe extern "C" fn(*mut BackendEntry) -> *const c_char,
    pub archive_entry_mode: unsafe extern "C" fn(*mut BackendEntry) -> mode_t,
    pub archive_entry_size: unsafe extern "C" fn(*mut BackendEntry) -> LaInt64,
    pub archive_entry_size_is_set: unsafe extern "C" fn(*mut BackendEntry) -> c_int,
    pub archive_entry_uid: unsafe extern "C" fn(*mut BackendEntry) -> LaInt64,
    pub archive_entry_gid: unsafe extern "C" fn(*mut BackendEntry) -> LaInt64,
    pub archive_entry_uname: unsafe extern "C" fn(*mut BackendEntry) -> *const c_char,
    pub archive_entry_gname: unsafe extern "C" fn(*mut BackendEntry) -> *const c_char,
    pub archive_entry_hardlink: unsafe extern "C" fn(*mut BackendEntry) -> *const c_char,
    pub archive_entry_symlink: unsafe extern "C" fn(*mut BackendEntry) -> *const c_char,
    pub archive_entry_symlink_type: unsafe extern "C" fn(*mut BackendEntry) -> c_int,
    pub archive_entry_nlink: unsafe extern "C" fn(*mut BackendEntry) -> c_uint,
    pub archive_entry_ino: unsafe extern "C" fn(*mut BackendEntry) -> LaInt64,
    pub archive_entry_dev: unsafe extern "C" fn(*mut BackendEntry) -> dev_t,
    pub archive_entry_rdev: unsafe extern "C" fn(*mut BackendEntry) -> dev_t,
    pub archive_entry_atime: unsafe extern "C" fn(*mut BackendEntry) -> LaInt64,
    pub archive_entry_atime_nsec: unsafe extern "C" fn(*mut BackendEntry) -> c_long,
    pub archive_entry_atime_is_set: unsafe extern "C" fn(*mut BackendEntry) -> c_int,
    pub archive_entry_birthtime: unsafe extern "C" fn(*mut BackendEntry) -> LaInt64,
    pub archive_entry_birthtime_nsec: unsafe extern "C" fn(*mut BackendEntry) -> c_long,
    pub archive_entry_birthtime_is_set: unsafe extern "C" fn(*mut BackendEntry) -> c_int,
    pub archive_entry_ctime: unsafe extern "C" fn(*mut BackendEntry) -> LaInt64,
    pub archive_entry_ctime_nsec: unsafe extern "C" fn(*mut BackendEntry) -> c_long,
    pub archive_entry_ctime_is_set: unsafe extern "C" fn(*mut BackendEntry) -> c_int,
    pub archive_entry_mtime: unsafe extern "C" fn(*mut BackendEntry) -> LaInt64,
    pub archive_entry_mtime_nsec: unsafe extern "C" fn(*mut BackendEntry) -> c_long,
    pub archive_entry_mtime_is_set: unsafe extern "C" fn(*mut BackendEntry) -> c_int,
    pub archive_entry_fflags: unsafe extern "C" fn(*mut BackendEntry, *mut c_ulong, *mut c_ulong),
    pub archive_entry_fflags_text: unsafe extern "C" fn(*mut BackendEntry) -> *const c_char,
    pub archive_entry_digest: unsafe extern "C" fn(*mut BackendEntry, c_int) -> *const c_uchar,
    pub archive_entry_mac_metadata:
        unsafe extern "C" fn(*mut BackendEntry, *mut size_t) -> *const c_void,
    pub archive_entry_is_data_encrypted: unsafe extern "C" fn(*mut BackendEntry) -> c_int,
    pub archive_entry_is_metadata_encrypted: unsafe extern "C" fn(*mut BackendEntry) -> c_int,
    pub archive_entry_acl_types: unsafe extern "C" fn(*mut BackendEntry) -> c_int,
    pub archive_entry_acl_reset: unsafe extern "C" fn(*mut BackendEntry, c_int) -> c_int,
    pub archive_entry_acl_next: unsafe extern "C" fn(
        *mut BackendEntry,
        c_int,
        *mut c_int,
        *mut c_int,
        *mut c_int,
        *mut c_int,
        *mut *const c_char,
    ) -> c_int,
    pub archive_entry_xattr_reset: unsafe extern "C" fn(*mut BackendEntry) -> c_int,
    pub archive_entry_xattr_next: unsafe extern "C" fn(
        *mut BackendEntry,
        *mut *const c_char,
        *mut *const c_void,
        *mut size_t,
    ) -> c_int,
    pub archive_entry_sparse_reset: unsafe extern "C" fn(*mut BackendEntry) -> c_int,
    pub archive_entry_sparse_next:
        unsafe extern "C" fn(*mut BackendEntry, *mut LaInt64, *mut LaInt64) -> c_int,
    pub archive_entry_copy_pathname: unsafe extern "C" fn(*mut BackendEntry, *const c_char),
    pub archive_entry_copy_sourcepath: unsafe extern "C" fn(*mut BackendEntry, *const c_char),
    pub archive_entry_set_mode: unsafe extern "C" fn(*mut BackendEntry, mode_t),
    pub archive_entry_set_size: unsafe extern "C" fn(*mut BackendEntry, LaInt64),
    pub archive_entry_unset_size: unsafe extern "C" fn(*mut BackendEntry),
    pub archive_entry_set_uid: unsafe extern "C" fn(*mut BackendEntry, LaInt64),
    pub archive_entry_set_gid: unsafe extern "C" fn(*mut BackendEntry, LaInt64),
    pub archive_entry_copy_uname: unsafe extern "C" fn(*mut BackendEntry, *const c_char),
    pub archive_entry_copy_gname: unsafe extern "C" fn(*mut BackendEntry, *const c_char),
    pub archive_entry_copy_hardlink: unsafe extern "C" fn(*mut BackendEntry, *const c_char),
    pub archive_entry_copy_symlink: unsafe extern "C" fn(*mut BackendEntry, *const c_char),
    pub archive_entry_set_symlink_type: unsafe extern "C" fn(*mut BackendEntry, c_int),
    pub archive_entry_set_nlink: unsafe extern "C" fn(*mut BackendEntry, c_uint),
    pub archive_entry_set_ino: unsafe extern "C" fn(*mut BackendEntry, LaInt64),
    pub archive_entry_set_dev: unsafe extern "C" fn(*mut BackendEntry, dev_t),
    pub archive_entry_set_rdev: unsafe extern "C" fn(*mut BackendEntry, dev_t),
    pub archive_entry_set_atime: unsafe extern "C" fn(*mut BackendEntry, LaInt64, c_long),
    pub archive_entry_unset_atime: unsafe extern "C" fn(*mut BackendEntry),
    pub archive_entry_set_birthtime: unsafe extern "C" fn(*mut BackendEntry, LaInt64, c_long),
    pub archive_entry_unset_birthtime: unsafe extern "C" fn(*mut BackendEntry),
    pub archive_entry_set_ctime: unsafe extern "C" fn(*mut BackendEntry, LaInt64, c_long),
    pub archive_entry_unset_ctime: unsafe extern "C" fn(*mut BackendEntry),
    pub archive_entry_set_mtime: unsafe extern "C" fn(*mut BackendEntry, LaInt64, c_long),
    pub archive_entry_unset_mtime: unsafe extern "C" fn(*mut BackendEntry),
    pub archive_entry_set_fflags: unsafe extern "C" fn(*mut BackendEntry, c_ulong, c_ulong),
    pub archive_entry_copy_mac_metadata:
        unsafe extern "C" fn(*mut BackendEntry, *const c_void, size_t),
    pub archive_entry_set_is_data_encrypted: unsafe extern "C" fn(*mut BackendEntry, c_char),
    pub archive_entry_set_is_metadata_encrypted: unsafe extern "C" fn(*mut BackendEntry, c_char),
    pub archive_entry_acl_clear: unsafe extern "C" fn(*mut BackendEntry),
    pub archive_entry_acl_add_entry:
        unsafe extern "C" fn(*mut BackendEntry, c_int, c_int, c_int, c_int, *const c_char) -> c_int,
    pub archive_entry_xattr_add_entry:
        unsafe extern "C" fn(*mut BackendEntry, *const c_char, *const c_void, size_t),
    pub archive_entry_sparse_add_entry: unsafe extern "C" fn(*mut BackendEntry, LaInt64, LaInt64),

    pub archive_read_new: unsafe extern "C" fn() -> *mut BackendArchive,
    pub archive_read_free: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_read_close: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_read_support_filter_all: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_read_support_filter_by_code:
        unsafe extern "C" fn(*mut BackendArchive, c_int) -> c_int,
    pub archive_read_support_filter_none: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_read_support_filter_bzip2: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_read_support_filter_compress: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_read_support_filter_gzip: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_read_support_filter_grzip: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_read_support_filter_lrzip: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_read_support_filter_lz4: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_read_support_filter_lzip: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_read_support_filter_lzma: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_read_support_filter_lzop: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_read_support_filter_program:
        unsafe extern "C" fn(*mut BackendArchive, *const c_char) -> c_int,
    pub archive_read_support_filter_program_signature:
        unsafe extern "C" fn(*mut BackendArchive, *const c_char, *const c_void, size_t) -> c_int,
    pub archive_read_support_filter_rpm: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_read_support_filter_uu: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_read_support_filter_xz: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_read_support_filter_zstd: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_read_support_format_7zip: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_read_support_format_ar: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_read_support_format_by_code:
        unsafe extern "C" fn(*mut BackendArchive, c_int) -> c_int,
    pub archive_read_support_format_cab: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_read_support_format_cpio: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_read_support_format_empty: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_read_support_format_gnutar: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_read_support_format_iso9660: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_read_support_format_lha: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_read_support_format_mtree: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_read_support_format_rar: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_read_support_format_rar5: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_read_support_format_raw: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_read_support_format_tar: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_read_support_format_warc: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_read_support_format_xar: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_read_support_format_zip: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_read_support_format_zip_seekable:
        unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_read_support_format_zip_streamable:
        unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_read_set_format: unsafe extern "C" fn(*mut BackendArchive, c_int) -> c_int,
    pub archive_read_append_filter: unsafe extern "C" fn(*mut BackendArchive, c_int) -> c_int,
    pub archive_read_append_filter_program:
        unsafe extern "C" fn(*mut BackendArchive, *const c_char) -> c_int,
    pub archive_read_append_filter_program_signature:
        unsafe extern "C" fn(*mut BackendArchive, *const c_char, *const c_void, size_t) -> c_int,
    pub archive_read_set_open_callback:
        unsafe extern "C" fn(*mut BackendArchive, BackendOpenCallback) -> c_int,
    pub archive_read_set_read_callback:
        unsafe extern "C" fn(*mut BackendArchive, BackendReadCallback) -> c_int,
    pub archive_read_set_seek_callback:
        unsafe extern "C" fn(*mut BackendArchive, BackendSeekCallback) -> c_int,
    pub archive_read_set_skip_callback:
        unsafe extern "C" fn(*mut BackendArchive, BackendSkipCallback) -> c_int,
    pub archive_read_set_close_callback:
        unsafe extern "C" fn(*mut BackendArchive, BackendCloseCallback) -> c_int,
    pub archive_read_set_switch_callback:
        unsafe extern "C" fn(*mut BackendArchive, BackendSwitchCallback) -> c_int,
    pub archive_read_set_callback_data:
        unsafe extern "C" fn(*mut BackendArchive, *mut c_void) -> c_int,
    pub archive_read_set_callback_data2:
        unsafe extern "C" fn(*mut BackendArchive, *mut c_void, c_uint) -> c_int,
    pub archive_read_add_callback_data:
        unsafe extern "C" fn(*mut BackendArchive, *mut c_void, c_uint) -> c_int,
    pub archive_read_append_callback_data:
        unsafe extern "C" fn(*mut BackendArchive, *mut c_void) -> c_int,
    pub archive_read_prepend_callback_data:
        unsafe extern "C" fn(*mut BackendArchive, *mut c_void) -> c_int,
    pub archive_read_open1: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_read_open_memory:
        unsafe extern "C" fn(*mut BackendArchive, *const c_void, size_t) -> c_int,
    pub archive_read_open_memory2:
        unsafe extern "C" fn(*mut BackendArchive, *const c_void, size_t, size_t) -> c_int,
    pub archive_read_open_filename:
        unsafe extern "C" fn(*mut BackendArchive, *const c_char, size_t) -> c_int,
    pub archive_read_open_filenames:
        unsafe extern "C" fn(*mut BackendArchive, *const *const c_char, size_t) -> c_int,
    pub archive_read_open_filename_w:
        unsafe extern "C" fn(*mut BackendArchive, *const wchar_t, size_t) -> c_int,
    pub archive_read_open_fd: unsafe extern "C" fn(*mut BackendArchive, c_int, size_t) -> c_int,
    pub archive_read_open_FILE: unsafe extern "C" fn(*mut BackendArchive, *mut c_void) -> c_int,
    pub archive_read_next_header:
        unsafe extern "C" fn(*mut BackendArchive, *mut *mut BackendEntry) -> c_int,
    pub archive_read_next_header2:
        unsafe extern "C" fn(*mut BackendArchive, *mut BackendEntry) -> c_int,
    pub archive_read_header_position: unsafe extern "C" fn(*mut BackendArchive) -> LaInt64,
    pub archive_read_has_encrypted_entries: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_read_format_capabilities: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_read_data:
        unsafe extern "C" fn(*mut BackendArchive, *mut c_void, size_t) -> LaSsize,
    pub archive_seek_data: unsafe extern "C" fn(*mut BackendArchive, LaInt64, c_int) -> LaInt64,
    pub archive_read_data_block: unsafe extern "C" fn(
        *mut BackendArchive,
        *mut *const c_void,
        *mut size_t,
        *mut LaInt64,
    ) -> c_int,
    pub archive_read_data_skip: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_read_data_into_fd: unsafe extern "C" fn(*mut BackendArchive, c_int) -> c_int,
    pub archive_read_set_format_option: unsafe extern "C" fn(
        *mut BackendArchive,
        *const c_char,
        *const c_char,
        *const c_char,
    ) -> c_int,
    pub archive_read_set_filter_option: unsafe extern "C" fn(
        *mut BackendArchive,
        *const c_char,
        *const c_char,
        *const c_char,
    ) -> c_int,
    pub archive_read_set_option: unsafe extern "C" fn(
        *mut BackendArchive,
        *const c_char,
        *const c_char,
        *const c_char,
    ) -> c_int,
    pub archive_read_set_options: unsafe extern "C" fn(*mut BackendArchive, *const c_char) -> c_int,
    pub archive_read_add_passphrase:
        unsafe extern "C" fn(*mut BackendArchive, *const c_char) -> c_int,
    pub archive_read_set_passphrase_callback:
        unsafe extern "C" fn(*mut BackendArchive, *mut c_void, BackendPassphraseCallback) -> c_int,

    pub archive_write_new: unsafe extern "C" fn() -> *mut BackendArchive,
    pub archive_write_free: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_close: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_set_bytes_per_block:
        unsafe extern "C" fn(*mut BackendArchive, c_int) -> c_int,
    pub archive_write_get_bytes_per_block: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_set_bytes_in_last_block:
        unsafe extern "C" fn(*mut BackendArchive, c_int) -> c_int,
    pub archive_write_get_bytes_in_last_block: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_set_skip_file:
        unsafe extern "C" fn(*mut BackendArchive, LaInt64, LaInt64) -> c_int,
    pub archive_write_add_filter: unsafe extern "C" fn(*mut BackendArchive, c_int) -> c_int,
    pub archive_write_add_filter_by_name:
        unsafe extern "C" fn(*mut BackendArchive, *const c_char) -> c_int,
    pub archive_write_add_filter_b64encode: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_add_filter_bzip2: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_add_filter_compress: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_add_filter_grzip: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_add_filter_gzip: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_add_filter_lrzip: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_add_filter_lz4: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_add_filter_lzip: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_add_filter_lzma: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_add_filter_lzop: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_add_filter_none: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_add_filter_program:
        unsafe extern "C" fn(*mut BackendArchive, *const c_char) -> c_int,
    pub archive_write_add_filter_uuencode: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_add_filter_xz: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_add_filter_zstd: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_set_format_7zip: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_set_format_ar_bsd: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_set_format_ar_svr4: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_set_format_cpio: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_set_format_cpio_bin: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_set_format_cpio_newc: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_set_format_cpio_odc: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_set_format_cpio_pwb: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_set_format_gnutar: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_set_format_iso9660: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_set_format_mtree: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_set_format_mtree_classic: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_set_format_pax: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_set_format_pax_restricted: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_set_format_raw: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_set_format_shar: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_set_format_shar_dump: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_set_format_ustar: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_set_format_v7tar: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_set_format_warc: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_set_format_xar: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_set_format_zip: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_open: unsafe extern "C" fn(
        *mut BackendArchive,
        *mut c_void,
        BackendOpenCallback,
        BackendWriteCallback,
        BackendCloseCallback,
    ) -> c_int,
    pub archive_write_open2: unsafe extern "C" fn(
        *mut BackendArchive,
        *mut c_void,
        BackendOpenCallback,
        BackendWriteCallback,
        BackendCloseCallback,
        BackendFreeCallback,
    ) -> c_int,
    pub archive_write_open_fd: unsafe extern "C" fn(*mut BackendArchive, c_int) -> c_int,
    pub archive_write_open_filename:
        unsafe extern "C" fn(*mut BackendArchive, *const c_char) -> c_int,
    pub archive_write_open_filename_w:
        unsafe extern "C" fn(*mut BackendArchive, *const wchar_t) -> c_int,
    pub archive_write_open_file: unsafe extern "C" fn(*mut BackendArchive, *const c_char) -> c_int,
    pub archive_write_open_FILE: unsafe extern "C" fn(*mut BackendArchive, *mut c_void) -> c_int,
    pub archive_write_open_memory:
        unsafe extern "C" fn(*mut BackendArchive, *mut c_void, size_t, *mut size_t) -> c_int,
    pub archive_write_header: unsafe extern "C" fn(*mut BackendArchive, *mut BackendEntry) -> c_int,
    pub archive_write_data:
        unsafe extern "C" fn(*mut BackendArchive, *const c_void, size_t) -> LaSsize,
    pub archive_write_data_block:
        unsafe extern "C" fn(*mut BackendArchive, *const c_void, size_t, LaInt64) -> LaSsize,
    pub archive_write_finish_entry: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_fail: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_set_filter_option: unsafe extern "C" fn(
        *mut BackendArchive,
        *const c_char,
        *const c_char,
        *const c_char,
    ) -> c_int,
    pub archive_write_set_format_option: unsafe extern "C" fn(
        *mut BackendArchive,
        *const c_char,
        *const c_char,
        *const c_char,
    ) -> c_int,
    pub archive_write_set_option: unsafe extern "C" fn(
        *mut BackendArchive,
        *const c_char,
        *const c_char,
        *const c_char,
    ) -> c_int,
    pub archive_write_set_options:
        unsafe extern "C" fn(*mut BackendArchive, *const c_char) -> c_int,
    pub archive_write_set_passphrase:
        unsafe extern "C" fn(*mut BackendArchive, *const c_char) -> c_int,
    pub archive_write_set_passphrase_callback:
        unsafe extern "C" fn(*mut BackendArchive, *mut c_void, BackendPassphraseCallback) -> c_int,
    pub archive_write_zip_set_compression_deflate:
        unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_zip_set_compression_store: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
}

unsafe impl Send for Api {}
unsafe impl Sync for Api {}

include!(concat!(env!("OUT_DIR"), "/backend_linked.rs"));

pub fn api() -> &'static Api {
    linked_api()
}
