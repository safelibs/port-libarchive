use std::ffi::{c_char, c_int, c_long};
use std::ptr;

use libc::wchar_t;

use crate::common::error::{ARCHIVE_EOF, ARCHIVE_FAILED, ARCHIVE_FATAL, ARCHIVE_OK};
use crate::common::helpers::{from_optional_c_str, from_optional_wide};
use crate::common::state::{core_from_archive, set_error_string};
use crate::entry::internal::from_raw as entry_from_raw;
use crate::ffi::{archive, archive_entry};
use crate::r#match::internal::{
    add_pattern, add_pattern_from_file, file_times_from_path, from_archive, new_archive,
    owner_excluded, parse_date, path_excluded, set_timefilter, time_excluded,
    validate_match_archive, validate_time_flag, MatchArchive, PathTimeFilter, Pattern,
};

#[no_mangle]
pub extern "C" fn archive_match_new() -> *mut archive {
    crate::common::panic_boundary::ffi_ptr(|| unsafe { new_archive() })
}

#[no_mangle]
pub unsafe extern "C" fn archive_match_free(a: *mut archive) -> c_int {
    crate::common::panic_boundary::ffi_int(crate::common::error::ARCHIVE_FATAL, || unsafe {
        if a.is_null() {
            return ARCHIVE_OK;
        }
        if validate_match_archive(a, "archive_match_free") == ARCHIVE_FATAL {
            return ARCHIVE_FATAL;
        }
        crate::r#match::internal::free_match_archive(a);
        ARCHIVE_OK
    })
}

#[no_mangle]
pub unsafe extern "C" fn archive_match_set_inclusion_recursion(
    a: *mut archive,
    enabled: c_int,
) -> c_int {
    crate::common::panic_boundary::ffi_int(crate::common::error::ARCHIVE_FATAL, || unsafe {
        if validate_match_archive(a, "archive_match_set_inclusion_recursion") == ARCHIVE_FATAL {
            return ARCHIVE_FATAL;
        }
        let Some(matcher) = from_archive(a) else {
            return ARCHIVE_FATAL;
        };
        matcher.recursive_include = enabled != 0;
        ARCHIVE_OK
    })
}

fn add_pattern_checked(
    a: *mut archive,
    value: Option<String>,
    inclusion: bool,
    function: &str,
) -> c_int {
    unsafe {
        if validate_match_archive(a, function) == ARCHIVE_FATAL {
            return ARCHIVE_FATAL;
        }
        let Some(matcher) = from_archive(a) else {
            return ARCHIVE_FATAL;
        };
        let Some(value) = value.filter(|value| !value.is_empty()) else {
            set_error_string(
                &mut matcher.core,
                libc::EINVAL,
                "pattern is empty".to_string(),
            );
            return ARCHIVE_FAILED;
        };
        if inclusion {
            add_pattern(&mut matcher.inclusions, value);
        } else {
            add_pattern(&mut matcher.exclusions, value);
        }
        ARCHIVE_OK
    }
}

#[no_mangle]
pub unsafe extern "C" fn archive_match_exclude_pattern(
    a: *mut archive,
    pattern: *const c_char,
) -> c_int {
    crate::common::panic_boundary::ffi_int(crate::common::error::ARCHIVE_FATAL, || unsafe {
        add_pattern_checked(
            a,
            from_optional_c_str(pattern),
            false,
            "archive_match_exclude_pattern",
        )
    })
}

#[no_mangle]
pub unsafe extern "C" fn archive_match_exclude_pattern_w(
    a: *mut archive,
    pattern: *const wchar_t,
) -> c_int {
    crate::common::panic_boundary::ffi_int(crate::common::error::ARCHIVE_FATAL, || unsafe {
        add_pattern_checked(
            a,
            from_optional_wide(pattern),
            false,
            "archive_match_exclude_pattern_w",
        )
    })
}

#[no_mangle]
pub unsafe extern "C" fn archive_match_include_pattern(
    a: *mut archive,
    pattern: *const c_char,
) -> c_int {
    crate::common::panic_boundary::ffi_int(crate::common::error::ARCHIVE_FATAL, || unsafe {
        add_pattern_checked(
            a,
            from_optional_c_str(pattern),
            true,
            "archive_match_include_pattern",
        )
    })
}

#[no_mangle]
pub unsafe extern "C" fn archive_match_include_pattern_w(
    a: *mut archive,
    pattern: *const wchar_t,
) -> c_int {
    crate::common::panic_boundary::ffi_int(crate::common::error::ARCHIVE_FATAL, || unsafe {
        add_pattern_checked(
            a,
            from_optional_wide(pattern),
            true,
            "archive_match_include_pattern_w",
        )
    })
}

