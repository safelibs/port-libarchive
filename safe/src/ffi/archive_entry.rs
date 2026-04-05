use std::ffi::{c_char, c_int, c_long, c_uchar, c_uint, c_ulong, c_void};

use libc::{dev_t, mode_t, size_t, stat, wchar_t};

use crate::ffi::{archive, archive_acl, archive_entry, archive_entry_linkresolver};

pub const AE_IFMT: mode_t = 0o170000;
pub const AE_IFREG: mode_t = 0o100000;
pub const AE_IFLNK: mode_t = 0o120000;
pub const AE_IFSOCK: mode_t = 0o140000;
pub const AE_IFCHR: mode_t = 0o020000;
pub const AE_IFBLK: mode_t = 0o060000;
pub const AE_IFDIR: mode_t = 0o040000;
pub const AE_IFIFO: mode_t = 0o010000;

pub const AE_SYMLINK_TYPE_UNDEFINED: c_int = 0;
pub const AE_SYMLINK_TYPE_FILE: c_int = 1;
pub const AE_SYMLINK_TYPE_DIRECTORY: c_int = 2;

pub const ARCHIVE_ENTRY_ACL_EXECUTE: c_int = 0x0000_0001;
pub const ARCHIVE_ENTRY_ACL_WRITE: c_int = 0x0000_0002;
pub const ARCHIVE_ENTRY_ACL_READ: c_int = 0x0000_0004;
pub const ARCHIVE_ENTRY_ACL_READ_DATA: c_int = 0x0000_0008;
pub const ARCHIVE_ENTRY_ACL_LIST_DIRECTORY: c_int = 0x0000_0008;
pub const ARCHIVE_ENTRY_ACL_WRITE_DATA: c_int = 0x0000_0010;
pub const ARCHIVE_ENTRY_ACL_ADD_FILE: c_int = 0x0000_0010;
pub const ARCHIVE_ENTRY_ACL_APPEND_DATA: c_int = 0x0000_0020;
pub const ARCHIVE_ENTRY_ACL_ADD_SUBDIRECTORY: c_int = 0x0000_0020;
pub const ARCHIVE_ENTRY_ACL_READ_NAMED_ATTRS: c_int = 0x0000_0040;
pub const ARCHIVE_ENTRY_ACL_WRITE_NAMED_ATTRS: c_int = 0x0000_0080;
pub const ARCHIVE_ENTRY_ACL_DELETE_CHILD: c_int = 0x0000_0100;
pub const ARCHIVE_ENTRY_ACL_READ_ATTRIBUTES: c_int = 0x0000_0200;
pub const ARCHIVE_ENTRY_ACL_WRITE_ATTRIBUTES: c_int = 0x0000_0400;
pub const ARCHIVE_ENTRY_ACL_DELETE: c_int = 0x0000_0800;
pub const ARCHIVE_ENTRY_ACL_READ_ACL: c_int = 0x0000_1000;
pub const ARCHIVE_ENTRY_ACL_WRITE_ACL: c_int = 0x0000_2000;
pub const ARCHIVE_ENTRY_ACL_WRITE_OWNER: c_int = 0x0000_4000;
pub const ARCHIVE_ENTRY_ACL_SYNCHRONIZE: c_int = 0x0000_8000;

pub const ARCHIVE_ENTRY_ACL_ENTRY_INHERITED: c_int = 0x0100_0000;
pub const ARCHIVE_ENTRY_ACL_ENTRY_FILE_INHERIT: c_int = 0x0200_0000;
pub const ARCHIVE_ENTRY_ACL_ENTRY_DIRECTORY_INHERIT: c_int = 0x0400_0000;
pub const ARCHIVE_ENTRY_ACL_ENTRY_NO_PROPAGATE_INHERIT: c_int = 0x0800_0000;
pub const ARCHIVE_ENTRY_ACL_ENTRY_INHERIT_ONLY: c_int = 0x1000_0000;
pub const ARCHIVE_ENTRY_ACL_ENTRY_SUCCESSFUL_ACCESS: c_int = 0x2000_0000;
pub const ARCHIVE_ENTRY_ACL_ENTRY_FAILED_ACCESS: c_int = 0x4000_0000;

