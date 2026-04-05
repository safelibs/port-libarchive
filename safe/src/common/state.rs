use std::ffi::{c_char, c_int, c_longlong, c_uint, c_ulong, c_void, CString};
use std::fs;
use std::path::PathBuf;
use std::ptr;

use crate::common::error::{
    ARCHIVE_EOF, ARCHIVE_FAILED, ARCHIVE_OK, ARCHIVE_READ_DISK_MAGIC, ARCHIVE_READ_MAGIC,
    ARCHIVE_STATE_CLOSED, ARCHIVE_STATE_NEW, ARCHIVE_WRITE_DISK_MAGIC, ARCHIVE_WRITE_MAGIC,
};
use crate::ffi::{archive, archive_entry};

#[repr(C)]
pub(crate) struct ArchiveString {
    pub(crate) s: *mut c_char,
    pub(crate) length: usize,
    pub(crate) buffer_length: usize,
}

type ArchiveCloseFn = unsafe extern "C" fn(*mut archive) -> c_int;
type ArchiveFreeFn = unsafe extern "C" fn(*mut archive) -> c_int;
type ArchiveWriteHeaderFn = unsafe extern "C" fn(*mut archive, *mut archive_entry) -> c_int;
type ArchiveWriteFinishEntryFn = unsafe extern "C" fn(*mut archive) -> c_int;
type ArchiveWriteDataFn = unsafe extern "C" fn(*mut archive, *const c_void, usize) -> isize;
type ArchiveWriteDataBlockFn =
    unsafe extern "C" fn(*mut archive, *const c_void, usize, c_longlong) -> isize;
type ArchiveReadNextHeaderFn =
    unsafe extern "C" fn(*mut archive, *mut *mut archive_entry) -> c_int;
type ArchiveReadNextHeader2Fn = unsafe extern "C" fn(*mut archive, *mut archive_entry) -> c_int;
type ArchiveReadDataBlockFn =
    unsafe extern "C" fn(*mut archive, *mut *const c_void, *mut usize, *mut c_longlong) -> c_int;
type ArchiveFilterCountFn = unsafe extern "C" fn(*mut archive) -> c_int;
type ArchiveFilterBytesFn = unsafe extern "C" fn(*mut archive, c_int) -> c_longlong;
type ArchiveFilterCodeFn = unsafe extern "C" fn(*mut archive, c_int) -> c_int;
type ArchiveFilterNameFn = unsafe extern "C" fn(*mut archive, c_int) -> *const c_char;

#[repr(C)]
pub(crate) struct ArchiveVTable {
    pub(crate) archive_close: ArchiveCloseFn,
    pub(crate) archive_free: ArchiveFreeFn,
    pub(crate) archive_write_header: Option<ArchiveWriteHeaderFn>,
    pub(crate) archive_write_finish_entry: Option<ArchiveWriteFinishEntryFn>,
    pub(crate) archive_write_data: Option<ArchiveWriteDataFn>,
    pub(crate) archive_write_data_block: Option<ArchiveWriteDataBlockFn>,
    pub(crate) archive_read_next_header: Option<ArchiveReadNextHeaderFn>,
    pub(crate) archive_read_next_header2: Option<ArchiveReadNextHeader2Fn>,
    pub(crate) archive_read_data_block: Option<ArchiveReadDataBlockFn>,
    pub(crate) archive_filter_count: ArchiveFilterCountFn,
    pub(crate) archive_filter_bytes: ArchiveFilterBytesFn,
    pub(crate) archive_filter_code: ArchiveFilterCodeFn,
    pub(crate) archive_filter_name: ArchiveFilterNameFn,
}

