use std::ffi::{c_char, c_int, c_void, CString};
use std::path::{Component, Path, PathBuf};
use std::ptr;

use libc::size_t;

use crate::common::backend::{api as backend_api, BackendArchive, BackendEntry};
use crate::common::error::{ARCHIVE_FAILED, ARCHIVE_FATAL, ARCHIVE_OK};
use crate::common::helpers::from_optional_c_str;
use crate::common::panic_boundary::ffi_int;
use crate::common::state::{
    archive_check_magic, archive_magic, clear_error, set_error_string, sync_backend_core,
    write_disk_from_archive, write_from_archive, ArchiveCloseCallback, ArchiveFreeCallback,
    ArchiveOpenCallback, ArchiveWriteCallback, WriteFilterConfig, WriteFormatConfig,
    WriteOpenConfig, WriteOptionConfig,
};
use crate::disk::custom_entry_to_backend;
use crate::entry::internal::{from_raw as entry_from_raw, AE_IFLNK, AE_IFMT};
use crate::ffi::{archive, archive_entry};

const ARCHIVE_EXTRACT_UNLINK: c_int = 0x0010;
const ARCHIVE_EXTRACT_SECURE_SYMLINKS: c_int = 0x0100;
const ARCHIVE_EXTRACT_SECURE_NODOTDOT: c_int = 0x0200;
const ARCHIVE_EXTRACT_SECURE_NOABSOLUTEPATHS: c_int = 0x10000;

