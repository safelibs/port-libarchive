use std::ffi::{c_char, c_int, c_void, CString};
use std::ptr;

use libc::{size_t, wchar_t};

use crate::common::backend::{api as backend_api, BackendArchive, BackendEntry};
use crate::common::error::{ARCHIVE_FATAL, ARCHIVE_OK, ARCHIVE_STATE_FATAL};
use crate::common::helpers::{from_optional_c_str, from_optional_wide, to_wide_null};
use crate::common::panic_boundary::ffi_int;
use crate::common::state::{
    archive_check_magic, archive_magic, clear_error, set_error_string, sync_backend_core,
    write_disk_from_archive, write_from_archive, ArchiveCloseCallback, ArchiveFreeCallback,
    ArchiveOpenCallback, ArchiveWriteCallback, WriteFilterConfig, WriteFormatConfig,
    WriteOpenConfig, WriteOptionConfig,
};
use crate::disk::{
    backend_entry_to_custom, custom_entry_to_backend, native_write_disk_data,
    native_write_disk_data_block, native_write_disk_finish_entry, native_write_disk_header,
};
use crate::ffi::{archive, archive_entry};

const ARCHIVE_FORMAT_CPIO: c_int = 0x10000;
const ARCHIVE_FORMAT_CPIO_POSIX: c_int = ARCHIVE_FORMAT_CPIO | 1;
const ARCHIVE_FORMAT_CPIO_BIN_LE: c_int = ARCHIVE_FORMAT_CPIO | 2;
const ARCHIVE_FORMAT_CPIO_SVR4_NOCRC: c_int = ARCHIVE_FORMAT_CPIO | 4;
const ARCHIVE_FORMAT_CPIO_PWB: c_int = ARCHIVE_FORMAT_CPIO | 7;
const ARCHIVE_FORMAT_SHAR: c_int = 0x20000;
const ARCHIVE_FORMAT_SHAR_BASE: c_int = ARCHIVE_FORMAT_SHAR | 1;
const ARCHIVE_FORMAT_SHAR_DUMP: c_int = ARCHIVE_FORMAT_SHAR | 2;
const ARCHIVE_FORMAT_TAR: c_int = 0x30000;
const ARCHIVE_FORMAT_ZIP: c_int = 0x50000;
const ARCHIVE_FORMAT_TAR_USTAR: c_int = ARCHIVE_FORMAT_TAR | 1;
const ARCHIVE_FORMAT_TAR_PAX_INTERCHANGE: c_int = ARCHIVE_FORMAT_TAR | 2;
const ARCHIVE_FORMAT_TAR_PAX_RESTRICTED: c_int = ARCHIVE_FORMAT_TAR | 3;
const ARCHIVE_FORMAT_TAR_GNUTAR: c_int = ARCHIVE_FORMAT_TAR | 4;
const ARCHIVE_FORMAT_RAW: c_int = 0x90000;

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

macro_rules! unsupported_backend_writer_stub {
    ($name:ident) => {
        #[no_mangle]
        pub extern "C" fn $name(_a: *mut BackendArchive) -> c_int {
            ARCHIVE_FATAL
        }
    };
}

unsupported_backend_writer_stub!(backend_archive_write_set_format_7zip);
unsupported_backend_writer_stub!(backend_archive_write_set_format_iso9660);
unsupported_backend_writer_stub!(backend_archive_write_set_format_mtree);
unsupported_backend_writer_stub!(backend_archive_write_set_format_warc);
unsupported_backend_writer_stub!(backend_archive_write_set_format_xar);

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
        WriteFilterConfig::Code(code) => {
            (backend_api().archive_write_add_filter)(handle.backend, *code)
        }
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
        WriteFormatConfig::Zip => (backend_api().archive_write_set_format_zip)(handle.backend),
    }
}

fn set_write_format_error(
    handle: &mut crate::common::state::WriteArchiveHandle,
    message: String,
) -> c_int {
    set_error_string(&mut handle.core, libc::EINVAL, message);
    handle.core.state = ARCHIVE_STATE_FATAL;
    ARCHIVE_FATAL
}

