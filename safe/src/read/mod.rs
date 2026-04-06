pub mod format;

use std::ffi::{c_char, c_int, c_void, CString};
use std::ptr;

use libc::{size_t, wchar_t};

use crate::common::backend::{
    api as backend_api, BackendArchive, BackendCloseCallback, BackendEntry, BackendOpenCallback,
    BackendPassphraseCallback, BackendReadCallback, BackendSeekCallback, BackendSkipCallback,
    BackendSwitchCallback,
};
use crate::common::error::{
    ARCHIVE_EOF, ARCHIVE_FAILED, ARCHIVE_FATAL, ARCHIVE_OK, ARCHIVE_READ_MAGIC, ARCHIVE_STATE_DATA,
    ARCHIVE_STATE_EOF, ARCHIVE_STATE_FATAL, ARCHIVE_STATE_HEADER, ARCHIVE_STATE_NEW, ARCHIVE_WARN,
};
use crate::common::helpers::from_optional_c_str;
use crate::common::panic_boundary::ffi_int;
use crate::common::state::{
    alloc_archive, archive_check_magic, archive_magic, clear_error, core_from_archive,
    free_archive, read_disk_from_archive, read_from_archive, set_error_string, sync_backend_core,
    ArchiveCloseCallback, ArchiveKind, ArchiveOpenCallback, ArchivePassphraseCallback,
    ArchiveReadCallback, ArchiveSeekCallback, ArchiveSkipCallback, ArchiveSwitchCallback,
    ReadCallbackNode,
};
use crate::disk::{
    backend_entry_to_custom, native_read_disk_data, native_read_disk_data_block,
    native_read_disk_next_header,
};
use crate::entry::internal::{clear_entry, from_raw};
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
}

fn header_status_has_entry(status: c_int) -> bool {
    matches!(status, ARCHIVE_OK | ARCHIVE_WARN)
}

unsafe fn mirror_archive_error(dst: *mut archive, src: *mut archive) {
    let Some(dst_core) = core_from_archive(dst) else {
        return;
    };
    let Some(src_core) = core_from_archive(src) else {
        return;
    };
    if let Some(error) = src_core.error_string.as_ref() {
        set_error_string(
            dst_core,
            src_core.archive_error_number,
            error.to_string_lossy().into_owned(),
        );
    }
}

const ARCHIVE_FILTER_NONE: c_int = 0;
const ARCHIVE_FILTER_GZIP: c_int = 1;
const ARCHIVE_FILTER_BZIP2: c_int = 2;
const ARCHIVE_FILTER_COMPRESS: c_int = 3;
const ARCHIVE_FILTER_LZMA: c_int = 5;
const ARCHIVE_FILTER_XZ: c_int = 6;
const ARCHIVE_FILTER_UU: c_int = 7;
const ARCHIVE_FILTER_RPM: c_int = 8;
const ARCHIVE_FILTER_LZIP: c_int = 9;
const ARCHIVE_FILTER_LRZIP: c_int = 10;
const ARCHIVE_FILTER_LZOP: c_int = 11;
const ARCHIVE_FILTER_GRZIP: c_int = 12;
const ARCHIVE_FILTER_LZ4: c_int = 13;
const ARCHIVE_FILTER_ZSTD: c_int = 14;

const ARCHIVE_FORMAT_BASE_MASK: c_int = 0xff0000;
const ARCHIVE_FORMAT_CPIO: c_int = 0x10000;
const ARCHIVE_FORMAT_TAR: c_int = 0x30000;
const ARCHIVE_FORMAT_ISO9660: c_int = 0x40000;
const ARCHIVE_FORMAT_ZIP: c_int = 0x50000;
const ARCHIVE_FORMAT_EMPTY: c_int = 0x60000;
const ARCHIVE_FORMAT_AR: c_int = 0x70000;
const ARCHIVE_FORMAT_MTREE: c_int = 0x80000;
const ARCHIVE_FORMAT_RAW: c_int = 0x90000;
const ARCHIVE_FORMAT_XAR: c_int = 0xA0000;
const ARCHIVE_FORMAT_LHA: c_int = 0xB0000;
const ARCHIVE_FORMAT_CAB: c_int = 0xC0000;
const ARCHIVE_FORMAT_RAR: c_int = 0xD0000;
const ARCHIVE_FORMAT_7ZIP: c_int = 0xE0000;
const ARCHIVE_FORMAT_WARC: c_int = 0xF0000;
const ARCHIVE_FORMAT_RAR_V5: c_int = 0x100000;

const ARCHIVE_READ_FORMAT_ENCRYPTION_UNSUPPORTED: c_int = -2;

const PLACEHOLDER_FORMAT_7ZIP: u32 = 1 << 0;
const PLACEHOLDER_FORMAT_CAB: u32 = 1 << 1;
const PLACEHOLDER_FORMAT_ISO9660: u32 = 1 << 2;
const PLACEHOLDER_FORMAT_LHA: u32 = 1 << 3;
const PLACEHOLDER_FORMAT_MTREE: u32 = 1 << 4;
const PLACEHOLDER_FORMAT_WARC: u32 = 1 << 7;
const PLACEHOLDER_FORMAT_XAR: u32 = 1 << 8;

fn validate_read_with_state(
    a: *mut archive,
    function: &str,
    allowed_states: u32,
) -> Option<&'static mut crate::common::state::ReadArchiveHandle> {
    unsafe {
        if archive_check_magic(a, ARCHIVE_READ_MAGIC, allowed_states, function) == ARCHIVE_FATAL {
            return None;
        }
        read_from_archive(a)
    }
}

