use std::ffi::{c_char, c_int, c_void};

use crate::common::backend::{BackendWriteDiskCleanupCallback, BackendWriteDiskLookupCallback};
use crate::ffi::archive;

unsafe extern "C" {
    pub fn archive_write_disk_new() -> *mut archive;
    pub fn archive_write_disk_set_skip_file(a: *mut archive, dev: i64, ino: i64) -> c_int;
    pub fn archive_write_disk_set_options(a: *mut archive, flags: c_int) -> c_int;
    pub fn archive_write_disk_set_standard_lookup(a: *mut archive) -> c_int;
    pub fn archive_write_disk_set_group_lookup(
        a: *mut archive,
        private_data: *mut c_void,
        lookup: BackendWriteDiskLookupCallback,
        cleanup: BackendWriteDiskCleanupCallback,
    ) -> c_int;
    pub fn archive_write_disk_set_user_lookup(
        a: *mut archive,
        private_data: *mut c_void,
        lookup: BackendWriteDiskLookupCallback,
        cleanup: BackendWriteDiskCleanupCallback,
    ) -> c_int;
    pub fn archive_write_disk_gid(a: *mut archive, name: *const c_char, gid: i64) -> i64;
    pub fn archive_write_disk_uid(a: *mut archive, name: *const c_char, uid: i64) -> i64;
}
