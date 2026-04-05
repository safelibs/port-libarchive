use std::ffi::{c_char, c_int, c_long, c_uchar, c_uint, c_ulong, c_void};
use std::ptr;

use libc::{dev_t, mode_t, size_t, stat, wchar_t};

use crate::common::error::{ARCHIVE_EOF, ARCHIVE_FATAL, ARCHIVE_OK};
use crate::common::helpers::{bool_to_int, from_optional_c_str, from_optional_wide};
use crate::entry::internal::{
    add_sparse, add_xattr, clear_acl, clear_entry, clone_entry, copy_stat, entry_has_acl,
    free_linkresolver, free_raw_entry, from_raw, linkify, materialize_stat, new_raw_entry,
    next_sparse, next_xattr, partial_links, reset_sparse, reset_xattrs, set_filetype, set_link_target,
    set_mode, set_perm, strmode, update_c_text, update_text, update_wide_text, AclState,
    ArchiveEntryData, LinkResolverData, ARCHIVE_ENTRY_ACL_ENTRY_DIRECTORY_INHERIT,
    ARCHIVE_ENTRY_ACL_ENTRY_FAILED_ACCESS, ARCHIVE_ENTRY_ACL_ENTRY_FILE_INHERIT,
    ARCHIVE_ENTRY_ACL_ENTRY_INHERIT_ONLY, ARCHIVE_ENTRY_ACL_ENTRY_INHERITED,
    ARCHIVE_ENTRY_ACL_ENTRY_NO_PROPAGATE_INHERIT, ARCHIVE_ENTRY_ACL_ENTRY_SUCCESSFUL_ACCESS,
    ARCHIVE_ENTRY_ACL_EXECUTE, ARCHIVE_ENTRY_ACL_READ, ARCHIVE_ENTRY_ACL_READ_ACL,
    ARCHIVE_ENTRY_ACL_READ_ATTRIBUTES, ARCHIVE_ENTRY_ACL_READ_DATA,
    ARCHIVE_ENTRY_ACL_READ_NAMED_ATTRS, ARCHIVE_ENTRY_ACL_SYNCHRONIZE,
    ARCHIVE_ENTRY_ACL_TYPE_ACCESS, ARCHIVE_ENTRY_ACL_TYPE_ALLOW, ARCHIVE_ENTRY_ACL_TYPE_DEFAULT,
    ARCHIVE_ENTRY_ACL_TYPE_DENY, ARCHIVE_ENTRY_ACL_WRITE, ARCHIVE_ENTRY_ACL_WRITE_ACL,
    ARCHIVE_ENTRY_ACL_WRITE_ATTRIBUTES, ARCHIVE_ENTRY_ACL_WRITE_DATA,
    ARCHIVE_ENTRY_ACL_WRITE_NAMED_ATTRS, ARCHIVE_ENTRY_ACL_WRITE_OWNER, AE_IFMT,
};
use crate::ffi::{archive, archive_acl, archive_entry, archive_entry_linkresolver};

fn mark_dirty(entry: &mut ArchiveEntryData) {
    entry.stat_dirty = true;
    entry.strmode_cache = None;
}

fn devmajor(dev: dev_t) -> dev_t {
    (dev >> 8) & 0xfff
}

fn devminor(dev: dev_t) -> dev_t {
    dev & 0xff
}

fn makedev(major: dev_t, minor: dev_t) -> dev_t {
    ((major & 0xfff) << 8) | (minor & 0xff)
}

