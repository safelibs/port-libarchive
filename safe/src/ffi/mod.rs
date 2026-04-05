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

mod bootstrap;