unsafe fn ensure_read_backend(handle: &mut crate::common::state::ReadArchiveHandle) -> c_int {
    if handle.backend.is_null() {
        handle.backend = (backend_api().archive_read_new)();
        if handle.backend.is_null() {
            set_error_string(
                &mut handle.core,
                libc::ENOMEM,
                "failed to create reader backend".to_string(),
            );
            return ARCHIVE_FATAL;
        }
    }
    ARCHIVE_OK
}

unsafe fn clear_backend_error(handle: &mut crate::common::state::ReadArchiveHandle) {
    if !handle.backend.is_null() {
        (backend_api().archive_clear_error)(handle.backend);
    }
}

unsafe fn finish_reader_status(
    a: *mut archive,
    handle: &mut crate::common::state::ReadArchiveHandle,
    status: c_int,
) -> c_int {
    sync_backend_core(a);
    if status == ARCHIVE_OK {
        handle.backend_opened = true;
        handle.core.state = ARCHIVE_STATE_HEADER;
    } else if status == ARCHIVE_FATAL {
        handle.core.state = ARCHIVE_STATE_FATAL;
    }
    status
}

unsafe fn placeholder_format_warning(
    handle: &mut crate::common::state::ReadArchiveHandle,
    bit: u32,
    name: &str,
) -> c_int {
    handle.placeholder_formats |= bit;
    set_error_string(
        &mut handle.core,
        -1,
        format!("reader support for `{name}` is deferred in this port"),
    );
    ARCHIVE_WARN
}

unsafe extern "C" fn passphrase_callback_shim(
    _backend: *mut BackendArchive,
    client_data: *mut c_void,
) -> *const c_char {
    let handle = &mut *(client_data as *mut crate::common::state::ReadArchiveHandle);
    handle.passphrase_cb.map_or(ptr::null(), |callback| {
        callback(
            (handle as *mut crate::common::state::ReadArchiveHandle).cast(),
            handle.passphrase_client_data,
        )
    })
}

fn emulate_placeholder_format_option(
    handle: &mut crate::common::state::ReadArchiveHandle,
    module: Option<&str>,
    option: Option<&str>,
) -> Option<c_int> {
    if (handle.placeholder_formats & PLACEHOLDER_FORMAT_ISO9660) == 0 {
        return None;
    }
    let module = module.unwrap_or("");
    let option = option.unwrap_or("");
    let known = (module.is_empty() || module == "iso9660") && option == "joliet";
    if known {
        clear_error(&mut handle.core);
        return Some(ARCHIVE_OK);
    }
    if module == "iso9660" {
        set_error_string(
            &mut handle.core,
            -1,
            format!("Undefined option: `iso9660:{option}'"),
        );
        return Some(ARCHIVE_FAILED);
    }
    None
}

fn emulate_placeholder_format_options_string(
    handle: &mut crate::common::state::ReadArchiveHandle,
    options: Option<&str>,
) -> Option<c_int> {
    if (handle.placeholder_formats & PLACEHOLDER_FORMAT_ISO9660) == 0 {
        return None;
    }
    let options = options.unwrap_or("");
    if options.is_empty() || options.chars().all(|ch| ch == ',') {
        clear_error(&mut handle.core);
        return Some(ARCHIVE_OK);
    }

    let mut saw_known = false;
    for token in options.split(',') {
        if token.is_empty() {
            continue;
        }
        let normalized = token.strip_prefix('!').unwrap_or(token);
        if normalized == "joliet" || normalized == "iso9660:joliet" {
            saw_known = true;
            continue;
        }
        if normalized.starts_with("iso9660:") {
            let option = normalized.trim_start_matches("iso9660:");
            set_error_string(
                &mut handle.core,
                -1,
                format!("Undefined option: `iso9660:{option}'"),
            );
            return Some(ARCHIVE_FAILED);
        }
        if saw_known {
            set_error_string(
                &mut handle.core,
                -1,
                format!("Undefined option: `{normalized}'"),
            );
            return Some(ARCHIVE_FAILED);
        }
    }

    if saw_known {
        clear_error(&mut handle.core);
        Some(ARCHIVE_OK)
    } else {
        None
    }
}

fn parse_zisofs_layout_option(value: &str) -> Option<(u8, u64)> {
    let (block_shift, uncompressed_size) = value.split_once(':')?;
    let block_shift = block_shift.parse::<u8>().ok()?;
    let uncompressed_size = uncompressed_size.parse::<u64>().ok()?;
    Some((block_shift, uncompressed_size))
}