fn add_pattern_file_checked(
    a: *mut archive,
    path: Option<String>,
    null_separator: c_int,
    inclusion: bool,
    function: &str,
) -> c_int {
    unsafe {
        if validate_match_archive(a, function) == ARCHIVE_FATAL {
            return ARCHIVE_FATAL;
        }
        let Some(matcher) = from_archive(a) else {
            return ARCHIVE_FATAL;
        };
        let Some(path) = path.filter(|path| !path.is_empty()) else {
            set_error_string(
                &mut matcher.core,
                libc::EINVAL,
                "pathname is empty".to_string(),
            );
            return ARCHIVE_FAILED;
        };
        let status = if inclusion {
            add_pattern_from_file(&mut matcher.inclusions, &path, null_separator)
        } else {
            add_pattern_from_file(&mut matcher.exclusions, &path, null_separator)
        };
        if status != ARCHIVE_OK {
            set_error_string(
                &mut matcher.core,
                libc::ENOENT,
                format!("Failed to read {path}"),
            );
        }
        status
    }
}

#[no_mangle]
pub unsafe extern "C" fn archive_match_exclude_pattern_from_file(
    a: *mut archive,
    path: *const c_char,
    null_separator: c_int,
) -> c_int {
    crate::common::panic_boundary::ffi_int(crate::common::error::ARCHIVE_FATAL, || unsafe {
        add_pattern_file_checked(
            a,
            from_optional_c_str(path),
            null_separator,
            false,
            "archive_match_exclude_pattern_from_file",
        )
    })
}

#[no_mangle]
pub unsafe extern "C" fn archive_match_exclude_pattern_from_file_w(
    a: *mut archive,
    path: *const wchar_t,
    null_separator: c_int,
) -> c_int {
    crate::common::panic_boundary::ffi_int(crate::common::error::ARCHIVE_FATAL, || unsafe {
        add_pattern_file_checked(
            a,
            from_optional_wide(path),
            null_separator,
            false,
            "archive_match_exclude_pattern_from_file_w",
        )
    })
}

#[no_mangle]
pub unsafe extern "C" fn archive_match_include_pattern_from_file(
    a: *mut archive,
    path: *const c_char,
    null_separator: c_int,
) -> c_int {
    crate::common::panic_boundary::ffi_int(crate::common::error::ARCHIVE_FATAL, || unsafe {
        add_pattern_file_checked(
            a,
            from_optional_c_str(path),
            null_separator,
            true,
            "archive_match_include_pattern_from_file",
        )
    })
}

#[no_mangle]
pub unsafe extern "C" fn archive_match_include_pattern_from_file_w(
    a: *mut archive,
    path: *const wchar_t,
    null_separator: c_int,
) -> c_int {
    crate::common::panic_boundary::ffi_int(crate::common::error::ARCHIVE_FATAL, || unsafe {
        add_pattern_file_checked(
            a,
            from_optional_wide(path),
            null_separator,
            true,
            "archive_match_include_pattern_from_file_w",
        )
    })
}

#[no_mangle]
pub unsafe extern "C" fn archive_match_path_excluded(
    a: *mut archive,
    entry: *mut archive_entry,
) -> c_int {
    crate::common::panic_boundary::ffi_int(0, || unsafe {
        if validate_match_archive(a, "archive_match_path_excluded") == ARCHIVE_FATAL {
            return ARCHIVE_FATAL;
        }
        let Some(matcher) = from_archive(a) else {
            return ARCHIVE_FATAL;
        };
        let Some(entry) = entry_from_raw(entry) else {
            set_error_string(&mut matcher.core, libc::EINVAL, "entry is NULL".to_string());
            return ARCHIVE_FAILED;
        };
        let Some(path) = entry.pathname.get_str() else {
            return 0;
        };
        path_excluded(matcher, path)
    })
}

#[no_mangle]
pub unsafe extern "C" fn archive_match_excluded(
    a: *mut archive,
    entry: *mut archive_entry,
) -> c_int {
    crate::common::panic_boundary::ffi_int(0, || unsafe {
        if validate_match_archive(a, "archive_match_excluded") == ARCHIVE_FATAL {
            return ARCHIVE_FATAL;
        }
        let Some(matcher) = from_archive(a) else {
            return ARCHIVE_FATAL;
        };
        if entry.is_null() {
            set_error_string(&mut matcher.core, libc::EINVAL, "entry is NULL".to_string());
            return ARCHIVE_FAILED;
        }
        let path_status = archive_match_path_excluded(a, entry);
        if path_status != 0 {
            return path_status;
        }
        let time_status = archive_match_time_excluded(a, entry);
        if time_status != 0 {
            return time_status;
        }
        archive_match_owner_excluded(a, entry)
    })
}

