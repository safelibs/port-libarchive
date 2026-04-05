use std::ffi::{c_char, c_int, c_long, c_uint, c_ulong, c_void, CString};
use std::sync::OnceLock;

use libc::{dev_t, mode_t, size_t, wchar_t, RTLD_LOCAL, RTLD_NOW};

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
pub type BackendReadDiskExcludedCallback =
    Option<unsafe extern "C" fn(*mut BackendArchive, *mut c_void, *mut BackendEntry)>;
pub type BackendReadDiskMetadataFilterCallback =
    Option<unsafe extern "C" fn(*mut BackendArchive, *mut c_void, *mut BackendEntry) -> c_int>;

pub type BackendWriteDiskLookupCallback =
    Option<unsafe extern "C" fn(*mut c_void, *const c_char, LaInt64) -> LaInt64>;
pub type BackendWriteDiskCleanupCallback = Option<unsafe extern "C" fn(*mut c_void)>;

macro_rules! load_symbol {
    ($handle:expr, $symbol:literal, $ty:ty) => {{
        let symbol = unsafe { libc::dlsym($handle, concat!($symbol, "\0").as_ptr().cast()) };
        assert!(!symbol.is_null(), "missing backend symbol {}", $symbol);
        unsafe { std::mem::transmute::<*mut c_void, $ty>(symbol) }
    }};
}

pub struct Api {
    _library: *mut c_void,

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
    pub archive_read_support_filter_xz: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_read_support_filter_zstd: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_read_support_format_all: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_read_support_format_empty: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_read_support_format_raw: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_read_open_memory:
        unsafe extern "C" fn(*mut BackendArchive, *const c_void, size_t) -> c_int,
    pub archive_read_open_filename:
        unsafe extern "C" fn(*mut BackendArchive, *const c_char, size_t) -> c_int,
    pub archive_read_open_filename_w:
        unsafe extern "C" fn(*mut BackendArchive, *const wchar_t, size_t) -> c_int,
    pub archive_read_next_header:
        unsafe extern "C" fn(*mut BackendArchive, *mut *mut BackendEntry) -> c_int,
    pub archive_read_next_header2:
        unsafe extern "C" fn(*mut BackendArchive, *mut BackendEntry) -> c_int,
    pub archive_read_data:
        unsafe extern "C" fn(*mut BackendArchive, *mut c_void, size_t) -> LaSsize,
    pub archive_read_data_block: unsafe extern "C" fn(
        *mut BackendArchive,
        *mut *const c_void,
        *mut size_t,
        *mut LaInt64,
    ) -> c_int,
    pub archive_read_extract:
        unsafe extern "C" fn(*mut BackendArchive, *mut BackendEntry, c_int) -> c_int,
    pub archive_read_extract2:
        unsafe extern "C" fn(*mut BackendArchive, *mut BackendEntry, *mut BackendArchive) -> c_int,

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
    pub archive_write_set_format: unsafe extern "C" fn(*mut BackendArchive, c_int) -> c_int,
    pub archive_write_set_format_by_name:
        unsafe extern "C" fn(*mut BackendArchive, *const c_char) -> c_int,
    pub archive_write_set_format_ar_bsd: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_set_format_ar_svr4: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_set_format_cpio: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_set_format_cpio_bin: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_set_format_cpio_newc: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_set_format_cpio_odc: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_set_format_cpio_pwb: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_set_format_gnutar: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_set_format_pax: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_set_format_pax_restricted: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_set_format_raw: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_set_format_shar: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_set_format_shar_dump: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_set_format_ustar: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_set_format_v7tar: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_set_format_filter_by_ext:
        unsafe extern "C" fn(*mut BackendArchive, *const c_char) -> c_int,
    pub archive_write_set_format_filter_by_ext_def:
        unsafe extern "C" fn(*mut BackendArchive, *const c_char, *const c_char) -> c_int,
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
    pub archive_write_open_filename:
        unsafe extern "C" fn(*mut BackendArchive, *const c_char) -> c_int,
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

    pub archive_write_disk_new: unsafe extern "C" fn() -> *mut BackendArchive,
    pub archive_write_disk_set_skip_file:
        unsafe extern "C" fn(*mut BackendArchive, LaInt64, LaInt64) -> c_int,
    pub archive_write_disk_set_options: unsafe extern "C" fn(*mut BackendArchive, c_int) -> c_int,
    pub archive_write_disk_set_standard_lookup: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_write_disk_set_group_lookup: unsafe extern "C" fn(
        *mut BackendArchive,
        *mut c_void,
        BackendWriteDiskLookupCallback,
        BackendWriteDiskCleanupCallback,
    ) -> c_int,
    pub archive_write_disk_set_user_lookup: unsafe extern "C" fn(
        *mut BackendArchive,
        *mut c_void,
        BackendWriteDiskLookupCallback,
        BackendWriteDiskCleanupCallback,
    ) -> c_int,
    pub archive_write_disk_gid:
        unsafe extern "C" fn(*mut BackendArchive, *const c_char, LaInt64) -> LaInt64,
    pub archive_write_disk_uid:
        unsafe extern "C" fn(*mut BackendArchive, *const c_char, LaInt64) -> LaInt64,