fn resolve_write_format_by_name(
    handle: &mut crate::common::state::WriteArchiveHandle,
    name: &str,
) -> Result<WriteFormatConfig, c_int> {
    let format = match name {
        "ar" | "arbsd" => WriteFormatConfig::ArBsd,
        "argnu" | "arsvr4" => WriteFormatConfig::ArSvr4,
        "bin" => WriteFormatConfig::CpioBin,
        "bsdtar" | "paxr" | "rpax" => WriteFormatConfig::PaxRestricted,
        "cpio" => WriteFormatConfig::Cpio,
        "gnutar" => WriteFormatConfig::Gnutar,
        "newc" => WriteFormatConfig::CpioNewc,
        "odc" => WriteFormatConfig::CpioOdc,
        "oldtar" | "v7" | "v7tar" => WriteFormatConfig::V7tar,
        "pax" | "posix" => WriteFormatConfig::Pax,
        "pwb" => WriteFormatConfig::CpioPwb,
        "raw" => WriteFormatConfig::Raw,
        "shar" => WriteFormatConfig::Shar,
        "shardump" => WriteFormatConfig::SharDump,
        "ustar" => WriteFormatConfig::Ustar,
        "zip" => WriteFormatConfig::Zip,
        _ => {
            return Err(set_write_format_error(
                handle,
                format!("No such format '{name}'"),
            ));
        }
    };
    Ok(format)
}

fn resolve_write_format_code(
    handle: &mut crate::common::state::WriteArchiveHandle,
    code: c_int,
) -> Result<WriteFormatConfig, c_int> {
    let format = match code {
        ARCHIVE_FORMAT_CPIO => WriteFormatConfig::Cpio,
        ARCHIVE_FORMAT_CPIO_BIN_LE => WriteFormatConfig::CpioBin,
        ARCHIVE_FORMAT_CPIO_PWB => WriteFormatConfig::CpioPwb,
        ARCHIVE_FORMAT_CPIO_POSIX => WriteFormatConfig::CpioOdc,
        ARCHIVE_FORMAT_CPIO_SVR4_NOCRC => WriteFormatConfig::CpioNewc,
        ARCHIVE_FORMAT_RAW => WriteFormatConfig::Raw,
        ARCHIVE_FORMAT_SHAR | ARCHIVE_FORMAT_SHAR_BASE => WriteFormatConfig::Shar,
        ARCHIVE_FORMAT_SHAR_DUMP => WriteFormatConfig::SharDump,
        ARCHIVE_FORMAT_TAR => WriteFormatConfig::PaxRestricted,
        ARCHIVE_FORMAT_TAR_GNUTAR => WriteFormatConfig::Gnutar,
        ARCHIVE_FORMAT_TAR_PAX_INTERCHANGE => WriteFormatConfig::Pax,
        ARCHIVE_FORMAT_TAR_PAX_RESTRICTED => WriteFormatConfig::PaxRestricted,
        ARCHIVE_FORMAT_TAR_USTAR => WriteFormatConfig::Ustar,
        ARCHIVE_FORMAT_ZIP => WriteFormatConfig::Zip,
        _ => return Err(set_write_format_error(handle, "No such format".to_string())),
    };
    Ok(format)
}

