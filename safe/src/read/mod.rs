use std::ffi::{c_char, c_int, c_void};
use std::ptr;

use libc::{size_t, wchar_t};

use crate::common::backend::{api as backend_api, BackendArchive, BackendEntry};
use crate::common::error::{ARCHIVE_EOF, ARCHIVE_FATAL, ARCHIVE_OK};
use crate::common::panic_boundary::ffi_int;
use crate::common::state::{
    archive_check_magic, archive_magic, clear_error, read_disk_from_archive, read_from_archive,
    sync_backend_core,
};
use crate::disk::{backend_entry_to_custom, custom_entry_to_backend};
use crate::ffi::{archive, archive_entry};

enum ReadLike<'a> {
    Archive(&'a mut crate::common::state::ReadArchiveHandle),
    Disk(&'a mut crate::common::state::ReadDiskArchiveHandle),
}

impl<'a> ReadLike<'a> {
    unsafe fn from_archive(a: *mut archive, function: &str) -> Option<Self> {
        match archive_magic(a) {
            crate::common::error::ARCHIVE_READ_MAGIC => {
                if archive_check_magic(
                    a,
                    crate::common::error::ARCHIVE_READ_MAGIC,
                    crate::common::error::ARCHIVE_STATE_ANY,
                    function,
                ) == ARCHIVE_FATAL
                {
                    return None;
                }
                read_from_archive(a).map(Self::Archive)
            }
            crate::common::error::ARCHIVE_READ_DISK_MAGIC => {
                if archive_check_magic(
                    a,
                    crate::common::error::ARCHIVE_READ_DISK_MAGIC,
                    crate::common::error::ARCHIVE_STATE_ANY,
                    function,
                ) == ARCHIVE_FATAL
                {
                    return None;
                }
                read_disk_from_archive(a).map(Self::Disk)
            }
            _ => None,
        }
    }

    fn backend(&mut self) -> *mut BackendArchive {
        match self {
            Self::Archive(handle) => handle.backend,
            Self::Disk(handle) => handle.backend,
        }
    }

    fn core(&mut self) -> &mut crate::common::state::ArchiveCore {
        match self {
            Self::Archive(handle) => &mut handle.core,
            Self::Disk(handle) => &mut handle.core,
        }
    }

    fn custom_entry_slot(&mut self) -> &mut *mut archive_entry {
        match self {
            Self::Archive(handle) => &mut handle.entry,
            Self::Disk(handle) => &mut handle.entry,
        }
    }

    fn backend_entry_slot(&mut self) -> &mut *mut BackendEntry {
        match self {
            Self::Archive(handle) => &mut handle.current_entry,
            Self::Disk(handle) => &mut handle.current_entry,
        }
    }
}

fn validate_read(
    a: *mut archive,
    function: &str,
) -> Option<&'static mut crate::common::state::ReadArchiveHandle> {
    unsafe {
        if archive_check_magic(
            a,
            crate::common::error::ARCHIVE_READ_MAGIC,
            crate::common::error::ARCHIVE_STATE_ANY,
            function,
        ) == ARCHIVE_FATAL
        {
            return None;
        }
        read_from_archive(a)
    }
}

macro_rules! read_filter_support {
    ($name:ident, $backend:ident) => {
        #[no_mangle]
        pub extern "C" fn $name(a: *mut archive) -> c_int {
            ffi_int(ARCHIVE_FATAL, || unsafe {
                let Some(handle) = validate_read(a, stringify!($name)) else {
                    return ARCHIVE_FATAL;
                };
                clear_error(&mut handle.core);
                let status = (backend_api().$backend)(handle.backend);
                sync_backend_core(a);
                status
            })
        }
    };
}