#[no_mangle]
pub unsafe extern "C" fn archive_match_path_unmatched_inclusions(a: *mut archive) -> c_int {
    crate::common::panic_boundary::ffi_int(0, || unsafe {
        if validate_match_archive(a, "archive_match_unmatched_inclusions") == ARCHIVE_FATAL {
            return ARCHIVE_FATAL;
        }
        from_archive(a).map_or(0, |matcher| matcher.inclusions.unmatched_count())
    })
}

#[no_mangle]
pub unsafe extern "C" fn archive_match_path_unmatched_inclusions_next(
    a: *mut archive,
    unmatched: *mut *const c_char,
) -> c_int {
    crate::common::panic_boundary::ffi_int(crate::common::error::ARCHIVE_FATAL, || unsafe {
        if validate_match_archive(a, "archive_match_unmatched_inclusions_next") == ARCHIVE_FATAL {
            return ARCHIVE_FATAL;
        }
        let Some(matcher) = from_archive(a) else {
            return ARCHIVE_FATAL;
        };
        let Some((c_ptr, _)) = matcher.inclusions.unmatched_next(false) else {
            if !unmatched.is_null() {
                *unmatched = ptr::null();
            }
            return ARCHIVE_EOF;
        };
        if !unmatched.is_null() {
            *unmatched = c_ptr;
        }
        ARCHIVE_OK
    })
}

#[no_mangle]
pub unsafe extern "C" fn archive_match_path_unmatched_inclusions_next_w(
    a: *mut archive,
    unmatched: *mut *const wchar_t,
) -> c_int {
    crate::common::panic_boundary::ffi_int(crate::common::error::ARCHIVE_FATAL, || unsafe {
        if validate_match_archive(a, "archive_match_unmatched_inclusions_next_w") == ARCHIVE_FATAL {
            return ARCHIVE_FATAL;
        }
        let Some(matcher) = from_archive(a) else {
            return ARCHIVE_FATAL;
        };
        let Some((_, w_ptr)) = matcher.inclusions.unmatched_next(true) else {
            if !unmatched.is_null() {
                *unmatched = ptr::null();
            }
            return ARCHIVE_EOF;
        };
        if !unmatched.is_null() {
            *unmatched = w_ptr;
        }
        ARCHIVE_OK
    })
}

#[no_mangle]
pub unsafe extern "C" fn archive_match_include_time(
    a: *mut archive,
    flag: c_int,
    sec: i64,
    nsec: c_long,
) -> c_int {
    crate::common::panic_boundary::ffi_int(crate::common::error::ARCHIVE_FATAL, || unsafe {
        if validate_match_archive(a, "archive_match_include_time") == ARCHIVE_FATAL {
            return ARCHIVE_FATAL;
        }
        let Some(matcher) = from_archive(a) else {
            return ARCHIVE_FATAL;
        };
        let status = validate_time_flag(matcher, flag);
        if status != ARCHIVE_OK {
            return status;
        }
        set_timefilter(matcher, flag, sec, nsec as i64);
        ARCHIVE_OK
    })
}

fn include_date_impl(a: *mut archive, flag: c_int, text: Option<String>, function: &str) -> c_int {
    unsafe {
        if validate_match_archive(a, function) == ARCHIVE_FATAL {
            return ARCHIVE_FATAL;
        }
        let Some(matcher) = from_archive(a) else {
            return ARCHIVE_FATAL;
        };
        let status = validate_time_flag(matcher, flag);
        if status != ARCHIVE_OK {
            return status;
        }
        let Some(text) = text.filter(|text| !text.is_empty()) else {
            set_error_string(&mut matcher.core, libc::EINVAL, "date is empty".to_string());
            return ARCHIVE_FAILED;
        };
        let Some(timestamp) = parse_date(matcher.now, &text) else {
            set_error_string(
                &mut matcher.core,
                libc::EINVAL,
                "invalid date string".to_string(),
            );
            return ARCHIVE_FAILED;
        };
        set_timefilter(matcher, flag, timestamp, 0);
        ARCHIVE_OK
    }
}

#[no_mangle]
pub unsafe extern "C" fn archive_match_include_date(
    a: *mut archive,
    flag: c_int,
    text: *const c_char,
) -> c_int {
    crate::common::panic_boundary::ffi_int(crate::common::error::ARCHIVE_FATAL, || unsafe {
        include_date_impl(
            a,
            flag,
            from_optional_c_str(text),
            "archive_match_include_date",
        )
    })
}

