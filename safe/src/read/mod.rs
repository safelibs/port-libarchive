use std::ffi::{c_char, c_int, c_void, CString};
use std::ptr;

use libc::{size_t, wchar_t};

use crate::common::backend::{api as backend_api, BackendArchive, BackendEntry};
use crate::common::error::{ARCHIVE_EOF, ARCHIVE_FATAL, ARCHIVE_OK};
use crate::common::helpers::{from_optional_c_str, from_optional_wide, to_wide_null};
use crate::common::panic_boundary::ffi_int;
use crate::common::state::{
    alloc_archive, archive_check_magic, archive_magic, clear_error, free_archive,
    read_disk_from_archive, read_from_archive, sync_backend_core, ArchiveKind,
    ReadFilterRegistration, ReadFormatRegistration, ReadSourceConfig,
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

unsafe fn apply_read_filter(
    handle: &mut crate::common::state::ReadArchiveHandle,
    filter: ReadFilterRegistration,
) -> c_int {
    match filter {
        ReadFilterRegistration::All => (backend_api().archive_read_support_filter_all)(handle.backend),
        ReadFilterRegistration::None => (backend_api().archive_read_support_filter_none)(handle.backend),
        ReadFilterRegistration::Bzip2 => {
            (backend_api().archive_read_support_filter_bzip2)(handle.backend)
        }
        ReadFilterRegistration::Compress => {
            (backend_api().archive_read_support_filter_compress)(handle.backend)
        }
        ReadFilterRegistration::Gzip => {
            (backend_api().archive_read_support_filter_gzip)(handle.backend)
        }
        ReadFilterRegistration::Grzip => {
            (backend_api().archive_read_support_filter_grzip)(handle.backend)
        }
        ReadFilterRegistration::Lrzip => {
            (backend_api().archive_read_support_filter_lrzip)(handle.backend)
        }
        ReadFilterRegistration::Lz4 => (backend_api().archive_read_support_filter_lz4)(handle.backend),
        ReadFilterRegistration::Lzip => {
            (backend_api().archive_read_support_filter_lzip)(handle.backend)
        }
        ReadFilterRegistration::Lzma => {
            (backend_api().archive_read_support_filter_lzma)(handle.backend)
        }
        ReadFilterRegistration::Lzop => {
            (backend_api().archive_read_support_filter_lzop)(handle.backend)
        }
        ReadFilterRegistration::Xz => (backend_api().archive_read_support_filter_xz)(handle.backend),
        ReadFilterRegistration::Zstd => {
            (backend_api().archive_read_support_filter_zstd)(handle.backend)
        }
    }
}

unsafe fn apply_read_format(
    handle: &mut crate::common::state::ReadArchiveHandle,
    format: ReadFormatRegistration,
) -> c_int {
    match format {
        ReadFormatRegistration::All => (backend_api().archive_read_support_format_all)(handle.backend),
        ReadFormatRegistration::Empty => {
            (backend_api().archive_read_support_format_empty)(handle.backend)
        }
        ReadFormatRegistration::Raw => (backend_api().archive_read_support_format_raw)(handle.backend),
    }
}

unsafe fn ensure_read_backend_open(
    handle: &mut crate::common::state::ReadArchiveHandle,
) -> c_int {
    if handle.backend.is_null() {
        handle.backend = (backend_api().archive_read_new)();
        if handle.backend.is_null() {
            crate::common::state::set_error_string(
                &mut handle.core,
                libc::ENOMEM,
                "failed to create reader backend".to_string(),
            );
            return ARCHIVE_FATAL;
        }
        let filters = handle.filter_registrations.clone();
        for filter in filters {
            let status = apply_read_filter(handle, filter);
            if status != ARCHIVE_OK {
                return status;
            }
        }
        let formats = handle.format_registrations.clone();
        for format in formats {
            let status = apply_read_format(handle, format);
            if status != ARCHIVE_OK {
                return status;
            }
        }
    }
    if handle.backend_opened {
        return ARCHIVE_OK;
    }

    let status = match &handle.source {
        ReadSourceConfig::None => ARCHIVE_OK,
        ReadSourceConfig::Memory { buffer, size } => {
            (backend_api().archive_read_open_memory)(handle.backend, *buffer, *size)
        }
        ReadSourceConfig::Filename { path, block_size } => {
            let path = CString::new(path.as_str()).expect("path");
            (backend_api().archive_read_open_filename)(handle.backend, path.as_ptr(), *block_size)
        }
        ReadSourceConfig::Filenames { paths, block_size } => {
            let Some(first) = paths.first() else {
                return ARCHIVE_FATAL;
            };
            let path = CString::new(first.as_str()).expect("path");
            (backend_api().archive_read_open_filename)(handle.backend, path.as_ptr(), *block_size)
        }
        ReadSourceConfig::FilenameW { path, block_size } => {
            let wide = to_wide_null(path);
            (backend_api().archive_read_open_filename_w)(handle.backend, wide.as_ptr(), *block_size)
        }
    };
    if status == ARCHIVE_OK {
        handle.backend_opened = true;
    }
    status
}

fn push_or_apply_filter(
    handle: &mut crate::common::state::ReadArchiveHandle,
    registration: ReadFilterRegistration,
) -> c_int {
    unsafe {
        if handle.backend.is_null() {
            handle.filter_registrations.push(registration);
            ARCHIVE_OK
        } else {
            apply_read_filter(handle, registration)
        }
    }
}

fn push_or_apply_format(
    handle: &mut crate::common::state::ReadArchiveHandle,
    registration: ReadFormatRegistration,
) -> c_int {
    unsafe {
        if handle.backend.is_null() {
            handle.format_registrations.push(registration);
            ARCHIVE_OK
        } else {
            apply_read_format(handle, registration)
        }
    }
}

macro_rules! read_filter_support {
    ($name:ident, $registration:expr) => {
        #[no_mangle]
        pub extern "C" fn $name(a: *mut archive) -> c_int {
            ffi_int(ARCHIVE_FATAL, || unsafe {
                let Some(handle) = validate_read(a, stringify!($name)) else {
                    return ARCHIVE_FATAL;
                };
                clear_error(&mut handle.core);
                let status = push_or_apply_filter(handle, $registration);
                sync_backend_core(a);
                status
            })
        }
    };
}

read_filter_support!(archive_read_support_filter_all, ReadFilterRegistration::All);
read_filter_support!(archive_read_support_filter_none, ReadFilterRegistration::None);
read_filter_support!(archive_read_support_filter_bzip2, ReadFilterRegistration::Bzip2);
read_filter_support!(
    archive_read_support_filter_compress,
    ReadFilterRegistration::Compress
);
read_filter_support!(archive_read_support_filter_gzip, ReadFilterRegistration::Gzip);
read_filter_support!(archive_read_support_filter_grzip, ReadFilterRegistration::Grzip);
read_filter_support!(archive_read_support_filter_lrzip, ReadFilterRegistration::Lrzip);
read_filter_support!(archive_read_support_filter_lz4, ReadFilterRegistration::Lz4);
read_filter_support!(archive_read_support_filter_lzip, ReadFilterRegistration::Lzip);
read_filter_support!(archive_read_support_filter_lzma, ReadFilterRegistration::Lzma);
read_filter_support!(archive_read_support_filter_lzop, ReadFilterRegistration::Lzop);
read_filter_support!(archive_read_support_filter_xz, ReadFilterRegistration::Xz);
read_filter_support!(archive_read_support_filter_zstd, ReadFilterRegistration::Zstd);

macro_rules! read_format_support {
    ($name:ident, $registration:expr) => {
        #[no_mangle]
        pub extern "C" fn $name(a: *mut archive) -> c_int {
            ffi_int(ARCHIVE_FATAL, || unsafe {
                let Some(handle) = validate_read(a, stringify!($name)) else {
                    return ARCHIVE_FATAL;
                };
                clear_error(&mut handle.core);
                let status = push_or_apply_format(handle, $registration);
                sync_backend_core(a);
                status
            })
        }
    };
}

read_format_support!(archive_read_support_format_all, ReadFormatRegistration::All);
read_format_support!(archive_read_support_format_empty, ReadFormatRegistration::Empty);
read_format_support!(archive_read_support_format_raw, ReadFormatRegistration::Raw);

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
        handle.source = ReadSourceConfig::Memory { buffer, size };
        if handle.backend.is_null() {
            ARCHIVE_OK
        } else {
            ensure_read_backend_open(handle)
        }
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
        let Some(path) = from_optional_c_str(path) else {
            return ARCHIVE_FATAL;
        };
        clear_error(&mut handle.core);
        handle.source = ReadSourceConfig::Filename { path, block_size };
        if handle.backend.is_null() {
            ARCHIVE_OK
        } else {
            ensure_read_backend_open(handle)
        }
    })
}