fn emulate_security_format_option(
    handle: &mut crate::common::state::ReadArchiveHandle,
    module: Option<&str>,
    option: Option<&str>,
    value: Option<&str>,
) -> Option<c_int> {
    let module = module.unwrap_or("");
    let option = option.unwrap_or("");
    if !module.is_empty() && module != "iso9660" {
        return None;
    }
    if option != "zisofs-layout" {
        return None;
    }

    let Some(value) = value else {
        set_error_string(
            &mut handle.core,
            libc::EINVAL,
            "iso9660:zisofs-layout requires BLOCK_SHIFT:UNCOMPRESSED_SIZE".to_string(),
        );
        return Some(ARCHIVE_FAILED);
    };
    let Some((block_shift, uncompressed_size)) = parse_zisofs_layout_option(value) else {
        set_error_string(
            &mut handle.core,
            libc::EINVAL,
            format!("Invalid iso9660:zisofs-layout value `{value}`"),
        );
        return Some(ARCHIVE_FAILED);
    };
    if crate::read::format::checked_zisofs_layout(block_shift, uncompressed_size, usize::MAX as u64)
        .is_some()
    {
        clear_error(&mut handle.core);
        Some(ARCHIVE_OK)
    } else {
        set_error_string(
            &mut handle.core,
            libc::EOVERFLOW,
            format!(
                "Unsafe zisofs layout for a {}-bit target: block shift {block_shift}, size {uncompressed_size}",
                usize::BITS
            ),
        );
        Some(ARCHIVE_FAILED)
    }
}

macro_rules! backend_reader_filter_support {
    ($name:ident, $field:ident) => {
        #[no_mangle]
        pub extern "C" fn $name(a: *mut archive) -> c_int {
            ffi_int(ARCHIVE_FATAL, || unsafe {
                let Some(handle) =
                    validate_read_with_state(a, stringify!($name), ARCHIVE_STATE_NEW)
                else {
                    return ARCHIVE_FATAL;
                };
                clear_error(&mut handle.core);
                let status = ensure_read_backend(handle);
                if status != ARCHIVE_OK {
                    return status;
                }
                clear_backend_error(handle);
                let status = (backend_api().$field)(handle.backend);
                sync_backend_core(a);
                status
            })
        }
    };
}

macro_rules! backend_reader_format_support {
    ($name:ident, $field:ident) => {
        #[no_mangle]
        pub extern "C" fn $name(a: *mut archive) -> c_int {
            ffi_int(ARCHIVE_FATAL, || unsafe {
                let Some(handle) =
                    validate_read_with_state(a, stringify!($name), ARCHIVE_STATE_NEW)
                else {
                    return ARCHIVE_FATAL;
                };
                clear_error(&mut handle.core);
                let status = ensure_read_backend(handle);
                if status != ARCHIVE_OK {
                    return status;
                }
                clear_backend_error(handle);
                let status = (backend_api().$field)(handle.backend);
                sync_backend_core(a);
                status
            })
        }
    };
}

macro_rules! placeholder_reader_format_support {
    ($name:ident, $bit:expr, $label:literal) => {
        #[no_mangle]
        pub extern "C" fn $name(a: *mut archive) -> c_int {
            ffi_int(ARCHIVE_FATAL, || unsafe {
                let Some(handle) =
                    validate_read_with_state(a, stringify!($name), ARCHIVE_STATE_NEW)
                else {
                    return ARCHIVE_FATAL;
                };
                placeholder_format_warning(handle, $bit, $label)
            })
        }
    };
}

backend_reader_filter_support!(
    archive_read_support_filter_all,
    archive_read_support_filter_all
);
backend_reader_filter_support!(
    archive_read_support_filter_none,
    archive_read_support_filter_none
);
backend_reader_filter_support!(
    archive_read_support_filter_bzip2,
    archive_read_support_filter_bzip2
);
backend_reader_filter_support!(
    archive_read_support_filter_compress,
    archive_read_support_filter_compress
);
backend_reader_filter_support!(
    archive_read_support_filter_gzip,
    archive_read_support_filter_gzip
);
backend_reader_filter_support!(
    archive_read_support_filter_grzip,
    archive_read_support_filter_grzip
);
backend_reader_filter_support!(
    archive_read_support_filter_lrzip,
    archive_read_support_filter_lrzip
);
backend_reader_filter_support!(
    archive_read_support_filter_lz4,
    archive_read_support_filter_lz4
);
backend_reader_filter_support!(
    archive_read_support_filter_lzip,
    archive_read_support_filter_lzip
);
backend_reader_filter_support!(
    archive_read_support_filter_lzma,
    archive_read_support_filter_lzma
);
backend_reader_filter_support!(
    archive_read_support_filter_lzop,
    archive_read_support_filter_lzop
);
backend_reader_filter_support!(
    archive_read_support_filter_rpm,
    archive_read_support_filter_rpm
);
backend_reader_filter_support!(
    archive_read_support_filter_uu,
    archive_read_support_filter_uu
);
backend_reader_filter_support!(
    archive_read_support_filter_xz,
    archive_read_support_filter_xz
);
backend_reader_filter_support!(
    archive_read_support_filter_zstd,
    archive_read_support_filter_zstd
);

backend_reader_format_support!(
    archive_read_support_format_ar,
    archive_read_support_format_ar
);
backend_reader_format_support!(
    archive_read_support_format_cpio,
    archive_read_support_format_cpio
);
backend_reader_format_support!(
    archive_read_support_format_empty,
    archive_read_support_format_empty
);
backend_reader_format_support!(
    archive_read_support_format_gnutar,
    archive_read_support_format_gnutar
);
backend_reader_format_support!(
    archive_read_support_format_raw,
    archive_read_support_format_raw
);
backend_reader_format_support!(
    archive_read_support_format_tar,
    archive_read_support_format_tar
);

