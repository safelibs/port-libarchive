use std::ffi::{c_char, c_int, c_void};

use libc::{stat, wchar_t};

use crate::common::backend::{BackendReadDiskCleanupCallback, BackendReadDiskLookupCallback};
use crate::common::state::{ReadDiskExcludedCallback, ReadDiskMetadataFilterCallback};
use crate::ffi::{archive, archive_entry};

extern "C" {
    pub fn archive_read_disk_new() -> *mut archive;
    pub fn archive_read_disk_set_symlink_logical(a: *mut archive) -> c_int;
    pub fn archive_read_disk_set_symlink_physical(a: *mut archive) -> c_int;
    pub fn archive_read_disk_set_symlink_hybrid(a: *mut archive) -> c_int;
    pub fn archive_read_disk_entry_from_file(
        a: *mut archive,
        entry: *mut archive_entry,
        fd: c_int,
        st: *const stat,
    ) -> c_int;
    pub fn archive_read_disk_gname(a: *mut archive, gid: i64) -> *const c_char;
    pub fn archive_read_disk_uname(a: *mut archive, uid: i64) -> *const c_char;
    pub fn archive_read_disk_set_standard_lookup(a: *mut archive) -> c_int;
    pub fn archive_read_disk_set_gname_lookup(
        a: *mut archive,
        private_data: *mut c_void,
        lookup: BackendReadDiskLookupCallback,
        cleanup: BackendReadDiskCleanupCallback,
    ) -> c_int;
    pub fn archive_read_disk_set_uname_lookup(
        a: *mut archive,
        private_data: *mut c_void,
        lookup: BackendReadDiskLookupCallback,
        cleanup: BackendReadDiskCleanupCallback,
    ) -> c_int;
    pub fn archive_read_disk_open(a: *mut archive, path: *const c_char) -> c_int;
    pub fn archive_read_disk_open_w(a: *mut archive, path: *const wchar_t) -> c_int;
    pub fn archive_read_disk_descend(a: *mut archive) -> c_int;
    pub fn archive_read_disk_can_descend(a: *mut archive) -> c_int;
    pub fn archive_read_disk_current_filesystem(a: *mut archive) -> c_int;
    pub fn archive_read_disk_current_filesystem_is_synthetic(a: *mut archive) -> c_int;
    pub fn archive_read_disk_current_filesystem_is_remote(a: *mut archive) -> c_int;
    pub fn archive_read_disk_set_atime_restored(a: *mut archive) -> c_int;
    pub fn archive_read_disk_set_behavior(a: *mut archive, flags: c_int) -> c_int;
    pub fn archive_read_disk_set_matching(
        a: *mut archive,
        matching: *mut archive,
        excluded: Option<ReadDiskExcludedCallback>,
        client_data: *mut c_void,
    ) -> c_int;
    pub fn archive_read_disk_set_metadata_filter_callback(
        a: *mut archive,
        callback: Option<ReadDiskMetadataFilterCallback>,
        client_data: *mut c_void,
    ) -> c_int;
}
