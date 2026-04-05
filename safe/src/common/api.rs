use std::ffi::{c_char, c_int, CStr};
use std::ptr;
use std::sync::Once;

use crate::common::error::{ARCHIVE_FATAL, ARCHIVE_OK};
use crate::common::helpers::from_optional_c_str;
use crate::common::state::{
    archive_check_magic, clear_error, close_archive, core_from_archive, error_string_ptr,
    free_archive, set_error_option, ArchiveKind,
};
use crate::ffi::archive;
use crate::generated::{LIBARCHIVE_VERSION_NUMBER, LIBARCHIVE_VERSION_STRING};

static VERSION_STRING: &[u8] = b"libarchive 3.7.2\0";
static VERSION_DETAILS: &[u8] = b"libarchive 3.7.2\0";
static BZLIB_VERSION: &[u8] = b"rust-builtin\0";
static LZ4_VERSION: &[u8] = b"rust-builtin\0";
static LZMA_VERSION: &[u8] = b"rust-builtin\0";
static ZSTD_VERSION: &[u8] = b"rust-builtin\0";
static ZLIB_VERSION: &[u8] = b"rust-builtin\0";

#[link(name = "archive_variadic_shim", kind = "static")]
unsafe extern "C" {
    fn archive_variadic_shim_link_anchor();
    fn archive_variadic_shim_set_callback(
        callback: unsafe extern "C" fn(*mut archive, c_int, *const c_char),
    );
}

static VARIADIC_SHIM_INIT: Once = Once::new();

unsafe extern "C" fn archive_set_error_bridge(
    a: *mut archive,
    error_number: c_int,
    message: *const c_char,
) {
    if let Some(core) = core_from_archive(a) {
        set_error_option(core, error_number, from_optional_c_str(message));
    }
}

pub(crate) fn ensure_variadic_shim_initialized() {
    VARIADIC_SHIM_INIT.call_once(|| unsafe {
        archive_variadic_shim_link_anchor();
        archive_variadic_shim_set_callback(archive_set_error_bridge);
    });
}

#[no_mangle]
pub unsafe extern "C" fn archive_clear_error(a: *mut archive) {
    if let Some(core) = core_from_archive(a) {
        clear_error(core);
    }
}

#[no_mangle]
pub unsafe extern "C" fn archive_copy_error(dest: *mut archive, src: *mut archive) {
    let Some(dest) = core_from_archive(dest) else {
        return;
    };
    let Some(src) = core_from_archive(src) else {
        clear_error(dest);
        return;
    };
    set_error_option(
        dest,
        src.archive_error_number,
        src.error_string
            .as_ref()
            .map(|value| value.to_string_lossy().into_owned()),
    );
}

#[no_mangle]
pub unsafe extern "C" fn archive_errno(a: *mut archive) -> c_int {
    core_from_archive(a).map_or(0, |core| core.archive_error_number)
}

#[no_mangle]
pub unsafe extern "C" fn archive_error_string(a: *mut archive) -> *const c_char {
    core_from_archive(a).map_or(ptr::null(), |core| error_string_ptr(core))
}

#[no_mangle]
pub unsafe extern "C" fn archive_free(a: *mut archive) -> c_int {
    free_archive(a)
}

#[no_mangle]
pub unsafe extern "C" fn archive_read_free(a: *mut archive) -> c_int {
    if archive_check_magic(
        a,
        crate::common::error::ARCHIVE_READ_MAGIC,
        crate::common::error::ARCHIVE_STATE_ANY,
        "archive_read_free",
    ) == ARCHIVE_FATAL
    {
        return ARCHIVE_FATAL;
    }
    free_archive(a)
}

#[no_mangle]
pub unsafe extern "C" fn archive_write_free(a: *mut archive) -> c_int {
    let magic = crate::common::state::archive_magic(a);
    let expected = if magic == crate::common::error::ARCHIVE_WRITE_DISK_MAGIC {
        crate::common::error::ARCHIVE_WRITE_DISK_MAGIC
    } else {
        crate::common::error::ARCHIVE_WRITE_MAGIC
    };
    if archive_check_magic(
        a,
        expected,
        crate::common::error::ARCHIVE_STATE_ANY,
        "archive_write_free",
    ) == ARCHIVE_FATAL
    {
        return ARCHIVE_FATAL;
    }
    free_archive(a)
}

#[no_mangle]
pub unsafe extern "C" fn archive_read_finish(a: *mut archive) -> c_int {
    archive_read_free(a)
}