backend_reader_format_support!(
    archive_read_support_format_7zip,
    archive_read_support_format_7zip
);
backend_reader_format_support!(
    archive_read_support_format_cab,
    archive_read_support_format_cab
);
backend_reader_format_support!(
    archive_read_support_format_iso9660,
    archive_read_support_format_iso9660
);
backend_reader_format_support!(
    archive_read_support_format_lha,
    archive_read_support_format_lha
);
backend_reader_format_support!(
    archive_read_support_format_rar,
    archive_read_support_format_rar
);
backend_reader_format_support!(
    archive_read_support_format_rar5,
    archive_read_support_format_rar5
);
#[no_mangle]
pub extern "C" fn archive_read_support_compression_all(a: *mut archive) -> c_int {
    crate::common::panic_boundary::ffi_int(crate::common::error::ARCHIVE_FATAL, || unsafe {
        archive_read_support_filter_all(a)
    })
}

#[no_mangle]
pub extern "C" fn archive_read_support_compression_bzip2(a: *mut archive) -> c_int {
    crate::common::panic_boundary::ffi_int(crate::common::error::ARCHIVE_FATAL, || unsafe {
        archive_read_support_filter_bzip2(a)
    })
}

#[no_mangle]
pub extern "C" fn archive_read_support_compression_compress(a: *mut archive) -> c_int {
    crate::common::panic_boundary::ffi_int(crate::common::error::ARCHIVE_FATAL, || unsafe {
        archive_read_support_filter_compress(a)
    })
}

#[no_mangle]
pub extern "C" fn archive_read_support_compression_gzip(a: *mut archive) -> c_int {
    crate::common::panic_boundary::ffi_int(crate::common::error::ARCHIVE_FATAL, || unsafe {
        archive_read_support_filter_gzip(a)
    })
}

#[no_mangle]
pub extern "C" fn archive_read_support_compression_lzip(a: *mut archive) -> c_int {
    crate::common::panic_boundary::ffi_int(crate::common::error::ARCHIVE_FATAL, || unsafe {
        archive_read_support_filter_lzip(a)
    })
}

#[no_mangle]
pub extern "C" fn archive_read_support_compression_lzma(a: *mut archive) -> c_int {
    crate::common::panic_boundary::ffi_int(crate::common::error::ARCHIVE_FATAL, || unsafe {
        archive_read_support_filter_lzma(a)
    })
}

#[no_mangle]
pub extern "C" fn archive_read_support_compression_none(a: *mut archive) -> c_int {
    crate::common::panic_boundary::ffi_int(crate::common::error::ARCHIVE_FATAL, || unsafe {
        archive_read_support_filter_none(a)
    })
}

#[no_mangle]
pub extern "C" fn archive_read_support_compression_program(
    a: *mut archive,
    command: *const c_char,
) -> c_int {
    crate::common::panic_boundary::ffi_int(crate::common::error::ARCHIVE_FATAL, || unsafe {
        archive_read_support_filter_program(a, command)
    })
}

#[no_mangle]
pub extern "C" fn archive_read_support_compression_program_signature(
    a: *mut archive,
    command: *const c_char,
    signature: *const c_void,
    signature_len: size_t,
) -> c_int {
    crate::common::panic_boundary::ffi_int(crate::common::error::ARCHIVE_FATAL, || unsafe {
        archive_read_support_filter_program_signature(a, command, signature, signature_len)
    })
}

#[no_mangle]
pub extern "C" fn archive_read_support_compression_rpm(a: *mut archive) -> c_int {
    crate::common::panic_boundary::ffi_int(crate::common::error::ARCHIVE_FATAL, || unsafe {
        archive_read_support_filter_rpm(a)
    })
}

#[no_mangle]
pub extern "C" fn archive_read_support_compression_uu(a: *mut archive) -> c_int {
    crate::common::panic_boundary::ffi_int(crate::common::error::ARCHIVE_FATAL, || unsafe {
        archive_read_support_filter_uu(a)
    })
}

#[no_mangle]
pub extern "C" fn archive_read_support_compression_xz(a: *mut archive) -> c_int {
    crate::common::panic_boundary::ffi_int(crate::common::error::ARCHIVE_FATAL, || unsafe {
        archive_read_support_filter_xz(a)
    })
}

#[no_mangle]
pub extern "C" fn archive_read_support_filter_program(
    a: *mut archive,
    command: *const c_char,
) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) =
            validate_read_with_state(a, "archive_read_support_filter_program", ARCHIVE_STATE_NEW)
        else {
            return ARCHIVE_FATAL;
        };
        clear_error(&mut handle.core);
        let status = ensure_read_backend(handle);
        if status != ARCHIVE_OK {
            return status;
        }
        clear_backend_error(handle);
        let status = (backend_api().archive_read_support_filter_program)(handle.backend, command);
        sync_backend_core(a);
        status
    })
}

#[no_mangle]
pub extern "C" fn archive_read_support_filter_program_signature(
    a: *mut archive,
    command: *const c_char,
    signature: *const c_void,
    signature_len: size_t,
) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_read_with_state(
            a,
            "archive_read_support_filter_program_signature",
            ARCHIVE_STATE_NEW,
        ) else {
            return ARCHIVE_FATAL;
        };
        clear_error(&mut handle.core);
        let status = ensure_read_backend(handle);
        if status != ARCHIVE_OK {
            return status;
        }
        clear_backend_error(handle);
        let status = (backend_api().archive_read_support_filter_program_signature)(
            handle.backend,
            command,
            signature,
            signature_len,
        );
        sync_backend_core(a);
        status
    })
}