enum WriteLike<'a> {
    Archive(&'a mut crate::common::state::WriteArchiveHandle),
    Disk(&'a mut crate::common::state::WriteDiskArchiveHandle),
}

impl<'a> WriteLike<'a> {
    unsafe fn from_archive(a: *mut archive, function: &str) -> Option<Self> {
        match archive_magic(a) {
            crate::common::error::ARCHIVE_WRITE_MAGIC => {
                if archive_check_magic(
                    a,
                    crate::common::error::ARCHIVE_WRITE_MAGIC,
                    crate::common::error::ARCHIVE_STATE_ANY,
                    function,
                ) == ARCHIVE_FATAL
                {
                    return None;
                }
                write_from_archive(a).map(Self::Archive)
            }
            crate::common::error::ARCHIVE_WRITE_DISK_MAGIC => {
                if archive_check_magic(
                    a,
                    crate::common::error::ARCHIVE_WRITE_DISK_MAGIC,
                    crate::common::error::ARCHIVE_STATE_ANY,
                    function,
                ) == ARCHIVE_FATAL
                {
                    return None;
                }
                write_disk_from_archive(a).map(Self::Disk)
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
}

fn validate_writer(
    a: *mut archive,
    function: &str,
) -> Option<&'static mut crate::common::state::WriteArchiveHandle> {
    unsafe {
        if archive_check_magic(
            a,
            crate::common::error::ARCHIVE_WRITE_MAGIC,
            crate::common::error::ARCHIVE_STATE_ANY,
            function,
        ) == ARCHIVE_FATAL
        {
            return None;
        }
        write_from_archive(a)
    }
}

unsafe extern "C" fn open_callback_shim(
    _backend: *mut BackendArchive,
    client_data: *mut c_void,
) -> c_int {
    let handle = &mut *(client_data as *mut crate::common::state::WriteArchiveHandle);
    handle.open_cb.map_or(ARCHIVE_OK, |callback| {
        callback(
            (handle as *mut crate::common::state::WriteArchiveHandle).cast(),
            handle.client_data,
        )
    })
}

unsafe extern "C" fn write_callback_shim(
    _backend: *mut BackendArchive,
    client_data: *mut c_void,
    buffer: *const c_void,
    length: size_t,
) -> isize {
    let handle = &mut *(client_data as *mut crate::common::state::WriteArchiveHandle);
    handle.write_cb.map_or(ARCHIVE_FATAL as isize, |callback| {
        callback(
            (handle as *mut crate::common::state::WriteArchiveHandle).cast(),
            handle.client_data,
            buffer,
            length,
        )
    })
}

unsafe extern "C" fn close_callback_shim(
    _backend: *mut BackendArchive,
    client_data: *mut c_void,
) -> c_int {
    let handle = &mut *(client_data as *mut crate::common::state::WriteArchiveHandle);
    handle.close_cb.map_or(ARCHIVE_OK, |callback| {
        callback(
            (handle as *mut crate::common::state::WriteArchiveHandle).cast(),
            handle.client_data,
        )
    })
}

unsafe extern "C" fn free_callback_shim(
    _backend: *mut BackendArchive,
    client_data: *mut c_void,
) -> c_int {
    let handle = &mut *(client_data as *mut crate::common::state::WriteArchiveHandle);
    handle.free_cb.map_or(ARCHIVE_OK, |callback| {
        callback(
            (handle as *mut crate::common::state::WriteArchiveHandle).cast(),
            handle.client_data,
        )
    })
}

fn string_ptr(value: Option<&str>) -> *const c_char {
    value
        .map(|value| CString::new(value).expect("config string").into_raw().cast_const())
        .unwrap_or(ptr::null())
}

unsafe fn with_c_string<F>(value: &str, f: F) -> c_int
where
    F: FnOnce(*const c_char) -> c_int,
{
    let value = CString::new(value).expect("config string");
    f(value.as_ptr())
}

unsafe fn apply_write_filter(
    handle: &mut crate::common::state::WriteArchiveHandle,
    filter: &WriteFilterConfig,
) -> c_int {
    match filter {
        WriteFilterConfig::Code(code) => (backend_api().archive_write_add_filter)(handle.backend, *code),
        WriteFilterConfig::Name(name) => with_c_string(name, |name| {
            (backend_api().archive_write_add_filter_by_name)(handle.backend, name)
        }),
        WriteFilterConfig::Program(command) => with_c_string(command, |command| {
            (backend_api().archive_write_add_filter_program)(handle.backend, command)
        }),
        WriteFilterConfig::B64Encode => {
            (backend_api().archive_write_add_filter_b64encode)(handle.backend)
        }
        WriteFilterConfig::Bzip2 => (backend_api().archive_write_add_filter_bzip2)(handle.backend),
        WriteFilterConfig::Compress => {
            (backend_api().archive_write_add_filter_compress)(handle.backend)
        }
        WriteFilterConfig::Grzip => (backend_api().archive_write_add_filter_grzip)(handle.backend),
        WriteFilterConfig::Gzip => (backend_api().archive_write_add_filter_gzip)(handle.backend),
        WriteFilterConfig::Lrzip => (backend_api().archive_write_add_filter_lrzip)(handle.backend),
        WriteFilterConfig::Lz4 => (backend_api().archive_write_add_filter_lz4)(handle.backend),
        WriteFilterConfig::Lzip => (backend_api().archive_write_add_filter_lzip)(handle.backend),
        WriteFilterConfig::Lzma => (backend_api().archive_write_add_filter_lzma)(handle.backend),
        WriteFilterConfig::Lzop => (backend_api().archive_write_add_filter_lzop)(handle.backend),
        WriteFilterConfig::None => (backend_api().archive_write_add_filter_none)(handle.backend),
        WriteFilterConfig::Uuencode => {
            (backend_api().archive_write_add_filter_uuencode)(handle.backend)
        }
        WriteFilterConfig::Xz => (backend_api().archive_write_add_filter_xz)(handle.backend),
        WriteFilterConfig::Zstd => (backend_api().archive_write_add_filter_zstd)(handle.backend),
    }
}

unsafe fn apply_write_format(
    handle: &mut crate::common::state::WriteArchiveHandle,
    format: &WriteFormatConfig,
) -> c_int {
    match format {
        WriteFormatConfig::Code(code) => (backend_api().archive_write_set_format)(handle.backend, *code),
        WriteFormatConfig::Name(name) => with_c_string(name, |name| {
            (backend_api().archive_write_set_format_by_name)(handle.backend, name)
        }),
        WriteFormatConfig::ByExt {
            filename,
            default_ext,
        } => {
            let filename = CString::new(filename.as_str()).expect("filename");
            match default_ext {
                Some(default_ext) => {
                    let default_ext = CString::new(default_ext.as_str()).expect("default ext");
                    (backend_api().archive_write_set_format_filter_by_ext_def)(
                        handle.backend,
                        filename.as_ptr(),
                        default_ext.as_ptr(),
                    )
                }
                None => {
                    (backend_api().archive_write_set_format_filter_by_ext)(
                        handle.backend,
                        filename.as_ptr(),
                    )
                }
            }
        }
        WriteFormatConfig::ArBsd => (backend_api().archive_write_set_format_ar_bsd)(handle.backend),
        WriteFormatConfig::ArSvr4 => {
            (backend_api().archive_write_set_format_ar_svr4)(handle.backend)
        }
        WriteFormatConfig::Cpio => (backend_api().archive_write_set_format_cpio)(handle.backend),
        WriteFormatConfig::CpioBin => {
            (backend_api().archive_write_set_format_cpio_bin)(handle.backend)
        }
        WriteFormatConfig::CpioNewc => {
            (backend_api().archive_write_set_format_cpio_newc)(handle.backend)
        }
        WriteFormatConfig::CpioOdc => {
            (backend_api().archive_write_set_format_cpio_odc)(handle.backend)
        }
        WriteFormatConfig::CpioPwb => {
            (backend_api().archive_write_set_format_cpio_pwb)(handle.backend)
        }
        WriteFormatConfig::Gnutar => {
            (backend_api().archive_write_set_format_gnutar)(handle.backend)
        }
        WriteFormatConfig::Pax => (backend_api().archive_write_set_format_pax)(handle.backend),
        WriteFormatConfig::PaxRestricted => {
            (backend_api().archive_write_set_format_pax_restricted)(handle.backend)
        }
        WriteFormatConfig::Raw => (backend_api().archive_write_set_format_raw)(handle.backend),
        WriteFormatConfig::Shar => (backend_api().archive_write_set_format_shar)(handle.backend),
        WriteFormatConfig::SharDump => {
            (backend_api().archive_write_set_format_shar_dump)(handle.backend)
        }
        WriteFormatConfig::Ustar => (backend_api().archive_write_set_format_ustar)(handle.backend),
        WriteFormatConfig::V7tar => (backend_api().archive_write_set_format_v7tar)(handle.backend),
    }
}

unsafe fn apply_write_option(
    handle: &mut crate::common::state::WriteArchiveHandle,
    option: &WriteOptionConfig,
) -> c_int {
    match option {
        WriteOptionConfig::FilterOption {
            module,
            option,
            value,
        } => {
            let module = module
                .as_deref()
                .map(|value| CString::new(value).expect("module"));
            let option = option
                .as_deref()
                .map(|value| CString::new(value).expect("option"));
            let value = value
                .as_deref()
                .map(|value| CString::new(value).expect("value"));
            (backend_api().archive_write_set_filter_option)(
                handle.backend,
                module.as_ref().map_or(ptr::null(), |value| value.as_ptr()),
                option.as_ref().map_or(ptr::null(), |value| value.as_ptr()),
                value.as_ref().map_or(ptr::null(), |value| value.as_ptr()),
            )
        }
        WriteOptionConfig::FormatOption {
            module,
            option,
            value,
        } => {
            let module = module
                .as_deref()
                .map(|value| CString::new(value).expect("module"));
            let option = option
                .as_deref()
                .map(|value| CString::new(value).expect("option"));
            let value = value
                .as_deref()
                .map(|value| CString::new(value).expect("value"));
            (backend_api().archive_write_set_format_option)(
                handle.backend,
                module.as_ref().map_or(ptr::null(), |value| value.as_ptr()),
                option.as_ref().map_or(ptr::null(), |value| value.as_ptr()),
                value.as_ref().map_or(ptr::null(), |value| value.as_ptr()),
            )
        }
        WriteOptionConfig::Option {
            module,
            option,
            value,
        } => {
            let module = module
                .as_deref()
                .map(|value| CString::new(value).expect("module"));
            let option = option
                .as_deref()
                .map(|value| CString::new(value).expect("option"));
            let value = value
                .as_deref()
                .map(|value| CString::new(value).expect("value"));
            (backend_api().archive_write_set_option)(
                handle.backend,
                module.as_ref().map_or(ptr::null(), |value| value.as_ptr()),
                option.as_ref().map_or(ptr::null(), |value| value.as_ptr()),
                value.as_ref().map_or(ptr::null(), |value| value.as_ptr()),
            )
        }
        WriteOptionConfig::Options(options) => with_c_string(options, |options| {
            (backend_api().archive_write_set_options)(handle.backend, options)
        }),
        WriteOptionConfig::Passphrase(passphrase) => with_c_string(passphrase, |passphrase| {
            (backend_api().archive_write_set_passphrase)(handle.backend, passphrase)
        }),
    }
}

unsafe fn ensure_write_backend(
    handle: &mut crate::common::state::WriteArchiveHandle,
) -> c_int {
    if handle.backend.is_null() {
        handle.backend = (backend_api().archive_write_new)();
        if handle.backend.is_null() {
            set_error_string(&mut handle.core, libc::ENOMEM, "failed to create writer backend".to_string());
            return ARCHIVE_FATAL;
        }

        let mut status =
            (backend_api().archive_write_set_bytes_per_block)(handle.backend, handle.bytes_per_block);
        if status != ARCHIVE_OK {
            return status;
        }
        status = (backend_api().archive_write_set_bytes_in_last_block)(
            handle.backend,
            handle.bytes_in_last_block,
        );
        if status != ARCHIVE_OK {
            return status;
        }
        if let Some((dev, ino)) = handle.skip_file {
            status = (backend_api().archive_write_set_skip_file)(handle.backend, dev, ino);
            if status != ARCHIVE_OK {
                return status;
            }
        }
        if let Some(format) = handle.format.clone() {
            status = apply_write_format(handle, &format);
            if status != ARCHIVE_OK {
                return status;
            }
        }
        let filters = handle.filters.clone();
        for filter in filters {
            status = apply_write_filter(handle, &filter);
            if status != ARCHIVE_OK {
                return status;
            }
        }
        let options = handle.options.clone();
        for option in options {
            status = apply_write_option(handle, &option);
            if status != ARCHIVE_OK {
                return status;
            }
        }
    }
    ARCHIVE_OK
}

unsafe fn ensure_write_backend_open(
    handle: &mut crate::common::state::WriteArchiveHandle,
) -> c_int {
    let status = ensure_write_backend(handle);
    if status != ARCHIVE_OK || handle.backend_opened {
        return status;
    }

    let status = match &handle.open_target {
        WriteOpenConfig::None => ARCHIVE_OK,
        WriteOpenConfig::Callbacks => (backend_api().archive_write_open2)(
            handle.backend,
            (handle as *mut crate::common::state::WriteArchiveHandle).cast(),
            handle.open_cb.map(|_| {
                open_callback_shim as unsafe extern "C" fn(*mut BackendArchive, *mut c_void) -> c_int
            }),
            handle.write_cb.map(|_| {
                write_callback_shim
                    as unsafe extern "C" fn(
                        *mut BackendArchive,
                        *mut c_void,
                        *const c_void,
                        size_t,
                    ) -> isize
            }),
            handle.close_cb.map(|_| {
                close_callback_shim
                    as unsafe extern "C" fn(*mut BackendArchive, *mut c_void) -> c_int
            }),
            handle.free_cb.map(|_| {
                free_callback_shim
                    as unsafe extern "C" fn(*mut BackendArchive, *mut c_void) -> c_int
            }),
        ),
        WriteOpenConfig::Memory { buffer, size, used } => {
            (backend_api().archive_write_open_memory)(handle.backend, *buffer, *size, *used)
        }
        WriteOpenConfig::Filename(path) => with_c_string(path, |path| {
            (backend_api().archive_write_open_filename)(handle.backend, path)
        }),
    };
    if status == ARCHIVE_OK {
        handle.backend_opened = true;
    }
    status
}

unsafe fn ensure_write_disk_backend(
    handle: &mut crate::common::state::WriteDiskArchiveHandle,
) -> c_int {
    if handle.backend.is_null() {
        handle.backend = (backend_api().archive_write_disk_new)();
        if handle.backend.is_null() {
            set_error_string(
                &mut handle.core,
                libc::ENOMEM,
                "failed to create write-disk backend".to_string(),
            );
            return ARCHIVE_FATAL;
        }
        let mut status = (backend_api().archive_write_disk_set_options)(handle.backend, handle.options);
        if status != ARCHIVE_OK {
            return status;
        }
        if let Some((dev, ino)) = handle.skip_file {
            status = (backend_api().archive_write_disk_set_skip_file)(handle.backend, dev, ino);
            if status != ARCHIVE_OK {
                return status;
            }
        }
    }
    ARCHIVE_OK
}

fn is_symlink(metadata: &std::fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

fn path_has_dotdot(path: &Path) -> bool {
    path.components().any(|component| matches!(component, Component::ParentDir))
}

fn check_secure_symlink_path(path: &Path, allow_unlink: bool) -> Result<(), &'static str> {
    let mut current = PathBuf::new();
    let mut components = path.components().peekable();
    while let Some(component) = components.next() {
        match component {
            Component::CurDir => {}
            Component::RootDir | Component::Prefix(_) => current.push(component.as_os_str()),
            Component::Normal(part) => {
                current.push(part);
                let is_final = components.peek().is_none();
                if let Ok(metadata) = std::fs::symlink_metadata(&current) {
                    if is_symlink(&metadata) {
                        if allow_unlink && is_final {
                            continue;
                        }
                        return Err("path traverses an existing symlink");
                    }
                }
            }
            Component::ParentDir => {}
        }
    }
    Ok(())
}

unsafe fn prevalidate_disk_path(
    handle: &mut crate::common::state::WriteDiskArchiveHandle,
    entry: *mut archive_entry,
) -> c_int {
    let Some(entry_data) = entry_from_raw(entry) else {
        return ARCHIVE_FATAL;
    };
    let Some(pathname) = entry_data.pathname.get_str() else {
        set_error_string(&mut handle.core, libc::EINVAL, "entry pathname is missing".to_string());
        return ARCHIVE_FATAL;
    };
    let path = Path::new(pathname);

    if (handle.options & ARCHIVE_EXTRACT_SECURE_NOABSOLUTEPATHS) != 0 && path.is_absolute() {
        set_error_string(&mut handle.core, libc::EINVAL, "absolute paths are not permitted".to_string());
        return ARCHIVE_FAILED;
    }
    if (handle.options & ARCHIVE_EXTRACT_SECURE_NODOTDOT) != 0 && path_has_dotdot(path) {
        set_error_string(
            &mut handle.core,
            libc::EINVAL,
            "path contains '..' and secure nodotdot is enabled".to_string(),
        );
        return ARCHIVE_FAILED;
    }
    if (handle.options & ARCHIVE_EXTRACT_SECURE_SYMLINKS) != 0 {
        let allow_unlink = (handle.options & ARCHIVE_EXTRACT_UNLINK) != 0;
        let mut check_path = PathBuf::new();
        if path.is_absolute() {
            check_path.push(path);
        } else {
            check_path.push(path);
        }
        if let Err(message) = check_secure_symlink_path(&check_path, allow_unlink) {
            let is_symlink_entry = (entry_data.mode & AE_IFMT) == AE_IFLNK;
            if !is_symlink_entry || !allow_unlink {
                set_error_string(&mut handle.core, libc::ELOOP, message.to_string());
                return ARCHIVE_FAILED;
            }
        }
    }

    ARCHIVE_OK
}

fn push_or_apply_filter(
    handle: &mut crate::common::state::WriteArchiveHandle,
    filter: WriteFilterConfig,
) -> c_int {
    unsafe {
        if handle.backend.is_null() {
            handle.filters.push(filter);
            ARCHIVE_OK
        } else {
            apply_write_filter(handle, &filter)
        }
    }
}

fn set_or_apply_format(
    handle: &mut crate::common::state::WriteArchiveHandle,
    format: WriteFormatConfig,
) -> c_int {
    unsafe {
        if handle.backend.is_null() {
            handle.format = Some(format);
            ARCHIVE_OK
        } else {
            let status = apply_write_format(handle, &format);
            if status == ARCHIVE_OK {
                handle.format = Some(format);
            }
            status
        }
    }
}

fn push_or_apply_option(
    handle: &mut crate::common::state::WriteArchiveHandle,
    option: WriteOptionConfig,
) -> c_int {
    unsafe {
        if handle.backend.is_null() {
            handle.options.push(option);
            ARCHIVE_OK
        } else {
            let status = apply_write_option(handle, &option);
            if status == ARCHIVE_OK {
                handle.options.push(option);
            }
            status
        }
    }
}

macro_rules! writer_filter_call0 {
    ($name:ident, $variant:expr) => {
        #[no_mangle]
        pub extern "C" fn $name(a: *mut archive) -> c_int {
            ffi_int(ARCHIVE_FATAL, || unsafe {
                let Some(handle) = validate_writer(a, stringify!($name)) else {
                    return ARCHIVE_FATAL;
                };
                clear_error(&mut handle.core);
                let status = push_or_apply_filter(handle, $variant);
                sync_backend_core(a);
                status
            })
        }
    };
}

macro_rules! writer_format_call0 {
    ($name:ident, $variant:expr) => {
        #[no_mangle]
        pub extern "C" fn $name(a: *mut archive) -> c_int {
            ffi_int(ARCHIVE_FATAL, || unsafe {
                let Some(handle) = validate_writer(a, stringify!($name)) else {
                    return ARCHIVE_FATAL;
                };
                clear_error(&mut handle.core);
                let status = set_or_apply_format(handle, $variant);
                sync_backend_core(a);
                status
            })
        }
    };
}

#[no_mangle]
pub extern "C" fn archive_write_set_bytes_per_block(a: *mut archive, value: c_int) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_writer(a, "archive_write_set_bytes_per_block") else {
            return ARCHIVE_FATAL;
        };
        handle.bytes_per_block = value;
        let status = if handle.backend.is_null() {
            ARCHIVE_OK
        } else {
            (backend_api().archive_write_set_bytes_per_block)(handle.backend, value)
        };
        sync_backend_core(a);
        status
    })
}

