#![allow(warnings, clippy::all)]

#[path = "libarchive/foundation/mod.rs"]
mod foundation;
#[path = "support/mod.rs"]
mod support;

use archive::common::error::{ARCHIVE_EOF, ARCHIVE_OK};
use archive::entry::EntryHandle;
use archive::ffi::archive_common as common_ffi;
use archive::ffi::archive_match_api as ffi;
use archive::r#match::MatchHandle;

#[test]
fn match_handles_include_exclude_patterns_and_unmatched_tracking() {
    let mut matcher = MatchHandle::new();
    assert_eq!(ARCHIVE_OK, matcher.set_inclusion_recursion(false));
    assert_eq!(ARCHIVE_OK, matcher.include_pattern("a/b/"));

    let mut entry = EntryHandle::new();
    entry.set_pathname("a/b/c");
    assert_eq!(1, matcher.path_excluded(&entry));
    assert_eq!(1, matcher.unmatched_inclusions());
    assert_eq!(
        Some(String::from("a/b")),
        matcher.unmatched_inclusions_next()
    );
    assert_eq!(None, matcher.unmatched_inclusions_next());

    let mut matcher = MatchHandle::new();
    assert_eq!(ARCHIVE_OK, matcher.exclude_pattern("^aa*"));
    let mut entry = EntryHandle::new();
    entry.set_pathname("aa1234");
    assert_eq!(1, matcher.path_excluded(&entry));
    assert_eq!(1, matcher.excluded(&entry));
    entry.clear();
    entry.set_pathname("a1234");
    assert_eq!(0, matcher.path_excluded(&entry));
}

#[test]
fn match_supports_pattern_files_owner_filters_and_time_filters() {
    let path = support::write_temp_file("patterns", b"second\nfour\n");
    let mut matcher = MatchHandle::new();
    let path_c = archive::util::c_string(path.to_str().unwrap());
    unsafe {
        assert_eq!(
            ARCHIVE_OK,
            ffi::archive_match_exclude_pattern_from_file(matcher.as_ptr(), path_c.as_ptr(), 0)
        );
    }

    let mut entry = EntryHandle::new();
    entry.set_pathname("first");
    assert_eq!(0, matcher.path_excluded(&entry));
    entry.clear();
    entry.set_pathname("second");
    assert_eq!(1, matcher.path_excluded(&entry));

    let mut matcher = MatchHandle::new();
    assert_eq!(ARCHIVE_OK, matcher.include_uid(1000));
    assert_eq!(ARCHIVE_OK, matcher.include_gid(1002));
    assert_eq!(ARCHIVE_OK, matcher.include_uname("foo"));
    assert_eq!(ARCHIVE_OK, matcher.include_gname("bar"));

    let mut owner = foundation::regular_entry("owner", 0o644);
    owner.set_uid(1000);
    owner.set_gid(1002);
    owner.set_uname("foo");
    owner.set_gname("bar");
    assert_eq!(0, matcher.owner_excluded(&owner));

    owner.set_uid(0);
    assert_eq!(1, matcher.owner_excluded(&owner));

    let mut matcher = MatchHandle::new();
    assert_eq!(
        ARCHIVE_OK,
        matcher.include_date(
            common_ffi::ARCHIVE_MATCH_MTIME
                | common_ffi::ARCHIVE_MATCH_NEWER
                | common_ffi::ARCHIVE_MATCH_EQUAL,
            "Jan 1, 1970 UTC",
        )
    );
    let mut timed = foundation::regular_entry("time", 0o644);
    timed.set_mtime(-1, 0);
    assert_eq!(1, matcher.time_excluded(&timed));
    timed.set_mtime(0, 0);
    assert_eq!(0, matcher.time_excluded(&timed));
    timed.set_mtime(1, 0);
    assert_eq!(0, matcher.time_excluded(&timed));
}

#[test]
fn raw_match_api_reports_eof_for_consumed_unmatched_patterns() {
    let mut matcher = MatchHandle::new();
    assert_eq!(ARCHIVE_OK, matcher.include_pattern("^aa*"));
    let mut entry = EntryHandle::new();
    entry.set_pathname("aa1234");
    assert_eq!(0, matcher.path_excluded(&entry));

    let mut unmatched = std::ptr::null();
    let status = unsafe {
        ffi::archive_match_path_unmatched_inclusions_next(matcher.as_ptr(), &mut unmatched)
    };
    assert_eq!(ARCHIVE_EOF, status);
    assert!(unmatched.is_null());
}