read_filter_support!(
    archive_read_support_filter_all,
    archive_read_support_filter_all
);
read_filter_support!(
    archive_read_support_filter_none,
    archive_read_support_filter_none
);
read_filter_support!(
    archive_read_support_filter_bzip2,
    archive_read_support_filter_bzip2
);
read_filter_support!(
    archive_read_support_filter_compress,
    archive_read_support_filter_compress
);
read_filter_support!(
    archive_read_support_filter_gzip,
    archive_read_support_filter_gzip
);
read_filter_support!(
    archive_read_support_filter_grzip,
    archive_read_support_filter_grzip
);
read_filter_support!(
    archive_read_support_filter_lrzip,
    archive_read_support_filter_lrzip
);
read_filter_support!(
    archive_read_support_filter_lz4,
    archive_read_support_filter_lz4
);
read_filter_support!(
    archive_read_support_filter_lzip,
    archive_read_support_filter_lzip
);
read_filter_support!(
    archive_read_support_filter_lzma,
    archive_read_support_filter_lzma
);
read_filter_support!(
    archive_read_support_filter_lzop,
    archive_read_support_filter_lzop
);
read_filter_support!(
    archive_read_support_filter_xz,
    archive_read_support_filter_xz
);
read_filter_support!(
    archive_read_support_filter_zstd,
    archive_read_support_filter_zstd
);

macro_rules! read_format_support {
    ($name:ident, $backend:ident) => {
        #[no_mangle]
        pub extern "C" fn $name(a: *mut archive) -> c_int {
            ffi_int(ARCHIVE_FATAL, || unsafe {
                let Some(handle) = validate_read(a, stringify!($name)) else {
                    return ARCHIVE_FATAL;
                };
                clear_error(&mut handle.core);
                let status = (backend_api().$backend)(handle.backend);
                sync_backend_core(a);
                status
            })
        }
    };
}

read_format_support!(
    archive_read_support_format_all,
    archive_read_support_format_all
);
read_format_support!(
    archive_read_support_format_empty,
    archive_read_support_format_empty
);
read_format_support!(
    archive_read_support_format_raw,
    archive_read_support_format_raw
);

#[no_mangle]
pub extern "C" fn archive_read_open_memory(
    a: *mut archive,
    buffer: *const c_void,
    size: size_t,
) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_read(a, "archive_read_open_memory") else {
            return ARCHIVE_FATAL;
        };
        clear_error(&mut handle.core);
        let status = (backend_api().archive_read_open_memory)(handle.backend, buffer, size);
        sync_backend_core(a);
        status
    })
}

#[no_mangle]
pub extern "C" fn archive_read_open_filename(
    a: *mut archive,
    path: *const c_char,
    block_size: size_t,
) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_read(a, "archive_read_open_filename") else {
            return ARCHIVE_FATAL;
        };
        clear_error(&mut handle.core);
        let status = (backend_api().archive_read_open_filename)(handle.backend, path, block_size);
        sync_backend_core(a);
        status
    })
}

#[no_mangle]
pub extern "C" fn archive_read_open_filenames(
    a: *mut archive,
    paths: *const *const c_char,
    block_size: size_t,
) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        if paths.is_null() {
            return ARCHIVE_FATAL;
        }
        archive_read_open_filename(a, *paths, block_size)
    })
}

#[no_mangle]
pub extern "C" fn archive_read_open_filename_w(
    a: *mut archive,
    path: *const wchar_t,
    block_size: size_t,
) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_read(a, "archive_read_open_filename_w") else {
            return ARCHIVE_FATAL;
        };
        clear_error(&mut handle.core);
        let status = (backend_api().archive_read_open_filename_w)(handle.backend, path, block_size);
        sync_backend_core(a);
        status
    })
}

#[no_mangle]
pub extern "C" fn archive_read_next_header(
    a: *mut archive,
    entry: *mut *mut archive_entry,
) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(mut handle) = ReadLike::from_archive(a, "archive_read_next_header") else {
            return ARCHIVE_FATAL;
        };
        clear_error(handle.core());
        let mut backend_entry = ptr::null_mut();
        let status = (backend_api().archive_read_next_header)(handle.backend(), &mut backend_entry);
        *handle.backend_entry_slot() = backend_entry;
        if status == ARCHIVE_OK {
            if (*handle.custom_entry_slot()).is_null() {
                *handle.custom_entry_slot() =
                    crate::entry::internal::new_raw_entry(ptr::null_mut());
            }
            if (*handle.custom_entry_slot()).is_null() {
                return ARCHIVE_FATAL;
            }
            let convert_status =
                backend_entry_to_custom(backend_entry, *handle.custom_entry_slot());
            if convert_status != ARCHIVE_OK {
                return convert_status;
            }
            if !entry.is_null() {
                *entry = *handle.custom_entry_slot();
            }
        } else if !entry.is_null() {
            *entry = ptr::null_mut();
        }
        sync_backend_core(a);
        status
    })
}