fn resolve_write_format_by_ext(
    handle: &mut crate::common::state::WriteArchiveHandle,
    filename: &str,
    default_ext: Option<&str>,
) -> Result<(WriteFormatConfig, WriteFilterConfig), c_int> {
    fn mapping(filename: &str) -> Option<(WriteFormatConfig, WriteFilterConfig)> {
        if filename.ends_with(".cpio") {
            Some((WriteFormatConfig::Cpio, WriteFilterConfig::None))
        } else if filename.ends_with(".a") || filename.ends_with(".ar") {
            Some((WriteFormatConfig::ArSvr4, WriteFilterConfig::None))
        } else if filename.ends_with(".tar") {
            Some((WriteFormatConfig::PaxRestricted, WriteFilterConfig::None))
        } else if filename.ends_with(".tgz") || filename.ends_with(".tar.gz") {
            Some((WriteFormatConfig::PaxRestricted, WriteFilterConfig::Gzip))
        } else if filename.ends_with(".tar.bz2") {
            Some((WriteFormatConfig::PaxRestricted, WriteFilterConfig::Bzip2))
        } else if filename.ends_with(".tar.xz") {
            Some((WriteFormatConfig::PaxRestricted, WriteFilterConfig::Xz))
        } else if filename.ends_with(".zip") || filename.ends_with(".jar") {
            Some((WriteFormatConfig::Zip, WriteFilterConfig::None))
        } else {
            None
        }
    }

    if let Some(config) = mapping(filename) {
        return Ok(config);
    }
    if let Some(default_ext) = default_ext {
        if let Some(config) = mapping(default_ext) {
            return Ok(config);
        }
    }
    Err(set_write_format_error(
        handle,
        format!("No such format '{filename}'"),
    ))
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

unsafe fn ensure_write_backend(handle: &mut crate::common::state::WriteArchiveHandle) -> c_int {
    if handle.backend.is_null() {
        handle.backend = (backend_api().archive_write_new)();
        if handle.backend.is_null() {
            set_error_string(
                &mut handle.core,
                libc::ENOMEM,
                "failed to create writer backend".to_string(),
            );
            return ARCHIVE_FATAL;
        }

        let mut status = (backend_api().archive_write_set_bytes_per_block)(
            handle.backend,
            handle.bytes_per_block,
        );
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
                open_callback_shim
                    as unsafe extern "C" fn(*mut BackendArchive, *mut c_void) -> c_int
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
        WriteOpenConfig::Fd(fd) => (backend_api().archive_write_open_fd)(handle.backend, *fd),
        WriteOpenConfig::Filename(path) => with_c_string(path, |path| {
            (backend_api().archive_write_open_filename)(handle.backend, path)
        }),
        WriteOpenConfig::FilenameW(path) => {
            let path = to_wide_null(path);
            (backend_api().archive_write_open_filename_w)(handle.backend, path.as_ptr())
        }
        WriteOpenConfig::File(file) => (backend_api().archive_write_open_FILE)(handle.backend, *file),
    };
    if status == ARCHIVE_OK {
        handle.backend_opened = true;
    }
    status
}

fn push_or_apply_filter(
    handle: &mut crate::common::state::WriteArchiveHandle,
    filter: WriteFilterConfig,
) -> c_int {
    unsafe {
        let status = ensure_write_backend(handle);
        if status != ARCHIVE_OK {
            return status;
        }
        let status = apply_write_filter(handle, &filter);
        if status == ARCHIVE_OK {
            handle.filters.push(filter);
        }
        status
    }
}

fn set_or_apply_format(
    handle: &mut crate::common::state::WriteArchiveHandle,
    format: WriteFormatConfig,
) -> c_int {
    unsafe {
        let status = ensure_write_backend(handle);
        if status != ARCHIVE_OK {
            return status;
        }
        let status = apply_write_format(handle, &format);
        if status == ARCHIVE_OK {
            handle.format = Some(format);
        }
        status
    }
}

fn push_or_apply_option(
    handle: &mut crate::common::state::WriteArchiveHandle,
    option: WriteOptionConfig,
) -> c_int {
    unsafe {
        let status = ensure_write_backend(handle);
        if status != ARCHIVE_OK {
            return status;
        }
        let status = apply_write_option(handle, &option);
        if status == ARCHIVE_OK {
            handle.options.push(option);
        }
        status
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
writer_filter_call0!(
    archive_write_add_filter_compress,
    WriteFilterConfig::Compress
);
writer_filter_call0!(archive_write_add_filter_grzip, WriteFilterConfig::Grzip);
writer_filter_call0!(archive_write_add_filter_gzip, WriteFilterConfig::Gzip);
writer_filter_call0!(archive_write_add_filter_lrzip, WriteFilterConfig::Lrzip);
writer_filter_call0!(archive_write_add_filter_lz4, WriteFilterConfig::Lz4);
writer_filter_call0!(archive_write_add_filter_lzip, WriteFilterConfig::Lzip);
writer_filter_call0!(archive_write_add_filter_lzma, WriteFilterConfig::Lzma);
writer_filter_call0!(archive_write_add_filter_lzop, WriteFilterConfig::Lzop);
writer_filter_call0!(archive_write_add_filter_none, WriteFilterConfig::None);
writer_filter_call0!(
    archive_write_add_filter_uuencode,
    WriteFilterConfig::Uuencode
);
writer_filter_call0!(archive_write_add_filter_xz, WriteFilterConfig::Xz);
writer_filter_call0!(archive_write_add_filter_zstd, WriteFilterConfig::Zstd);

writer_format_call0!(archive_write_set_format_ar_bsd, WriteFormatConfig::ArBsd);
writer_format_call0!(archive_write_set_format_ar_svr4, WriteFormatConfig::ArSvr4);
writer_format_call0!(archive_write_set_format_cpio, WriteFormatConfig::Cpio);
writer_format_call0!(
    archive_write_set_format_cpio_bin,
    WriteFormatConfig::CpioBin
);
writer_format_call0!(
    archive_write_set_format_cpio_newc,
    WriteFormatConfig::CpioNewc
);
writer_format_call0!(
    archive_write_set_format_cpio_odc,
    WriteFormatConfig::CpioOdc
);
writer_format_call0!(
    archive_write_set_format_cpio_pwb,
    WriteFormatConfig::CpioPwb
);
writer_format_call0!(archive_write_set_format_gnutar, WriteFormatConfig::Gnutar);
writer_format_call0!(archive_write_set_format_pax, WriteFormatConfig::Pax);
writer_format_call0!(
    archive_write_set_format_pax_restricted,
    WriteFormatConfig::PaxRestricted
);
writer_format_call0!(archive_write_set_format_raw, WriteFormatConfig::Raw);
writer_format_call0!(archive_write_set_format_shar, WriteFormatConfig::Shar);
writer_format_call0!(
    archive_write_set_format_shar_dump,
    WriteFormatConfig::SharDump
);
writer_format_call0!(archive_write_set_format_ustar, WriteFormatConfig::Ustar);
writer_format_call0!(archive_write_set_format_v7tar, WriteFormatConfig::V7tar);
writer_format_call0!(archive_write_set_format_zip, WriteFormatConfig::Zip);

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
        let Ok(format) = resolve_write_format_code(handle, format_code) else {
            return ARCHIVE_FATAL;
        };
        let status = set_or_apply_format(handle, format);
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
        let Ok(format) = resolve_write_format_by_name(handle, &format_name) else {
            return ARCHIVE_FATAL;
        };
        let status = set_or_apply_format(handle, format);
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
        clear_error(&mut handle.core);
        let Ok((format, filter)) = resolve_write_format_by_ext(handle, &filename, None) else {
            return ARCHIVE_FATAL;
        };
        let status = set_or_apply_format(handle, format);
        let status = if status == ARCHIVE_OK {
            push_or_apply_filter(handle, filter)
        } else {
            status
        };
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
        clear_error(&mut handle.core);
        let default_ext = from_optional_c_str(default_ext);
        let Ok((format, filter)) =
            resolve_write_format_by_ext(handle, &filename, default_ext.as_deref())
        else {
            return ARCHIVE_FATAL;
        };
        let status = set_or_apply_format(handle, format);
        let status = if status == ARCHIVE_OK {
            push_or_apply_filter(handle, filter)
        } else {
            status
        };
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
        ensure_write_backend_open(handle)
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
        ensure_write_backend_open(handle)
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
        ensure_write_backend_open(handle)
    })
}