#[no_mangle]
pub extern "C" fn archive_read_support_format_all(a: *mut archive) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) =
            validate_read_with_state(a, "archive_read_support_format_all", ARCHIVE_STATE_NEW)
        else {
            return ARCHIVE_FATAL;
        };
        let mut result = ARCHIVE_OK;
        for status in [
            archive_read_support_format_ar(a),
            archive_read_support_format_cpio(a),
            archive_read_support_format_empty(a),
            archive_read_support_format_tar(a),
            archive_read_support_format_raw(a),
        ] {
            if status <= ARCHIVE_FAILED {
                return status;
            }
            if status < result {
                result = status;
            }
        }
        clear_error(&mut handle.core);
        clear_backend_error(handle);
        ARCHIVE_OK
    })
}

#[no_mangle]
pub extern "C" fn archive_read_open_memory(
    a: *mut archive,
    buffer: *const c_void,
    size: size_t,
) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) =
            validate_read_with_state(a, "archive_read_open_memory", ARCHIVE_STATE_NEW)
        else {
            return ARCHIVE_FATAL;
        };
        clear_error(&mut handle.core);
        let status = ensure_read_backend(handle);
        if status != ARCHIVE_OK {
            return status;
        }
        clear_backend_error(handle);
        let status = (backend_api().archive_read_open_memory)(handle.backend, buffer, size);
        finish_reader_status(a, handle, status)
    })
}

#[no_mangle]
pub extern "C" fn archive_read_open_filename(
    a: *mut archive,
    path: *const c_char,
    block_size: size_t,
) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) =
            validate_read_with_state(a, "archive_read_open_filename", ARCHIVE_STATE_NEW)
        else {
            return ARCHIVE_FATAL;
        };
        clear_error(&mut handle.core);
        let status = ensure_read_backend(handle);
        if status != ARCHIVE_OK {
            return status;
        }
        clear_backend_error(handle);
        let status = (backend_api().archive_read_open_filename)(handle.backend, path, block_size);
        finish_reader_status(a, handle, status)
    })
}

unsafe fn require_open_reader(
    handle: &mut crate::common::state::ReadArchiveHandle,
    function: &str,
) -> c_int {
    if handle.backend.is_null() || !handle.backend_opened {
        set_error_string(
            &mut handle.core,
            -1,
            format!("INTERNAL ERROR: Function '{function}' invoked with archive structure in state 'new'"),
        );
        return ARCHIVE_FATAL;
    }
    ARCHIVE_OK
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
            ReadLike::Archive(reader) => {
                let status = require_open_reader(reader, "archive_read_next_header");
                if status != ARCHIVE_OK {
                    return status;
                }
                clear_error(&mut reader.core);
                if reader.entry.is_null() {
                    reader.entry = crate::entry::internal::new_raw_entry(ptr::null_mut());
                }
                if reader.entry.is_null() {
                    return ARCHIVE_FATAL;
                }
                let mut backend_entry = ptr::null_mut();
                let status =
                    (backend_api().archive_read_next_header)(reader.backend, &mut backend_entry);
                reader.current_entry = backend_entry;
                if !backend_entry.is_null() {
                    let convert_status = backend_entry_to_custom(backend_entry, reader.entry);
                    if convert_status != ARCHIVE_OK {
                        return convert_status;
                    }
                } else if let Some(entry_data) = from_raw(reader.entry) {
                    clear_entry(entry_data);
                }
                if !entry.is_null() {
                    *entry = reader.entry;
                }
                match status {
                    ARCHIVE_OK | ARCHIVE_WARN => reader.core.state = ARCHIVE_STATE_DATA,
                    ARCHIVE_EOF => reader.core.state = ARCHIVE_STATE_EOF,
                    ARCHIVE_FATAL => reader.core.state = ARCHIVE_STATE_FATAL,
                    _ => {}
                }
                sync_backend_core(a);
                return status;
            }
            ReadLike::Disk(reader) => {
                clear_error(&mut reader.core);
                if reader.entry.is_null() {
                    reader.entry = crate::entry::internal::new_raw_entry(ptr::null_mut());
                }
                if reader.entry.is_null() {
                    return ARCHIVE_FATAL;
                }
                let status = native_read_disk_next_header(reader, reader.entry);
                if status != ARCHIVE_OK {
                    if let Some(entry_data) = from_raw(reader.entry) {
                        clear_entry(entry_data);
                    }
                }
                if !entry.is_null() {
                    *entry = reader.entry;
                }
                return status;
            }
        }
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
            ReadLike::Archive(reader) => {
                let status = require_open_reader(reader, "archive_read_next_header2");
                if status != ARCHIVE_OK {
                    return status;
                }
                clear_error(&mut reader.core);
                if let Some(entry_data) = from_raw(entry) {
                    clear_entry(entry_data);
                }
                let mut backend_entry = ptr::null_mut();
                let status =
                    (backend_api().archive_read_next_header)(reader.backend, &mut backend_entry);
                reader.current_entry = backend_entry;
                if !backend_entry.is_null() {
                    let convert_status = backend_entry_to_custom(backend_entry, entry);
                    if convert_status != ARCHIVE_OK {
                        return convert_status;
                    }
                }
                match status {
                    ARCHIVE_OK | ARCHIVE_WARN => reader.core.state = ARCHIVE_STATE_DATA,
                    ARCHIVE_EOF => reader.core.state = ARCHIVE_STATE_EOF,
                    ARCHIVE_FATAL => reader.core.state = ARCHIVE_STATE_FATAL,
                    _ => {}
                }
                sync_backend_core(a);
                return status;
            }
            ReadLike::Disk(reader) => {
                clear_error(&mut reader.core);
                return native_read_disk_next_header(reader, entry);
            }
        }
    })
}