#[no_mangle]
pub unsafe extern "C" fn archive_match_include_date_w(
    a: *mut archive,
    flag: c_int,
    text: *const wchar_t,
) -> c_int {
    crate::common::panic_boundary::ffi_int(crate::common::error::ARCHIVE_FATAL, || unsafe {
        include_date_impl(
            a,
            flag,
            from_optional_wide(text),
            "archive_match_include_date_w",
        )
    })
}

fn include_file_time_impl(
    a: *mut archive,
    flag: c_int,
    path: Option<String>,
    function: &str,
) -> c_int {
    unsafe {
        if validate_match_archive(a, function) == ARCHIVE_FATAL {
            return ARCHIVE_FATAL;
        }
        let Some(matcher) = from_archive(a) else {
            return ARCHIVE_FATAL;
        };
        let status = validate_time_flag(matcher, flag);
        if status != ARCHIVE_OK {
            return status;
        }
        let Some(path) = path.filter(|path| !path.is_empty()) else {
            set_error_string(
                &mut matcher.core,
                libc::EINVAL,
                "pathname is empty".to_string(),
            );
            return ARCHIVE_FAILED;
        };
        let Some(times) = file_times_from_path(&path) else {
            set_error_string(
                &mut matcher.core,
                libc::ENOENT,
                "Failed to stat()".to_string(),
            );
            return ARCHIVE_FAILED;
        };
        let sec = if (flag & crate::ffi::archive_common::ARCHIVE_MATCH_MTIME) != 0 {
            times.mtime_sec
        } else {
            times.ctime_sec
        };
        let nsec = if (flag & crate::ffi::archive_common::ARCHIVE_MATCH_MTIME) != 0 {
            times.mtime_nsec
        } else {
            times.ctime_nsec
        };
        set_timefilter(matcher, flag, sec, nsec);
        ARCHIVE_OK
    }
}

#[no_mangle]
pub unsafe extern "C" fn archive_match_include_file_time(
    a: *mut archive,
    flag: c_int,
    path: *const c_char,
) -> c_int {
    crate::common::panic_boundary::ffi_int(crate::common::error::ARCHIVE_FATAL, || unsafe {
        include_file_time_impl(
            a,
            flag,
            from_optional_c_str(path),
            "archive_match_include_file_time",
        )
    })
}

#[no_mangle]
pub unsafe extern "C" fn archive_match_include_file_time_w(
    a: *mut archive,
    flag: c_int,
    path: *const wchar_t,
) -> c_int {
    crate::common::panic_boundary::ffi_int(crate::common::error::ARCHIVE_FATAL, || unsafe {
        include_file_time_impl(
            a,
            flag,
            from_optional_wide(path),
            "archive_match_include_file_time_w",
        )
    })
}

#[no_mangle]
pub unsafe extern "C" fn archive_match_exclude_entry(
    a: *mut archive,
    flag: c_int,
    entry: *mut archive_entry,
) -> c_int {
    crate::common::panic_boundary::ffi_int(crate::common::error::ARCHIVE_FATAL, || unsafe {
        if validate_match_archive(a, "archive_match_exclude_entry") == ARCHIVE_FATAL {
            return ARCHIVE_FATAL;
        }
        let Some(matcher) = from_archive(a) else {
            return ARCHIVE_FATAL;
        };
        let status = validate_time_flag(matcher, flag);
        if status != ARCHIVE_OK {
            return status;
        }
        let Some(entry) = entry_from_raw(entry) else {
            set_error_string(&mut matcher.core, libc::EINVAL, "entry is NULL".to_string());
            return ARCHIVE_FAILED;
        };
        let Some(path) = entry.pathname.get_str() else {
            set_error_string(
                &mut matcher.core,
                libc::EINVAL,
                "pathname is NULL".to_string(),
            );
            return ARCHIVE_FAILED;
        };
        matcher.path_time_filters.insert(
            path.to_string(),
            PathTimeFilter {
                flag,
                mtime_sec: entry.mtime.sec,
                mtime_nsec: entry.mtime.nsec as i64,
                ctime_sec: entry.ctime.sec,
                ctime_nsec: entry.ctime.nsec as i64,
            },
        );
        ARCHIVE_OK
    })
}