#[no_mangle]
pub extern "C" fn archive_write_set_bytes_in_last_block(a: *mut archive, value: c_int) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_writer(a, "archive_write_set_bytes_in_last_block") else {
            return ARCHIVE_FATAL;
        };
        handle.bytes_in_last_block = value;
        let status = if handle.backend.is_null() {
            ARCHIVE_OK
        } else {
            (backend_api().archive_write_set_bytes_in_last_block)(handle.backend, value)
        };
        sync_backend_core(a);
        status
    })
}

writer_filter_call0!(
    archive_write_add_filter_b64encode,
    WriteFilterConfig::B64Encode
);
writer_filter_call0!(archive_write_add_filter_bzip2, WriteFilterConfig::Bzip2);
writer_filter_call0!(archive_write_add_filter_compress, WriteFilterConfig::Compress);
writer_filter_call0!(archive_write_add_filter_grzip, WriteFilterConfig::Grzip);
writer_filter_call0!(archive_write_add_filter_gzip, WriteFilterConfig::Gzip);
writer_filter_call0!(archive_write_add_filter_lrzip, WriteFilterConfig::Lrzip);
writer_filter_call0!(archive_write_add_filter_lz4, WriteFilterConfig::Lz4);
writer_filter_call0!(archive_write_add_filter_lzip, WriteFilterConfig::Lzip);
writer_filter_call0!(archive_write_add_filter_lzma, WriteFilterConfig::Lzma);
writer_filter_call0!(archive_write_add_filter_lzop, WriteFilterConfig::Lzop);
writer_filter_call0!(archive_write_add_filter_none, WriteFilterConfig::None);
writer_filter_call0!(archive_write_add_filter_uuencode, WriteFilterConfig::Uuencode);
writer_filter_call0!(archive_write_add_filter_xz, WriteFilterConfig::Xz);
writer_filter_call0!(archive_write_add_filter_zstd, WriteFilterConfig::Zstd);

