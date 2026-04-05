use std::ffi::{c_char, c_int, CString};
use std::fs;
use std::path::PathBuf;
use std::ptr;

use libc::{size_t, wchar_t};

use crate::common::error::{
    ARCHIVE_EOF, ARCHIVE_FAILED, ARCHIVE_FATAL, ARCHIVE_MATCH_MAGIC, ARCHIVE_OK,
    ARCHIVE_READ_DISK_MAGIC, ARCHIVE_READ_MAGIC, ARCHIVE_STATE_ANY, ARCHIVE_STATE_CLOSED,
    ARCHIVE_STATE_FATAL, ARCHIVE_STATE_NEW, ARCHIVE_WRITE_DISK_MAGIC, ARCHIVE_WRITE_MAGIC,
};
use crate::common::helpers::{from_optional_c_str, from_optional_wide};
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
    pub(crate) data: Vec<u8>,
    pub(crate) header_emitted: bool,
    pub(crate) data_emitted: bool,
    pub(crate) entry: *mut archive_entry,
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

pub(crate) fn alloc_archive(kind: ArchiveKind) -> *mut archive {
    match kind {
        ArchiveKind::Read => Box::into_raw(Box::new(ReadArchiveHandle {
            core: ArchiveCore::new(kind),
            data: Vec::new(),
            header_emitted: false,
            data_emitted: false,
            entry: ptr::null_mut(),
        })) as *mut archive,
        _ => Box::into_raw(Box::new(ArchiveBaseHandle {
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
            format!(
                "PROGRAMMER ERROR: Function '{function}' invoked on wrong archive object"
            ),
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
    core.error_string = message
        .map(|message| CString::new(message).expect("error message must not contain NUL"));
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
            drop(Box::from_raw(handle));
        }
        ARCHIVE_MATCH_MAGIC => {
            crate::r#match::internal::free_match_archive(a);
        }
        ARCHIVE_WRITE_MAGIC | ARCHIVE_READ_DISK_MAGIC | ARCHIVE_WRITE_DISK_MAGIC => {
            drop(Box::from_raw(a.cast::<ArchiveBaseHandle>()));
        }
        _ => return ARCHIVE_FATAL,
    }

    ARCHIVE_OK
}

pub(crate) unsafe fn close_archive(a: *mut archive) -> c_int {
    let Some(core) = core_from_archive(a) else {
        return ARCHIVE_FATAL;
    };
    core.state = ARCHIVE_STATE_CLOSED;
    ARCHIVE_OK
}

pub(crate) unsafe fn read_archive_support_format(a: *mut archive) -> c_int {
    if archive_check_magic(a, ARCHIVE_READ_MAGIC, ARCHIVE_STATE_ANY, "archive_read_support_format")
        == ARCHIVE_FATAL
    {
        return ARCHIVE_FATAL;
    }
    ARCHIVE_OK
}

pub(crate) unsafe fn read_archive_open_filename(
    a: *mut archive,
    path: *const c_char,
) -> c_int {
    if path.is_null() {
        return ARCHIVE_FAILED;
    }
    let Some(path) = from_optional_c_str(path) else {
        return ARCHIVE_FAILED;
    };
    read_archive_load_path(a, &PathBuf::from(path))
}

pub(crate) unsafe fn read_archive_open_filename_w(
    a: *mut archive,
    path: *const wchar_t,
) -> c_int {
    if path.is_null() {
        return ARCHIVE_FAILED;
    }
    let Some(path) = from_optional_wide(path) else {
        return ARCHIVE_FAILED;
    };
    read_archive_load_path(a, &PathBuf::from(path))
}

unsafe fn read_archive_load_path(a: *mut archive, path: &PathBuf) -> c_int {
    if archive_check_magic(a, ARCHIVE_READ_MAGIC, ARCHIVE_STATE_ANY, "archive_read_open_filename")
        == ARCHIVE_FATAL
    {
        return ARCHIVE_FATAL;
    }
    let Some(handle) = read_from_archive(a) else {
        return ARCHIVE_FATAL;
    };

    match fs::read(path) {
        Ok(data) => {
            handle.data = data;
            handle.header_emitted = false;
            handle.data_emitted = false;
            ARCHIVE_OK
        }
        Err(err) => {
            set_error_string(&mut handle.core, err.raw_os_error().unwrap_or(-1), err.to_string());
            ARCHIVE_FAILED
        }
    }
}

pub(crate) unsafe fn read_archive_next_header(
    a: *mut archive,
    entry: *mut *mut archive_entry,
) -> c_int {
    if archive_check_magic(a, ARCHIVE_READ_MAGIC, ARCHIVE_STATE_ANY, "archive_read_next_header")
        == ARCHIVE_FATAL
    {
        return ARCHIVE_FATAL;
    }
    let Some(handle) = read_from_archive(a) else {
        return ARCHIVE_FATAL;
    };

    if handle.header_emitted || handle.data.is_empty() {
        if !entry.is_null() {
            *entry = ptr::null_mut();
        }
        return ARCHIVE_EOF;
    }

    if handle.entry.is_null() {
        handle.entry = crate::entry::internal::new_raw_entry(ptr::null_mut());
        if handle.entry.is_null() {
            return ARCHIVE_FATAL;
        }
    }
    handle.header_emitted = true;
    if !entry.is_null() {
        *entry = handle.entry;
    }
    ARCHIVE_OK
}

pub(crate) unsafe fn read_archive_data_block(
    a: *mut archive,
    buff: *mut *const std::ffi::c_void,
    size: *mut size_t,
    offset: *mut i64,
) -> c_int {
    if archive_check_magic(a, ARCHIVE_READ_MAGIC, ARCHIVE_STATE_ANY, "archive_read_data_block")
        == ARCHIVE_FATAL
    {
        return ARCHIVE_FATAL;
    }
    let Some(handle) = read_from_archive(a) else {
        return ARCHIVE_FATAL;
    };

    if handle.data_emitted {
        if !buff.is_null() {
            *buff = ptr::null();
        }
        if !size.is_null() {
            *size = 0;
        }
        if !offset.is_null() {
            *offset = 0;
        }
        return ARCHIVE_EOF;
    }

    handle.data_emitted = true;
    if !buff.is_null() {
        *buff = handle.data.as_ptr().cast();
    }
    if !size.is_null() {
        *size = handle.data.len();
    }
    if !offset.is_null() {
        *offset = 0;
    }
    ARCHIVE_OK
}
