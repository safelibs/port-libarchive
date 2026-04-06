use std::ffi::CString;
use std::ptr;

use archive::common::error::{ARCHIVE_EOF, ARCHIVE_OK};
use archive::ffi::archive_common as common;
use archive::ffi::archive_entry_api as entry;
use archive::ffi::archive_read as read;
use archive::ffi::archive_write as write;

pub unsafe fn write_single_entry_archive<F>(
    pathname: &str,
    contents: &[u8],
    configure: F,
) -> Vec<u8>
where
    F: FnOnce(*mut archive::ffi::archive),
{
    let writer = write::archive_write_new();
    assert!(!writer.is_null());
    configure(writer);
    assert_eq!(ARCHIVE_OK, write::archive_write_add_filter_none(writer));

    let mut buffer = vec![0u8; 1024 * 1024];
    let mut used = 0usize;
    assert_eq!(
        ARCHIVE_OK,
        write::archive_write_open_memory(
            writer,
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            &mut used,
        )
    );

    let raw_entry = entry::archive_entry_new();
    assert!(!raw_entry.is_null());
    let pathname = CString::new(pathname).unwrap();
    entry::archive_entry_copy_pathname(raw_entry, pathname.as_ptr());
    entry::archive_entry_set_mode(raw_entry, entry::AE_IFREG | 0o644);
    entry::archive_entry_set_size(raw_entry, contents.len() as i64);
    assert_eq!(ARCHIVE_OK, write::archive_write_header(writer, raw_entry));
    if !contents.is_empty() {
        assert_eq!(
            contents.len() as isize,
            write::archive_write_data(writer, contents.as_ptr().cast(), contents.len())
        );
    }
    assert_eq!(ARCHIVE_OK, write::archive_write_close(writer));
    assert_eq!(ARCHIVE_OK, write::archive_write_free(writer));
    entry::archive_entry_free(raw_entry);
    buffer.truncate(used);
    buffer
}

pub unsafe fn first_entry_from_memory(bytes: &[u8]) -> (String, Vec<u8>) {
    first_entry_from_memory_with_reader(bytes, |_| {})
}

pub unsafe fn first_entry_from_memory_with_reader<F>(
    bytes: &[u8],
    configure: F,
) -> (String, Vec<u8>)
where
    F: FnOnce(*mut archive::ffi::archive),
{
    let reader = read::archive_read_new();
    assert!(!reader.is_null());
    assert_eq!(ARCHIVE_OK, read::archive_read_support_filter_all(reader));
    assert_eq!(ARCHIVE_OK, read::archive_read_support_format_all(reader));
    configure(reader);
    assert_eq!(
        ARCHIVE_OK,
        read::archive_read_open_memory(reader, bytes.as_ptr().cast(), bytes.len())
    );

    let mut raw_entry = ptr::null_mut();
    loop {
        assert_eq!(
            ARCHIVE_OK,
            read::archive_read_next_header(reader, &mut raw_entry)
        );
        if entry::archive_entry_filetype(raw_entry) == entry::AE_IFDIR {
            assert_eq!(ARCHIVE_OK, read::archive_read_data_skip(reader));
            continue;
        }

        let pathname = std::ffi::CStr::from_ptr(entry::archive_entry_pathname(raw_entry))
            .to_string_lossy()
            .into_owned();
        let mut data = vec![0u8; 4096];
        let read_size = read::archive_read_data(reader, data.as_mut_ptr().cast(), data.len());
        assert!(read_size >= 0);
        data.truncate(read_size as usize);

        assert_eq!(ARCHIVE_OK, common::archive_read_free(reader));
        return (pathname, data);
    }
}
