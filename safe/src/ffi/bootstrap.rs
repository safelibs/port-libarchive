use std::ffi::c_char;
use std::os::raw::c_int;

use libc::{size_t, wchar_t};

use crate::common::panic_boundary::{ffi_int, ffi_ptr};
use crate::common::state::{
    alloc_archive, read_archive_open_filename, read_archive_open_filename_w,
    read_archive_open_filenames, read_archive_support_format, ArchiveKind,
};
use crate::ffi::archive;

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
pub extern "C" fn archive_read_support_format_raw(a: *mut archive) -> c_int {
    ffi_int(crate::common::error::ARCHIVE_FATAL, || unsafe {
        read_archive_support_format(a)
    })
}

#[no_mangle]
pub extern "C" fn archive_read_support_format_empty(a: *mut archive) -> c_int {
    ffi_int(crate::common::error::ARCHIVE_FATAL, || unsafe {
        read_archive_support_format(a)
    })
}

#[no_mangle]
pub extern "C" fn archive_read_open_filename(
    a: *mut archive,
    path: *const c_char,
    _block_size: size_t,
) -> c_int {
    ffi_int(crate::common::error::ARCHIVE_FATAL, || unsafe {
        read_archive_open_filename(a, path)
    })
}

#[no_mangle]
pub extern "C" fn archive_read_open_filenames(
    a: *mut archive,
    paths: *const *const c_char,
    _block_size: size_t,
) -> c_int {
    ffi_int(crate::common::error::ARCHIVE_FATAL, || unsafe {
        read_archive_open_filenames(a, paths)
    })
}

#[no_mangle]
pub extern "C" fn archive_read_open_filename_w(
    a: *mut archive,
    path: *const wchar_t,
    _block_size: size_t,
) -> c_int {
    ffi_int(crate::common::error::ARCHIVE_FATAL, || unsafe {
        read_archive_open_filename_w(a, path)
    })
}