#[no_mangle]
pub unsafe extern "C" fn archive_match_time_excluded(
    a: *mut archive,
    entry: *mut archive_entry,
) -> c_int {
    crate::common::panic_boundary::ffi_int(0, || unsafe {
        if validate_match_archive(a, "archive_match_time_excluded") == ARCHIVE_FATAL {
            return ARCHIVE_FATAL;
        }
        let Some(matcher) = from_archive(a) else {
            return ARCHIVE_FATAL;
        };
        if entry.is_null() {
            set_error_string(&mut matcher.core, libc::EINVAL, "entry is NULL".to_string());
            return ARCHIVE_FAILED;
        }
        time_excluded(matcher, entry)
    })
}

#[no_mangle]
pub unsafe extern "C" fn archive_match_include_uid(a: *mut archive, uid: i64) -> c_int {
    crate::common::panic_boundary::ffi_int(crate::common::error::ARCHIVE_FATAL, || unsafe {
        if validate_match_archive(a, "archive_match_include_uid") == ARCHIVE_FATAL {
            return ARCHIVE_FATAL;
        }
        from_archive(a).map_or(ARCHIVE_FATAL, |matcher| {
            matcher.inclusion_uids.insert(uid);
            ARCHIVE_OK
        })
    })
}

#[no_mangle]
pub unsafe extern "C" fn archive_match_include_gid(a: *mut archive, gid: i64) -> c_int {
    crate::common::panic_boundary::ffi_int(crate::common::error::ARCHIVE_FATAL, || unsafe {
        if validate_match_archive(a, "archive_match_include_gid") == ARCHIVE_FATAL {
            return ARCHIVE_FATAL;
        }
        from_archive(a).map_or(ARCHIVE_FATAL, |matcher| {
            matcher.inclusion_gids.insert(gid);
            ARCHIVE_OK
        })
    })
}

fn add_owner_name(a: *mut archive, value: Option<String>, gname: bool, function: &str) -> c_int {
    unsafe {
        if validate_match_archive(a, function) == ARCHIVE_FATAL {
            return ARCHIVE_FATAL;
        }
        let Some(matcher) = from_archive(a) else {
            return ARCHIVE_FATAL;
        };
        let Some(value) = value.filter(|value| !value.is_empty()) else {
            set_error_string(&mut matcher.core, libc::EINVAL, "name is empty".to_string());
            return ARCHIVE_FAILED;
        };
        let pattern = Pattern::new(value);
        if gname {
            matcher.inclusion_gnames.push(pattern);
        } else {
            matcher.inclusion_unames.push(pattern);
        }
        ARCHIVE_OK
    }
}

#[no_mangle]
pub unsafe extern "C" fn archive_match_include_uname(
    a: *mut archive,
    name: *const c_char,
) -> c_int {
    crate::common::panic_boundary::ffi_int(crate::common::error::ARCHIVE_FATAL, || unsafe {
        add_owner_name(
            a,
            from_optional_c_str(name),
            false,
            "archive_match_include_uname",
        )
    })
}

#[no_mangle]
pub unsafe extern "C" fn archive_match_include_uname_w(
    a: *mut archive,
    name: *const wchar_t,
) -> c_int {
    crate::common::panic_boundary::ffi_int(crate::common::error::ARCHIVE_FATAL, || unsafe {
        add_owner_name(
            a,
            from_optional_wide(name),
            false,
            "archive_match_include_uname_w",
        )
    })
}

#[no_mangle]
pub unsafe extern "C" fn archive_match_include_gname(
    a: *mut archive,
    name: *const c_char,
) -> c_int {
    crate::common::panic_boundary::ffi_int(crate::common::error::ARCHIVE_FATAL, || unsafe {
        add_owner_name(
            a,
            from_optional_c_str(name),
            true,
            "archive_match_include_gname",
        )
    })
}

#[no_mangle]
pub unsafe extern "C" fn archive_match_include_gname_w(
    a: *mut archive,
    name: *const wchar_t,
) -> c_int {
    crate::common::panic_boundary::ffi_int(crate::common::error::ARCHIVE_FATAL, || unsafe {
        add_owner_name(
            a,
            from_optional_wide(name),
            true,
            "archive_match_include_gname_w",
        )
    })
}

#[no_mangle]
pub unsafe extern "C" fn archive_match_owner_excluded(
    a: *mut archive,
    entry: *mut archive_entry,
) -> c_int {
    crate::common::panic_boundary::ffi_int(0, || unsafe {
        if validate_match_archive(a, "archive_match_owner_excluded") == ARCHIVE_FATAL {
            return ARCHIVE_FATAL;
        }
        let Some(matcher) = from_archive(a) else {
            return ARCHIVE_FATAL;
        };
        if entry.is_null() {
            set_error_string(&mut matcher.core, libc::EINVAL, "entry is NULL".to_string());
            return ARCHIVE_FAILED;
        }
        owner_excluded(matcher, entry)
    })
}