#[no_mangle]
pub unsafe extern "C" fn archive_write_finish(a: *mut archive) -> c_int {
    archive_write_free(a)
}

#[no_mangle]
pub unsafe extern "C" fn archive_read_close(a: *mut archive) -> c_int {
    close_archive(a)
}

#[no_mangle]
pub unsafe extern "C" fn archive_write_close(a: *mut archive) -> c_int {
    close_archive(a)
}

#[no_mangle]
pub unsafe extern "C" fn archive_compression(a: *mut archive) -> c_int {
    archive_filter_code(a, 0)
}

#[no_mangle]
pub unsafe extern "C" fn archive_compression_name(a: *mut archive) -> *const c_char {
    archive_filter_name(a, 0)
}

#[no_mangle]
pub unsafe extern "C" fn archive_filter_count(a: *mut archive) -> c_int {
    if core_from_archive(a).is_none() {
        return ARCHIVE_FATAL;
    }
    0
}

#[no_mangle]
pub unsafe extern "C" fn archive_filter_bytes(_a: *mut archive, _n: c_int) -> i64 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn archive_filter_code(_a: *mut archive, _n: c_int) -> c_int {
    0
}

#[no_mangle]
pub unsafe extern "C" fn archive_filter_name(_a: *mut archive, _n: c_int) -> *const c_char {
    ptr::null()
}

#[no_mangle]
pub unsafe extern "C" fn archive_format(a: *mut archive) -> c_int {
    core_from_archive(a).map_or(0, |core| core.archive_format)
}

#[no_mangle]
pub unsafe extern "C" fn archive_format_name(a: *mut archive) -> *const c_char {
    core_from_archive(a)
        .and_then(|core| core.archive_format_name.as_ref())
        .map_or(ptr::null(), |name| name.as_ptr())
}

#[no_mangle]
pub unsafe extern "C" fn archive_file_count(a: *mut archive) -> c_int {
    core_from_archive(a).map_or(0, |core| core.file_count)
}

#[no_mangle]
pub unsafe extern "C" fn archive_position_compressed(a: *mut archive) -> i64 {
    core_from_archive(a).map_or(0, |core| core.position_compressed)
}

#[no_mangle]
pub unsafe extern "C" fn archive_position_uncompressed(a: *mut archive) -> i64 {
    core_from_archive(a).map_or(0, |core| core.position_uncompressed)
}

#[no_mangle]
pub extern "C" fn archive_version_number() -> c_int {
    LIBARCHIVE_VERSION_NUMBER
}

#[no_mangle]
pub extern "C" fn archive_version_string() -> *const c_char {
    VERSION_STRING.as_ptr().cast()
}

#[no_mangle]
pub extern "C" fn archive_version_details() -> *const c_char {
    VERSION_DETAILS.as_ptr().cast()
}

#[no_mangle]
pub extern "C" fn archive_bzlib_version() -> *const c_char {
    BZLIB_VERSION.as_ptr().cast()
}

#[no_mangle]
pub extern "C" fn archive_liblz4_version() -> *const c_char {
    LZ4_VERSION.as_ptr().cast()
}

#[no_mangle]
pub extern "C" fn archive_liblzma_version() -> *const c_char {
    LZMA_VERSION.as_ptr().cast()
}

#[no_mangle]
pub extern "C" fn archive_libzstd_version() -> *const c_char {
    ZSTD_VERSION.as_ptr().cast()
}

#[no_mangle]
pub extern "C" fn archive_zlib_version() -> *const c_char {
    ZLIB_VERSION.as_ptr().cast()
}

#[no_mangle]
pub unsafe extern "C" fn archive_utility_string_sort(strings: *mut *mut c_char) -> c_int {
    if strings.is_null() {
        return ARCHIVE_OK;
    }

    let mut values = Vec::new();
    let mut current = strings;
    while !(*current).is_null() {
        values.push(*current);
        current = current.add(1);
    }

    values.sort_by(|left, right| {
        let left = CStr::from_ptr(*left).to_bytes();
        let right = CStr::from_ptr(*right).to_bytes();
        left.cmp(right)
    });

    for (index, value) in values.into_iter().enumerate() {
        *strings.add(index) = value;
    }
    ARCHIVE_OK
}

pub(crate) fn version_string() -> &'static str {
    LIBARCHIVE_VERSION_STRING
}

pub(crate) fn is_match_archive(a: *mut archive) -> bool {
    unsafe { crate::common::state::archive_magic(a) == ArchiveKind::Match as u32 }
}