writer_format_call0!(archive_write_set_format_ar_bsd, WriteFormatConfig::ArBsd);
writer_format_call0!(archive_write_set_format_ar_svr4, WriteFormatConfig::ArSvr4);
writer_format_call0!(archive_write_set_format_cpio, WriteFormatConfig::Cpio);
writer_format_call0!(archive_write_set_format_cpio_bin, WriteFormatConfig::CpioBin);
writer_format_call0!(archive_write_set_format_cpio_newc, WriteFormatConfig::CpioNewc);
writer_format_call0!(archive_write_set_format_cpio_odc, WriteFormatConfig::CpioOdc);
writer_format_call0!(archive_write_set_format_cpio_pwb, WriteFormatConfig::CpioPwb);
writer_format_call0!(archive_write_set_format_gnutar, WriteFormatConfig::Gnutar);
writer_format_call0!(archive_write_set_format_pax, WriteFormatConfig::Pax);
writer_format_call0!(
    archive_write_set_format_pax_restricted,
    WriteFormatConfig::PaxRestricted
);
writer_format_call0!(archive_write_set_format_raw, WriteFormatConfig::Raw);
writer_format_call0!(archive_write_set_format_shar, WriteFormatConfig::Shar);
writer_format_call0!(archive_write_set_format_shar_dump, WriteFormatConfig::SharDump);
writer_format_call0!(archive_write_set_format_ustar, WriteFormatConfig::Ustar);
writer_format_call0!(archive_write_set_format_v7tar, WriteFormatConfig::V7tar);