#[no_mangle]
pub extern "C" fn archive_read_data(a: *mut archive, buffer: *mut c_void, size: size_t) -> isize {
    crate::common::panic_boundary::ffi_value(
        crate::common::error::ARCHIVE_FATAL as isize,
        || unsafe {
            unsafe {
                let Some(mut handle) = ReadLike::from_archive(a, "archive_read_data") else {
                    return ARCHIVE_FATAL as isize;
                };
                match &mut handle {
                    ReadLike::Archive(reader) => {
                        let status = require_open_reader(reader, "archive_read_data");
                        if status != ARCHIVE_OK {
                            return status as isize;
                        }
                        let status =
                            (backend_api().archive_read_data)(reader.backend, buffer, size);
                        sync_backend_core(a);
                        return status;
                    }
                    ReadLike::Disk(reader) => native_read_disk_data(reader, buffer, size),
                }
            }
        },
    )
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
            ReadLike::Archive(reader) => {
                let status = require_open_reader(reader, "archive_read_data_block");
                if status != ARCHIVE_OK {
                    return status;
                }
                let status =
                    (backend_api().archive_read_data_block)(reader.backend, buffer, size, offset);
                sync_backend_core(a);
                return status;
            }
            ReadLike::Disk(reader) => native_read_disk_data_block(reader, buffer, size, offset),
        }
    })
}

#[no_mangle]
pub extern "C" fn archive_read_header_position(a: *mut archive) -> i64 {
    crate::common::panic_boundary::ffi_value(0, || unsafe {
        unsafe {
            let Some(handle) = validate_read_with_state(a, "archive_read_header_position", !0)
            else {
                return ARCHIVE_FATAL as i64;
            };
            if handle.backend.is_null() {
                0
            } else {
                (backend_api().archive_read_header_position)(handle.backend)
            }
        }
    })
}

#[no_mangle]
pub extern "C" fn archive_read_has_encrypted_entries(a: *mut archive) -> c_int {
    crate::common::panic_boundary::ffi_int(crate::common::error::ARCHIVE_FATAL, || unsafe {
        unsafe {
            let Some(handle) =
                validate_read_with_state(a, "archive_read_has_encrypted_entries", !0)
            else {
                return ARCHIVE_FATAL;
            };
            if handle.backend.is_null() {
                ARCHIVE_READ_FORMAT_ENCRYPTION_UNSUPPORTED
            } else {
                (backend_api().archive_read_has_encrypted_entries)(handle.backend)
            }
        }
    })
}

#[no_mangle]
pub extern "C" fn archive_read_format_capabilities(a: *mut archive) -> c_int {
    crate::common::panic_boundary::ffi_int(0, || unsafe {
        unsafe {
            let Some(handle) = validate_read_with_state(a, "archive_read_format_capabilities", !0)
            else {
                return ARCHIVE_FATAL;
            };
            if handle.backend.is_null() {
                0
            } else {
                (backend_api().archive_read_format_capabilities)(handle.backend)
            }
        }
    })
}

#[no_mangle]
pub extern "C" fn archive_seek_data(a: *mut archive, offset: i64, whence: c_int) -> i64 {
    crate::common::panic_boundary::ffi_value(
        crate::common::error::ARCHIVE_FATAL as i64,
        || unsafe {
            unsafe {
                let Some(handle) = validate_read_with_state(a, "archive_seek_data", !0) else {
                    return ARCHIVE_FATAL as i64;
                };
                if handle.backend.is_null() || !handle.backend_opened {
                    set_error_string(
                        &mut handle.core,
                        -1,
                        "No archive is currently open".to_string(),
                    );
                    return ARCHIVE_FATAL as i64;
                }
                (backend_api().archive_seek_data)(handle.backend, offset, whence)
            }
        },
    )
}

#[no_mangle]
pub extern "C" fn archive_read_data_skip(a: *mut archive) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(mut handle) = ReadLike::from_archive(a, "archive_read_data_skip") else {
            return ARCHIVE_FATAL;
        };
        match &mut handle {
            ReadLike::Archive(reader) => {
                let status = require_open_reader(reader, "archive_read_data_skip");
                if status != ARCHIVE_OK {
                    return status;
                }
                let status = (backend_api().archive_read_data_skip)(reader.backend);
                if status == ARCHIVE_OK {
                    reader.core.state = ARCHIVE_STATE_HEADER;
                } else if status == ARCHIVE_FATAL {
                    reader.core.state = ARCHIVE_STATE_FATAL;
                }
                sync_backend_core(a);
                status
            }
            ReadLike::Disk(reader) => {
                let mut saw_block = false;
                let mut previous_offset = 0u64;
                let mut previous_size = 0u64;
                let mut block = ptr::null();
                let mut block_size = 0usize;
                let mut block_offset = 0i64;
                loop {
                    let status = native_read_disk_data_block(
                        reader,
                        &mut block,
                        &mut block_size,
                        &mut block_offset,
                    );
                    if status == ARCHIVE_EOF {
                        return ARCHIVE_OK;
                    }
                    if status != ARCHIVE_OK {
                        return status;
                    }
                    let Some(current_offset) = u64::try_from(block_offset).ok() else {
                        set_error_string(
                            &mut reader.core,
                            libc::EINVAL,
                            "disk reader returned a negative block offset".to_string(),
                        );
                        return ARCHIVE_FATAL;
                    };
                    let current_size = block_size as u64;
                    if saw_block {
                        if !crate::read::format::monotonic_seek_ok(
                            previous_offset,
                            current_offset,
                            i64::MAX as u64,
                        ) || !crate::read::format::forward_progress(
                            previous_offset,
                            current_offset,
                            previous_size,
                            current_size,
                        ) {
                            set_error_string(
                                &mut reader.core,
                                libc::EIO,
                                "disk reader made no forward progress while skipping data"
                                    .to_string(),
                            );
                            return ARCHIVE_FATAL;
                        }
                    } else if current_size == 0 {
                        set_error_string(
                            &mut reader.core,
                            libc::EIO,
                            "disk reader returned an empty block while skipping data".to_string(),
                        );
                        return ARCHIVE_FATAL;
                    }
                    saw_block = true;
                    previous_offset = current_offset;
                    previous_size = current_size;
                }
            }
        }
    })
}