#[no_mangle]
pub extern "C" fn archive_read_next_header2(a: *mut archive, entry: *mut archive_entry) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        if entry.is_null() {
            return ARCHIVE_FATAL;
        }
        let Some(mut handle) = ReadLike::from_archive(a, "archive_read_next_header2") else {
            return ARCHIVE_FATAL;
        };
        clear_error(handle.core());
        let mut backend_entry = ptr::null_mut();
        let status = (backend_api().archive_read_next_header)(handle.backend(), &mut backend_entry);
        *handle.backend_entry_slot() = backend_entry;
        if status == ARCHIVE_OK {
            let convert_status = backend_entry_to_custom(backend_entry, entry);
            if convert_status != ARCHIVE_OK {
                return convert_status;
            }
        }
        sync_backend_core(a);
        status
    })
}

#[no_mangle]
pub extern "C" fn archive_read_data(a: *mut archive, buffer: *mut c_void, size: size_t) -> isize {
    unsafe {
        let Some(mut handle) = ReadLike::from_archive(a, "archive_read_data") else {
            return ARCHIVE_FATAL as isize;
        };
        let status = (backend_api().archive_read_data)(handle.backend(), buffer, size);
        sync_backend_core(a);
        status
    }
}

#[no_mangle]
pub extern "C" fn archive_read_data_block(
    a: *mut archive,
    buffer: *mut *const c_void,
    size: *mut size_t,
    offset: *mut i64,
) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(mut handle) = ReadLike::from_archive(a, "archive_read_data_block") else {
            return ARCHIVE_FATAL;
        };
        let status =
            (backend_api().archive_read_data_block)(handle.backend(), buffer, size, offset);
        sync_backend_core(a);
        status
    })
}

#[no_mangle]
pub extern "C" fn archive_read_extract(
    a: *mut archive,
    entry: *mut archive_entry,
    flags: c_int,
) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_read(a, "archive_read_extract") else {
            return ARCHIVE_FATAL;
        };
        let backend_entry = if !handle.current_entry.is_null() {
            handle.current_entry
        } else {
            let backend = (backend_api().archive_entry_new)();
            if backend.is_null() {
                return ARCHIVE_FATAL;
            }
            let status = custom_entry_to_backend(entry, backend);
            if status != ARCHIVE_OK {
                (backend_api().archive_entry_free)(backend);
                return status;
            }
            let result = (backend_api().archive_read_extract)(handle.backend, backend, flags);
            (backend_api().archive_entry_free)(backend);
            sync_backend_core(a);
            return result;
        };
        let status = (backend_api().archive_read_extract)(handle.backend, backend_entry, flags);
        sync_backend_core(a);
        status
    })
}

#[no_mangle]
pub extern "C" fn archive_read_extract2(
    a: *mut archive,
    entry: *mut archive_entry,
    disk: *mut archive,
) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_read(a, "archive_read_extract2") else {
            return ARCHIVE_FATAL;
        };
        let Some(disk_handle) = crate::common::state::write_disk_from_archive(disk) else {
            return ARCHIVE_FATAL;
        };
        let backend_entry = if !handle.current_entry.is_null() {
            handle.current_entry
        } else {
            let backend = (backend_api().archive_entry_new)();
            if backend.is_null() {
                return ARCHIVE_FATAL;
            }
            let status = custom_entry_to_backend(entry, backend);
            if status != ARCHIVE_OK {
                (backend_api().archive_entry_free)(backend);
                return status;
            }
            let result =
                (backend_api().archive_read_extract2)(handle.backend, backend, disk_handle.backend);
            (backend_api().archive_entry_free)(backend);
            sync_backend_core(a);
            return result;
        };
        let status = (backend_api().archive_read_extract2)(
            handle.backend,
            backend_entry,
            disk_handle.backend,
        );
        sync_backend_core(a);
        status
    })
}