    pub archive_read_disk_new: unsafe extern "C" fn() -> *mut BackendArchive,
    pub archive_read_disk_set_symlink_logical: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_read_disk_set_symlink_physical: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_read_disk_set_symlink_hybrid: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_read_disk_entry_from_file:
        unsafe extern "C" fn(*mut BackendArchive, *mut BackendEntry, c_int, *const c_void) -> c_int,
    pub archive_read_disk_gname:
        unsafe extern "C" fn(*mut BackendArchive, LaInt64) -> *const c_char,
    pub archive_read_disk_uname:
        unsafe extern "C" fn(*mut BackendArchive, LaInt64) -> *const c_char,
    pub archive_read_disk_set_standard_lookup: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_read_disk_set_gname_lookup: unsafe extern "C" fn(
        *mut BackendArchive,
        *mut c_void,
        BackendReadDiskLookupCallback,
        BackendReadDiskCleanupCallback,
    ) -> c_int,
    pub archive_read_disk_set_uname_lookup: unsafe extern "C" fn(
        *mut BackendArchive,
        *mut c_void,
        BackendReadDiskLookupCallback,
        BackendReadDiskCleanupCallback,
    ) -> c_int,
    pub archive_read_disk_open: unsafe extern "C" fn(*mut BackendArchive, *const c_char) -> c_int,
    pub archive_read_disk_open_w:
        unsafe extern "C" fn(*mut BackendArchive, *const wchar_t) -> c_int,
    pub archive_read_disk_descend: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_read_disk_can_descend: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_read_disk_current_filesystem: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_read_disk_current_filesystem_is_synthetic:
        unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_read_disk_current_filesystem_is_remote:
        unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_read_disk_set_atime_restored: unsafe extern "C" fn(*mut BackendArchive) -> c_int,
    pub archive_read_disk_set_behavior: unsafe extern "C" fn(*mut BackendArchive, c_int) -> c_int,
    pub archive_read_disk_set_matching: unsafe extern "C" fn(
        *mut BackendArchive,
        *mut BackendArchive,
        BackendReadDiskExcludedCallback,
        *mut c_void,
    ) -> c_int,
    pub archive_read_disk_set_metadata_filter_callback: unsafe extern "C" fn(
        *mut BackendArchive,
        BackendReadDiskMetadataFilterCallback,
        *mut c_void,
    ) -> c_int,
}

unsafe impl Send for Api {}
unsafe impl Sync for Api {}

fn load_library() -> *mut c_void {
    const CANDIDATES: &[&str] = &[
        "/lib/x86_64-linux-gnu/libarchive.so.13",
        "/usr/lib/x86_64-linux-gnu/libarchive.so.13",
        "/lib64/libarchive.so.13",
        "/usr/lib64/libarchive.so.13",
        "libarchive.so.13",
        "libarchive.so",
    ];

    for candidate in CANDIDATES {
        let Ok(path) = CString::new(*candidate) else {
            continue;
        };
        let handle = unsafe { libc::dlopen(path.as_ptr(), RTLD_NOW | RTLD_LOCAL) };
        if !handle.is_null() {
            return handle;
        }
    }

    panic!("failed to load libarchive backend");
}

