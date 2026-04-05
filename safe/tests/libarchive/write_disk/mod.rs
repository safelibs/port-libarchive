#![allow(dead_code)]

use std::ffi::{c_int, c_void, CString};
use std::ptr;

use archive::common::error::{ARCHIVE_EOF, ARCHIVE_OK};
use archive::ffi::archive_common as common;
use archive::ffi::archive_entry_api as entry;
use archive::ffi::archive_read as read;
use archive::ffi::archive_write as write;

pub const ARCHIVE_EXTRACT_SECURE_SYMLINKS: c_int = 0x0100;
pub const ARCHIVE_EXTRACT_SECURE_NODOTDOT: c_int = 0x0200;
pub const ARCHIVE_EXTRACT_SECURE_NOABSOLUTEPATHS: c_int = 0x10000;
pub const ARCHIVE_EXTRACT_SAFE_WRITES: c_int = 0x40000;
pub const ARCHIVE_READDISK_HONOR_NODUMP: c_int = 0x0002;

pub struct MemoryArchive {
    pub buffer: Vec<u8>,
    pub used: usize,
}

impl MemoryArchive {
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: vec![0; capacity],
            used: 0,
        }
    }
}

pub unsafe fn write_single_file_archive(pathname: &str, contents: &[u8]) -> MemoryArchive {
    let mut archive = MemoryArchive::new(64 * 1024);
    let writer = write::archive_write_new();
    assert!(!writer.is_null());
    assert_eq!(ARCHIVE_OK, write::archive_write_set_format_pax(writer));
    assert_eq!(ARCHIVE_OK, write::archive_write_add_filter_none(writer));
    assert_eq!(
        ARCHIVE_OK,
        write::archive_write_open_memory(
            writer,
            archive.buffer.as_mut_ptr().cast(),
            archive.buffer.len(),
            &mut archive.used,
        )
    );

    let entry = entry::archive_entry_new();
    assert!(!entry.is_null());
    let pathname = CString::new(pathname).unwrap();
    entry::archive_entry_copy_pathname(entry, pathname.as_ptr());
    entry::archive_entry_set_mode(entry, entry::AE_IFREG | 0o644);
    entry::archive_entry_set_size(entry, contents.len() as i64);
    assert_eq!(ARCHIVE_OK, write::archive_write_header(writer, entry));
    assert_eq!(
        contents.len() as isize,
        write::archive_write_data(writer, contents.as_ptr().cast(), contents.len())
    );
    assert_eq!(ARCHIVE_OK, write::archive_write_close(writer));
    assert_eq!(ARCHIVE_OK, write::archive_write_free(writer));
    entry::archive_entry_free(entry);
    archive
}

pub struct CallbackBuffer {
    pub bytes: Vec<u8>,
}

pub unsafe extern "C" fn short_write_callback(
    _a: *mut archive::ffi::archive,
    client_data: *mut c_void,
    buffer: *const c_void,
    length: usize,
) -> isize {
    let state = &mut *(client_data as *mut CallbackBuffer);
    let chunk = length.min(17);
    let slice = std::slice::from_raw_parts(buffer.cast::<u8>(), chunk);
    state.bytes.extend_from_slice(slice);
    chunk as isize
}

pub unsafe fn read_first_file_from_memory(archive_bytes: &[u8]) -> (String, Vec<u8>) {
    let reader = read::archive_read_new();
    assert!(!reader.is_null());
    assert_eq!(ARCHIVE_OK, read::archive_read_support_filter_all(reader));
    assert_eq!(ARCHIVE_OK, read::archive_read_support_format_all(reader));
    assert_eq!(
        ARCHIVE_OK,
        read::archive_read_open_memory(reader, archive_bytes.as_ptr().cast(), archive_bytes.len())
    );

    let mut raw_entry = ptr::null_mut();
    assert_eq!(
        ARCHIVE_OK,
        read::archive_read_next_header(reader, &mut raw_entry)
    );
    let pathname = unsafe { std::ffi::CStr::from_ptr(entry::archive_entry_pathname(raw_entry)) }
        .to_string_lossy()
        .into_owned();

    let mut data = vec![0; 256];
    let size = read::archive_read_data(reader, data.as_mut_ptr().cast(), data.len());
    assert!(size >= 0);
    data.truncate(size as usize);

    assert_eq!(
        ARCHIVE_EOF,
        read::archive_read_next_header(reader, &mut raw_entry)
    );
    assert_eq!(ARCHIVE_OK, common::archive_read_free(reader));
    (pathname, data)
}