#[no_mangle]
pub extern "C" fn archive_write_fail(a: *mut archive) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_writer(a, "archive_write_fail") else {
            return ARCHIVE_FATAL;
        };
        let status = ensure_write_backend_open(handle);
        if status != ARCHIVE_OK {
            return status;
        }
        let status = (backend_api().archive_write_fail)(handle.backend);
        sync_backend_core(a);
        status
    })
}

#[no_mangle]
pub extern "C" fn archive_write_get_bytes_per_block(a: *mut archive) -> c_int {
    unsafe {
        let Some(handle) = validate_writer(a, "archive_write_get_bytes_per_block") else {
            return ARCHIVE_FATAL;
        };
        if handle.backend.is_null() {
            handle.bytes_per_block
        } else {
            (backend_api().archive_write_get_bytes_per_block)(handle.backend)
        }
    }
}

#[no_mangle]
pub extern "C" fn archive_write_get_bytes_in_last_block(a: *mut archive) -> c_int {
    unsafe {
        let Some(handle) = validate_writer(a, "archive_write_get_bytes_in_last_block") else {
            return ARCHIVE_FATAL;
        };
        if handle.backend.is_null() {
            handle.bytes_in_last_block
        } else {
            (backend_api().archive_write_get_bytes_in_last_block)(handle.backend)
        }
    }
}

