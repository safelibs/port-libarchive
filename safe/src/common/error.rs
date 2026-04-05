use std::os::raw::c_int;

pub const ARCHIVE_EOF: c_int = 1;
pub const ARCHIVE_OK: c_int = 0;
pub const ARCHIVE_RETRY: c_int = -10;
pub const ARCHIVE_WARN: c_int = -20;
pub const ARCHIVE_FAILED: c_int = -25;
pub const ARCHIVE_FATAL: c_int = -30;

pub const ARCHIVE_READ_MAGIC: u32 = 0x00de_b0c5;
pub const ARCHIVE_WRITE_MAGIC: u32 = 0xb0c5_c0de;
pub const ARCHIVE_READ_DISK_MAGIC: u32 = 0x0bad_b0c5;
pub const ARCHIVE_WRITE_DISK_MAGIC: u32 = 0xc001_b0c5;

pub const ARCHIVE_STATE_NEW: u32 = 0x0001;
pub const ARCHIVE_STATE_HEADER: u32 = 0x0002;
pub const ARCHIVE_STATE_DATA: u32 = 0x0004;
pub const ARCHIVE_STATE_EOF: u32 = 0x0010;
pub const ARCHIVE_STATE_CLOSED: u32 = 0x0020;
pub const ARCHIVE_STATE_FATAL: u32 = 0x8000;
