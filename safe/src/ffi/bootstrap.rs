use crate::common::panic_boundary::ffi_ptr;
use crate::common::state::{alloc_archive, ArchiveKind};
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