#[no_mangle]
pub extern "C" fn archive_write_set_skip_file(a: *mut archive, dev: i64, ino: i64) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_writer(a, "archive_write_set_skip_file") else {
            return ARCHIVE_FATAL;
        };
        handle.skip_file = Some((dev, ino));
        let status = if handle.backend.is_null() {
            ARCHIVE_OK
        } else {
            (backend_api().archive_write_set_skip_file)(handle.backend, dev, ino)
        };
        sync_backend_core(a);
        status
    })
}

#[no_mangle]
pub extern "C" fn archive_write_add_filter(a: *mut archive, filter_code: c_int) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_writer(a, "archive_write_add_filter") else {
            return ARCHIVE_FATAL;
        };
        clear_error(&mut handle.core);
        let status = push_or_apply_filter(handle, WriteFilterConfig::Code(filter_code));
        sync_backend_core(a);
        status
    })
}

#[no_mangle]
pub extern "C" fn archive_write_add_filter_by_name(
    a: *mut archive,
    filter_name: *const c_char,
) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_writer(a, "archive_write_add_filter_by_name") else {
            return ARCHIVE_FATAL;
        };
        let Some(filter_name) = from_optional_c_str(filter_name) else {
            return ARCHIVE_FATAL;
        };
        clear_error(&mut handle.core);
        let status = push_or_apply_filter(handle, WriteFilterConfig::Name(filter_name));
        sync_backend_core(a);
        status
    })
}

#[no_mangle]
pub extern "C" fn archive_write_add_filter_program(
    a: *mut archive,
    command: *const c_char,
) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_writer(a, "archive_write_add_filter_program") else {
            return ARCHIVE_FATAL;
        };
        let Some(command) = from_optional_c_str(command) else {
            return ARCHIVE_FATAL;
        };
        clear_error(&mut handle.core);
        let status = push_or_apply_filter(handle, WriteFilterConfig::Program(command));
        sync_backend_core(a);
        status
    })
}

#[no_mangle]
pub extern "C" fn archive_write_set_compression_bzip2(a: *mut archive) -> c_int {
    archive_write_add_filter_bzip2(a)
}

#[no_mangle]
pub extern "C" fn archive_write_set_compression_compress(a: *mut archive) -> c_int {
    archive_write_add_filter_compress(a)
}

#[no_mangle]
pub extern "C" fn archive_write_set_compression_gzip(a: *mut archive) -> c_int {
    archive_write_add_filter_gzip(a)
}

#[no_mangle]
pub extern "C" fn archive_write_set_compression_lzip(a: *mut archive) -> c_int {
    archive_write_add_filter_lzip(a)
}

#[no_mangle]
pub extern "C" fn archive_write_set_compression_lzma(a: *mut archive) -> c_int {
    archive_write_add_filter_lzma(a)
}

#[no_mangle]
pub extern "C" fn archive_write_set_compression_none(a: *mut archive) -> c_int {
    archive_write_add_filter_none(a)
}

#[no_mangle]
pub extern "C" fn archive_write_set_compression_program(
    a: *mut archive,
    command: *const c_char,
) -> c_int {
    archive_write_add_filter_program(a, command)
}

#[no_mangle]
pub extern "C" fn archive_write_set_compression_xz(a: *mut archive) -> c_int {
    archive_write_add_filter_xz(a)
}

#[no_mangle]
pub extern "C" fn archive_write_set_format(a: *mut archive, format_code: c_int) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_writer(a, "archive_write_set_format") else {
            return ARCHIVE_FATAL;
        };
        clear_error(&mut handle.core);
        let status = set_or_apply_format(handle, WriteFormatConfig::Code(format_code));
        sync_backend_core(a);
        status
    })
}

