use std::ffi::{c_char, c_int, c_void, CString};
use std::ptr;

use libc::size_t;

use crate::common::api::ensure_variadic_shim_initialized;
use crate::common::backend::{api as backend_api, BackendArchive, BackendEntry};
use crate::common::error::{
    ARCHIVE_FATAL, ARCHIVE_MATCH_MAGIC, ARCHIVE_OK, ARCHIVE_READ_DISK_MAGIC, ARCHIVE_READ_MAGIC,
    ARCHIVE_STATE_ANY, ARCHIVE_STATE_CLOSED, ARCHIVE_STATE_FATAL, ARCHIVE_STATE_NEW,
    ARCHIVE_WRITE_DISK_MAGIC, ARCHIVE_WRITE_MAGIC,
};
use crate::ffi::{archive, archive_entry};

#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ArchiveKind {
    Read = ARCHIVE_READ_MAGIC,
    Write = ARCHIVE_WRITE_MAGIC,
    ReadDisk = ARCHIVE_READ_DISK_MAGIC,
    WriteDisk = ARCHIVE_WRITE_DISK_MAGIC,
    Match = ARCHIVE_MATCH_MAGIC,
}

pub(crate) type ArchiveOpenCallback = unsafe extern "C" fn(*mut archive, *mut c_void) -> c_int;
pub(crate) type ArchiveWriteCallback =
    unsafe extern "C" fn(*mut archive, *mut c_void, *const c_void, size_t) -> isize;
pub(crate) type ArchiveCloseCallback = unsafe extern "C" fn(*mut archive, *mut c_void) -> c_int;
pub(crate) type ArchiveFreeCallback = unsafe extern "C" fn(*mut archive, *mut c_void) -> c_int;
pub(crate) type ReadDiskExcludedCallback =
    unsafe extern "C" fn(*mut archive, *mut c_void, *mut archive_entry);
pub(crate) type ReadDiskMetadataFilterCallback =
    unsafe extern "C" fn(*mut archive, *mut c_void, *mut archive_entry) -> c_int;

#[repr(C)]
pub(crate) struct ArchiveCore {
    pub(crate) magic: u32,
    pub(crate) state: u32,
    pub(crate) archive_format: c_int,
    pub(crate) archive_format_name: Option<CString>,
    pub(crate) file_count: c_int,
    pub(crate) archive_error_number: c_int,
    pub(crate) error_string: Option<CString>,
    pub(crate) position_compressed: i64,
    pub(crate) position_uncompressed: i64,
}

#[repr(C)]
pub(crate) struct ArchiveBaseHandle {
    pub(crate) core: ArchiveCore,
}

#[repr(C)]
pub(crate) struct ReadArchiveHandle {
    pub(crate) core: ArchiveCore,
    pub(crate) backend: *mut BackendArchive,
    pub(crate) entry: *mut archive_entry,
    pub(crate) current_entry: *mut BackendEntry,
}

#[repr(C)]
pub(crate) struct WriteArchiveHandle {
    pub(crate) core: ArchiveCore,
    pub(crate) backend: *mut BackendArchive,
    pub(crate) client_data: *mut c_void,
    pub(crate) open_cb: Option<ArchiveOpenCallback>,
    pub(crate) write_cb: Option<ArchiveWriteCallback>,
    pub(crate) close_cb: Option<ArchiveCloseCallback>,
    pub(crate) free_cb: Option<ArchiveFreeCallback>,
}

#[repr(C)]
pub(crate) struct ReadDiskArchiveHandle {
    pub(crate) core: ArchiveCore,
    pub(crate) backend: *mut BackendArchive,
    pub(crate) entry: *mut archive_entry,
    pub(crate) current_entry: *mut BackendEntry,
    pub(crate) backend_match: *mut BackendArchive,
    pub(crate) excluded_cb: Option<ReadDiskExcludedCallback>,
    pub(crate) excluded_client_data: *mut c_void,
    pub(crate) metadata_filter_cb: Option<ReadDiskMetadataFilterCallback>,
    pub(crate) metadata_filter_client_data: *mut c_void,
}

#[repr(C)]
pub(crate) struct WriteDiskArchiveHandle {
    pub(crate) core: ArchiveCore,
    pub(crate) backend: *mut BackendArchive,
}

impl ArchiveCore {
    pub(crate) fn new(kind: ArchiveKind) -> Self {
        Self {
            magic: kind as u32,
            state: ARCHIVE_STATE_NEW,
            archive_format: 0,
            archive_format_name: None,
            file_count: 0,
            archive_error_number: 0,
            error_string: None,
            position_compressed: 0,
            position_uncompressed: 0,
        }
    }
}

pub(crate) unsafe fn core_from_archive<'a>(a: *mut archive) -> Option<&'a mut ArchiveCore> {
    a.cast::<ArchiveCore>().as_mut()
}

pub(crate) unsafe fn read_from_archive<'a>(a: *mut archive) -> Option<&'a mut ReadArchiveHandle> {
    a.cast::<ReadArchiveHandle>().as_mut()
}