#[no_mangle]
pub extern "C" fn archive_read_open_filenames(
    a: *mut archive,
    paths: *const *const c_char,
    block_size: size_t,
) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_read(a, "archive_read_open_filenames") else {
            return ARCHIVE_FATAL;
        };
        if paths.is_null() {
            return ARCHIVE_FATAL;
        }
        let mut values = Vec::new();
        let mut current = paths;
        while !(*current).is_null() {
            if let Some(path) = from_optional_c_str(*current) {
                values.push(path);
            }
            current = current.add(1);
        }
        handle.source = ReadSourceConfig::Filenames {
            paths: values,
            block_size,
        };
        if handle.backend.is_null() {
            ARCHIVE_OK
        } else {
            ensure_read_backend_open(handle)
        }
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
        let Some(path) = from_optional_wide(path) else {
            return ARCHIVE_FATAL;
        };
        clear_error(&mut handle.core);
        handle.source = ReadSourceConfig::FilenameW { path, block_size };
        if handle.backend.is_null() {
            ARCHIVE_OK
        } else {
            ensure_read_backend_open(handle)
        }
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
        match &mut handle {
            ReadLike::Archive(handle) => {
                let status = ensure_read_backend_open(handle);
                if status != ARCHIVE_OK {
                    return status;
                }
            }
            ReadLike::Disk(_) => {}
        }
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
        match &mut handle {
            ReadLike::Archive(handle) => {
                let status = ensure_read_backend_open(handle);
                if status != ARCHIVE_OK {
                    return status;
                }
            }
            ReadLike::Disk(_) => {}
        }
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
        match &mut handle {
            ReadLike::Archive(handle) => {
                let status = ensure_read_backend_open(handle);
                if status != ARCHIVE_OK {
                    return status as isize;
                }
            }
            ReadLike::Disk(_) => {}
        }
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
        match &mut handle {
            ReadLike::Archive(handle) => {
                let status = ensure_read_backend_open(handle);
                if status != ARCHIVE_OK {
                    return status;
                }
            }
            ReadLike::Disk(_) => {}
        }
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
        let disk = alloc_archive(ArchiveKind::WriteDisk);
        if disk.is_null() {
            return ARCHIVE_FATAL;
        }
        let mut status = crate::disk::archive_write_disk_set_options(disk, flags);
        if status == ARCHIVE_OK {
            status = archive_read_extract2(a, entry, disk);
        }
        let free_status = free_archive(disk);
        if status == ARCHIVE_OK {
            free_status
        } else {
            status
        }
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
        if crate::common::state::write_disk_from_archive(disk).is_none() {
            return ARCHIVE_FATAL;
        }
        let entry_ptr = if !entry.is_null() {
            entry
        } else if !handle.entry.is_null() {
            handle.entry
        } else {
            return ARCHIVE_FATAL;
        };

        let status = crate::write::archive_write_header(disk, entry_ptr);
        if status != ARCHIVE_OK {
            return status;
        }

        loop {
            let mut block = ptr::null();
            let mut block_size = 0usize;
            let mut block_offset = 0i64;
            let status = archive_read_data_block(a, &mut block, &mut block_size, &mut block_offset);
            if status == ARCHIVE_EOF {
                return crate::write::archive_write_finish_entry(disk);
            }
            if status != ARCHIVE_OK {
                return status;
            }
            let write_status =
                crate::write::archive_write_data_block(disk, block, block_size, block_offset);
            if write_status < 0 {
                return write_status as c_int;
            }
        }
    })
}