#[no_mangle]
pub extern "C" fn archive_write_set_format_by_name(
    a: *mut archive,
    format_name: *const c_char,
) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_writer(a, "archive_write_set_format_by_name") else {
            return ARCHIVE_FATAL;
        };
        let Some(format_name) = from_optional_c_str(format_name) else {
            return ARCHIVE_FATAL;
        };
        clear_error(&mut handle.core);
        let status = set_or_apply_format(handle, WriteFormatConfig::Name(format_name));
        sync_backend_core(a);
        status
    })
}

#[no_mangle]
pub extern "C" fn archive_write_set_format_filter_by_ext(
    a: *mut archive,
    filename: *const c_char,
) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_writer(a, "archive_write_set_format_filter_by_ext") else {
            return ARCHIVE_FATAL;
        };
        let Some(filename) = from_optional_c_str(filename) else {
            return ARCHIVE_FATAL;
        };
        let status = set_or_apply_format(
            handle,
            WriteFormatConfig::ByExt {
                filename,
                default_ext: None,
            },
        );
        sync_backend_core(a);
        status
    })
}

#[no_mangle]
pub extern "C" fn archive_write_set_format_filter_by_ext_def(
    a: *mut archive,
    filename: *const c_char,
    default_ext: *const c_char,
) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_writer(a, "archive_write_set_format_filter_by_ext_def") else {
            return ARCHIVE_FATAL;
        };
        let Some(filename) = from_optional_c_str(filename) else {
            return ARCHIVE_FATAL;
        };
        let status = set_or_apply_format(
            handle,
            WriteFormatConfig::ByExt {
                filename,
                default_ext: from_optional_c_str(default_ext),
            },
        );
        sync_backend_core(a);
        status
    })
}

#[no_mangle]
pub extern "C" fn archive_write_open(
    a: *mut archive,
    client_data: *mut c_void,
    open_cb: Option<ArchiveOpenCallback>,
    write_cb: Option<ArchiveWriteCallback>,
    close_cb: Option<ArchiveCloseCallback>,
) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_writer(a, "archive_write_open") else {
            return ARCHIVE_FATAL;
        };
        handle.client_data = client_data;
        handle.open_cb = open_cb;
        handle.write_cb = write_cb;
        handle.close_cb = close_cb;
        handle.free_cb = None;
        handle.open_target = WriteOpenConfig::Callbacks;
        if handle.backend.is_null() {
            ARCHIVE_OK
        } else {
            ensure_write_backend_open(handle)
        }
    })
}

#[no_mangle]
pub extern "C" fn archive_write_open2(
    a: *mut archive,
    client_data: *mut c_void,
    open_cb: Option<ArchiveOpenCallback>,
    write_cb: Option<ArchiveWriteCallback>,
    close_cb: Option<ArchiveCloseCallback>,
    free_cb: Option<ArchiveFreeCallback>,
) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_writer(a, "archive_write_open2") else {
            return ARCHIVE_FATAL;
        };
        handle.client_data = client_data;
        handle.open_cb = open_cb;
        handle.write_cb = write_cb;
        handle.close_cb = close_cb;
        handle.free_cb = free_cb;
        handle.open_target = WriteOpenConfig::Callbacks;
        if handle.backend.is_null() {
            ARCHIVE_OK
        } else {
            ensure_write_backend_open(handle)
        }
    })
}

#[no_mangle]
pub extern "C" fn archive_write_open_filename(a: *mut archive, file: *const c_char) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_writer(a, "archive_write_open_filename") else {
            return ARCHIVE_FATAL;
        };
        let Some(file) = from_optional_c_str(file) else {
            return ARCHIVE_FATAL;
        };
        clear_error(&mut handle.core);
        handle.open_target = WriteOpenConfig::Filename(file);
        if handle.backend.is_null() {
            ARCHIVE_OK
        } else {
            ensure_write_backend_open(handle)
        }
    })
}

#[no_mangle]
pub extern "C" fn archive_write_open_memory(
    a: *mut archive,
    buffer: *mut c_void,
    size: size_t,
    used: *mut size_t,
) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_writer(a, "archive_write_open_memory") else {
            return ARCHIVE_FATAL;
        };
        clear_error(&mut handle.core);
        handle.open_target = WriteOpenConfig::Memory { buffer, size, used };
        if handle.backend.is_null() {
            ARCHIVE_OK
        } else {
            ensure_write_backend_open(handle)
        }
    })
}

#[no_mangle]
pub extern "C" fn archive_write_header(a: *mut archive, entry: *mut archive_entry) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(mut handle) = WriteLike::from_archive(a, "archive_write_header") else {
            return ARCHIVE_FATAL;
        };
        if entry.is_null() {
            return ARCHIVE_FATAL;
        }

        let status = match &mut handle {
            WriteLike::Archive(handle) => ensure_write_backend_open(handle),
            WriteLike::Disk(handle) => {
                let status = prevalidate_disk_path(handle, entry);
                if status != ARCHIVE_OK {
                    status
                } else {
                    ensure_write_disk_backend(handle)
                }
            }
        };
        if status != ARCHIVE_OK {
            return status;
        }

        let backend_entry = (backend_api().archive_entry_new)();
        if backend_entry.is_null() {
            return ARCHIVE_FATAL;
        }
        let result = if custom_entry_to_backend(entry, backend_entry) != ARCHIVE_OK {
            ARCHIVE_FATAL
        } else {
            (backend_api().archive_write_header)(handle.backend(), backend_entry)
        };
        (backend_api().archive_entry_free)(backend_entry);
        sync_backend_core(a);
        result
    })
}