pub(crate) unsafe fn write_from_archive<'a>(a: *mut archive) -> Option<&'a mut WriteArchiveHandle> {
    a.cast::<WriteArchiveHandle>().as_mut()
}

pub(crate) unsafe fn read_disk_from_archive<'a>(
    a: *mut archive,
) -> Option<&'a mut ReadDiskArchiveHandle> {
    a.cast::<ReadDiskArchiveHandle>().as_mut()
}

pub(crate) unsafe fn write_disk_from_archive<'a>(
    a: *mut archive,
) -> Option<&'a mut WriteDiskArchiveHandle> {
    a.cast::<WriteDiskArchiveHandle>().as_mut()
}

pub(crate) unsafe fn backend_archive(a: *mut archive) -> *mut BackendArchive {
    match archive_magic(a) {
        ARCHIVE_READ_MAGIC => read_from_archive(a).map_or(ptr::null_mut(), |handle| handle.backend),
        ARCHIVE_WRITE_MAGIC => {
            write_from_archive(a).map_or(ptr::null_mut(), |handle| handle.backend)
        }
        ARCHIVE_READ_DISK_MAGIC => {
            read_disk_from_archive(a).map_or(ptr::null_mut(), |handle| handle.backend)
        }
        ARCHIVE_WRITE_DISK_MAGIC => {
            write_disk_from_archive(a).map_or(ptr::null_mut(), |handle| handle.backend)
        }
        _ => ptr::null_mut(),
    }
}

pub(crate) fn alloc_archive(kind: ArchiveKind) -> *mut archive {
    ensure_variadic_shim_initialized();
    match kind {
        ArchiveKind::Read => {
            let backend = unsafe { (backend_api().archive_read_new)() };
            if backend.is_null() {
                return ptr::null_mut();
            }
            Box::into_raw(Box::new(ReadArchiveHandle {
                core: ArchiveCore::new(kind),
                backend,
                entry: ptr::null_mut(),
                current_entry: ptr::null_mut(),
            })) as *mut archive
        }
        ArchiveKind::Write => {
            let backend = unsafe { (backend_api().archive_write_new)() };
            if backend.is_null() {
                return ptr::null_mut();
            }
            Box::into_raw(Box::new(WriteArchiveHandle {
                core: ArchiveCore::new(kind),
                backend,
                client_data: ptr::null_mut(),
                open_cb: None,
                write_cb: None,
                close_cb: None,
                free_cb: None,
            })) as *mut archive
        }
        ArchiveKind::ReadDisk => {
            let backend = unsafe { (backend_api().archive_read_disk_new)() };
            if backend.is_null() {
                return ptr::null_mut();
            }
            Box::into_raw(Box::new(ReadDiskArchiveHandle {
                core: ArchiveCore::new(kind),
                backend,
                entry: ptr::null_mut(),
                current_entry: ptr::null_mut(),
                backend_match: ptr::null_mut(),
                excluded_cb: None,
                excluded_client_data: ptr::null_mut(),
                metadata_filter_cb: None,
                metadata_filter_client_data: ptr::null_mut(),
            })) as *mut archive
        }
        ArchiveKind::WriteDisk => {
            let backend = unsafe { (backend_api().archive_write_disk_new)() };
            if backend.is_null() {
                return ptr::null_mut();
            }
            Box::into_raw(Box::new(WriteDiskArchiveHandle {
                core: ArchiveCore::new(kind),
                backend,
            })) as *mut archive
        }
        ArchiveKind::Match => Box::into_raw(Box::new(ArchiveBaseHandle {
            core: ArchiveCore::new(kind),
        })) as *mut archive,
    }
}

pub(crate) unsafe fn archive_magic(a: *mut archive) -> u32 {
    core_from_archive(a).map_or(0, |core| core.magic)
}

pub(crate) unsafe fn archive_check_magic(
    a: *mut archive,
    expected_magic: u32,
    allowed_states: u32,
    function: &str,
) -> c_int {
    let Some(core) = core_from_archive(a) else {
        return ARCHIVE_FATAL;
    };

    if core.magic != expected_magic {
        set_error_string(
            core,
            -1,
            format!("PROGRAMMER ERROR: Function '{function}' invoked on wrong archive object"),
        );
        core.state = ARCHIVE_STATE_FATAL;
        return ARCHIVE_FATAL;
    }

    if (core.state & allowed_states) == 0 {
        if core.state != ARCHIVE_STATE_FATAL {
            set_error_string(
                core,
                -1,
                format!(
                    "INTERNAL ERROR: Function '{function}' invoked with archive structure in state '{}'",
                    state_name(core.state)
                ),
            );
        }
        core.state = ARCHIVE_STATE_FATAL;
        return ARCHIVE_FATAL;
    }

    ARCHIVE_OK
}

