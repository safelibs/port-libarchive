#[repr(C)]
pub struct archive {
    _private: [u8; 0],
}

#[repr(C)]
pub struct archive_entry {
    _private: [u8; 0],
}

#[repr(C)]
pub struct archive_acl {
    _private: [u8; 0],
}

#[repr(C)]
pub struct archive_entry_linkresolver {
    _private: [u8; 0],
}

pub mod archive_common;
#[path = "archive_entry.rs"]
pub mod archive_entry_api;
#[path = "archive_match.rs"]
pub mod archive_match_api;
pub mod archive_options;
pub mod archive_read;
pub mod archive_read_disk;
pub mod archive_write;
pub mod archive_write_disk;

mod bootstrap;