fn parse_fflags_text(text: &str) -> (c_ulong, c_ulong) {
    const FS_APPEND_FL: c_ulong = 0x0000_0020;
    const FS_IMMUTABLE_FL: c_ulong = 0x0000_0010;
    const FS_NODUMP_FL: c_ulong = 0x0000_0040;
    const FS_UNRM_FL: c_ulong = 0x0000_0002;

    let mut set = 0;
    let mut clear = 0;
    for token in text.split(',').map(str::trim).filter(|token| !token.is_empty()) {
        let (negated, name) = token
            .strip_prefix("no")
            .map_or((false, token), |name| (true, name));
        let bit = match name {
            "sappnd" | "uappnd" => FS_APPEND_FL,
            "schg" | "uchg" => FS_IMMUTABLE_FL,
            "dump" => FS_NODUMP_FL,
            "undel" | "uunlink" => FS_UNRM_FL,
            _ => 0,
        };
        if negated {
            clear |= bit;
        } else {
            set |= bit;
        }
    }
    (set, clear)
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_new() -> *mut archive_entry {
    new_raw_entry(ptr::null_mut())
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_new2(a: *mut archive) -> *mut archive_entry {
    new_raw_entry(a)
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_free(entry: *mut archive_entry) {
    free_raw_entry(entry);
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_clear(entry: *mut archive_entry) -> *mut archive_entry {
    if let Some(entry_data) = from_raw(entry) {
        clear_entry(entry_data);
    }
    entry
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_clone(entry: *mut archive_entry) -> *mut archive_entry {
    let Some(entry_data) = from_raw(entry) else {
        return ptr::null_mut();
    };
    Box::into_raw(Box::new(clone_entry(entry_data))) as *mut archive_entry
}

macro_rules! time_getters {
    ($name:ident, $field:ident, $ty:ty) => {
        #[no_mangle]
        pub unsafe extern "C" fn $name(entry: *mut archive_entry) -> $ty {
            from_raw(entry).map_or(0, |entry_data| entry_data.$field.sec as $ty)
        }
    };
}

time_getters!(archive_entry_atime, atime, i64);
time_getters!(archive_entry_birthtime, birthtime, i64);
time_getters!(archive_entry_ctime, ctime, i64);
time_getters!(archive_entry_mtime, mtime, i64);

macro_rules! time_nsec_getters {
    ($name:ident, $field:ident) => {
        #[no_mangle]
        pub unsafe extern "C" fn $name(entry: *mut archive_entry) -> c_long {
            from_raw(entry).map_or(0, |entry_data| entry_data.$field.nsec)
        }
    };
}

time_nsec_getters!(archive_entry_atime_nsec, atime);
time_nsec_getters!(archive_entry_birthtime_nsec, birthtime);
time_nsec_getters!(archive_entry_ctime_nsec, ctime);
time_nsec_getters!(archive_entry_mtime_nsec, mtime);

macro_rules! time_is_set {
    ($name:ident, $field:ident) => {
        #[no_mangle]
        pub unsafe extern "C" fn $name(entry: *mut archive_entry) -> c_int {
            from_raw(entry).map_or(0, |entry_data| bool_to_int(entry_data.$field.set))
        }
    };
}

time_is_set!(archive_entry_atime_is_set, atime);
time_is_set!(archive_entry_birthtime_is_set, birthtime);
time_is_set!(archive_entry_ctime_is_set, ctime);
time_is_set!(archive_entry_mtime_is_set, mtime);

macro_rules! time_setters {
    ($name:ident, $field:ident) => {
        #[no_mangle]
        pub unsafe extern "C" fn $name(entry: *mut archive_entry, sec: i64, nsec: c_long) {
            if let Some(entry_data) = from_raw(entry) {
                entry_data.$field.set(sec, nsec as i64);
                mark_dirty(entry_data);
            }
        }
    };
}

time_setters!(archive_entry_set_atime, atime);
time_setters!(archive_entry_set_birthtime, birthtime);
time_setters!(archive_entry_set_ctime, ctime);
time_setters!(archive_entry_set_mtime, mtime);

macro_rules! time_unsetters {
    ($name:ident, $field:ident) => {
        #[no_mangle]
        pub unsafe extern "C" fn $name(entry: *mut archive_entry) {
            if let Some(entry_data) = from_raw(entry) {
                entry_data.$field.unset();
                mark_dirty(entry_data);
            }
        }
    };
}

time_unsetters!(archive_entry_unset_atime, atime);
time_unsetters!(archive_entry_unset_birthtime, birthtime);
time_unsetters!(archive_entry_unset_ctime, ctime);
time_unsetters!(archive_entry_unset_mtime, mtime);

#[no_mangle]
pub unsafe extern "C" fn archive_entry_dev(entry: *mut archive_entry) -> dev_t {
    from_raw(entry).map_or(0, |entry_data| entry_data.dev)
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_dev_is_set(entry: *mut archive_entry) -> c_int {
    from_raw(entry).map_or(0, |entry_data| bool_to_int(entry_data.dev_set))
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_devmajor(entry: *mut archive_entry) -> dev_t {
    from_raw(entry).map_or(0, |entry_data| devmajor(entry_data.dev))
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_devminor(entry: *mut archive_entry) -> dev_t {
    from_raw(entry).map_or(0, |entry_data| devminor(entry_data.dev))
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_rdev(entry: *mut archive_entry) -> dev_t {
    from_raw(entry).map_or(0, |entry_data| entry_data.rdev)
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_rdevmajor(entry: *mut archive_entry) -> dev_t {
    from_raw(entry).map_or(0, |entry_data| devmajor(entry_data.rdev))
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_rdevminor(entry: *mut archive_entry) -> dev_t {
    from_raw(entry).map_or(0, |entry_data| devminor(entry_data.rdev))
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_set_dev(entry: *mut archive_entry, dev: dev_t) {
    if let Some(entry_data) = from_raw(entry) {
        entry_data.dev = dev;
        entry_data.dev_set = true;
        mark_dirty(entry_data);
    }
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_set_devmajor(entry: *mut archive_entry, dev: dev_t) {
    if let Some(entry_data) = from_raw(entry) {
        entry_data.dev = makedev(dev, devminor(entry_data.dev));
        entry_data.dev_set = true;
        mark_dirty(entry_data);
    }
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_set_devminor(entry: *mut archive_entry, dev: dev_t) {
    if let Some(entry_data) = from_raw(entry) {
        entry_data.dev = makedev(devmajor(entry_data.dev), dev);
        entry_data.dev_set = true;
        mark_dirty(entry_data);
    }
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_set_rdev(entry: *mut archive_entry, dev: dev_t) {
    if let Some(entry_data) = from_raw(entry) {
        entry_data.rdev = dev;
        mark_dirty(entry_data);
    }
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_set_rdevmajor(entry: *mut archive_entry, dev: dev_t) {
    if let Some(entry_data) = from_raw(entry) {
        entry_data.rdev = makedev(dev, devminor(entry_data.rdev));
        mark_dirty(entry_data);
    }
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_set_rdevminor(entry: *mut archive_entry, dev: dev_t) {
    if let Some(entry_data) = from_raw(entry) {
        entry_data.rdev = makedev(devmajor(entry_data.rdev), dev);
        mark_dirty(entry_data);
    }
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_filetype(entry: *mut archive_entry) -> mode_t {
    from_raw(entry).map_or(0, |entry_data| entry_data.mode & AE_IFMT)
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_perm(entry: *mut archive_entry) -> mode_t {
    from_raw(entry).map_or(0, |entry_data| entry_data.mode & 0o7777)
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_mode(entry: *mut archive_entry) -> mode_t {
    from_raw(entry).map_or(0, |entry_data| entry_data.mode)
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_set_filetype(entry: *mut archive_entry, filetype: c_uint) {
    if let Some(entry_data) = from_raw(entry) {
        set_filetype(entry_data, filetype as mode_t);
    }
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_set_perm(entry: *mut archive_entry, perm: mode_t) {
    if let Some(entry_data) = from_raw(entry) {
        set_perm(entry_data, perm);
    }
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_set_mode(entry: *mut archive_entry, mode: mode_t) {
    if let Some(entry_data) = from_raw(entry) {
        set_mode(entry_data, mode);
    }
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_fflags(
    entry: *mut archive_entry,
    set: *mut c_ulong,
    clear: *mut c_ulong,
) {
    if let Some(entry_data) = from_raw(entry) {
        if !set.is_null() {
            *set = entry_data.fflags_set;
        }
        if !clear.is_null() {
            *clear = entry_data.fflags_clear;
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_fflags_text(_entry: *mut archive_entry) -> *const c_char {
    ptr::null()
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_set_fflags(
    entry: *mut archive_entry,
    set: c_ulong,
    clear: c_ulong,
) {
    if let Some(entry_data) = from_raw(entry) {
        entry_data.fflags_set = set;
        entry_data.fflags_clear = clear;
    }
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_copy_fflags_text(
    entry: *mut archive_entry,
    text: *const c_char,
) -> *const c_char {
    if let Some(entry_data) = from_raw(entry) {
        let (set, clear) = parse_fflags_text(&from_optional_c_str(text).unwrap_or_default());
        entry_data.fflags_set = set;
        entry_data.fflags_clear = clear;
    }
    ptr::null()
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_copy_fflags_text_w(
    entry: *mut archive_entry,
    text: *const wchar_t,
) -> *const wchar_t {
    if let Some(entry_data) = from_raw(entry) {
        let (set, clear) = parse_fflags_text(&from_optional_wide(text).unwrap_or_default());
        entry_data.fflags_set = set;
        entry_data.fflags_clear = clear;
    }
    ptr::null()
}

macro_rules! int_getters {
    ($name:ident, $field:ident, $ty:ty) => {
        #[no_mangle]
        pub unsafe extern "C" fn $name(entry: *mut archive_entry) -> $ty {
            from_raw(entry).map_or(0, |entry_data| entry_data.$field as $ty)
        }
    };
}

int_getters!(archive_entry_gid, gid, i64);
int_getters!(archive_entry_uid, uid, i64);
int_getters!(archive_entry_ino, ino, i64);
int_getters!(archive_entry_ino64, ino, i64);
int_getters!(archive_entry_size, size, i64);

#[no_mangle]
pub unsafe extern "C" fn archive_entry_nlink(entry: *mut archive_entry) -> c_uint {
    from_raw(entry).map_or(0, |entry_data| entry_data.nlink)
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_ino_is_set(entry: *mut archive_entry) -> c_int {
    from_raw(entry).map_or(0, |entry_data| bool_to_int(entry_data.ino_set))
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_size_is_set(entry: *mut archive_entry) -> c_int {
    from_raw(entry).map_or(0, |entry_data| bool_to_int(entry_data.size_set))
}

macro_rules! int_setters {
    ($name:ident, $field:ident, $ty:ty, $extra:expr) => {
        #[no_mangle]
        pub unsafe extern "C" fn $name(entry: *mut archive_entry, value: $ty) {
            if let Some(entry_data) = from_raw(entry) {
                entry_data.$field = value as _;
                $extra(entry_data);
            }
        }
    };
}

int_setters!(archive_entry_set_gid, gid, i64, |entry_data: &mut ArchiveEntryData| {
    mark_dirty(entry_data);
});
int_setters!(archive_entry_set_uid, uid, i64, |entry_data: &mut ArchiveEntryData| {
    mark_dirty(entry_data);
});
int_setters!(archive_entry_set_ino, ino, i64, |entry_data: &mut ArchiveEntryData| {
    entry_data.ino_set = true;
    mark_dirty(entry_data);
});
int_setters!(archive_entry_set_ino64, ino, i64, |entry_data: &mut ArchiveEntryData| {
    entry_data.ino_set = true;
    mark_dirty(entry_data);
});
int_setters!(archive_entry_set_size, size, i64, |entry_data: &mut ArchiveEntryData| {
    entry_data.size_set = true;
    mark_dirty(entry_data);
});

#[no_mangle]
pub unsafe extern "C" fn archive_entry_unset_size(entry: *mut archive_entry) {
    if let Some(entry_data) = from_raw(entry) {
        entry_data.size = 0;
        entry_data.size_set = false;
        mark_dirty(entry_data);
    }
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_set_nlink(entry: *mut archive_entry, value: c_uint) {
    if let Some(entry_data) = from_raw(entry) {
        entry_data.nlink = value;
        mark_dirty(entry_data);
    }
}

macro_rules! text_getters {
    ($name:ident, $field:ident) => {
        #[no_mangle]
        pub unsafe extern "C" fn $name(entry: *mut archive_entry) -> *const c_char {
            from_raw(entry).map_or(ptr::null(), |entry_data| entry_data.$field.as_c_ptr())
        }
    };
}

macro_rules! text_wide_getters {
    ($name:ident, $field:ident) => {
        #[no_mangle]
        pub unsafe extern "C" fn $name(entry: *mut archive_entry) -> *const wchar_t {
            from_raw(entry).map_or(ptr::null(), |entry_data| entry_data.$field.as_wide_ptr())
        }
    };
}

text_getters!(archive_entry_gname, gname);
text_getters!(archive_entry_gname_utf8, gname);
text_getters!(archive_entry_hardlink, hardlink);
text_getters!(archive_entry_hardlink_utf8, hardlink);
text_getters!(archive_entry_pathname, pathname);
text_getters!(archive_entry_pathname_utf8, pathname);
text_getters!(archive_entry_sourcepath, sourcepath);
text_getters!(archive_entry_symlink, symlink);
text_getters!(archive_entry_symlink_utf8, symlink);
text_getters!(archive_entry_uname, uname);
text_getters!(archive_entry_uname_utf8, uname);

text_wide_getters!(archive_entry_gname_w, gname);
text_wide_getters!(archive_entry_hardlink_w, hardlink);
text_wide_getters!(archive_entry_pathname_w, pathname);
text_wide_getters!(archive_entry_sourcepath_w, sourcepath);
text_wide_getters!(archive_entry_symlink_w, symlink);
text_wide_getters!(archive_entry_uname_w, uname);

macro_rules! text_setters {
    ($name:ident, $field:ident, c) => {
        #[no_mangle]
        pub unsafe extern "C" fn $name(entry: *mut archive_entry, value: *const c_char) {
            if let Some(entry_data) = from_raw(entry) {
                update_c_text(&mut entry_data.$field, value);
                mark_dirty(entry_data);
            }
        }
    };
    ($name:ident, $field:ident, w) => {
        #[no_mangle]
        pub unsafe extern "C" fn $name(entry: *mut archive_entry, value: *const wchar_t) {
            if let Some(entry_data) = from_raw(entry) {
                update_wide_text(&mut entry_data.$field, value);
                mark_dirty(entry_data);
            }
        }
    };
}

text_setters!(archive_entry_set_gname, gname, c);
text_setters!(archive_entry_set_gname_utf8, gname, c);
text_setters!(archive_entry_copy_gname, gname, c);
text_setters!(archive_entry_copy_gname_w, gname, w);
text_setters!(archive_entry_set_hardlink, hardlink, c);
text_setters!(archive_entry_set_hardlink_utf8, hardlink, c);
text_setters!(archive_entry_copy_hardlink, hardlink, c);
text_setters!(archive_entry_copy_hardlink_w, hardlink, w);
text_setters!(archive_entry_set_pathname, pathname, c);
text_setters!(archive_entry_set_pathname_utf8, pathname, c);
text_setters!(archive_entry_copy_pathname, pathname, c);
text_setters!(archive_entry_copy_pathname_w, pathname, w);
text_setters!(archive_entry_copy_sourcepath, sourcepath, c);
text_setters!(archive_entry_copy_sourcepath_w, sourcepath, w);
text_setters!(archive_entry_set_symlink, symlink, c);
text_setters!(archive_entry_set_symlink_utf8, symlink, c);
text_setters!(archive_entry_copy_symlink, symlink, c);
text_setters!(archive_entry_copy_symlink_w, symlink, w);
text_setters!(archive_entry_set_uname, uname, c);
text_setters!(archive_entry_set_uname_utf8, uname, c);
text_setters!(archive_entry_copy_uname, uname, c);
text_setters!(archive_entry_copy_uname_w, uname, w);

macro_rules! text_updates {
    ($name:ident, $field:ident, c) => {
        #[no_mangle]
        pub unsafe extern "C" fn $name(entry: *mut archive_entry, value: *const c_char) -> c_int {
            if let Some(entry_data) = from_raw(entry) {
                update_c_text(&mut entry_data.$field, value);
                mark_dirty(entry_data);
                1
            } else {
                0
            }
        }
    };
}

text_updates!(archive_entry_update_gname_utf8, gname, c);
text_updates!(archive_entry_update_hardlink_utf8, hardlink, c);
text_updates!(archive_entry_update_pathname_utf8, pathname, c);
text_updates!(archive_entry_update_symlink_utf8, symlink, c);
text_updates!(archive_entry_update_uname_utf8, uname, c);

#[no_mangle]
pub unsafe extern "C" fn archive_entry_set_link(entry: *mut archive_entry, value: *const c_char) {
    if let Some(entry_data) = from_raw(entry) {
        set_link_target(entry_data, from_optional_c_str(value));
        mark_dirty(entry_data);
    }
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_set_link_utf8(
    entry: *mut archive_entry,
    value: *const c_char,
) {
    archive_entry_set_link(entry, value);
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_copy_link(entry: *mut archive_entry, value: *const c_char) {
    archive_entry_set_link(entry, value);
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_copy_link_w(
    entry: *mut archive_entry,
    value: *const wchar_t,
) {
    if let Some(entry_data) = from_raw(entry) {
        set_link_target(entry_data, from_optional_wide(value));
        mark_dirty(entry_data);
    }
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_update_link_utf8(
    entry: *mut archive_entry,
    value: *const c_char,
) -> c_int {
    archive_entry_set_link(entry, value);
    1
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_symlink_type(entry: *mut archive_entry) -> c_int {
    from_raw(entry).map_or(0, |entry_data| entry_data.symlink_type)
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_set_symlink_type(entry: *mut archive_entry, value: c_int) {
    if let Some(entry_data) = from_raw(entry) {
        entry_data.symlink_type = value;
    }
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_is_data_encrypted(entry: *mut archive_entry) -> c_int {
    from_raw(entry).map_or(0, |entry_data| bool_to_int(entry_data.data_encrypted))
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_is_metadata_encrypted(entry: *mut archive_entry) -> c_int {
    from_raw(entry).map_or(0, |entry_data| bool_to_int(entry_data.metadata_encrypted))
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_is_encrypted(entry: *mut archive_entry) -> c_int {
    from_raw(entry).map_or(0, |entry_data| {
        bool_to_int(entry_data.data_encrypted || entry_data.metadata_encrypted)
    })
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_set_is_data_encrypted(
    entry: *mut archive_entry,
    encrypted: i8,
) {
    if let Some(entry_data) = from_raw(entry) {
        entry_data.data_encrypted = encrypted != 0;
    }
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_set_is_metadata_encrypted(
    entry: *mut archive_entry,
    encrypted: i8,
) {
    if let Some(entry_data) = from_raw(entry) {
        entry_data.metadata_encrypted = encrypted != 0;
    }
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_mac_metadata(
    entry: *mut archive_entry,
    size: *mut size_t,
) -> *const c_void {
    let Some(entry_data) = from_raw(entry) else {
        return ptr::null();
    };
    if !size.is_null() {
        *size = entry_data.mac_metadata.len();
    }
    if entry_data.mac_metadata.is_empty() {
        ptr::null()
    } else {
        entry_data.mac_metadata.as_ptr().cast()
    }
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_copy_mac_metadata(
    entry: *mut archive_entry,
    metadata: *const c_void,
    size: size_t,
) {
    if let Some(entry_data) = from_raw(entry) {
        if metadata.is_null() || size == 0 {
            entry_data.mac_metadata.clear();
        } else {
            entry_data.mac_metadata =
                std::slice::from_raw_parts(metadata.cast::<u8>(), size).to_vec();
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_copy_stat(entry: *mut archive_entry, st: *const stat) {
    if let (Some(entry_data), Some(st)) = (from_raw(entry), st.as_ref()) {
        copy_stat(entry_data, st);
    }
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_stat(entry: *mut archive_entry) -> *const stat {
    from_raw(entry)
        .map(|entry_data| materialize_stat(entry_data) as *const stat)
        .unwrap_or(ptr::null())
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_strmode(entry: *mut archive_entry) -> *const c_char {
    from_raw(entry).map_or(ptr::null(), strmode)
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_digest(
    _entry: *mut archive_entry,
    _digest_type: c_int,
) -> *const c_uchar {
    ptr::null()
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_acl(entry: *mut archive_entry) -> *mut archive_acl {
    from_raw(entry)
        .map(|entry_data| ptr::addr_of_mut!(entry_data.acl).cast::<archive_acl>())
        .unwrap_or(ptr::null_mut())
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_acl_clear(entry: *mut archive_entry) {
    if let Some(entry_data) = from_raw(entry) {
        clear_acl(entry_data);
    }
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_acl_add_entry(
    entry: *mut archive_entry,
    entry_type: c_int,
    permset: c_int,
    tag: c_int,
    qual: c_int,
    name: *const c_char,
) -> c_int {
    let Some(entry_data) = from_raw(entry) else {
        return ARCHIVE_FATAL;
    };
    let status = entry_data
        .acl
        .add_entry(&mut entry_data.mode, entry_type, permset, tag, qual, from_optional_c_str(name));
    entry_data.strmode_cache = None;
    status
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_acl_add_entry_w(
    entry: *mut archive_entry,
    entry_type: c_int,
    permset: c_int,
    tag: c_int,
    qual: c_int,
    name: *const wchar_t,
) -> c_int {
    let Some(entry_data) = from_raw(entry) else {
        return ARCHIVE_FATAL;
    };
    let status = entry_data
        .acl
        .add_entry(&mut entry_data.mode, entry_type, permset, tag, qual, from_optional_wide(name));
    entry_data.strmode_cache = None;
    status
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_acl_count(
    entry: *mut archive_entry,
    want_type: c_int,
) -> c_int {
    from_raw(entry).map_or(0, |entry_data| entry_data.acl.count(want_type))
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_acl_reset(
    entry: *mut archive_entry,
    want_type: c_int,
) -> c_int {
    from_raw(entry).map_or(0, |entry_data| entry_data.acl.reset(entry_data.mode, want_type))
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_acl_next(
    entry: *mut archive_entry,
    _want_type: c_int,
    entry_type: *mut c_int,
    permset: *mut c_int,
    tag: *mut c_int,
    qual: *mut c_int,
    name: *mut *const c_char,
) -> c_int {
    from_raw(entry).map_or(ARCHIVE_EOF, |entry_data| {
        entry_data
            .acl
            .next(entry_type, permset, tag, qual, name)
    })
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_acl_text(
    entry: *mut archive_entry,
    flags: c_int,
) -> *const c_char {
    from_raw(entry)
        .map(|entry_data| entry_data.acl.text_ptr(entry_data.mode, flags))
        .unwrap_or(ptr::null())
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_acl_text_w(
    entry: *mut archive_entry,
    flags: c_int,
) -> *const wchar_t {
    from_raw(entry)
        .map(|entry_data| entry_data.acl.text_w_ptr(entry_data.mode, flags))
        .unwrap_or(ptr::null())
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_acl_to_text(
    entry: *mut archive_entry,
    text_len: *mut isize,
    flags: c_int,
) -> *mut c_char {
    from_raw(entry)
        .map(|entry_data| entry_data.acl.to_text_malloc(entry_data.mode, flags, text_len))
        .unwrap_or(ptr::null_mut())
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_acl_to_text_w(
    entry: *mut archive_entry,
    text_len: *mut isize,
    flags: c_int,
) -> *mut wchar_t {
    from_raw(entry)
        .map(|entry_data| entry_data.acl.to_text_w_malloc(entry_data.mode, flags, text_len))
        .unwrap_or(ptr::null_mut())
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_acl_types(entry: *mut archive_entry) -> c_int {
    from_raw(entry).map_or(0, |entry_data| entry_data.acl.types())
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_acl_from_text(
    entry: *mut archive_entry,
    text: *const c_char,
    want_type: c_int,
) -> c_int {
    let Some(entry_data) = from_raw(entry) else {
        return ARCHIVE_FATAL;
    };
    let text = from_optional_c_str(text).unwrap_or_default();
    let status = entry_data.acl.from_text(&mut entry_data.mode, &text, want_type);
    entry_data.strmode_cache = None;
    status
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_acl_from_text_w(
    entry: *mut archive_entry,
    text: *const wchar_t,
    want_type: c_int,
) -> c_int {
    let Some(entry_data) = from_raw(entry) else {
        return ARCHIVE_FATAL;
    };
    let text = from_optional_wide(text).unwrap_or_default();
    let status = entry_data.acl.from_text(&mut entry_data.mode, &text, want_type);
    entry_data.strmode_cache = None;
    status
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_xattr_clear(entry: *mut archive_entry) {
    if let Some(entry_data) = from_raw(entry) {
        entry_data.xattrs.clear();
        entry_data.xattr_iter = 0;
    }
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_xattr_add_entry(
    entry: *mut archive_entry,
    name: *const c_char,
    value: *const c_void,
    size: size_t,
) {
    if let Some(entry_data) = from_raw(entry) {
        add_xattr(entry_data, name, value, size);
    }
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_xattr_count(entry: *mut archive_entry) -> c_int {
    from_raw(entry).map_or(0, |entry_data| entry_data.xattrs.len() as c_int)
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_xattr_reset(entry: *mut archive_entry) -> c_int {
    from_raw(entry).map_or(0, reset_xattrs)
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_xattr_next(
    entry: *mut archive_entry,
    name: *mut *const c_char,
    value: *mut *const c_void,
    size: *mut size_t,
) -> c_int {
    from_raw(entry).map_or(crate::common::error::ARCHIVE_WARN, |entry_data| {
        next_xattr(entry_data, name, value, size)
    })
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_sparse_clear(entry: *mut archive_entry) {
    if let Some(entry_data) = from_raw(entry) {
        entry_data.sparse.clear();
        entry_data.sparse_iter = 0;
    }
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_sparse_add_entry(
    entry: *mut archive_entry,
    offset: i64,
    length: i64,
) {
    if let Some(entry_data) = from_raw(entry) {
        add_sparse(entry_data, offset, length);
    }
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_sparse_count(entry: *mut archive_entry) -> c_int {
    from_raw(entry).map_or(0, |entry_data| entry_data.sparse.len() as c_int)
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_sparse_reset(entry: *mut archive_entry) -> c_int {
    from_raw(entry).map_or(0, reset_sparse)
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_sparse_next(
    entry: *mut archive_entry,
    offset: *mut i64,
    length: *mut i64,
) -> c_int {
    from_raw(entry).map_or(crate::common::error::ARCHIVE_WARN, |entry_data| {
        next_sparse(entry_data, offset, length)
    })
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_linkresolver_new() -> *mut archive_entry_linkresolver {
    Box::into_raw(Box::new(LinkResolverData {
        strategy: crate::ffi::archive_common::ARCHIVE_FORMAT_CPIO_POSIX,
        entries: std::collections::HashMap::new(),
    })) as *mut archive_entry_linkresolver
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_linkresolver_set_strategy(
    resolver: *mut archive_entry_linkresolver,
    format_code: c_int,
) {
    if let Some(resolver) = resolver.cast::<LinkResolverData>().as_mut() {
        resolver.strategy = format_code;
    }
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_linkresolver_free(
    resolver: *mut archive_entry_linkresolver,
) {
    free_linkresolver(resolver);
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_linkify(
    resolver: *mut archive_entry_linkresolver,
    entry: *mut *mut archive_entry,
    spare: *mut *mut archive_entry,
) {
    if let Some(resolver) = resolver.cast::<LinkResolverData>().as_mut() {
        linkify(resolver, entry, spare);
    }
}

#[no_mangle]
pub unsafe extern "C" fn archive_entry_partial_links(
    resolver: *mut archive_entry_linkresolver,
    links: *mut c_uint,
) -> *mut archive_entry {
    resolver
        .cast::<LinkResolverData>()
        .as_mut()
        .map(|resolver| partial_links(resolver, links))
        .unwrap_or(ptr::null_mut())
}
