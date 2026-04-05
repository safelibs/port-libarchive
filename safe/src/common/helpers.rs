use std::ffi::{c_char, c_int, CStr, CString};
use std::ptr;

use libc::{malloc, wchar_t};

pub(crate) fn from_optional_c_str(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        None
    } else {
        Some(
            unsafe { CStr::from_ptr(ptr) }
                .to_string_lossy()
                .into_owned(),
        )
    }
}

pub(crate) fn from_optional_wide(ptr: *const wchar_t) -> Option<String> {
    if ptr.is_null() {
        return None;
    }

    let mut values = Vec::new();
    let mut current = ptr;
    unsafe {
        while *current != 0 {
            values.push(*current as u32);
            current = current.add(1);
        }
    }

    Some(
        values
            .into_iter()
            .map(|value| char::from_u32(value).unwrap_or(char::REPLACEMENT_CHARACTER))
            .collect(),
    )
}

pub(crate) fn to_wide_null(value: &str) -> Vec<wchar_t> {
    value
        .encode_utf16()
        .map(|unit| unit as wchar_t)
        .chain([0])
        .collect()
}

pub(crate) fn clone_c_string(value: Option<&str>) -> Option<CString> {
    value.map(|value| CString::new(value).expect("string must not contain NUL"))
}

pub(crate) fn empty_if_none(value: Option<&CString>) -> *const c_char {
    value.map_or(ptr::null(), |value| value.as_ptr())
}

pub(crate) fn empty_if_none_wide(value: Option<&Vec<wchar_t>>) -> *const wchar_t {
    value.map_or(ptr::null(), |value| value.as_ptr())
}

pub(crate) unsafe fn malloc_bytes(bytes: &[u8]) -> *mut c_char {
    let ptr = malloc(bytes.len()).cast::<u8>();
    if ptr.is_null() {
        return ptr.cast::<c_char>();
    }
    ptr.copy_from_nonoverlapping(bytes.as_ptr(), bytes.len());
    ptr.cast::<c_char>()
}

pub(crate) unsafe fn malloc_wide(values: &[wchar_t]) -> *mut wchar_t {
    let size = std::mem::size_of_val(values);
    let ptr = malloc(size).cast::<wchar_t>();
    if ptr.is_null() {
        return ptr;
    }
    ptr.copy_from_nonoverlapping(values.as_ptr(), values.len());
    ptr
}

pub(crate) fn normalize_nanos(sec: i64, nsec: i64) -> (i64, i64) {
    let billion = 1_000_000_000i64;
    let mut sec = sec + nsec.div_euclid(billion);
    let mut nsec = nsec.rem_euclid(billion);
    if nsec < 0 {
        sec -= 1;
        nsec += billion;
    }
    (sec, nsec)
}

pub(crate) fn bool_to_int(value: bool) -> c_int {
    if value { 1 } else { 0 }
}