#[repr(C)]
pub(crate) struct ArchiveHandle {
    pub(crate) magic: c_uint,
    pub(crate) state: c_uint,
    pub(crate) vtable: *const ArchiveVTable,
    pub(crate) archive_format: c_int,
    pub(crate) archive_format_name: *const c_char,
    pub(crate) file_count: c_int,
    pub(crate) archive_error_number: c_int,
    pub(crate) error: *const c_char,
    pub(crate) error_string: ArchiveString,
    pub(crate) current_code: *mut c_char,
    pub(crate) current_codepage: c_uint,
    pub(crate) current_oemcp: c_uint,
    pub(crate) sconv: *mut c_void,
    pub(crate) read_data_block: *const c_char,
    pub(crate) read_data_offset: c_longlong,
    pub(crate) read_data_output_offset: c_longlong,
    pub(crate) read_data_remaining: usize,
    pub(crate) read_data_is_posix_read: c_char,
    pub(crate) read_data_requested: usize,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum ArchiveKind {
    Read,
    Write,
    ReadDisk,
    WriteDisk,
}

impl ArchiveKind {
    fn magic(self) -> c_uint {
        match self {
            Self::Read => ARCHIVE_READ_MAGIC,
            Self::Write => ARCHIVE_WRITE_MAGIC,
            Self::ReadDisk => ARCHIVE_READ_DISK_MAGIC,
            Self::WriteDisk => ARCHIVE_WRITE_DISK_MAGIC,
        }
    }
}

impl ArchiveHandle {
    fn new(kind: ArchiveKind) -> Self {
        Self {
            magic: kind.magic(),
            state: ARCHIVE_STATE_NEW,
            vtable: ptr::addr_of!(ARCHIVE_STUB_VTABLE),
            archive_format: 0,
            archive_format_name: ptr::null(),
            file_count: 0,
            archive_error_number: 0,
            error: ptr::null(),
            error_string: ArchiveString {
                s: ptr::null_mut(),
                length: 0,
                buffer_length: 0,
            },
            current_code: ptr::null_mut(),
            current_codepage: 0,
            current_oemcp: 0,
            sconv: ptr::null_mut(),
            read_data_block: ptr::null(),
            read_data_offset: 0,
            read_data_output_offset: 0,
            read_data_remaining: 0,
            read_data_is_posix_read: 0,
            read_data_requested: 0,
        }
    }
}

#[repr(C)]
struct ReadArchiveHandle {
    base: ArchiveHandle,
    data: Vec<u8>,
    header_emitted: bool,
    data_emitted: bool,
    entry: *mut archive_entry,
}

unsafe extern "C" {
    fn archive_string_free(astring: *mut ArchiveString);
    fn archive_entry_new() -> *mut archive_entry;
    fn archive_entry_free(entry: *mut archive_entry);
}

unsafe extern "C" fn archive_stub_close(a: *mut archive) -> c_int {
    if a.is_null() {
        return ARCHIVE_OK;
    }

    let handle = &mut *a.cast::<ArchiveHandle>();
    handle.state = ARCHIVE_STATE_CLOSED;
    ARCHIVE_OK
}

unsafe extern "C" fn archive_stub_free(a: *mut archive) -> c_int {
    if a.is_null() {
        return ARCHIVE_OK;
    }

    let handle = a.cast::<ArchiveHandle>();
    archive_string_free(ptr::addr_of_mut!((*handle).error_string));
    drop(Box::from_raw(handle));
    ARCHIVE_OK
}

unsafe extern "C" fn archive_read_free_impl(a: *mut archive) -> c_int {
    if a.is_null() {
        return ARCHIVE_OK;
    }

    let handle = a.cast::<ReadArchiveHandle>();
    if !(*handle).entry.is_null() {
        archive_entry_free((*handle).entry);
    }
    archive_string_free(ptr::addr_of_mut!((*handle).base.error_string));
    drop(Box::from_raw(handle));
    ARCHIVE_OK
}

unsafe extern "C" fn archive_read_next_header_impl(
    a: *mut archive,
    entry: *mut *mut archive_entry,
) -> c_int {
    if a.is_null() {
        return ARCHIVE_FAILED;
    }

    let handle = &mut *a.cast::<ReadArchiveHandle>();
    if handle.header_emitted {
        if !entry.is_null() {
            *entry = ptr::null_mut();
        }
        return ARCHIVE_EOF;
    }
    if handle.data.is_empty() {
        handle.header_emitted = true;
        if !entry.is_null() {
            *entry = ptr::null_mut();
        }
        return ARCHIVE_EOF;
    }

    if handle.entry.is_null() {
        handle.entry = archive_entry_new();
        if handle.entry.is_null() {
            return ARCHIVE_FAILED;
        }
    }
    handle.header_emitted = true;
    if !entry.is_null() {
        *entry = handle.entry;
    }
    ARCHIVE_OK
}

unsafe extern "C" fn archive_read_data_block_impl(
    a: *mut archive,
    buff: *mut *const c_void,
    size: *mut usize,
    offset: *mut c_longlong,
) -> c_int {
    if a.is_null() {
        return ARCHIVE_FAILED;
    }

    let handle = &mut *a.cast::<ReadArchiveHandle>();
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
        *buff = handle.data.as_ptr().cast::<c_void>();
    }
    if !size.is_null() {
        *size = handle.data.len();
    }
    if !offset.is_null() {
        *offset = 0;
    }
    ARCHIVE_OK
}

