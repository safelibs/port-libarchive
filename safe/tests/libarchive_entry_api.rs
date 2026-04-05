#![allow(warnings, clippy::all)]

#[path = "libarchive/foundation/mod.rs"]
mod foundation;
#[path = "support/mod.rs"]
mod support;

use std::ptr;

use archive::entry::EntryHandle;
use archive::ffi::archive_common as common_ffi;
use archive::ffi::archive_entry_api as ffi;
use archive::util::ArchiveHandle;

#[test]
fn entry_round_trips_core_fields_and_materialized_stat() {
    let source = ArchiveHandle::reader();
    let mut entry = EntryHandle::new2(source.as_ptr());

    entry.set_pathname_w("dir/file");
    entry.set_uname("user");
    entry.set_gname("group");
    entry.set_size(123);
    entry.set_uid(7);
    entry.set_gid(9);
    entry.set_mode(ffi::AE_IFREG | 0o644);

    unsafe {
        ffi::archive_entry_set_atime(entry.as_ptr(), 13_580, 1_000_000_001);
        ffi::archive_entry_set_birthtime(entry.as_ptr(), 17_581, -24_990);
    }

    assert_eq!(Some(String::from("dir/file")), entry.pathname());
    assert_eq!(Some(String::from("dir/file")), entry.pathname_utf8());
    assert_eq!(Some(String::from("user")), entry.uname());
    assert_eq!(Some(String::from("group")), entry.gname());
    assert_eq!(123, entry.size());
    assert_eq!(7, entry.uid());
    assert_eq!(9, entry.gid());
    assert_eq!((13_581, 1), entry.atime());
    assert_eq!(
        unsafe { ffi::archive_entry_birthtime(entry.as_ptr()) },
        17_580
    );
    assert_eq!(
        unsafe { ffi::archive_entry_birthtime_nsec(entry.as_ptr()) },
        999_975_010
    );

    let stat = entry.stat();
    assert_eq!(123, stat.st_size);
    assert_eq!(7, stat.st_uid as i64);
    assert_eq!(9, stat.st_gid as i64);
    assert_eq!(ffi::AE_IFREG | 0o644, stat.st_mode);

    let clone = entry.clone_entry();
    entry.set_pathname("changed");
    entry.set_uname("changed-user");
    assert_eq!(Some(String::from("dir/file")), clone.pathname());
    assert_eq!(Some(String::from("user")), clone.uname());
}

#[test]
fn entry_acl_xattr_sparse_and_strmode_cover_foundation_collections() {
    let mut entry = foundation::regular_entry("file", 0o640);

    assert_eq!(
        archive::common::error::ARCHIVE_OK,
        entry.add_acl(
            ffi::ARCHIVE_ENTRY_ACL_TYPE_ACCESS,
            0o7,
            ffi::ARCHIVE_ENTRY_ACL_GROUP,
            78,
            "group78",
        )
    );

    let acl_text = entry
        .acl_to_text(ffi::ARCHIVE_ENTRY_ACL_STYLE_EXTRA_ID)
        .expect("ACL text");
    assert!(acl_text.contains("group:group78:rwx:78"));
    assert_eq!(Some(String::from("-rw-r-----+")), entry.strmode());

    entry.add_xattr("user.alpha", b"one");
    entry.add_xattr("user.beta", b"two");
    assert_eq!(
        vec![
            (String::from("user.alpha"), b"one".to_vec()),
            (String::from("user.beta"), b"two".to_vec()),
        ],
        entry.xattrs()
    );

    entry.set_size(12_000);
    entry.add_sparse(0, 4096);
    entry.add_sparse(8192, 1024);
    assert_eq!(vec![(0, 4096), (8192, 1024)], entry.sparse_entries());
}

#[test]
fn linkresolver_supports_tar_and_new_cpio_strategies() {
    unsafe {
        let resolver = ffi::archive_entry_linkresolver_new();
        assert!(!resolver.is_null());
        ffi::archive_entry_linkresolver_set_strategy(
            resolver,
            common_ffi::ARCHIVE_FORMAT_TAR_USTAR,
        );

        let mut tar_entry = foundation::regular_entry("test2", 0o644).into_raw();
        ffi::archive_entry_set_ino(tar_entry, 2);
        ffi::archive_entry_set_dev(tar_entry, 2);
        ffi::archive_entry_set_nlink(tar_entry, 2);
        ffi::archive_entry_set_size(tar_entry, 10);
        let mut spare = ptr::null_mut();
        ffi::archive_entry_linkify(resolver, &mut tar_entry, &mut spare);
        assert!(spare.is_null());
        assert_eq!(
            Some(String::from("test2")),
            support::c_str(ffi::archive_entry_pathname(tar_entry))
        );
        assert_eq!(None, support::c_str(ffi::archive_entry_hardlink(tar_entry)));
        assert_eq!(10, ffi::archive_entry_size(tar_entry));

        ffi::archive_entry_linkify(resolver, &mut tar_entry, &mut spare);
        assert!(spare.is_null());
        assert_eq!(
            Some(String::from("test2")),
            support::c_str(ffi::archive_entry_hardlink(tar_entry))
        );
        assert_eq!(0, ffi::archive_entry_size(tar_entry));
        ffi::archive_entry_free(tar_entry);
        ffi::archive_entry_linkresolver_free(resolver);

        let resolver = ffi::archive_entry_linkresolver_new();
        assert!(!resolver.is_null());
        ffi::archive_entry_linkresolver_set_strategy(
            resolver,
            common_ffi::ARCHIVE_FORMAT_CPIO_SVR4_NOCRC,
        );

        let mut first = foundation::regular_entry("test2", 0o644).into_raw();
        ffi::archive_entry_set_ino(first, 2);
        ffi::archive_entry_set_dev(first, 2);
        ffi::archive_entry_set_nlink(first, 3);
        ffi::archive_entry_set_size(first, 10);
        let mut spare = ptr::null_mut();
        ffi::archive_entry_linkify(resolver, &mut first, &mut spare);
        assert!(first.is_null());
        assert!(spare.is_null());

        let mut second = foundation::regular_entry("test3", 0o644).into_raw();
        ffi::archive_entry_set_ino(second, 2);
        ffi::archive_entry_set_dev(second, 2);
        ffi::archive_entry_set_nlink(second, 2);
        ffi::archive_entry_set_size(second, 10);
        ffi::archive_entry_linkify(resolver, &mut second, &mut spare);
        assert!(spare.is_null());
        assert_eq!(
            Some(String::from("test2")),
            support::c_str(ffi::archive_entry_pathname(second))
        );
        assert_eq!(0, ffi::archive_entry_size(second));
        ffi::archive_entry_free(second);

        let mut third = foundation::regular_entry("test4", 0o644).into_raw();
        ffi::archive_entry_set_ino(third, 2);
        ffi::archive_entry_set_dev(third, 2);
        ffi::archive_entry_set_nlink(third, 3);
        ffi::archive_entry_set_size(third, 10);
        ffi::archive_entry_linkify(resolver, &mut third, &mut spare);
        assert_eq!(
            Some(String::from("test3")),
            support::c_str(ffi::archive_entry_pathname(third))
        );
        assert_eq!(0, ffi::archive_entry_size(third));
        assert_eq!(
            Some(String::from("test4")),
            support::c_str(ffi::archive_entry_pathname(spare))
        );
        assert_eq!(10, ffi::archive_entry_size(spare));

        ffi::archive_entry_free(third);
        ffi::archive_entry_free(spare);
        ffi::archive_entry_linkresolver_free(resolver);
    }
}