#[no_mangle]
pub extern "C" fn archive_write_open_fd(a: *mut archive, fd: c_int) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_writer(a, "archive_write_open_fd") else {
            return ARCHIVE_FATAL;
        };
        clear_error(&mut handle.core);
        handle.open_target = WriteOpenConfig::Fd(fd);
        ensure_write_backend_open(handle)
    })
}

#[no_mangle]
pub extern "C" fn archive_write_open_filename_w(
    a: *mut archive,
    file: *const wchar_t,
) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_writer(a, "archive_write_open_filename_w") else {
            return ARCHIVE_FATAL;
        };
        let Some(file) = from_optional_wide(file) else {
            return ARCHIVE_FATAL;
        };
        clear_error(&mut handle.core);
        handle.open_target = WriteOpenConfig::FilenameW(file);
        ensure_write_backend_open(handle)
    })
}

#[no_mangle]
pub extern "C" fn archive_write_open_file(a: *mut archive, file: *const c_char) -> c_int {
    archive_write_open_filename(a, file)
}

#[no_mangle]
pub extern "C" fn archive_write_open_FILE(a: *mut archive, file: *mut libc::FILE) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_writer(a, "archive_write_open_FILE") else {
            return ARCHIVE_FATAL;
        };
        clear_error(&mut handle.core);
        handle.open_target = WriteOpenConfig::File(file.cast());
        ensure_write_backend_open(handle)
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
        ensure_write_backend_open(handle)
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
                if handle.extraction.current.is_some() {
                    let finish_status = native_write_disk_finish_entry(handle);
                    if finish_status < ARCHIVE_OK {
                        finish_status
                    } else {
                        native_write_disk_header(handle, entry)
                    }
                } else {
                    native_write_disk_header(handle, entry)
                }
            }
        };
        if status != ARCHIVE_OK {
            return status;
        }

        match &mut handle {
            WriteLike::Archive(writer) => {
                let backend_entry = (backend_api().archive_entry_new)();
                if backend_entry.is_null() {
                    return ARCHIVE_FATAL;
                }
                let result = if custom_entry_to_backend(entry, backend_entry) != ARCHIVE_OK {
                    ARCHIVE_FATAL
                } else {
                    let result =
                        (backend_api().archive_write_header)(writer.backend, backend_entry);
                    if matches!(result, ARCHIVE_OK | crate::common::error::ARCHIVE_WARN) {
                        let sync_status = backend_entry_to_custom(backend_entry, entry);
                        if sync_status != ARCHIVE_OK {
                            (backend_api().archive_entry_free)(backend_entry);
                            sync_backend_core(a);
                            return sync_status;
                        }
                    }
                    result
                };
                (backend_api().archive_entry_free)(backend_entry);
                sync_backend_core(a);
                result
            }
            WriteLike::Disk(_) => status,
        }
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
            WriteLike::Archive(writer) => {
                let status = ensure_write_backend_open(writer);
                if status != ARCHIVE_OK {
                    return status as isize;
                }
                let status = (backend_api().archive_write_data)(writer.backend, buffer, size);
                sync_backend_core(a);
                status
            }
            WriteLike::Disk(writer) => native_write_disk_data(writer, buffer, size),
        }
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
            WriteLike::Archive(writer) => {
                let status = ensure_write_backend_open(writer);
                if status != ARCHIVE_OK {
                    return status as isize;
                }
                let status =
                    (backend_api().archive_write_data_block)(writer.backend, buffer, size, offset);
                sync_backend_core(a);
                status
            }
            WriteLike::Disk(writer) => native_write_disk_data_block(writer, buffer, size, offset),
        }
    }
}

#[no_mangle]
pub extern "C" fn archive_write_finish_entry(a: *mut archive) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(mut handle) = WriteLike::from_archive(a, "archive_write_finish_entry") else {
            return ARCHIVE_FATAL;
        };
        match &mut handle {
            WriteLike::Archive(writer) => {
                let status = ensure_write_backend_open(writer);
                if status != ARCHIVE_OK {
                    return status;
                }
                let status = (backend_api().archive_write_finish_entry)(writer.backend);
                sync_backend_core(a);
                status
            }
            WriteLike::Disk(writer) => native_write_disk_finish_entry(writer),
        }
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