unsafe extern "C" fn archive_stub_filter_count(_a: *mut archive) -> c_int {
    0
}

unsafe extern "C" fn archive_stub_filter_bytes(_a: *mut archive, _n: c_int) -> c_longlong {
    0
}

unsafe extern "C" fn archive_stub_filter_code(_a: *mut archive, _n: c_int) -> c_int {
    0
}

unsafe extern "C" fn archive_stub_filter_name(_a: *mut archive, _n: c_int) -> *const c_char {
    ptr::null()
}

static ARCHIVE_STUB_VTABLE: ArchiveVTable = ArchiveVTable {
    archive_close: archive_stub_close,
    archive_free: archive_stub_free,
    archive_write_header: None,
    archive_write_finish_entry: None,
    archive_write_data: None,
    archive_write_data_block: None,
    archive_read_next_header: None,
    archive_read_next_header2: None,
    archive_read_data_block: None,
    archive_filter_count: archive_stub_filter_count,
    archive_filter_bytes: archive_stub_filter_bytes,
    archive_filter_code: archive_stub_filter_code,
    archive_filter_name: archive_stub_filter_name,
};

static ARCHIVE_READ_VTABLE: ArchiveVTable = ArchiveVTable {
    archive_close: archive_stub_close,
    archive_free: archive_read_free_impl,
    archive_write_header: None,
    archive_write_finish_entry: None,
    archive_write_data: None,
    archive_write_data_block: None,
    archive_read_next_header: Some(archive_read_next_header_impl),
    archive_read_next_header2: None,
    archive_read_data_block: Some(archive_read_data_block_impl),
    archive_filter_count: archive_stub_filter_count,
    archive_filter_bytes: archive_stub_filter_bytes,
    archive_filter_code: archive_stub_filter_code,
    archive_filter_name: archive_stub_filter_name,
};

pub(crate) fn alloc_archive(kind: ArchiveKind) -> *mut archive {
    match kind {
        ArchiveKind::Read => Box::into_raw(Box::new(ReadArchiveHandle {
            base: ArchiveHandle {
                vtable: ptr::addr_of!(ARCHIVE_READ_VTABLE),
                ..ArchiveHandle::new(kind)
            },
            data: Vec::new(),
            header_emitted: false,
            data_emitted: false,
            entry: ptr::null_mut(),
        })) as *mut archive,
        _ => Box::into_raw(Box::new(ArchiveHandle::new(kind))) as *mut archive,
    }
}

pub(crate) unsafe fn read_archive_support_format(_a: *mut archive) -> c_int {
    ARCHIVE_OK
}

pub(crate) unsafe fn read_archive_open_filename(a: *mut archive, path: *const c_char) -> c_int {
    if a.is_null() || path.is_null() {
        return ARCHIVE_FAILED;
    }

    let path = match std::ffi::CStr::from_ptr(path).to_str() {
        Ok(path) => PathBuf::from(path),
        Err(_) => return ARCHIVE_FAILED,
    };
    read_archive_load_path(a, &path)
}

pub(crate) unsafe fn read_archive_open_filename_w(
    a: *mut archive,
    path: *const libc::wchar_t,
) -> c_int {
    if a.is_null() || path.is_null() {
        return ARCHIVE_FAILED;
    }

    let mut chars = Vec::new();
    let mut current = path;
    while *current != 0 {
        chars.push(std::char::from_u32(*current as u32).unwrap_or(char::REPLACEMENT_CHARACTER));
        current = current.add(1);
    }
    let path: String = chars.into_iter().collect();
    read_archive_load_path(a, &PathBuf::from(path))
}

unsafe fn read_archive_load_path(a: *mut archive, path: &PathBuf) -> c_int {
    let handle = &mut *a.cast::<ReadArchiveHandle>();
    match fs::read(path) {
        Ok(data) => {
            handle.data = data;
            handle.header_emitted = false;
            handle.data_emitted = false;
            ARCHIVE_OK
        }
        Err(_) => ARCHIVE_FAILED,
    }
}
