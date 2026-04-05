use std::os::raw::{c_char, c_int};

use crate::common::error::{ARCHIVE_FATAL, ARCHIVE_OK};
use crate::common::panic_boundary::{ffi_const_ptr, ffi_int, ffi_ptr, ffi_void};
use crate::common::state::{alloc_archive, alloc_entry, free_archive, free_entry, ArchiveKind};
use crate::ffi::{archive, archive_entry};
use crate::generated::{
    LIBARCHIVE_VERSION_DETAILS_BYTES, LIBARCHIVE_VERSION_NUMBER, LIBARCHIVE_VERSION_STRING_BYTES,
};

#[no_mangle]
pub extern "C" fn archive_version_number() -> c_int {
    ffi_int(ARCHIVE_FATAL, || LIBARCHIVE_VERSION_NUMBER)
}

#[no_mangle]
pub extern "C" fn archive_version_string() -> *const c_char {
    ffi_const_ptr(|| LIBARCHIVE_VERSION_STRING_BYTES.as_ptr().cast::<c_char>())
}

#[no_mangle]
pub extern "C" fn archive_version_details() -> *const c_char {
    ffi_const_ptr(|| LIBARCHIVE_VERSION_DETAILS_BYTES.as_ptr().cast::<c_char>())
}

#[no_mangle]
pub extern "C" fn archive_read_new() -> *mut archive {
    ffi_ptr(|| alloc_archive(ArchiveKind::Read))
}

#[no_mangle]
pub extern "C" fn archive_write_new() -> *mut archive {
    ffi_ptr(|| alloc_archive(ArchiveKind::Write))
}

#[no_mangle]
pub extern "C" fn archive_read_disk_new() -> *mut archive {
    ffi_ptr(|| alloc_archive(ArchiveKind::ReadDisk))
}

#[no_mangle]
pub extern "C" fn archive_write_disk_new() -> *mut archive {
    ffi_ptr(|| alloc_archive(ArchiveKind::WriteDisk))
}

#[no_mangle]
pub extern "C" fn archive_match_new() -> *mut archive {
    ffi_ptr(|| alloc_archive(ArchiveKind::Match))
}

#[no_mangle]
pub extern "C" fn archive_entry_new() -> *mut archive_entry {
    ffi_ptr(|| alloc_entry(std::ptr::null_mut()))
}

#[no_mangle]
pub extern "C" fn archive_entry_new2(source_archive: *mut archive) -> *mut archive_entry {
    ffi_ptr(|| alloc_entry(source_archive))
}

#[no_mangle]
pub extern "C" fn archive_free(handle: *mut archive) -> c_int {
    ffi_int(ARCHIVE_FATAL, || {
        unsafe {
            free_archive(handle);
        }
        ARCHIVE_OK
    })
}

#[no_mangle]
pub extern "C" fn archive_read_free(handle: *mut archive) -> c_int {
    ffi_int(ARCHIVE_FATAL, || archive_free(handle))
}

#[no_mangle]
pub extern "C" fn archive_write_free(handle: *mut archive) -> c_int {
    ffi_int(ARCHIVE_FATAL, || archive_free(handle))
}

#[no_mangle]
pub extern "C" fn archive_match_free(handle: *mut archive) -> c_int {
    ffi_int(ARCHIVE_FATAL, || archive_free(handle))
}

#[no_mangle]
pub extern "C" fn archive_entry_free(entry: *mut archive_entry) {
    ffi_void(|| unsafe {
        free_entry(entry);
    });
}