#[no_mangle]
pub extern "C" fn archive_read_data_into_fd(a: *mut archive, fd: c_int) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(mut handle) = ReadLike::from_archive(a, "archive_read_data_into_fd") else {
            return ARCHIVE_FATAL;
        };
        match &mut handle {
            ReadLike::Archive(reader) => {
                let status = require_open_reader(reader, "archive_read_data_into_fd");
                if status != ARCHIVE_OK {
                    return status;
                }
                let status = (backend_api().archive_read_data_into_fd)(reader.backend, fd);
                sync_backend_core(a);
                status
            }
            ReadLike::Disk(_) => ARCHIVE_FATAL,
        }
    })
}

#[no_mangle]
pub extern "C" fn archive_read_set_format_option(
    a: *mut archive,
    module: *const c_char,
    option: *const c_char,
    value: *const c_char,
) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) =
            validate_read_with_state(a, "archive_read_set_format_option", ARCHIVE_STATE_NEW)
        else {
            return ARCHIVE_FATAL;
        };
        if let Some(status) = emulate_security_format_option(
            handle,
            from_optional_c_str(module).as_deref(),
            from_optional_c_str(option).as_deref(),
            from_optional_c_str(value).as_deref(),
        ) {
            clear_backend_error(handle);
            return status;
        }
        if let Some(status) = emulate_placeholder_format_option(
            handle,
            from_optional_c_str(module).as_deref(),
            from_optional_c_str(option).as_deref(),
        ) {
            clear_backend_error(handle);
            return status;
        }
        clear_error(&mut handle.core);
        let status = ensure_read_backend(handle);
        if status != ARCHIVE_OK {
            return status;
        }
        clear_backend_error(handle);
        let status =
            (backend_api().archive_read_set_format_option)(handle.backend, module, option, value);
        sync_backend_core(a);
        status
    })
}

#[no_mangle]
pub extern "C" fn archive_read_set_filter_option(
    a: *mut archive,
    module: *const c_char,
    option: *const c_char,
    value: *const c_char,
) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) =
            validate_read_with_state(a, "archive_read_set_filter_option", ARCHIVE_STATE_NEW)
        else {
            return ARCHIVE_FATAL;
        };
        clear_error(&mut handle.core);
        let status = ensure_read_backend(handle);
        if status != ARCHIVE_OK {
            return status;
        }
        clear_backend_error(handle);
        let status =
            (backend_api().archive_read_set_filter_option)(handle.backend, module, option, value);
        sync_backend_core(a);
        status
    })
}

#[no_mangle]
pub extern "C" fn archive_read_set_option(
    a: *mut archive,
    module: *const c_char,
    option: *const c_char,
    value: *const c_char,
) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) =
            validate_read_with_state(a, "archive_read_set_option", ARCHIVE_STATE_NEW)
        else {
            return ARCHIVE_FATAL;
        };
        if let Some(status) = emulate_security_format_option(
            handle,
            from_optional_c_str(module).as_deref(),
            from_optional_c_str(option).as_deref(),
            from_optional_c_str(value).as_deref(),
        ) {
            clear_backend_error(handle);
            return status;
        }
        if let Some(status) = emulate_placeholder_format_option(
            handle,
            from_optional_c_str(module).as_deref(),
            from_optional_c_str(option).as_deref(),
        ) {
            clear_backend_error(handle);
            return status;
        }
        clear_error(&mut handle.core);
        let status = ensure_read_backend(handle);
        if status != ARCHIVE_OK {
            return status;
        }
        clear_backend_error(handle);
        let status = (backend_api().archive_read_set_option)(handle.backend, module, option, value);
        sync_backend_core(a);
        status
    })
}

#[no_mangle]
pub extern "C" fn archive_read_set_options(a: *mut archive, options: *const c_char) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) =
            validate_read_with_state(a, "archive_read_set_options", ARCHIVE_STATE_NEW)
        else {
            return ARCHIVE_FATAL;
        };
        if let Some(status) = emulate_placeholder_format_options_string(
            handle,
            from_optional_c_str(options).as_deref(),
        ) {
            clear_backend_error(handle);
            return status;
        }
        clear_error(&mut handle.core);
        let status = ensure_read_backend(handle);
        if status != ARCHIVE_OK {
            return status;
        }
        clear_backend_error(handle);
        let status = (backend_api().archive_read_set_options)(handle.backend, options);
        sync_backend_core(a);
        status
    })
}

