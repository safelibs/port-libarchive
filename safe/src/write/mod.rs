use std::ffi::{c_char, c_int, c_void};
use std::ptr;

use libc::size_t;

use crate::common::backend::{api as backend_api, BackendArchive};
use crate::common::error::{ARCHIVE_FATAL, ARCHIVE_OK};
use crate::common::panic_boundary::ffi_int;
use crate::common::state::{
    archive_check_magic, archive_magic, clear_error, sync_backend_core, write_disk_from_archive,
    write_from_archive, ArchiveCloseCallback, ArchiveFreeCallback, ArchiveOpenCallback,
    ArchiveWriteCallback,
};
use crate::disk::custom_entry_to_backend;
use crate::ffi::{archive, archive_entry};

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

macro_rules! writer_call0 {
    ($name:ident, $backend:ident) => {
        #[no_mangle]
        pub extern "C" fn $name(a: *mut archive) -> c_int {
            ffi_int(ARCHIVE_FATAL, || unsafe {
                let Some(handle) = validate_writer(a, stringify!($name)) else {
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

macro_rules! writer_call1_int {
    ($name:ident, $backend:ident, $ty:ty) => {
        #[no_mangle]
        pub extern "C" fn $name(a: *mut archive, value: $ty) -> c_int {
            ffi_int(ARCHIVE_FATAL, || unsafe {
                let Some(handle) = validate_writer(a, stringify!($name)) else {
                    return ARCHIVE_FATAL;
                };
                clear_error(&mut handle.core);
                let status = (backend_api().$backend)(handle.backend, value);
                sync_backend_core(a);
                status
            })
        }
    };
}

writer_call1_int!(
    archive_write_set_bytes_per_block,
    archive_write_set_bytes_per_block,
    c_int
);
writer_call1_int!(
    archive_write_set_bytes_in_last_block,
    archive_write_set_bytes_in_last_block,
    c_int
);
writer_call0!(
    archive_write_add_filter_b64encode,
    archive_write_add_filter_b64encode
);
writer_call0!(
    archive_write_add_filter_bzip2,
    archive_write_add_filter_bzip2
);
writer_call0!(
    archive_write_add_filter_compress,
    archive_write_add_filter_compress
);
writer_call0!(
    archive_write_add_filter_grzip,
    archive_write_add_filter_grzip
);
writer_call0!(archive_write_add_filter_gzip, archive_write_add_filter_gzip);
writer_call0!(
    archive_write_add_filter_lrzip,
    archive_write_add_filter_lrzip
);
writer_call0!(archive_write_add_filter_lz4, archive_write_add_filter_lz4);
writer_call0!(archive_write_add_filter_lzip, archive_write_add_filter_lzip);
writer_call0!(archive_write_add_filter_lzma, archive_write_add_filter_lzma);
writer_call0!(archive_write_add_filter_lzop, archive_write_add_filter_lzop);
writer_call0!(archive_write_add_filter_none, archive_write_add_filter_none);
writer_call0!(
    archive_write_add_filter_uuencode,
    archive_write_add_filter_uuencode
);
writer_call0!(archive_write_add_filter_xz, archive_write_add_filter_xz);
writer_call0!(archive_write_add_filter_zstd, archive_write_add_filter_zstd);
writer_call0!(
    archive_write_set_format_ar_bsd,
    archive_write_set_format_ar_bsd
);
writer_call0!(
    archive_write_set_format_ar_svr4,
    archive_write_set_format_ar_svr4
);
writer_call0!(archive_write_set_format_cpio, archive_write_set_format_cpio);
writer_call0!(
    archive_write_set_format_cpio_bin,
    archive_write_set_format_cpio_bin
);
writer_call0!(
    archive_write_set_format_cpio_newc,
    archive_write_set_format_cpio_newc
);
writer_call0!(
    archive_write_set_format_cpio_odc,
    archive_write_set_format_cpio_odc
);
writer_call0!(
    archive_write_set_format_cpio_pwb,
    archive_write_set_format_cpio_pwb
);
writer_call0!(
    archive_write_set_format_gnutar,
    archive_write_set_format_gnutar
);
writer_call0!(archive_write_set_format_pax, archive_write_set_format_pax);
writer_call0!(
    archive_write_set_format_pax_restricted,
    archive_write_set_format_pax_restricted
);
writer_call0!(archive_write_set_format_raw, archive_write_set_format_raw);
writer_call0!(archive_write_set_format_shar, archive_write_set_format_shar);
writer_call0!(
    archive_write_set_format_shar_dump,
    archive_write_set_format_shar_dump
);
writer_call0!(
    archive_write_set_format_ustar,
    archive_write_set_format_ustar
);
writer_call0!(
    archive_write_set_format_v7tar,
    archive_write_set_format_v7tar
);
writer_call0!(archive_write_fail, archive_write_fail);

#[no_mangle]
pub extern "C" fn archive_write_get_bytes_per_block(a: *mut archive) -> c_int {
    unsafe {
        let Some(handle) = validate_writer(a, "archive_write_get_bytes_per_block") else {
            return ARCHIVE_FATAL;
        };
        (backend_api().archive_write_get_bytes_per_block)(handle.backend)
    }
}

#[no_mangle]
pub extern "C" fn archive_write_get_bytes_in_last_block(a: *mut archive) -> c_int {
    unsafe {
        let Some(handle) = validate_writer(a, "archive_write_get_bytes_in_last_block") else {
            return ARCHIVE_FATAL;
        };
        (backend_api().archive_write_get_bytes_in_last_block)(handle.backend)
    }
}

#[no_mangle]
pub extern "C" fn archive_write_set_skip_file(a: *mut archive, dev: i64, ino: i64) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_writer(a, "archive_write_set_skip_file") else {
            return ARCHIVE_FATAL;
        };
        (backend_api().archive_write_set_skip_file)(handle.backend, dev, ino)
    })
}

#[no_mangle]
pub extern "C" fn archive_write_add_filter(a: *mut archive, filter_code: c_int) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_writer(a, "archive_write_add_filter") else {
            return ARCHIVE_FATAL;
        };
        clear_error(&mut handle.core);
        let status = (backend_api().archive_write_add_filter)(handle.backend, filter_code);
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
        clear_error(&mut handle.core);
        let status = (backend_api().archive_write_add_filter_by_name)(handle.backend, filter_name);
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
        clear_error(&mut handle.core);
        let status = (backend_api().archive_write_add_filter_program)(handle.backend, command);
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
        let status = (backend_api().archive_write_set_format)(handle.backend, format_code);
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
        clear_error(&mut handle.core);
        let status = (backend_api().archive_write_set_format_by_name)(handle.backend, format_name);
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
        (backend_api().archive_write_set_format_filter_by_ext)(handle.backend, filename)
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
        (backend_api().archive_write_set_format_filter_by_ext_def)(
            handle.backend,
            filename,
            default_ext,
        )
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
        (backend_api().archive_write_open)(
            handle.backend,
            (handle as *mut crate::common::state::WriteArchiveHandle).cast(),
            open_cb.map(|_| {
                open_callback_shim
                    as unsafe extern "C" fn(*mut BackendArchive, *mut c_void) -> c_int
            }),
            write_cb.map(|_| {
                write_callback_shim
                    as unsafe extern "C" fn(
                        *mut BackendArchive,
                        *mut c_void,
                        *const c_void,
                        size_t,
                    ) -> isize
            }),
            close_cb.map(|_| {
                close_callback_shim
                    as unsafe extern "C" fn(*mut BackendArchive, *mut c_void) -> c_int
            }),
        )
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
        (backend_api().archive_write_open2)(
            handle.backend,
            (handle as *mut crate::common::state::WriteArchiveHandle).cast(),
            open_cb.map(|_| {
                open_callback_shim
                    as unsafe extern "C" fn(*mut BackendArchive, *mut c_void) -> c_int
            }),
            write_cb.map(|_| {
                write_callback_shim
                    as unsafe extern "C" fn(
                        *mut BackendArchive,
                        *mut c_void,
                        *const c_void,
                        size_t,
                    ) -> isize
            }),
            close_cb.map(|_| {
                close_callback_shim
                    as unsafe extern "C" fn(*mut BackendArchive, *mut c_void) -> c_int
            }),
            free_cb.map(|_| {
                free_callback_shim
                    as unsafe extern "C" fn(*mut BackendArchive, *mut c_void) -> c_int
            }),
        )
    })
}

#[no_mangle]
pub extern "C" fn archive_write_open_filename(a: *mut archive, file: *const c_char) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_writer(a, "archive_write_open_filename") else {
            return ARCHIVE_FATAL;
        };
        clear_error(&mut handle.core);
        let status = (backend_api().archive_write_open_filename)(handle.backend, file);
        sync_backend_core(a);
        status
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
        let status = (backend_api().archive_write_open_memory)(handle.backend, buffer, size, used);
        sync_backend_core(a);
        status
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
        (backend_api().archive_write_set_filter_option)(handle.backend, module, option, value)
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
        (backend_api().archive_write_set_format_option)(handle.backend, module, option, value)
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
        (backend_api().archive_write_set_option)(handle.backend, module, option, value)
    })
}

#[no_mangle]
pub extern "C" fn archive_write_set_options(a: *mut archive, options: *const c_char) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_writer(a, "archive_write_set_options") else {
            return ARCHIVE_FATAL;
        };
        (backend_api().archive_write_set_options)(handle.backend, options)
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
        (backend_api().archive_write_set_passphrase)(handle.backend, passphrase)
    })
}