pub const ARCHIVE_ENTRY_ACL_TYPE_ACCESS: c_int = 0x0000_0100;
pub const ARCHIVE_ENTRY_ACL_TYPE_DEFAULT: c_int = 0x0000_0200;
pub const ARCHIVE_ENTRY_ACL_TYPE_ALLOW: c_int = 0x0000_0400;
pub const ARCHIVE_ENTRY_ACL_TYPE_DENY: c_int = 0x0000_0800;
pub const ARCHIVE_ENTRY_ACL_TYPE_AUDIT: c_int = 0x0000_1000;
pub const ARCHIVE_ENTRY_ACL_TYPE_ALARM: c_int = 0x0000_2000;
pub const ARCHIVE_ENTRY_ACL_TYPE_POSIX1E: c_int =
    ARCHIVE_ENTRY_ACL_TYPE_ACCESS | ARCHIVE_ENTRY_ACL_TYPE_DEFAULT;
pub const ARCHIVE_ENTRY_ACL_TYPE_NFS4: c_int = ARCHIVE_ENTRY_ACL_TYPE_ALLOW
    | ARCHIVE_ENTRY_ACL_TYPE_DENY
    | ARCHIVE_ENTRY_ACL_TYPE_AUDIT
    | ARCHIVE_ENTRY_ACL_TYPE_ALARM;

pub const ARCHIVE_ENTRY_ACL_USER: c_int = 10001;
pub const ARCHIVE_ENTRY_ACL_USER_OBJ: c_int = 10002;
pub const ARCHIVE_ENTRY_ACL_GROUP: c_int = 10003;
pub const ARCHIVE_ENTRY_ACL_GROUP_OBJ: c_int = 10004;
pub const ARCHIVE_ENTRY_ACL_MASK: c_int = 10005;
pub const ARCHIVE_ENTRY_ACL_OTHER: c_int = 10006;
pub const ARCHIVE_ENTRY_ACL_EVERYONE: c_int = 10107;

pub const ARCHIVE_ENTRY_ACL_STYLE_EXTRA_ID: c_int = 0x0000_0001;
pub const ARCHIVE_ENTRY_ACL_STYLE_MARK_DEFAULT: c_int = 0x0000_0002;
pub const ARCHIVE_ENTRY_ACL_STYLE_SOLARIS: c_int = 0x0000_0004;
pub const ARCHIVE_ENTRY_ACL_STYLE_SEPARATOR_COMMA: c_int = 0x0000_0008;
pub const ARCHIVE_ENTRY_ACL_STYLE_COMPACT: c_int = 0x0000_0010;