#[no_mangle]
pub extern "C" fn archive_read_add_passphrase(a: *mut archive, passphrase: *const c_char) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) =
            validate_read_with_state(a, "archive_read_add_passphrase", ARCHIVE_STATE_NEW)
        else {
            return ARCHIVE_FATAL;
        };
        clear_error(&mut handle.core);
        let status = ensure_read_backend(handle);
        if status != ARCHIVE_OK {
            return status;
        }
        clear_backend_error(handle);
        let status = (backend_api().archive_read_add_passphrase)(handle.backend, passphrase);
        sync_backend_core(a);
        status
    })
}

#[no_mangle]
pub extern "C" fn archive_read_set_passphrase_callback(
    a: *mut archive,
    client_data: *mut c_void,
    callback: Option<ArchivePassphraseCallback>,
) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) =
            validate_read_with_state(a, "archive_read_set_passphrase_callback", ARCHIVE_STATE_NEW)
        else {
            return ARCHIVE_FATAL;
        };
        handle.passphrase_client_data = client_data;
        handle.passphrase_cb = callback;
        let status = ensure_read_backend(handle);
        if status != ARCHIVE_OK {
            return status;
        }
        clear_backend_error(handle);
        (backend_api().archive_read_set_passphrase_callback)(
            handle.backend,
            (handle as *mut crate::common::state::ReadArchiveHandle).cast(),
            callback.map(|_| {
                passphrase_callback_shim
                    as unsafe extern "C" fn(*mut BackendArchive, *mut c_void) -> *const c_char
            }),
        )
    })
}

#[no_mangle]
pub extern "C" fn archive_read_extract_set_progress_callback(
    a: *mut archive,
    progress_func: Option<unsafe extern "C" fn(*mut c_void)>,
    user_data: *mut c_void,
) {
    crate::common::panic_boundary::ffi_void(|| unsafe {
        unsafe {
            let Some(handle) = validate_read_with_state(
                a,
                "archive_read_extract_set_progress_callback",
                crate::common::error::ARCHIVE_STATE_ANY,
            ) else {
                return;
            };
            handle.extract_progress = progress_func;
            handle.extract_progress_user_data = user_data;
        }
    })
}

#[no_mangle]
pub extern "C" fn archive_read_extract_set_skip_file(a: *mut archive, dev: i64, ino: i64) {
    crate::common::panic_boundary::ffi_void(|| unsafe {
        unsafe {
            let Some(handle) = validate_read_with_state(
                a,
                "archive_read_extract_set_skip_file",
                crate::common::error::ARCHIVE_STATE_ANY,
            ) else {
                return;
            };
            handle.extract_skip_file = Some((dev, ino));
        }
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
        let Some(handle) = validate_read_with_state(a, "archive_read_extract2", !0) else {
            return ARCHIVE_FATAL;
        };
        if crate::common::state::write_disk_from_archive(disk).is_none() {
            return ARCHIVE_FATAL;
        }
        if let Some((dev, ino)) = handle.extract_skip_file {
            let status = crate::disk::archive_write_disk_set_skip_file(disk, dev, ino);
            if status != ARCHIVE_OK {
                return status;
            }
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
            mirror_archive_error(a, disk);
            return status;
        }

        let mut saw_block = false;
        let mut previous_offset = 0u64;
        let mut previous_size = 0u64;
        loop {
            let mut block = ptr::null();
            let mut block_size = 0usize;
            let mut block_offset = 0i64;
            let status = archive_read_data_block(a, &mut block, &mut block_size, &mut block_offset);
            if status == ARCHIVE_EOF {
                let finish_status = crate::write::archive_write_finish_entry(disk);
                if finish_status != ARCHIVE_OK {
                    mirror_archive_error(a, disk);
                }
                return finish_status;
            }
            if status != ARCHIVE_OK {
                return status;
            }
            if let Some(progress) = handle.extract_progress {
                progress(handle.extract_progress_user_data);
            }
            let Some(current_offset) = u64::try_from(block_offset).ok() else {
                set_error_string(
                    &mut handle.core,
                    libc::EINVAL,
                    "reader returned a negative block offset during extraction".to_string(),
                );
                return ARCHIVE_FATAL;
            };
            let current_size = block_size as u64;
            if saw_block {
                if !crate::read::format::monotonic_seek_ok(
                    previous_offset,
                    current_offset,
                    i64::MAX as u64,
                ) || !crate::read::format::forward_progress(
                    previous_offset,
                    current_offset,
                    previous_size,
                    current_size,
                ) {
                    set_error_string(
                        &mut handle.core,
                        libc::EIO,
                        "reader made no forward progress while extracting data".to_string(),
                    );
                    return ARCHIVE_FATAL;
                }
            } else if current_size == 0 {
                set_error_string(
                    &mut handle.core,
                    libc::EIO,
                    "reader returned an empty block while extracting data".to_string(),
                );
                return ARCHIVE_FATAL;
            }
            saw_block = true;
            previous_offset = current_offset;
            previous_size = current_size;
            let write_status =
                crate::write::archive_write_data_block(disk, block, block_size, block_offset);
            if write_status < 0 {
                mirror_archive_error(a, disk);
                return write_status as c_int;
            }
        }
    })
}