fn load() -> Api {
    let handle = load_library();
    Api {
        _library: handle,
        archive_errno: load_symbol!(
            handle,
            "archive_errno",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_error_string: load_symbol!(
            handle,
            "archive_error_string",
            unsafe extern "C" fn(*mut BackendArchive) -> *const c_char
        ),
        archive_file_count: load_symbol!(
            handle,
            "archive_file_count",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_filter_bytes: load_symbol!(
            handle,
            "archive_filter_bytes",
            unsafe extern "C" fn(*mut BackendArchive, c_int) -> LaInt64
        ),
        archive_filter_code: load_symbol!(
            handle,
            "archive_filter_code",
            unsafe extern "C" fn(*mut BackendArchive, c_int) -> c_int
        ),
        archive_filter_count: load_symbol!(
            handle,
            "archive_filter_count",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_filter_name: load_symbol!(
            handle,
            "archive_filter_name",
            unsafe extern "C" fn(*mut BackendArchive, c_int) -> *const c_char
        ),
        archive_format: load_symbol!(
            handle,
            "archive_format",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_format_name: load_symbol!(
            handle,
            "archive_format_name",
            unsafe extern "C" fn(*mut BackendArchive) -> *const c_char
        ),
        archive_position_compressed: load_symbol!(
            handle,
            "archive_position_compressed",
            unsafe extern "C" fn(*mut BackendArchive) -> LaInt64
        ),
        archive_position_uncompressed: load_symbol!(
            handle,
            "archive_position_uncompressed",
            unsafe extern "C" fn(*mut BackendArchive) -> LaInt64
        ),
        archive_version_details: load_symbol!(
            handle,
            "archive_version_details",
            unsafe extern "C" fn() -> *const c_char
        ),
        archive_bzlib_version: load_symbol!(
            handle,
            "archive_bzlib_version",
            unsafe extern "C" fn() -> *const c_char
        ),
        archive_liblz4_version: load_symbol!(
            handle,
            "archive_liblz4_version",
            unsafe extern "C" fn() -> *const c_char
        ),
        archive_liblzma_version: load_symbol!(
            handle,
            "archive_liblzma_version",
            unsafe extern "C" fn() -> *const c_char
        ),
        archive_libzstd_version: load_symbol!(
            handle,
            "archive_libzstd_version",
            unsafe extern "C" fn() -> *const c_char
        ),
        archive_zlib_version: load_symbol!(
            handle,
            "archive_zlib_version",
            unsafe extern "C" fn() -> *const c_char
        ),
        archive_match_new: load_symbol!(
            handle,
            "archive_match_new",
            unsafe extern "C" fn() -> *mut BackendArchive
        ),
        archive_match_free: load_symbol!(
            handle,
            "archive_match_free",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_match_set_inclusion_recursion: load_symbol!(
            handle,
            "archive_match_set_inclusion_recursion",
            unsafe extern "C" fn(*mut BackendArchive, c_int) -> c_int
        ),
        archive_match_exclude_pattern: load_symbol!(
            handle,
            "archive_match_exclude_pattern",
            unsafe extern "C" fn(*mut BackendArchive, *const c_char) -> c_int
        ),
        archive_match_include_pattern: load_symbol!(
            handle,
            "archive_match_include_pattern",
            unsafe extern "C" fn(*mut BackendArchive, *const c_char) -> c_int
        ),
        archive_match_include_time: load_symbol!(
            handle,
            "archive_match_include_time",
            unsafe extern "C" fn(*mut BackendArchive, c_int, LaInt64, c_long) -> c_int
        ),
        archive_match_include_uid: load_symbol!(
            handle,
            "archive_match_include_uid",
            unsafe extern "C" fn(*mut BackendArchive, LaInt64) -> c_int
        ),
        archive_match_include_gid: load_symbol!(
            handle,
            "archive_match_include_gid",
            unsafe extern "C" fn(*mut BackendArchive, LaInt64) -> c_int
        ),
        archive_match_include_uname: load_symbol!(
            handle,
            "archive_match_include_uname",
            unsafe extern "C" fn(*mut BackendArchive, *const c_char) -> c_int
        ),
        archive_match_include_gname: load_symbol!(
            handle,
            "archive_match_include_gname",
            unsafe extern "C" fn(*mut BackendArchive, *const c_char) -> c_int
        ),
        archive_entry_new: load_symbol!(
            handle,
            "archive_entry_new",
            unsafe extern "C" fn() -> *mut BackendEntry
        ),
        archive_entry_free: load_symbol!(
            handle,
            "archive_entry_free",
            unsafe extern "C" fn(*mut BackendEntry)
        ),
        archive_entry_clear: load_symbol!(
            handle,
            "archive_entry_clear",
            unsafe extern "C" fn(*mut BackendEntry) -> *mut BackendEntry
        ),
        archive_entry_pathname: load_symbol!(
            handle,
            "archive_entry_pathname",
            unsafe extern "C" fn(*mut BackendEntry) -> *const c_char
        ),
        archive_entry_mode: load_symbol!(
            handle,
            "archive_entry_mode",
            unsafe extern "C" fn(*mut BackendEntry) -> mode_t
        ),
        archive_entry_size: load_symbol!(
            handle,
            "archive_entry_size",
            unsafe extern "C" fn(*mut BackendEntry) -> LaInt64
        ),
        archive_entry_size_is_set: load_symbol!(
            handle,
            "archive_entry_size_is_set",
            unsafe extern "C" fn(*mut BackendEntry) -> c_int
        ),
        archive_entry_uid: load_symbol!(
            handle,
            "archive_entry_uid",
            unsafe extern "C" fn(*mut BackendEntry) -> LaInt64
        ),
        archive_entry_gid: load_symbol!(
            handle,
            "archive_entry_gid",
            unsafe extern "C" fn(*mut BackendEntry) -> LaInt64
        ),
        archive_entry_uname: load_symbol!(
            handle,
            "archive_entry_uname",
            unsafe extern "C" fn(*mut BackendEntry) -> *const c_char
        ),
        archive_entry_gname: load_symbol!(
            handle,
            "archive_entry_gname",
            unsafe extern "C" fn(*mut BackendEntry) -> *const c_char
        ),
        archive_entry_hardlink: load_symbol!(
            handle,
            "archive_entry_hardlink",
            unsafe extern "C" fn(*mut BackendEntry) -> *const c_char
        ),
        archive_entry_symlink: load_symbol!(
            handle,
            "archive_entry_symlink",
            unsafe extern "C" fn(*mut BackendEntry) -> *const c_char
        ),
        archive_entry_symlink_type: load_symbol!(
            handle,
            "archive_entry_symlink_type",
            unsafe extern "C" fn(*mut BackendEntry) -> c_int
        ),
        archive_entry_nlink: load_symbol!(
            handle,
            "archive_entry_nlink",
            unsafe extern "C" fn(*mut BackendEntry) -> c_uint
        ),
        archive_entry_ino: load_symbol!(
            handle,
            "archive_entry_ino",
            unsafe extern "C" fn(*mut BackendEntry) -> LaInt64
        ),
        archive_entry_dev: load_symbol!(
            handle,
            "archive_entry_dev",
            unsafe extern "C" fn(*mut BackendEntry) -> dev_t
        ),
        archive_entry_rdev: load_symbol!(
            handle,
            "archive_entry_rdev",
            unsafe extern "C" fn(*mut BackendEntry) -> dev_t
        ),
        archive_entry_atime: load_symbol!(
            handle,
            "archive_entry_atime",
            unsafe extern "C" fn(*mut BackendEntry) -> LaInt64
        ),
        archive_entry_atime_nsec: load_symbol!(
            handle,
            "archive_entry_atime_nsec",
            unsafe extern "C" fn(*mut BackendEntry) -> c_long
        ),
        archive_entry_atime_is_set: load_symbol!(
            handle,
            "archive_entry_atime_is_set",
            unsafe extern "C" fn(*mut BackendEntry) -> c_int
        ),
        archive_entry_birthtime: load_symbol!(
            handle,
            "archive_entry_birthtime",
            unsafe extern "C" fn(*mut BackendEntry) -> LaInt64
        ),
        archive_entry_birthtime_nsec: load_symbol!(
            handle,
            "archive_entry_birthtime_nsec",
            unsafe extern "C" fn(*mut BackendEntry) -> c_long
        ),
        archive_entry_birthtime_is_set: load_symbol!(
            handle,
            "archive_entry_birthtime_is_set",
            unsafe extern "C" fn(*mut BackendEntry) -> c_int
        ),
        archive_entry_ctime: load_symbol!(
            handle,
            "archive_entry_ctime",
            unsafe extern "C" fn(*mut BackendEntry) -> LaInt64
        ),
        archive_entry_ctime_nsec: load_symbol!(
            handle,
            "archive_entry_ctime_nsec",
            unsafe extern "C" fn(*mut BackendEntry) -> c_long
        ),
        archive_entry_ctime_is_set: load_symbol!(
            handle,
            "archive_entry_ctime_is_set",
            unsafe extern "C" fn(*mut BackendEntry) -> c_int
        ),
        archive_entry_mtime: load_symbol!(
            handle,
            "archive_entry_mtime",
            unsafe extern "C" fn(*mut BackendEntry) -> LaInt64
        ),
        archive_entry_mtime_nsec: load_symbol!(
            handle,
            "archive_entry_mtime_nsec",
            unsafe extern "C" fn(*mut BackendEntry) -> c_long
        ),
        archive_entry_mtime_is_set: load_symbol!(
            handle,
            "archive_entry_mtime_is_set",
            unsafe extern "C" fn(*mut BackendEntry) -> c_int
        ),
        archive_entry_fflags: load_symbol!(
            handle,
            "archive_entry_fflags",
            unsafe extern "C" fn(*mut BackendEntry, *mut c_ulong, *mut c_ulong)
        ),
        archive_entry_mac_metadata: load_symbol!(
            handle,
            "archive_entry_mac_metadata",
            unsafe extern "C" fn(*mut BackendEntry, *mut size_t) -> *const c_void
        ),
        archive_entry_is_data_encrypted: load_symbol!(
            handle,
            "archive_entry_is_data_encrypted",
            unsafe extern "C" fn(*mut BackendEntry) -> c_int
        ),
        archive_entry_is_metadata_encrypted: load_symbol!(
            handle,
            "archive_entry_is_metadata_encrypted",
            unsafe extern "C" fn(*mut BackendEntry) -> c_int
        ),
        archive_entry_acl_types: load_symbol!(
            handle,
            "archive_entry_acl_types",
            unsafe extern "C" fn(*mut BackendEntry) -> c_int
        ),
        archive_entry_acl_reset: load_symbol!(
            handle,
            "archive_entry_acl_reset",
            unsafe extern "C" fn(*mut BackendEntry, c_int) -> c_int
        ),
        archive_entry_acl_next: load_symbol!(
            handle,
            "archive_entry_acl_next",
            unsafe extern "C" fn(
                *mut BackendEntry,
                c_int,
                *mut c_int,
                *mut c_int,
                *mut c_int,
                *mut c_int,
                *mut *const c_char,
            ) -> c_int
        ),
        archive_entry_xattr_reset: load_symbol!(
            handle,
            "archive_entry_xattr_reset",
            unsafe extern "C" fn(*mut BackendEntry) -> c_int
        ),
        archive_entry_xattr_next: load_symbol!(
            handle,
            "archive_entry_xattr_next",
            unsafe extern "C" fn(
                *mut BackendEntry,
                *mut *const c_char,
                *mut *const c_void,
                *mut size_t,
            ) -> c_int
        ),
        archive_entry_sparse_reset: load_symbol!(
            handle,
            "archive_entry_sparse_reset",
            unsafe extern "C" fn(*mut BackendEntry) -> c_int
        ),
        archive_entry_sparse_next: load_symbol!(
            handle,
            "archive_entry_sparse_next",
            unsafe extern "C" fn(*mut BackendEntry, *mut LaInt64, *mut LaInt64) -> c_int
        ),
        archive_entry_copy_pathname: load_symbol!(
            handle,
            "archive_entry_copy_pathname",
            unsafe extern "C" fn(*mut BackendEntry, *const c_char)
        ),
        archive_entry_set_mode: load_symbol!(
            handle,
            "archive_entry_set_mode",
            unsafe extern "C" fn(*mut BackendEntry, mode_t)
        ),
        archive_entry_set_size: load_symbol!(
            handle,
            "archive_entry_set_size",
            unsafe extern "C" fn(*mut BackendEntry, LaInt64)
        ),
        archive_entry_unset_size: load_symbol!(
            handle,
            "archive_entry_unset_size",
            unsafe extern "C" fn(*mut BackendEntry)
        ),
        archive_entry_set_uid: load_symbol!(
            handle,
            "archive_entry_set_uid",
            unsafe extern "C" fn(*mut BackendEntry, LaInt64)
        ),
        archive_entry_set_gid: load_symbol!(
            handle,
            "archive_entry_set_gid",
            unsafe extern "C" fn(*mut BackendEntry, LaInt64)
        ),
        archive_entry_copy_uname: load_symbol!(
            handle,
            "archive_entry_copy_uname",
            unsafe extern "C" fn(*mut BackendEntry, *const c_char)
        ),
        archive_entry_copy_gname: load_symbol!(
            handle,
            "archive_entry_copy_gname",
            unsafe extern "C" fn(*mut BackendEntry, *const c_char)
        ),
        archive_entry_copy_hardlink: load_symbol!(
            handle,
            "archive_entry_copy_hardlink",
            unsafe extern "C" fn(*mut BackendEntry, *const c_char)
        ),
        archive_entry_copy_symlink: load_symbol!(
            handle,
            "archive_entry_copy_symlink",
            unsafe extern "C" fn(*mut BackendEntry, *const c_char)
        ),
        archive_entry_set_symlink_type: load_symbol!(
            handle,
            "archive_entry_set_symlink_type",
            unsafe extern "C" fn(*mut BackendEntry, c_int)
        ),
        archive_entry_set_nlink: load_symbol!(
            handle,
            "archive_entry_set_nlink",
            unsafe extern "C" fn(*mut BackendEntry, c_uint)
        ),
        archive_entry_set_ino: load_symbol!(
            handle,
            "archive_entry_set_ino",
            unsafe extern "C" fn(*mut BackendEntry, LaInt64)
        ),
        archive_entry_set_dev: load_symbol!(
            handle,
            "archive_entry_set_dev",
            unsafe extern "C" fn(*mut BackendEntry, dev_t)
        ),
        archive_entry_set_rdev: load_symbol!(
            handle,
            "archive_entry_set_rdev",
            unsafe extern "C" fn(*mut BackendEntry, dev_t)
        ),
        archive_entry_set_atime: load_symbol!(
            handle,
            "archive_entry_set_atime",
            unsafe extern "C" fn(*mut BackendEntry, LaInt64, c_long)
        ),
        archive_entry_unset_atime: load_symbol!(
            handle,
            "archive_entry_unset_atime",
            unsafe extern "C" fn(*mut BackendEntry)
        ),
        archive_entry_set_birthtime: load_symbol!(
            handle,
            "archive_entry_set_birthtime",
            unsafe extern "C" fn(*mut BackendEntry, LaInt64, c_long)
        ),
        archive_entry_unset_birthtime: load_symbol!(
            handle,
            "archive_entry_unset_birthtime",
            unsafe extern "C" fn(*mut BackendEntry)
        ),
        archive_entry_set_ctime: load_symbol!(
            handle,
            "archive_entry_set_ctime",
            unsafe extern "C" fn(*mut BackendEntry, LaInt64, c_long)
        ),
        archive_entry_unset_ctime: load_symbol!(
            handle,
            "archive_entry_unset_ctime",
            unsafe extern "C" fn(*mut BackendEntry)
        ),
        archive_entry_set_mtime: load_symbol!(
            handle,
            "archive_entry_set_mtime",
            unsafe extern "C" fn(*mut BackendEntry, LaInt64, c_long)
        ),
        archive_entry_unset_mtime: load_symbol!(
            handle,
            "archive_entry_unset_mtime",
            unsafe extern "C" fn(*mut BackendEntry)
        ),
        archive_entry_set_fflags: load_symbol!(
            handle,
            "archive_entry_set_fflags",
            unsafe extern "C" fn(*mut BackendEntry, c_ulong, c_ulong)
        ),
        archive_entry_copy_mac_metadata: load_symbol!(
            handle,
            "archive_entry_copy_mac_metadata",
            unsafe extern "C" fn(*mut BackendEntry, *const c_void, size_t)
        ),
        archive_entry_set_is_data_encrypted: load_symbol!(
            handle,
            "archive_entry_set_is_data_encrypted",
            unsafe extern "C" fn(*mut BackendEntry, c_char)
        ),
        archive_entry_set_is_metadata_encrypted: load_symbol!(
            handle,
            "archive_entry_set_is_metadata_encrypted",
            unsafe extern "C" fn(*mut BackendEntry, c_char)
        ),
        archive_entry_acl_clear: load_symbol!(
            handle,
            "archive_entry_acl_clear",
            unsafe extern "C" fn(*mut BackendEntry)
        ),
        archive_entry_acl_add_entry: load_symbol!(
            handle,
            "archive_entry_acl_add_entry",
            unsafe extern "C" fn(
                *mut BackendEntry,
                c_int,
                c_int,
                c_int,
                c_int,
                *const c_char,
            ) -> c_int
        ),
        archive_entry_xattr_add_entry: load_symbol!(
            handle,
            "archive_entry_xattr_add_entry",
            unsafe extern "C" fn(*mut BackendEntry, *const c_char, *const c_void, size_t)
        ),
        archive_entry_sparse_add_entry: load_symbol!(
            handle,
            "archive_entry_sparse_add_entry",
            unsafe extern "C" fn(*mut BackendEntry, LaInt64, LaInt64)
        ),
        archive_read_new: load_symbol!(
            handle,
            "archive_read_new",
            unsafe extern "C" fn() -> *mut BackendArchive
        ),
        archive_read_free: load_symbol!(
            handle,
            "archive_read_free",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_read_close: load_symbol!(
            handle,
            "archive_read_close",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_read_support_filter_all: load_symbol!(
            handle,
            "archive_read_support_filter_all",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_read_support_filter_none: load_symbol!(
            handle,
            "archive_read_support_filter_none",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_read_support_filter_bzip2: load_symbol!(
            handle,
            "archive_read_support_filter_bzip2",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_read_support_filter_compress: load_symbol!(
            handle,
            "archive_read_support_filter_compress",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_read_support_filter_gzip: load_symbol!(
            handle,
            "archive_read_support_filter_gzip",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_read_support_filter_grzip: load_symbol!(
            handle,
            "archive_read_support_filter_grzip",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_read_support_filter_lrzip: load_symbol!(
            handle,
            "archive_read_support_filter_lrzip",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_read_support_filter_lz4: load_symbol!(
            handle,
            "archive_read_support_filter_lz4",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_read_support_filter_lzip: load_symbol!(
            handle,
            "archive_read_support_filter_lzip",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_read_support_filter_lzma: load_symbol!(
            handle,
            "archive_read_support_filter_lzma",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_read_support_filter_lzop: load_symbol!(
            handle,
            "archive_read_support_filter_lzop",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_read_support_filter_xz: load_symbol!(
            handle,
            "archive_read_support_filter_xz",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_read_support_filter_zstd: load_symbol!(
            handle,
            "archive_read_support_filter_zstd",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_read_support_format_all: load_symbol!(
            handle,
            "archive_read_support_format_all",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_read_support_format_empty: load_symbol!(
            handle,
            "archive_read_support_format_empty",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_read_support_format_raw: load_symbol!(
            handle,
            "archive_read_support_format_raw",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_read_open_memory: load_symbol!(
            handle,
            "archive_read_open_memory",
            unsafe extern "C" fn(*mut BackendArchive, *const c_void, size_t) -> c_int
        ),
        archive_read_open_filename: load_symbol!(
            handle,
            "archive_read_open_filename",
            unsafe extern "C" fn(*mut BackendArchive, *const c_char, size_t) -> c_int
        ),
        archive_read_open_filename_w: load_symbol!(
            handle,
            "archive_read_open_filename_w",
            unsafe extern "C" fn(*mut BackendArchive, *const wchar_t, size_t) -> c_int
        ),
        archive_read_next_header: load_symbol!(
            handle,
            "archive_read_next_header",
            unsafe extern "C" fn(*mut BackendArchive, *mut *mut BackendEntry) -> c_int
        ),
        archive_read_next_header2: load_symbol!(
            handle,
            "archive_read_next_header2",
            unsafe extern "C" fn(*mut BackendArchive, *mut BackendEntry) -> c_int
        ),
        archive_read_data: load_symbol!(
            handle,
            "archive_read_data",
            unsafe extern "C" fn(*mut BackendArchive, *mut c_void, size_t) -> LaSsize
        ),
        archive_read_data_block: load_symbol!(
            handle,
            "archive_read_data_block",
            unsafe extern "C" fn(
                *mut BackendArchive,
                *mut *const c_void,
                *mut size_t,
                *mut LaInt64,
            ) -> c_int
        ),
        archive_read_extract: load_symbol!(
            handle,
            "archive_read_extract",
            unsafe extern "C" fn(*mut BackendArchive, *mut BackendEntry, c_int) -> c_int
        ),
        archive_read_extract2: load_symbol!(
            handle,
            "archive_read_extract2",
            unsafe extern "C" fn(
                *mut BackendArchive,
                *mut BackendEntry,
                *mut BackendArchive,
            ) -> c_int
        ),
        archive_write_new: load_symbol!(
            handle,
            "archive_write_new",
            unsafe extern "C" fn() -> *mut BackendArchive
        ),
        archive_write_free: load_symbol!(
            handle,
            "archive_write_free",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_write_close: load_symbol!(
            handle,
            "archive_write_close",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_write_set_bytes_per_block: load_symbol!(
            handle,
            "archive_write_set_bytes_per_block",
            unsafe extern "C" fn(*mut BackendArchive, c_int) -> c_int
        ),
        archive_write_get_bytes_per_block: load_symbol!(
            handle,
            "archive_write_get_bytes_per_block",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_write_set_bytes_in_last_block: load_symbol!(
            handle,
            "archive_write_set_bytes_in_last_block",
            unsafe extern "C" fn(*mut BackendArchive, c_int) -> c_int
        ),
        archive_write_get_bytes_in_last_block: load_symbol!(
            handle,
            "archive_write_get_bytes_in_last_block",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_write_set_skip_file: load_symbol!(
            handle,
            "archive_write_set_skip_file",
            unsafe extern "C" fn(*mut BackendArchive, LaInt64, LaInt64) -> c_int
        ),
        archive_write_add_filter: load_symbol!(
            handle,
            "archive_write_add_filter",
            unsafe extern "C" fn(*mut BackendArchive, c_int) -> c_int
        ),
        archive_write_add_filter_by_name: load_symbol!(
            handle,
            "archive_write_add_filter_by_name",
            unsafe extern "C" fn(*mut BackendArchive, *const c_char) -> c_int
        ),
        archive_write_add_filter_b64encode: load_symbol!(
            handle,
            "archive_write_add_filter_b64encode",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_write_add_filter_bzip2: load_symbol!(
            handle,
            "archive_write_add_filter_bzip2",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_write_add_filter_compress: load_symbol!(
            handle,
            "archive_write_add_filter_compress",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_write_add_filter_grzip: load_symbol!(
            handle,
            "archive_write_add_filter_grzip",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_write_add_filter_gzip: load_symbol!(
            handle,
            "archive_write_add_filter_gzip",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_write_add_filter_lrzip: load_symbol!(
            handle,
            "archive_write_add_filter_lrzip",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_write_add_filter_lz4: load_symbol!(
            handle,
            "archive_write_add_filter_lz4",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_write_add_filter_lzip: load_symbol!(
            handle,
            "archive_write_add_filter_lzip",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_write_add_filter_lzma: load_symbol!(
            handle,
            "archive_write_add_filter_lzma",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_write_add_filter_lzop: load_symbol!(
            handle,
            "archive_write_add_filter_lzop",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_write_add_filter_none: load_symbol!(
            handle,
            "archive_write_add_filter_none",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_write_add_filter_program: load_symbol!(
            handle,
            "archive_write_add_filter_program",
            unsafe extern "C" fn(*mut BackendArchive, *const c_char) -> c_int
        ),
        archive_write_add_filter_uuencode: load_symbol!(
            handle,
            "archive_write_add_filter_uuencode",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_write_add_filter_xz: load_symbol!(
            handle,
            "archive_write_add_filter_xz",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_write_add_filter_zstd: load_symbol!(
            handle,
            "archive_write_add_filter_zstd",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_write_set_format: load_symbol!(
            handle,
            "archive_write_set_format",
            unsafe extern "C" fn(*mut BackendArchive, c_int) -> c_int
        ),
        archive_write_set_format_by_name: load_symbol!(
            handle,
            "archive_write_set_format_by_name",
            unsafe extern "C" fn(*mut BackendArchive, *const c_char) -> c_int
        ),
        archive_write_set_format_ar_bsd: load_symbol!(
            handle,
            "archive_write_set_format_ar_bsd",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_write_set_format_ar_svr4: load_symbol!(
            handle,
            "archive_write_set_format_ar_svr4",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_write_set_format_cpio: load_symbol!(
            handle,
            "archive_write_set_format_cpio",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_write_set_format_cpio_bin: load_symbol!(
            handle,
            "archive_write_set_format_cpio_bin",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_write_set_format_cpio_newc: load_symbol!(
            handle,
            "archive_write_set_format_cpio_newc",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_write_set_format_cpio_odc: load_symbol!(
            handle,
            "archive_write_set_format_cpio_odc",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_write_set_format_cpio_pwb: load_symbol!(
            handle,
            "archive_write_set_format_cpio_pwb",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_write_set_format_gnutar: load_symbol!(
            handle,
            "archive_write_set_format_gnutar",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_write_set_format_pax: load_symbol!(
            handle,
            "archive_write_set_format_pax",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_write_set_format_pax_restricted: load_symbol!(
            handle,
            "archive_write_set_format_pax_restricted",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_write_set_format_raw: load_symbol!(
            handle,
            "archive_write_set_format_raw",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_write_set_format_shar: load_symbol!(
            handle,
            "archive_write_set_format_shar",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_write_set_format_shar_dump: load_symbol!(
            handle,
            "archive_write_set_format_shar_dump",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_write_set_format_ustar: load_symbol!(
            handle,
            "archive_write_set_format_ustar",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_write_set_format_v7tar: load_symbol!(
            handle,
            "archive_write_set_format_v7tar",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_write_set_format_filter_by_ext: load_symbol!(
            handle,
            "archive_write_set_format_filter_by_ext",
            unsafe extern "C" fn(*mut BackendArchive, *const c_char) -> c_int
        ),
        archive_write_set_format_filter_by_ext_def: load_symbol!(
            handle,
            "archive_write_set_format_filter_by_ext_def",
            unsafe extern "C" fn(*mut BackendArchive, *const c_char, *const c_char) -> c_int
        ),
        archive_write_open: load_symbol!(
            handle,
            "archive_write_open",
            unsafe extern "C" fn(
                *mut BackendArchive,
                *mut c_void,
                BackendOpenCallback,
                BackendWriteCallback,
                BackendCloseCallback,
            ) -> c_int
        ),
        archive_write_open2: load_symbol!(
            handle,
            "archive_write_open2",
            unsafe extern "C" fn(
                *mut BackendArchive,
                *mut c_void,
                BackendOpenCallback,
                BackendWriteCallback,
                BackendCloseCallback,
                BackendFreeCallback,
            ) -> c_int
        ),
        archive_write_open_filename: load_symbol!(
            handle,
            "archive_write_open_filename",
            unsafe extern "C" fn(*mut BackendArchive, *const c_char) -> c_int
        ),
        archive_write_open_memory: load_symbol!(
            handle,
            "archive_write_open_memory",
            unsafe extern "C" fn(*mut BackendArchive, *mut c_void, size_t, *mut size_t) -> c_int
        ),
        archive_write_header: load_symbol!(
            handle,
            "archive_write_header",
            unsafe extern "C" fn(*mut BackendArchive, *mut BackendEntry) -> c_int
        ),
        archive_write_data: load_symbol!(
            handle,
            "archive_write_data",
            unsafe extern "C" fn(*mut BackendArchive, *const c_void, size_t) -> LaSsize
        ),
        archive_write_data_block: load_symbol!(
            handle,
            "archive_write_data_block",
            unsafe extern "C" fn(*mut BackendArchive, *const c_void, size_t, LaInt64) -> LaSsize
        ),
        archive_write_finish_entry: load_symbol!(
            handle,
            "archive_write_finish_entry",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_write_fail: load_symbol!(
            handle,
            "archive_write_fail",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_write_set_filter_option: load_symbol!(
            handle,
            "archive_write_set_filter_option",
            unsafe extern "C" fn(
                *mut BackendArchive,
                *const c_char,
                *const c_char,
                *const c_char,
            ) -> c_int
        ),
        archive_write_set_format_option: load_symbol!(
            handle,
            "archive_write_set_format_option",
            unsafe extern "C" fn(
                *mut BackendArchive,
                *const c_char,
                *const c_char,
                *const c_char,
            ) -> c_int
        ),
        archive_write_set_option: load_symbol!(
            handle,
            "archive_write_set_option",
            unsafe extern "C" fn(
                *mut BackendArchive,
                *const c_char,
                *const c_char,
                *const c_char,
            ) -> c_int
        ),
        archive_write_set_options: load_symbol!(
            handle,
            "archive_write_set_options",
            unsafe extern "C" fn(*mut BackendArchive, *const c_char) -> c_int
        ),
        archive_write_set_passphrase: load_symbol!(
            handle,
            "archive_write_set_passphrase",
            unsafe extern "C" fn(*mut BackendArchive, *const c_char) -> c_int
        ),
        archive_write_disk_new: load_symbol!(
            handle,
            "archive_write_disk_new",
            unsafe extern "C" fn() -> *mut BackendArchive
        ),
        archive_write_disk_set_skip_file: load_symbol!(
            handle,
            "archive_write_disk_set_skip_file",
            unsafe extern "C" fn(*mut BackendArchive, LaInt64, LaInt64) -> c_int
        ),
        archive_write_disk_set_options: load_symbol!(
            handle,
            "archive_write_disk_set_options",
            unsafe extern "C" fn(*mut BackendArchive, c_int) -> c_int
        ),
        archive_write_disk_set_standard_lookup: load_symbol!(
            handle,
            "archive_write_disk_set_standard_lookup",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_write_disk_set_group_lookup: load_symbol!(
            handle,
            "archive_write_disk_set_group_lookup",
            unsafe extern "C" fn(
                *mut BackendArchive,
                *mut c_void,
                BackendWriteDiskLookupCallback,
                BackendWriteDiskCleanupCallback,
            ) -> c_int
        ),
        archive_write_disk_set_user_lookup: load_symbol!(
            handle,
            "archive_write_disk_set_user_lookup",
            unsafe extern "C" fn(
                *mut BackendArchive,
                *mut c_void,
                BackendWriteDiskLookupCallback,
                BackendWriteDiskCleanupCallback,
            ) -> c_int
        ),
        archive_write_disk_gid: load_symbol!(
            handle,
            "archive_write_disk_gid",
            unsafe extern "C" fn(*mut BackendArchive, *const c_char, LaInt64) -> LaInt64
        ),
        archive_write_disk_uid: load_symbol!(
            handle,
            "archive_write_disk_uid",
            unsafe extern "C" fn(*mut BackendArchive, *const c_char, LaInt64) -> LaInt64
        ),
        archive_read_disk_new: load_symbol!(
            handle,
            "archive_read_disk_new",
            unsafe extern "C" fn() -> *mut BackendArchive
        ),
        archive_read_disk_set_symlink_logical: load_symbol!(
            handle,
            "archive_read_disk_set_symlink_logical",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_read_disk_set_symlink_physical: load_symbol!(
            handle,
            "archive_read_disk_set_symlink_physical",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_read_disk_set_symlink_hybrid: load_symbol!(
            handle,
            "archive_read_disk_set_symlink_hybrid",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_read_disk_entry_from_file: load_symbol!(
            handle,
            "archive_read_disk_entry_from_file",
            unsafe extern "C" fn(
                *mut BackendArchive,
                *mut BackendEntry,
                c_int,
                *const c_void,
            ) -> c_int
        ),
        archive_read_disk_gname: load_symbol!(
            handle,
            "archive_read_disk_gname",
            unsafe extern "C" fn(*mut BackendArchive, LaInt64) -> *const c_char
        ),
        archive_read_disk_uname: load_symbol!(
            handle,
            "archive_read_disk_uname",
            unsafe extern "C" fn(*mut BackendArchive, LaInt64) -> *const c_char
        ),
        archive_read_disk_set_standard_lookup: load_symbol!(
            handle,
            "archive_read_disk_set_standard_lookup",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_read_disk_set_gname_lookup: load_symbol!(
            handle,
            "archive_read_disk_set_gname_lookup",
            unsafe extern "C" fn(
                *mut BackendArchive,
                *mut c_void,
                BackendReadDiskLookupCallback,
                BackendReadDiskCleanupCallback,
            ) -> c_int
        ),
        archive_read_disk_set_uname_lookup: load_symbol!(
            handle,
            "archive_read_disk_set_uname_lookup",
            unsafe extern "C" fn(
                *mut BackendArchive,
                *mut c_void,
                BackendReadDiskLookupCallback,
                BackendReadDiskCleanupCallback,
            ) -> c_int
        ),
        archive_read_disk_open: load_symbol!(
            handle,
            "archive_read_disk_open",
            unsafe extern "C" fn(*mut BackendArchive, *const c_char) -> c_int
        ),
        archive_read_disk_open_w: load_symbol!(
            handle,
            "archive_read_disk_open_w",
            unsafe extern "C" fn(*mut BackendArchive, *const wchar_t) -> c_int
        ),
        archive_read_disk_descend: load_symbol!(
            handle,
            "archive_read_disk_descend",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_read_disk_can_descend: load_symbol!(
            handle,
            "archive_read_disk_can_descend",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_read_disk_current_filesystem: load_symbol!(
            handle,
            "archive_read_disk_current_filesystem",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_read_disk_current_filesystem_is_synthetic: load_symbol!(
            handle,
            "archive_read_disk_current_filesystem_is_synthetic",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_read_disk_current_filesystem_is_remote: load_symbol!(
            handle,
            "archive_read_disk_current_filesystem_is_remote",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_read_disk_set_atime_restored: load_symbol!(
            handle,
            "archive_read_disk_set_atime_restored",
            unsafe extern "C" fn(*mut BackendArchive) -> c_int
        ),
        archive_read_disk_set_behavior: load_symbol!(
            handle,
            "archive_read_disk_set_behavior",
            unsafe extern "C" fn(*mut BackendArchive, c_int) -> c_int
        ),
        archive_read_disk_set_matching: load_symbol!(
            handle,
            "archive_read_disk_set_matching",
            unsafe extern "C" fn(
                *mut BackendArchive,
                *mut BackendArchive,
                BackendReadDiskExcludedCallback,
                *mut c_void,
            ) -> c_int
        ),
        archive_read_disk_set_metadata_filter_callback: load_symbol!(
            handle,
            "archive_read_disk_set_metadata_filter_callback",
            unsafe extern "C" fn(
                *mut BackendArchive,
                BackendReadDiskMetadataFilterCallback,
                *mut c_void,
            ) -> c_int
        ),
    }
}

static API: OnceLock<Api> = OnceLock::new();

pub fn api() -> &'static Api {
    API.get_or_init(load)
}
