#![allow(dead_code)]

use std::ffi::{c_char, c_void, CStr};

use archive::common::error::ARCHIVE_OK;
use archive::ffi::archive_entry_api as entry;
use archive::ffi::archive_read as read;

pub unsafe fn entry_pathname(entry_ptr: *mut archive::ffi::archive_entry) -> String {
    CStr::from_ptr(entry::archive_entry_pathname(entry_ptr))
        .to_string_lossy()
        .into_owned()
}

pub struct CallbackReader {
    pub bytes: Vec<u8>,
    pub cursor: usize,
    pub chunk_size: usize,
    pub scratch: Vec<u8>,
    pub opens: usize,
    pub closes: usize,
    pub read_calls: usize,
}

impl CallbackReader {
    pub fn new(bytes: &[u8], chunk_size: usize) -> Self {
        Self {
            bytes: bytes.to_vec(),
            cursor: 0,
            chunk_size,
            scratch: Vec::new(),
            opens: 0,
            closes: 0,
            read_calls: 0,
        }
    }
}

pub unsafe extern "C" fn open_callback(
    _archive: *mut archive::ffi::archive,
    client_data: *mut c_void,
) -> i32 {
    let state = &mut *(client_data as *mut CallbackReader);
    state.opens += 1;
    state.cursor = 0;
    ARCHIVE_OK
}

pub unsafe extern "C" fn read_callback(
    _archive: *mut archive::ffi::archive,
    client_data: *mut c_void,
    buffer: *mut *const c_void,
) -> isize {
    let state = &mut *(client_data as *mut CallbackReader);
    if state.cursor >= state.bytes.len() {
        if !buffer.is_null() {
            *buffer = std::ptr::null();
        }
        return 0;
    }
    let end = (state.cursor + state.chunk_size).min(state.bytes.len());
    state.scratch.clear();
    state
        .scratch
        .extend_from_slice(&state.bytes[state.cursor..end]);
    state.cursor = end;
    state.read_calls += 1;
    if !buffer.is_null() {
        *buffer = state.scratch.as_ptr().cast();
    }
    state.scratch.len() as isize
}

pub unsafe extern "C" fn skip_callback(
    _archive: *mut archive::ffi::archive,
    client_data: *mut c_void,
    request: i64,
) -> i64 {
    let state = &mut *(client_data as *mut CallbackReader);
    let remaining = state.bytes.len().saturating_sub(state.cursor);
    let skip = remaining.min(request.max(0) as usize);
    state.cursor += skip;
    skip as i64
}

pub unsafe extern "C" fn close_callback(
    _archive: *mut archive::ffi::archive,
    client_data: *mut c_void,
) -> i32 {
    let state = &mut *(client_data as *mut CallbackReader);
    state.closes += 1;
    ARCHIVE_OK
}

pub unsafe extern "C" fn passphrase_callback(
    _archive: *mut archive::ffi::archive,
    _client_data: *mut c_void,
) -> *const c_char {
    c"secret".as_ptr()
}

pub unsafe extern "C" fn switch_callback(
    _archive: *mut archive::ffi::archive,
    _client_data1: *mut c_void,
    _client_data2: *mut c_void,
) -> i32 {
    ARCHIVE_OK
}

pub unsafe fn open_reader_with_callbacks(
    reader: *mut archive::ffi::archive,
    state: *mut CallbackReader,
) -> i32 {
    assert_eq!(ARCHIVE_OK, read::archive_read_support_filter_all(reader));
    assert_eq!(ARCHIVE_OK, read::archive_read_support_format_all(reader));
    read::archive_read_open2(
        reader,
        state.cast(),
        Some(open_callback),
        Some(read_callback),
        Some(skip_callback),
        Some(close_callback),
    )
}

pub unsafe fn configure_reader_callbacks(
    reader: *mut archive::ffi::archive,
    state: *mut CallbackReader,
) {
    assert_eq!(
        ARCHIVE_OK,
        read::archive_read_set_callback_data(reader, state.cast())
    );
    assert_eq!(
        ARCHIVE_OK,
        read::archive_read_set_open_callback(reader, Some(open_callback))
    );
    assert_eq!(
        ARCHIVE_OK,
        read::archive_read_set_read_callback(reader, Some(read_callback))
    );
    assert_eq!(
        ARCHIVE_OK,
        read::archive_read_set_skip_callback(reader, Some(skip_callback))
    );
    assert_eq!(
        ARCHIVE_OK,
        read::archive_read_set_close_callback(reader, Some(close_callback))
    );
}