fn state_name(state: u32) -> &'static str {
    match state {
        crate::common::error::ARCHIVE_STATE_NEW => "new",
        crate::common::error::ARCHIVE_STATE_HEADER => "header",
        crate::common::error::ARCHIVE_STATE_DATA => "data",
        crate::common::error::ARCHIVE_STATE_EOF => "eof",
        crate::common::error::ARCHIVE_STATE_CLOSED => "closed",
        crate::common::error::ARCHIVE_STATE_FATAL => "fatal",
        _ => "unknown",
    }
}

pub(crate) fn set_error_string(core: &mut ArchiveCore, errno: c_int, message: String) {
    core.archive_error_number = errno;
    core.error_string = Some(CString::new(message).expect("error message must not contain NUL"));
}

pub(crate) fn set_error_option(core: &mut ArchiveCore, errno: c_int, message: Option<String>) {
    core.archive_error_number = errno;
    core.error_string =
        message.map(|message| CString::new(message).expect("error message must not contain NUL"));
}

pub(crate) fn clear_error(core: &mut ArchiveCore) {
    core.archive_error_number = 0;
    core.error_string = None;
}

pub(crate) fn error_string_ptr(core: &ArchiveCore) -> *const c_char {
    core.error_string
        .as_ref()
        .map_or(ptr::null(), |value| value.as_ptr())
}

pub(crate) unsafe fn free_archive(a: *mut archive) -> c_int {
    if a.is_null() {
        return ARCHIVE_OK;
    }

    match archive_magic(a) {
        ARCHIVE_READ_MAGIC => {
            let handle = a.cast::<ReadArchiveHandle>();
            if !(*handle).entry.is_null() {
                crate::entry::internal::free_raw_entry((*handle).entry);
            }
            if !(*handle).backend.is_null() {
                (backend_api().archive_read_free)((*handle).backend);
            }
            drop(Box::from_raw(handle));
        }
        ARCHIVE_WRITE_MAGIC => {
            let handle = a.cast::<WriteArchiveHandle>();
            if !(*handle).backend.is_null() {
                (backend_api().archive_write_free)((*handle).backend);
            }
            drop(Box::from_raw(handle));
        }
        ARCHIVE_READ_DISK_MAGIC => {
            let handle = a.cast::<ReadDiskArchiveHandle>();
            if !(*handle).entry.is_null() {
                crate::entry::internal::free_raw_entry((*handle).entry);
            }
            if !(*handle).backend_match.is_null() {
                (backend_api().archive_match_free)((*handle).backend_match);
            }
            if !(*handle).backend.is_null() {
                (backend_api().archive_read_free)((*handle).backend);
            }
            drop(Box::from_raw(handle));
        }
        ARCHIVE_WRITE_DISK_MAGIC => {
            let handle = a.cast::<WriteDiskArchiveHandle>();
            if !(*handle).backend.is_null() {
                (backend_api().archive_write_free)((*handle).backend);
            }
            drop(Box::from_raw(handle));
        }
        ARCHIVE_MATCH_MAGIC => {
            crate::r#match::internal::free_match_archive(a);
        }
        _ => return ARCHIVE_FATAL,
    }

    ARCHIVE_OK
}

pub(crate) unsafe fn close_archive(a: *mut archive) -> c_int {
    let Some(core) = core_from_archive(a) else {
        return ARCHIVE_FATAL;
    };

    let status = match core.magic {
        ARCHIVE_READ_MAGIC | ARCHIVE_READ_DISK_MAGIC => {
            let backend = backend_archive(a);
            if backend.is_null() {
                ARCHIVE_OK
            } else {
                (backend_api().archive_read_close)(backend)
            }
        }
        ARCHIVE_WRITE_MAGIC | ARCHIVE_WRITE_DISK_MAGIC => {
            let backend = backend_archive(a);
            if backend.is_null() {
                ARCHIVE_OK
            } else {
                (backend_api().archive_write_close)(backend)
            }
        }
        _ => ARCHIVE_OK,
    };
    if status == ARCHIVE_OK {
        core.state = ARCHIVE_STATE_CLOSED;
    }
    status
}

pub(crate) unsafe fn sync_backend_core(a: *mut archive) {
    let Some(core) = core_from_archive(a) else {
        return;
    };
    if !matches!(core.magic, ARCHIVE_READ_MAGIC | ARCHIVE_WRITE_MAGIC) {
        return;
    }
    let backend = backend_archive(a);
    if backend.is_null() {
        return;
    }
    core.archive_format = (backend_api().archive_format)(backend);
    core.file_count = (backend_api().archive_file_count)(backend);
    core.position_compressed = (backend_api().archive_position_compressed)(backend);
    core.position_uncompressed = (backend_api().archive_position_uncompressed)(backend);
}

pub(crate) unsafe fn backend_error_number(a: *mut archive) -> c_int {
    let backend = backend_archive(a);
    if backend.is_null() {
        0
    } else {
        (backend_api().archive_errno)(backend)
    }
}

pub(crate) unsafe fn backend_error_string_ptr(a: *mut archive) -> *const c_char {
    let backend = backend_archive(a);
    if backend.is_null() {
        ptr::null()
    } else {
        (backend_api().archive_error_string)(backend)
    }
}
