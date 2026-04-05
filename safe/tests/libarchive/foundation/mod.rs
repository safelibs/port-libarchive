#![allow(dead_code)]

use archive::entry::EntryHandle;
use archive::ffi::archive_entry_api as entry_ffi;

pub fn regular_entry(path: &str, mode: libc::mode_t) -> EntryHandle {
    let mut entry = EntryHandle::new();
    entry.set_pathname(path);
    entry.set_mode(entry_ffi::AE_IFREG | mode);
    entry
}