#[no_mangle]
pub extern "C" fn archive_write_data(
    a: *mut archive,
    buffer: *const c_void,
    size: size_t,
) -> isize {
    unsafe {
        let Some(mut handle) = WriteLike::from_archive(a, "archive_write_data") else {
            return ARCHIVE_FATAL as isize;
        };
        match &mut handle {
            WriteLike::Archive(handle) => {
                let status = ensure_write_backend_open(handle);
                if status != ARCHIVE_OK {
                    return status as isize;
                }
            }
            WriteLike::Disk(handle) => {
                let status = ensure_write_disk_backend(handle);
                if status != ARCHIVE_OK {
                    return status as isize;
                }
            }
        }
        let status = (backend_api().archive_write_data)(handle.backend(), buffer, size);
        sync_backend_core(a);
        status
    }
}

#[no_mangle]
pub extern "C" fn archive_write_data_block(
    a: *mut archive,
    buffer: *const c_void,
    size: size_t,
    offset: i64,
) -> isize {
    unsafe {
        let Some(mut handle) = WriteLike::from_archive(a, "archive_write_data_block") else {
            return ARCHIVE_FATAL as isize;
        };
        match &mut handle {
            WriteLike::Archive(handle) => {
                let status = ensure_write_backend_open(handle);
                if status != ARCHIVE_OK {
                    return status as isize;
                }
            }
            WriteLike::Disk(handle) => {
                let status = ensure_write_disk_backend(handle);
                if status != ARCHIVE_OK {
                    return status as isize;
                }
            }
        }
        let status =
            (backend_api().archive_write_data_block)(handle.backend(), buffer, size, offset);
        sync_backend_core(a);
        status
    }
}

#[no_mangle]
pub extern "C" fn archive_write_finish_entry(a: *mut archive) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(mut handle) = WriteLike::from_archive(a, "archive_write_finish_entry") else {
            return ARCHIVE_FATAL;
        };
        match &mut handle {
            WriteLike::Archive(handle) => {
                let status = ensure_write_backend_open(handle);
                if status != ARCHIVE_OK {
                    return status;
                }
            }
            WriteLike::Disk(handle) => {
                let status = ensure_write_disk_backend(handle);
                if status != ARCHIVE_OK {
                    return status;
                }
            }
        }
        let status = (backend_api().archive_write_finish_entry)(handle.backend());
        sync_backend_core(a);
        status
    })
}

#[no_mangle]
pub extern "C" fn archive_write_set_filter_option(
    a: *mut archive,
    module: *const c_char,
    option: *const c_char,
    value: *const c_char,
) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_writer(a, "archive_write_set_filter_option") else {
            return ARCHIVE_FATAL;
        };
        push_or_apply_option(
            handle,
            WriteOptionConfig::FilterOption {
                module: from_optional_c_str(module),
                option: from_optional_c_str(option),
                value: from_optional_c_str(value),
            },
        )
    })
}

#[no_mangle]
pub extern "C" fn archive_write_set_format_option(
    a: *mut archive,
    module: *const c_char,
    option: *const c_char,
    value: *const c_char,
) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_writer(a, "archive_write_set_format_option") else {
            return ARCHIVE_FATAL;
        };
        push_or_apply_option(
            handle,
            WriteOptionConfig::FormatOption {
                module: from_optional_c_str(module),
                option: from_optional_c_str(option),
                value: from_optional_c_str(value),
            },
        )
    })
}

#[no_mangle]
pub extern "C" fn archive_write_set_option(
    a: *mut archive,
    module: *const c_char,
    option: *const c_char,
    value: *const c_char,
) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_writer(a, "archive_write_set_option") else {
            return ARCHIVE_FATAL;
        };
        push_or_apply_option(
            handle,
            WriteOptionConfig::Option {
                module: from_optional_c_str(module),
                option: from_optional_c_str(option),
                value: from_optional_c_str(value),
            },
        )
    })
}

#[no_mangle]
pub extern "C" fn archive_write_set_options(a: *mut archive, options: *const c_char) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_writer(a, "archive_write_set_options") else {
            return ARCHIVE_FATAL;
        };
        let Some(options) = from_optional_c_str(options) else {
            return ARCHIVE_FATAL;
        };
        push_or_apply_option(handle, WriteOptionConfig::Options(options))
    })
}

#[no_mangle]
pub extern "C" fn archive_write_set_passphrase(
    a: *mut archive,
    passphrase: *const c_char,
) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_writer(a, "archive_write_set_passphrase") else {
            return ARCHIVE_FATAL;
        };
        let Some(passphrase) = from_optional_c_str(passphrase) else {
            return ARCHIVE_FATAL;
        };
        push_or_apply_option(handle, WriteOptionConfig::Passphrase(passphrase))
    })
}