unsafe extern "C" {
    pub fn archive_entry_new() -> *mut archive_entry;
    pub fn archive_entry_new2(a: *mut archive) -> *mut archive_entry;
    pub fn archive_entry_free(entry: *mut archive_entry);
    pub fn archive_entry_clear(entry: *mut archive_entry) -> *mut archive_entry;
    pub fn archive_entry_clone(entry: *mut archive_entry) -> *mut archive_entry;

    pub fn archive_entry_atime(entry: *mut archive_entry) -> i64;
    pub fn archive_entry_atime_nsec(entry: *mut archive_entry) -> c_long;
    pub fn archive_entry_atime_is_set(entry: *mut archive_entry) -> c_int;
    pub fn archive_entry_birthtime(entry: *mut archive_entry) -> i64;
    pub fn archive_entry_birthtime_nsec(entry: *mut archive_entry) -> c_long;
    pub fn archive_entry_birthtime_is_set(entry: *mut archive_entry) -> c_int;
    pub fn archive_entry_ctime(entry: *mut archive_entry) -> i64;
    pub fn archive_entry_ctime_nsec(entry: *mut archive_entry) -> c_long;
    pub fn archive_entry_ctime_is_set(entry: *mut archive_entry) -> c_int;
    pub fn archive_entry_dev(entry: *mut archive_entry) -> dev_t;
    pub fn archive_entry_dev_is_set(entry: *mut archive_entry) -> c_int;
    pub fn archive_entry_devmajor(entry: *mut archive_entry) -> dev_t;
    pub fn archive_entry_devminor(entry: *mut archive_entry) -> dev_t;
    pub fn archive_entry_filetype(entry: *mut archive_entry) -> mode_t;
    pub fn archive_entry_fflags(entry: *mut archive_entry, set: *mut c_ulong, clear: *mut c_ulong);
    pub fn archive_entry_fflags_text(entry: *mut archive_entry) -> *const c_char;
    pub fn archive_entry_gid(entry: *mut archive_entry) -> i64;
    pub fn archive_entry_gname(entry: *mut archive_entry) -> *const c_char;
    pub fn archive_entry_gname_utf8(entry: *mut archive_entry) -> *const c_char;
    pub fn archive_entry_gname_w(entry: *mut archive_entry) -> *const wchar_t;
    pub fn archive_entry_hardlink(entry: *mut archive_entry) -> *const c_char;
    pub fn archive_entry_hardlink_utf8(entry: *mut archive_entry) -> *const c_char;
    pub fn archive_entry_hardlink_w(entry: *mut archive_entry) -> *const wchar_t;
    pub fn archive_entry_ino(entry: *mut archive_entry) -> i64;
    pub fn archive_entry_ino64(entry: *mut archive_entry) -> i64;
    pub fn archive_entry_ino_is_set(entry: *mut archive_entry) -> c_int;
    pub fn archive_entry_is_data_encrypted(entry: *mut archive_entry) -> c_int;
    pub fn archive_entry_is_encrypted(entry: *mut archive_entry) -> c_int;
    pub fn archive_entry_is_metadata_encrypted(entry: *mut archive_entry) -> c_int;
    pub fn archive_entry_mac_metadata(
        entry: *mut archive_entry,
        size: *mut size_t,
    ) -> *const c_void;
    pub fn archive_entry_mode(entry: *mut archive_entry) -> mode_t;
    pub fn archive_entry_mtime(entry: *mut archive_entry) -> i64;
    pub fn archive_entry_mtime_nsec(entry: *mut archive_entry) -> c_long;
    pub fn archive_entry_mtime_is_set(entry: *mut archive_entry) -> c_int;
    pub fn archive_entry_nlink(entry: *mut archive_entry) -> c_uint;
    pub fn archive_entry_pathname(entry: *mut archive_entry) -> *const c_char;
    pub fn archive_entry_pathname_utf8(entry: *mut archive_entry) -> *const c_char;
    pub fn archive_entry_pathname_w(entry: *mut archive_entry) -> *const wchar_t;
    pub fn archive_entry_perm(entry: *mut archive_entry) -> mode_t;
    pub fn archive_entry_rdev(entry: *mut archive_entry) -> dev_t;
    pub fn archive_entry_rdevmajor(entry: *mut archive_entry) -> dev_t;
    pub fn archive_entry_rdevminor(entry: *mut archive_entry) -> dev_t;
    pub fn archive_entry_sourcepath(entry: *mut archive_entry) -> *const c_char;
    pub fn archive_entry_sourcepath_w(entry: *mut archive_entry) -> *const wchar_t;
    pub fn archive_entry_size(entry: *mut archive_entry) -> i64;
    pub fn archive_entry_size_is_set(entry: *mut archive_entry) -> c_int;
    pub fn archive_entry_stat(entry: *mut archive_entry) -> *const stat;
    pub fn archive_entry_strmode(entry: *mut archive_entry) -> *const c_char;
    pub fn archive_entry_symlink(entry: *mut archive_entry) -> *const c_char;
    pub fn archive_entry_symlink_utf8(entry: *mut archive_entry) -> *const c_char;
    pub fn archive_entry_symlink_w(entry: *mut archive_entry) -> *const wchar_t;
    pub fn archive_entry_symlink_type(entry: *mut archive_entry) -> c_int;
    pub fn archive_entry_uid(entry: *mut archive_entry) -> i64;
    pub fn archive_entry_uname(entry: *mut archive_entry) -> *const c_char;
    pub fn archive_entry_uname_utf8(entry: *mut archive_entry) -> *const c_char;
    pub fn archive_entry_uname_w(entry: *mut archive_entry) -> *const wchar_t;
    pub fn archive_entry_digest(entry: *mut archive_entry, digest_type: c_int) -> *const c_uchar;

    pub fn archive_entry_set_atime(entry: *mut archive_entry, t: i64, ns: c_long);
    pub fn archive_entry_unset_atime(entry: *mut archive_entry);
    pub fn archive_entry_set_birthtime(entry: *mut archive_entry, t: i64, ns: c_long);
    pub fn archive_entry_unset_birthtime(entry: *mut archive_entry);
    pub fn archive_entry_set_ctime(entry: *mut archive_entry, t: i64, ns: c_long);
    pub fn archive_entry_unset_ctime(entry: *mut archive_entry);
    pub fn archive_entry_set_dev(entry: *mut archive_entry, d: dev_t);
    pub fn archive_entry_set_devmajor(entry: *mut archive_entry, d: dev_t);
    pub fn archive_entry_set_devminor(entry: *mut archive_entry, d: dev_t);
    pub fn archive_entry_set_filetype(entry: *mut archive_entry, filetype: c_uint);
    pub fn archive_entry_set_fflags(entry: *mut archive_entry, set: c_ulong, clear: c_ulong);
    pub fn archive_entry_copy_fflags_text(
        entry: *mut archive_entry,
        text: *const c_char,
    ) -> *const c_char;
    pub fn archive_entry_copy_fflags_text_w(
        entry: *mut archive_entry,
        text: *const wchar_t,
    ) -> *const wchar_t;
    pub fn archive_entry_set_gid(entry: *mut archive_entry, gid: i64);
    pub fn archive_entry_set_gname(entry: *mut archive_entry, name: *const c_char);
    pub fn archive_entry_set_gname_utf8(entry: *mut archive_entry, name: *const c_char);
    pub fn archive_entry_copy_gname(entry: *mut archive_entry, name: *const c_char);
    pub fn archive_entry_copy_gname_w(entry: *mut archive_entry, name: *const wchar_t);
    pub fn archive_entry_update_gname_utf8(entry: *mut archive_entry, name: *const c_char)
        -> c_int;
    pub fn archive_entry_set_hardlink(entry: *mut archive_entry, target: *const c_char);
    pub fn archive_entry_set_hardlink_utf8(entry: *mut archive_entry, target: *const c_char);
    pub fn archive_entry_copy_hardlink(entry: *mut archive_entry, target: *const c_char);
    pub fn archive_entry_copy_hardlink_w(entry: *mut archive_entry, target: *const wchar_t);
    pub fn archive_entry_update_hardlink_utf8(
        entry: *mut archive_entry,
        target: *const c_char,
    ) -> c_int;
    pub fn archive_entry_set_ino(entry: *mut archive_entry, ino: i64);
    pub fn archive_entry_set_ino64(entry: *mut archive_entry, ino: i64);
    pub fn archive_entry_set_link(entry: *mut archive_entry, target: *const c_char);
    pub fn archive_entry_set_link_utf8(entry: *mut archive_entry, target: *const c_char);
    pub fn archive_entry_copy_link(entry: *mut archive_entry, target: *const c_char);
    pub fn archive_entry_copy_link_w(entry: *mut archive_entry, target: *const wchar_t);
    pub fn archive_entry_update_link_utf8(
        entry: *mut archive_entry,
        target: *const c_char,
    ) -> c_int;
    pub fn archive_entry_set_mode(entry: *mut archive_entry, mode: mode_t);
    pub fn archive_entry_set_mtime(entry: *mut archive_entry, t: i64, ns: c_long);
    pub fn archive_entry_unset_mtime(entry: *mut archive_entry);
    pub fn archive_entry_set_nlink(entry: *mut archive_entry, nlink: c_uint);
    pub fn archive_entry_set_pathname(entry: *mut archive_entry, name: *const c_char);
    pub fn archive_entry_set_pathname_utf8(entry: *mut archive_entry, name: *const c_char);
    pub fn archive_entry_copy_pathname(entry: *mut archive_entry, name: *const c_char);
    pub fn archive_entry_copy_pathname_w(entry: *mut archive_entry, name: *const wchar_t);
    pub fn archive_entry_update_pathname_utf8(
        entry: *mut archive_entry,
        name: *const c_char,
    ) -> c_int;
    pub fn archive_entry_set_perm(entry: *mut archive_entry, perm: mode_t);
    pub fn archive_entry_set_rdev(entry: *mut archive_entry, rdev: dev_t);
    pub fn archive_entry_set_rdevmajor(entry: *mut archive_entry, rdev: dev_t);
    pub fn archive_entry_set_rdevminor(entry: *mut archive_entry, rdev: dev_t);
    pub fn archive_entry_set_size(entry: *mut archive_entry, size: i64);
    pub fn archive_entry_unset_size(entry: *mut archive_entry);
    pub fn archive_entry_copy_sourcepath(entry: *mut archive_entry, path: *const c_char);
    pub fn archive_entry_copy_sourcepath_w(entry: *mut archive_entry, path: *const wchar_t);
    pub fn archive_entry_set_symlink(entry: *mut archive_entry, linkname: *const c_char);
    pub fn archive_entry_set_symlink_type(entry: *mut archive_entry, symlink_type: c_int);
    pub fn archive_entry_set_symlink_utf8(entry: *mut archive_entry, linkname: *const c_char);
    pub fn archive_entry_copy_symlink(entry: *mut archive_entry, linkname: *const c_char);
    pub fn archive_entry_copy_symlink_w(entry: *mut archive_entry, linkname: *const wchar_t);
    pub fn archive_entry_update_symlink_utf8(
        entry: *mut archive_entry,
        linkname: *const c_char,
    ) -> c_int;
    pub fn archive_entry_set_uid(entry: *mut archive_entry, uid: i64);
    pub fn archive_entry_set_uname(entry: *mut archive_entry, name: *const c_char);
    pub fn archive_entry_set_uname_utf8(entry: *mut archive_entry, name: *const c_char);
    pub fn archive_entry_copy_uname(entry: *mut archive_entry, name: *const c_char);
    pub fn archive_entry_copy_uname_w(entry: *mut archive_entry, name: *const wchar_t);
    pub fn archive_entry_update_uname_utf8(entry: *mut archive_entry, name: *const c_char)
        -> c_int;
    pub fn archive_entry_set_is_data_encrypted(entry: *mut archive_entry, encrypted: c_char);
    pub fn archive_entry_set_is_metadata_encrypted(entry: *mut archive_entry, encrypted: c_char);
    pub fn archive_entry_copy_mac_metadata(
        entry: *mut archive_entry,
        metadata: *const c_void,
        size: size_t,
    );
    pub fn archive_entry_copy_stat(entry: *mut archive_entry, st: *const stat);

    pub fn archive_entry_acl(entry: *mut archive_entry) -> *mut archive_acl;
    pub fn archive_entry_acl_clear(entry: *mut archive_entry);
    pub fn archive_entry_acl_add_entry(
        entry: *mut archive_entry,
        entry_type: c_int,
        permset: c_int,
        tag: c_int,
        qual: c_int,
        name: *const c_char,
    ) -> c_int;
    pub fn archive_entry_acl_add_entry_w(
        entry: *mut archive_entry,
        entry_type: c_int,
        permset: c_int,
        tag: c_int,
        qual: c_int,
        name: *const wchar_t,
    ) -> c_int;
    pub fn archive_entry_acl_count(entry: *mut archive_entry, want_type: c_int) -> c_int;
    pub fn archive_entry_acl_from_text(
        entry: *mut archive_entry,
        text: *const c_char,
        want_type: c_int,
    ) -> c_int;
    pub fn archive_entry_acl_from_text_w(
        entry: *mut archive_entry,
        text: *const wchar_t,
        want_type: c_int,
    ) -> c_int;
    pub fn archive_entry_acl_next(
        entry: *mut archive_entry,
        want_type: c_int,
        entry_type: *mut c_int,
        permset: *mut c_int,
        tag: *mut c_int,
        qual: *mut c_int,
        name: *mut *const c_char,
    ) -> c_int;
    pub fn archive_entry_acl_reset(entry: *mut archive_entry, want_type: c_int) -> c_int;
    pub fn archive_entry_acl_text(entry: *mut archive_entry, flags: c_int) -> *const c_char;
    pub fn archive_entry_acl_text_w(entry: *mut archive_entry, flags: c_int) -> *const wchar_t;
    pub fn archive_entry_acl_to_text(
        entry: *mut archive_entry,
        text_len: *mut isize,
        flags: c_int,
    ) -> *mut c_char;
    pub fn archive_entry_acl_to_text_w(
        entry: *mut archive_entry,
        text_len: *mut isize,
        flags: c_int,
    ) -> *mut wchar_t;
    pub fn archive_entry_acl_types(entry: *mut archive_entry) -> c_int;

    pub fn archive_entry_xattr_clear(entry: *mut archive_entry);
    pub fn archive_entry_xattr_add_entry(
        entry: *mut archive_entry,
        name: *const c_char,
        value: *const c_void,
        size: size_t,
    );
    pub fn archive_entry_xattr_count(entry: *mut archive_entry) -> c_int;
    pub fn archive_entry_xattr_reset(entry: *mut archive_entry) -> c_int;
    pub fn archive_entry_xattr_next(
        entry: *mut archive_entry,
        name: *mut *const c_char,
        value: *mut *const c_void,
        size: *mut size_t,
    ) -> c_int;

    pub fn archive_entry_sparse_clear(entry: *mut archive_entry);
    pub fn archive_entry_sparse_add_entry(entry: *mut archive_entry, offset: i64, length: i64);
    pub fn archive_entry_sparse_count(entry: *mut archive_entry) -> c_int;
    pub fn archive_entry_sparse_reset(entry: *mut archive_entry) -> c_int;
    pub fn archive_entry_sparse_next(
        entry: *mut archive_entry,
        offset: *mut i64,
        length: *mut i64,
    ) -> c_int;

    pub fn archive_entry_linkresolver_new() -> *mut archive_entry_linkresolver;
    pub fn archive_entry_linkresolver_set_strategy(
        resolver: *mut archive_entry_linkresolver,
        format_code: c_int,
    );
    pub fn archive_entry_linkresolver_free(resolver: *mut archive_entry_linkresolver);
    pub fn archive_entry_linkify(
        resolver: *mut archive_entry_linkresolver,
        entry: *mut *mut archive_entry,
        spare: *mut *mut archive_entry,
    );
    pub fn archive_entry_partial_links(
        resolver: *mut archive_entry_linkresolver,
        links: *mut c_uint,
    ) -> *mut archive_entry;
}
