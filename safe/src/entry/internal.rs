use std::ffi::{c_char, c_int, c_long, c_uchar, c_ulong, c_void, CStr, CString};
use std::mem;
use std::ptr;

use libc::{dev_t, mode_t, size_t, stat, wchar_t};

use crate::common::error::{ARCHIVE_EOF, ARCHIVE_FAILED, ARCHIVE_OK, ARCHIVE_WARN};
use crate::common::helpers::{
    bool_to_int, clone_c_string, empty_if_none, empty_if_none_wide, from_optional_c_str,
    from_optional_wide, malloc_bytes, malloc_wide, normalize_nanos, to_wide_null,
};
use crate::ffi::{archive, archive_acl, archive_entry, archive_entry_linkresolver};

pub(crate) const AE_IFMT: mode_t = 0o170000;
pub(crate) const AE_IFREG: mode_t = 0o100000;
pub(crate) const AE_IFLNK: mode_t = 0o120000;
pub(crate) const AE_IFSOCK: mode_t = 0o140000;
pub(crate) const AE_IFCHR: mode_t = 0o020000;
pub(crate) const AE_IFBLK: mode_t = 0o060000;
pub(crate) const AE_IFDIR: mode_t = 0o040000;
pub(crate) const AE_IFIFO: mode_t = 0o010000;

pub(crate) const ARCHIVE_ENTRY_ACL_EXECUTE: c_int = 0x0000_0001;
pub(crate) const ARCHIVE_ENTRY_ACL_WRITE: c_int = 0x0000_0002;
pub(crate) const ARCHIVE_ENTRY_ACL_READ: c_int = 0x0000_0004;
pub(crate) const ARCHIVE_ENTRY_ACL_READ_DATA: c_int = 0x0000_0008;
pub(crate) const ARCHIVE_ENTRY_ACL_WRITE_DATA: c_int = 0x0000_0010;
pub(crate) const ARCHIVE_ENTRY_ACL_APPEND_DATA: c_int = 0x0000_0020;
pub(crate) const ARCHIVE_ENTRY_ACL_READ_NAMED_ATTRS: c_int = 0x0000_0040;
pub(crate) const ARCHIVE_ENTRY_ACL_WRITE_NAMED_ATTRS: c_int = 0x0000_0080;
pub(crate) const ARCHIVE_ENTRY_ACL_DELETE_CHILD: c_int = 0x0000_0100;
pub(crate) const ARCHIVE_ENTRY_ACL_READ_ATTRIBUTES: c_int = 0x0000_0200;
pub(crate) const ARCHIVE_ENTRY_ACL_WRITE_ATTRIBUTES: c_int = 0x0000_0400;
pub(crate) const ARCHIVE_ENTRY_ACL_DELETE: c_int = 0x0000_0800;
pub(crate) const ARCHIVE_ENTRY_ACL_READ_ACL: c_int = 0x0000_1000;
pub(crate) const ARCHIVE_ENTRY_ACL_WRITE_ACL: c_int = 0x0000_2000;
pub(crate) const ARCHIVE_ENTRY_ACL_WRITE_OWNER: c_int = 0x0000_4000;
pub(crate) const ARCHIVE_ENTRY_ACL_SYNCHRONIZE: c_int = 0x0000_8000;

pub(crate) const ARCHIVE_ENTRY_ACL_ENTRY_INHERITED: c_int = 0x0100_0000;
pub(crate) const ARCHIVE_ENTRY_ACL_ENTRY_FILE_INHERIT: c_int = 0x0200_0000;
pub(crate) const ARCHIVE_ENTRY_ACL_ENTRY_DIRECTORY_INHERIT: c_int = 0x0400_0000;
pub(crate) const ARCHIVE_ENTRY_ACL_ENTRY_NO_PROPAGATE_INHERIT: c_int = 0x0800_0000;
pub(crate) const ARCHIVE_ENTRY_ACL_ENTRY_INHERIT_ONLY: c_int = 0x1000_0000;
pub(crate) const ARCHIVE_ENTRY_ACL_ENTRY_SUCCESSFUL_ACCESS: c_int = 0x2000_0000;
pub(crate) const ARCHIVE_ENTRY_ACL_ENTRY_FAILED_ACCESS: c_int = 0x4000_0000;

pub(crate) const ARCHIVE_ENTRY_ACL_TYPE_ACCESS: c_int = 0x0000_0100;
pub(crate) const ARCHIVE_ENTRY_ACL_TYPE_DEFAULT: c_int = 0x0000_0200;
pub(crate) const ARCHIVE_ENTRY_ACL_TYPE_ALLOW: c_int = 0x0000_0400;
pub(crate) const ARCHIVE_ENTRY_ACL_TYPE_DENY: c_int = 0x0000_0800;
pub(crate) const ARCHIVE_ENTRY_ACL_TYPE_AUDIT: c_int = 0x0000_1000;
pub(crate) const ARCHIVE_ENTRY_ACL_TYPE_ALARM: c_int = 0x0000_2000;
pub(crate) const ARCHIVE_ENTRY_ACL_TYPE_POSIX1E: c_int =
    ARCHIVE_ENTRY_ACL_TYPE_ACCESS | ARCHIVE_ENTRY_ACL_TYPE_DEFAULT;
pub(crate) const ARCHIVE_ENTRY_ACL_TYPE_NFS4: c_int = ARCHIVE_ENTRY_ACL_TYPE_ALLOW
    | ARCHIVE_ENTRY_ACL_TYPE_DENY
    | ARCHIVE_ENTRY_ACL_TYPE_AUDIT
    | ARCHIVE_ENTRY_ACL_TYPE_ALARM;

pub(crate) const ARCHIVE_ENTRY_ACL_USER: c_int = 10001;
pub(crate) const ARCHIVE_ENTRY_ACL_USER_OBJ: c_int = 10002;
pub(crate) const ARCHIVE_ENTRY_ACL_GROUP: c_int = 10003;
pub(crate) const ARCHIVE_ENTRY_ACL_GROUP_OBJ: c_int = 10004;
pub(crate) const ARCHIVE_ENTRY_ACL_MASK: c_int = 10005;
pub(crate) const ARCHIVE_ENTRY_ACL_OTHER: c_int = 10006;
pub(crate) const ARCHIVE_ENTRY_ACL_EVERYONE: c_int = 10107;

pub(crate) const ARCHIVE_ENTRY_ACL_STYLE_EXTRA_ID: c_int = 0x0000_0001;
pub(crate) const ARCHIVE_ENTRY_ACL_STYLE_MARK_DEFAULT: c_int = 0x0000_0002;
pub(crate) const ARCHIVE_ENTRY_ACL_STYLE_SEPARATOR_COMMA: c_int = 0x0000_0008;
pub(crate) const ARCHIVE_ENTRY_ACL_STYLE_COMPACT: c_int = 0x0000_0010;

#[derive(Default)]
pub(crate) struct CachedText {
    value: Option<String>,
    bytes: Option<Vec<u8>>,
    c_value: Option<CString>,
    utf8_c_value: Option<CString>,
    w_value: Option<Vec<wchar_t>>,
}

impl Clone for CachedText {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
            bytes: self.bytes.clone(),
            c_value: None,
            utf8_c_value: None,
            w_value: None,
        }
    }
}

impl CachedText {
    fn clear_cache(&mut self) {
        self.c_value = None;
        self.utf8_c_value = None;
        self.w_value = None;
    }

    pub(crate) fn set(&mut self, value: Option<String>) {
        self.bytes = value.as_ref().map(|value| value.as_bytes().to_vec());
        self.value = value;
        self.clear_cache();
    }

    pub(crate) fn set_bytes(&mut self, value: Option<Vec<u8>>) {
        self.value = value
            .as_ref()
            .map(|value| String::from_utf8_lossy(value).into_owned());
        self.bytes = value;
        self.clear_cache();
    }

    pub(crate) fn get_str(&self) -> Option<&str> {
        self.value.as_deref()
    }

    pub(crate) fn get_bytes(&self) -> Option<&[u8]> {
        self.bytes.as_deref()
    }

    pub(crate) fn to_cstring(&self) -> Option<CString> {
        self.bytes
            .as_ref()
            .map(|value| CString::new(value.as_slice()).expect("text bytes must not contain NUL"))
    }

    pub(crate) fn as_c_ptr(&mut self) -> *const c_char {
        if self.bytes.is_none() {
            return ptr::null();
        }
        if self.c_value.is_none() {
            self.c_value = self.to_cstring();
        }
        empty_if_none(self.c_value.as_ref())
    }

    pub(crate) fn as_utf8_c_ptr(&mut self) -> *const c_char {
        if self.value.is_none() {
            return ptr::null();
        }
        if self.utf8_c_value.is_none() {
            self.utf8_c_value = clone_c_string(self.value.as_deref());
        }
        empty_if_none(self.utf8_c_value.as_ref())
    }

    pub(crate) fn as_wide_ptr(&mut self) -> *const wchar_t {
        if self.value.is_none() {
            return ptr::null();
        }
        if self.w_value.is_none() {
            self.w_value = self.value.as_deref().map(to_wide_null);
        }
        empty_if_none_wide(self.w_value.as_ref())
    }
}

#[derive(Clone, Copy, Default)]
pub(crate) struct EntryTime {
    pub(crate) sec: i64,
    pub(crate) nsec: c_long,
    pub(crate) set: bool,
}

impl EntryTime {
    pub(crate) fn set(&mut self, sec: i64, nsec: i64) {
        let (sec, nsec) = normalize_nanos(sec, nsec);
        self.sec = sec;
        self.nsec = nsec as c_long;
        self.set = true;
    }

    pub(crate) fn unset(&mut self) {
        *self = Self::default();
    }
}

#[derive(Clone)]
pub(crate) struct AclEntry {
    pub(crate) entry_type: c_int,
    pub(crate) permset: c_int,
    pub(crate) tag: c_int,
    pub(crate) qual: c_int,
    pub(crate) name: Option<String>,
}

#[derive(Default, Clone)]
pub(crate) struct AclState {
    pub(crate) entries: Vec<AclEntry>,
    pub(crate) acl_types: c_int,
    iter_entries: Vec<AclEntry>,
    iter_index: usize,
    iter_name_cache: Option<CString>,
    last_text_flags: Option<c_int>,
    text_cache: Option<CString>,
    text_w_cache: Option<Vec<wchar_t>>,
}

#[derive(Clone)]
pub(crate) struct XattrEntry {
    pub(crate) name: CString,
    pub(crate) value: Vec<u8>,
}

#[derive(Clone, Copy)]
pub(crate) struct SparseEntry {
    pub(crate) offset: i64,
    pub(crate) length: i64,
}

#[repr(C)]
pub(crate) struct ArchiveEntryData {
    pub(crate) source_archive: *mut archive,
    pub(crate) mode: mode_t,
    pub(crate) uid: i64,
    pub(crate) gid: i64,
    pub(crate) ino: i64,
    pub(crate) ino_set: bool,
    pub(crate) dev: dev_t,
    pub(crate) dev_set: bool,
    pub(crate) rdev: dev_t,
    pub(crate) nlink: u32,
    pub(crate) size: i64,
    pub(crate) size_set: bool,
    pub(crate) atime: EntryTime,
    pub(crate) birthtime: EntryTime,
    pub(crate) ctime: EntryTime,
    pub(crate) mtime: EntryTime,
    pub(crate) pathname: CachedText,
    pub(crate) hardlink: CachedText,
    pub(crate) symlink: CachedText,
    pub(crate) uname: CachedText,
    pub(crate) gname: CachedText,
    pub(crate) sourcepath: CachedText,
    pub(crate) symlink_type: c_int,
    pub(crate) data_encrypted: bool,
    pub(crate) metadata_encrypted: bool,
    pub(crate) mac_metadata: Vec<u8>,
    pub(crate) fflags_set: c_ulong,
    pub(crate) fflags_clear: c_ulong,
    pub(crate) acl: AclState,
    pub(crate) xattrs: Vec<XattrEntry>,
    pub(crate) xattr_iter: usize,
    pub(crate) sparse: Vec<SparseEntry>,
    pub(crate) sparse_iter: usize,
    pub(crate) strmode_cache: Option<CString>,
    pub(crate) stat_cache: stat,
    pub(crate) stat_dirty: bool,
}

impl Clone for ArchiveEntryData {
    fn clone(&self) -> Self {
        Self {
            source_archive: self.source_archive,
            mode: self.mode,
            uid: self.uid,
            gid: self.gid,
            ino: self.ino,
            ino_set: self.ino_set,
            dev: self.dev,
            dev_set: self.dev_set,
            rdev: self.rdev,
            nlink: self.nlink,
            size: self.size,
            size_set: self.size_set,
            atime: self.atime,
            birthtime: self.birthtime,
            ctime: self.ctime,
            mtime: self.mtime,
            pathname: self.pathname.clone(),
            hardlink: self.hardlink.clone(),
            symlink: self.symlink.clone(),
            uname: self.uname.clone(),
            gname: self.gname.clone(),
            sourcepath: self.sourcepath.clone(),
            symlink_type: self.symlink_type,
            data_encrypted: self.data_encrypted,
            metadata_encrypted: self.metadata_encrypted,
            mac_metadata: self.mac_metadata.clone(),
            fflags_set: self.fflags_set,
            fflags_clear: self.fflags_clear,
            acl: self.acl.clone(),
            xattrs: self.xattrs.clone(),
            xattr_iter: 0,
            sparse: self.sparse.clone(),
            sparse_iter: 0,
            strmode_cache: None,
            stat_cache: unsafe { mem::zeroed() },
            stat_dirty: true,
        }
    }
}

impl Default for ArchiveEntryData {
    fn default() -> Self {
        Self {
            source_archive: ptr::null_mut(),
            mode: 0,
            uid: 0,
            gid: 0,
            ino: 0,
            ino_set: false,
            dev: 0,
            dev_set: false,
            rdev: 0,
            nlink: 0,
            size: 0,
            size_set: false,
            atime: EntryTime::default(),
            birthtime: EntryTime::default(),
            ctime: EntryTime::default(),
            mtime: EntryTime::default(),
            pathname: CachedText::default(),
            hardlink: CachedText::default(),
            symlink: CachedText::default(),
            uname: CachedText::default(),
            gname: CachedText::default(),
            sourcepath: CachedText::default(),
            symlink_type: 0,
            data_encrypted: false,
            metadata_encrypted: false,
            mac_metadata: Vec::new(),
            fflags_set: 0,
            fflags_clear: 0,
            acl: AclState::default(),
            xattrs: Vec::new(),
            xattr_iter: 0,
            sparse: Vec::new(),
            sparse_iter: 0,
            strmode_cache: None,
            stat_cache: unsafe { mem::zeroed() },
            stat_dirty: true,
        }
    }
}

pub(crate) unsafe fn from_raw<'a>(entry: *mut archive_entry) -> Option<&'a mut ArchiveEntryData> {
    entry.cast::<ArchiveEntryData>().as_mut()
}

pub(crate) unsafe fn new_raw_entry(source_archive: *mut archive) -> *mut archive_entry {
    Box::into_raw(Box::new(ArchiveEntryData {
        source_archive,
        ..ArchiveEntryData::default()
    })) as *mut archive_entry
}

pub(crate) unsafe fn free_raw_entry(entry: *mut archive_entry) {
    if !entry.is_null() {
        drop(Box::from_raw(entry.cast::<ArchiveEntryData>()));
    }
}

pub(crate) fn clone_entry(entry: &ArchiveEntryData) -> ArchiveEntryData {
    entry.clone()
}

pub(crate) fn clear_entry(entry: &mut ArchiveEntryData) {
    let source_archive = entry.source_archive;
    *entry = ArchiveEntryData {
        source_archive,
        ..ArchiveEntryData::default()
    };
}

pub(crate) fn update_text(target: &mut CachedText, value: Option<String>) {
    target.set(value);
}

pub(crate) fn update_c_text(target: &mut CachedText, value: *const c_char) {
    target.set_bytes((!value.is_null()).then(|| unsafe { CStr::from_ptr(value).to_bytes().to_vec() }));
}

pub(crate) fn update_wide_text(target: &mut CachedText, value: *const wchar_t) {
    target.set(from_optional_wide(value));
}

pub(crate) fn set_mode(entry: &mut ArchiveEntryData, mode: mode_t) {
    entry.mode = mode;
    entry.stat_dirty = true;
    entry.strmode_cache = None;
}

pub(crate) fn set_perm(entry: &mut ArchiveEntryData, perm: mode_t) {
    entry.mode = (entry.mode & AE_IFMT) | (perm & 0o7777);
    entry.stat_dirty = true;
    entry.strmode_cache = None;
}

pub(crate) fn set_filetype(entry: &mut ArchiveEntryData, filetype: mode_t) {
    entry.mode = (entry.mode & 0o7777) | (filetype & AE_IFMT);
    entry.stat_dirty = true;
    entry.strmode_cache = None;
}

pub(crate) fn set_link_target(entry: &mut ArchiveEntryData, value: Option<String>) {
    if entry.symlink.get_str().is_some() {
        entry.symlink.set(value);
    } else {
        entry.hardlink.set(value);
    }
    entry.strmode_cache = None;
}

pub(crate) fn set_link_target_bytes(entry: &mut ArchiveEntryData, value: Option<Vec<u8>>) {
    if entry.symlink.get_bytes().is_some() {
        entry.symlink.set_bytes(value);
    } else {
        entry.hardlink.set_bytes(value);
    }
    entry.strmode_cache = None;
}

pub(crate) fn add_xattr(
    entry: &mut ArchiveEntryData,
    name: *const c_char,
    value: *const c_void,
    size: size_t,
) {
    let Some(name) = from_optional_c_str(name) else {
        return;
    };
    let bytes = if value.is_null() || size == 0 {
        Vec::new()
    } else {
        unsafe { std::slice::from_raw_parts(value.cast::<u8>(), size).to_vec() }
    };
    entry.xattrs.push(XattrEntry {
        name: CString::new(name).expect("xattr name"),
        value: bytes,
    });
}

pub(crate) fn reset_xattrs(entry: &mut ArchiveEntryData) -> c_int {
    entry.xattr_iter = 0;
    entry.xattrs.len() as c_int
}

pub(crate) unsafe fn next_xattr(
    entry: &mut ArchiveEntryData,
    name: *mut *const c_char,
    value: *mut *const c_void,
    size: *mut size_t,
) -> c_int {
    if entry.xattr_iter >= entry.xattrs.len() {
        if !name.is_null() {
            *name = ptr::null();
        }
        if !value.is_null() {
            *value = ptr::null();
        }
        if !size.is_null() {
            *size = 0;
        }
        return ARCHIVE_WARN;
    }

    let index = entry.xattr_iter;
    entry.xattr_iter += 1;
    let xattr = &entry.xattrs[index];
    if !name.is_null() {
        *name = xattr.name.as_ptr();
    }
    if !value.is_null() {
        *value = xattr.value.as_ptr().cast();
    }
    if !size.is_null() {
        *size = xattr.value.len();
    }
    ARCHIVE_OK
}

pub(crate) fn add_sparse(entry: &mut ArchiveEntryData, offset: i64, length: i64) {
    if offset < 0 || length < 0 {
        return;
    }
    if offset > i64::MAX - length {
        return;
    }
    if entry.size_set && offset + length > entry.size {
        return;
    }
    if let Some(last) = entry.sparse.last_mut() {
        if last.offset + last.length > offset {
            return;
        }
        if last.offset + last.length == offset {
            if last.length > i64::MAX - length {
                return;
            }
            last.length += length;
            return;
        }
    }
    entry.sparse.push(SparseEntry { offset, length });
}

fn normalize_sparse(entry: &mut ArchiveEntryData) {
    if entry.sparse.len() != 1 {
        return;
    }
    let sparse = entry.sparse[0];
    let size = if entry.size_set { entry.size } else { 0 };
    if sparse.offset == 0 && sparse.length >= size {
        entry.sparse.clear();
        entry.sparse_iter = 0;
    }
}

pub(crate) fn sparse_count(entry: &mut ArchiveEntryData) -> c_int {
    normalize_sparse(entry);
    entry.sparse.len() as c_int
}

pub(crate) fn reset_sparse(entry: &mut ArchiveEntryData) -> c_int {
    entry.sparse_iter = 0;
    sparse_count(entry)
}

pub(crate) unsafe fn next_sparse(
    entry: &mut ArchiveEntryData,
    offset: *mut i64,
    length: *mut i64,
) -> c_int {
    if entry.sparse_iter >= entry.sparse.len() {
        if !offset.is_null() {
            *offset = 0;
        }
        if !length.is_null() {
            *length = 0;
        }
        return ARCHIVE_WARN;
    }

    let sparse = entry.sparse[entry.sparse_iter];
    entry.sparse_iter += 1;
    if !offset.is_null() {
        *offset = sparse.offset;
    }
    if !length.is_null() {
        *length = sparse.length;
    }
    ARCHIVE_OK
}

fn acl_perm_to_mode(permset: c_int) -> mode_t {
    let mut mode = 0;
    if (permset & ARCHIVE_ENTRY_ACL_READ) != 0 {
        mode |= 0o4;
    }
    if (permset & ARCHIVE_ENTRY_ACL_WRITE) != 0 {
        mode |= 0o2;
    }
    if (permset & ARCHIVE_ENTRY_ACL_EXECUTE) != 0 {
        mode |= 0o1;
    }
    mode
}

fn mode_to_acl_perm(mode: mode_t) -> c_int {
    let mut perm = 0;
    if (mode & 0o4) != 0 {
        perm |= ARCHIVE_ENTRY_ACL_READ;
    }
    if (mode & 0o2) != 0 {
        perm |= ARCHIVE_ENTRY_ACL_WRITE;
    }
    if (mode & 0o1) != 0 {
        perm |= ARCHIVE_ENTRY_ACL_EXECUTE;
    }
    perm
}

fn is_posix_type(entry_type: c_int) -> bool {
    (entry_type & ARCHIVE_ENTRY_ACL_TYPE_POSIX1E) != 0
}

fn is_nfs4_type(entry_type: c_int) -> bool {
    (entry_type & ARCHIVE_ENTRY_ACL_TYPE_NFS4) != 0
}

impl AclState {
    fn invalidate_cache(&mut self) {
        self.last_text_flags = None;
        self.text_cache = None;
        self.text_w_cache = None;
    }

    pub(crate) fn clear(&mut self) {
        self.entries.clear();
        self.acl_types = 0;
        self.iter_entries.clear();
        self.iter_index = 0;
        self.iter_name_cache = None;
        self.invalidate_cache();
    }

    pub(crate) fn add_entry(
        &mut self,
        mode: &mut mode_t,
        entry_type: c_int,
        permset: c_int,
        tag: c_int,
        qual: c_int,
        name: Option<String>,
    ) -> c_int {
        if is_nfs4_type(entry_type) {
            if (self.acl_types & !ARCHIVE_ENTRY_ACL_TYPE_NFS4) != 0 {
                return ARCHIVE_FAILED;
            }
            match tag {
                ARCHIVE_ENTRY_ACL_USER
                | ARCHIVE_ENTRY_ACL_USER_OBJ
                | ARCHIVE_ENTRY_ACL_GROUP
                | ARCHIVE_ENTRY_ACL_GROUP_OBJ
                | ARCHIVE_ENTRY_ACL_EVERYONE => {}
                _ => return ARCHIVE_FAILED,
            }
        } else if is_posix_type(entry_type) {
            if (self.acl_types & !ARCHIVE_ENTRY_ACL_TYPE_POSIX1E) != 0 {
                return ARCHIVE_FAILED;
            }
            match tag {
                ARCHIVE_ENTRY_ACL_USER
                | ARCHIVE_ENTRY_ACL_USER_OBJ
                | ARCHIVE_ENTRY_ACL_GROUP
                | ARCHIVE_ENTRY_ACL_GROUP_OBJ
                | ARCHIVE_ENTRY_ACL_MASK
                | ARCHIVE_ENTRY_ACL_OTHER => {}
                _ => return ARCHIVE_FAILED,
            }
            if entry_type == ARCHIVE_ENTRY_ACL_TYPE_ACCESS
                && (permset & !0o7) == 0
                && matches!(
                    tag,
                    ARCHIVE_ENTRY_ACL_USER_OBJ
                        | ARCHIVE_ENTRY_ACL_GROUP_OBJ
                        | ARCHIVE_ENTRY_ACL_OTHER
                )
            {
                match tag {
                    ARCHIVE_ENTRY_ACL_USER_OBJ => {
                        *mode &= !0o700;
                        *mode |= (acl_perm_to_mode(permset) & 0o7) << 6;
                    }
                    ARCHIVE_ENTRY_ACL_GROUP_OBJ => {
                        *mode &= !0o070;
                        *mode |= (acl_perm_to_mode(permset) & 0o7) << 3;
                    }
                    ARCHIVE_ENTRY_ACL_OTHER => {
                        *mode &= !0o007;
                        *mode |= acl_perm_to_mode(permset) & 0o7;
                    }
                    _ => {}
                }
                self.invalidate_cache();
                return ARCHIVE_OK;
            }
        } else {
            return ARCHIVE_FAILED;
        }

        if !is_nfs4_type(entry_type) {
            if let Some(existing) = self.entries.iter_mut().find(|existing| {
                existing.entry_type == entry_type
                    && existing.tag == tag
                    && existing.qual == qual
                    && (qual != -1
                        || !matches!(tag, ARCHIVE_ENTRY_ACL_USER | ARCHIVE_ENTRY_ACL_GROUP))
            }) {
                existing.permset = permset;
                existing.name = name;
                self.acl_types |= entry_type;
                self.invalidate_cache();
                return ARCHIVE_OK;
            }
        }

        self.entries.push(AclEntry {
            entry_type,
            permset,
            tag,
            qual,
            name,
        });
        self.acl_types |= entry_type;
        self.invalidate_cache();
        ARCHIVE_OK
    }

    pub(crate) fn count(&self, want_type: c_int) -> c_int {
        let explicit = self
            .entries
            .iter()
            .filter(|entry| (entry.entry_type & want_type) != 0)
            .count() as c_int;
        if explicit > 0 && (want_type & ARCHIVE_ENTRY_ACL_TYPE_ACCESS) != 0 {
            explicit + 3
        } else {
            explicit
        }
    }

    pub(crate) fn reset(&mut self, mode: mode_t, want_type: c_int) -> c_int {
        let count = self.count(want_type);
        let cutoff = if (want_type & ARCHIVE_ENTRY_ACL_TYPE_ACCESS) != 0 {
            3
        } else {
            0
        };
        self.iter_entries.clear();
        self.iter_index = 0;
        self.iter_name_cache = None;

        if count <= cutoff {
            return count;
        }

        if (want_type & ARCHIVE_ENTRY_ACL_TYPE_ACCESS) != 0 {
            self.iter_entries.push(AclEntry {
                entry_type: ARCHIVE_ENTRY_ACL_TYPE_ACCESS,
                permset: mode_to_acl_perm((mode >> 6) & 0o7),
                tag: ARCHIVE_ENTRY_ACL_USER_OBJ,
                qual: -1,
                name: None,
            });
            self.iter_entries.push(AclEntry {
                entry_type: ARCHIVE_ENTRY_ACL_TYPE_ACCESS,
                permset: mode_to_acl_perm((mode >> 3) & 0o7),
                tag: ARCHIVE_ENTRY_ACL_GROUP_OBJ,
                qual: -1,
                name: None,
            });
            self.iter_entries.push(AclEntry {
                entry_type: ARCHIVE_ENTRY_ACL_TYPE_ACCESS,
                permset: mode_to_acl_perm(mode & 0o7),
                tag: ARCHIVE_ENTRY_ACL_OTHER,
                qual: -1,
                name: None,
            });
        }

        self.iter_entries.extend(
            self.entries
                .iter()
                .filter(|entry| (entry.entry_type & want_type) != 0)
                .cloned(),
        );
        count
    }

    pub(crate) unsafe fn next(
        &mut self,
        entry_type: *mut c_int,
        permset: *mut c_int,
        tag: *mut c_int,
        qual: *mut c_int,
        name: *mut *const c_char,
    ) -> c_int {
        if self.iter_index >= self.iter_entries.len() {
            if !entry_type.is_null() {
                *entry_type = 0;
            }
            if !permset.is_null() {
                *permset = 0;
            }
            if !tag.is_null() {
                *tag = 0;
            }
            if !qual.is_null() {
                *qual = -1;
            }
            if !name.is_null() {
                *name = ptr::null();
            }
            return ARCHIVE_EOF;
        }

        let current = &self.iter_entries[self.iter_index];
        self.iter_index += 1;

        if !entry_type.is_null() {
            *entry_type = current.entry_type;
        }
        if !permset.is_null() {
            *permset = current.permset;
        }
        if !tag.is_null() {
            *tag = current.tag;
        }
        if !qual.is_null() {
            *qual = current.qual;
        }
        if !name.is_null() {
            self.iter_name_cache = current
                .name
                .as_ref()
                .map(|name| CString::new(name.as_str()).expect("ACL name"));
            *name = self
                .iter_name_cache
                .as_ref()
                .map_or(ptr::null(), |value| value.as_ptr());
        }
        ARCHIVE_OK
    }

    pub(crate) fn types(&self) -> c_int {
        self.acl_types
    }

    fn text_want_type(&self, flags: c_int) -> c_int {
        if (self.acl_types & ARCHIVE_ENTRY_ACL_TYPE_NFS4) != 0 {
            if (self.acl_types & ARCHIVE_ENTRY_ACL_TYPE_POSIX1E) != 0 {
                return 0;
            }
            return ARCHIVE_ENTRY_ACL_TYPE_NFS4;
        }

        let mut want_type = 0;
        if (flags & ARCHIVE_ENTRY_ACL_TYPE_ACCESS) != 0 {
            want_type |= ARCHIVE_ENTRY_ACL_TYPE_ACCESS;
        }
        if (flags & ARCHIVE_ENTRY_ACL_TYPE_DEFAULT) != 0 {
            want_type |= ARCHIVE_ENTRY_ACL_TYPE_DEFAULT;
        }
        if want_type == 0 {
            ARCHIVE_ENTRY_ACL_TYPE_POSIX1E
        } else {
            want_type
        }
    }

    fn posix_entries_for_text(&self, mode: mode_t, want_type: c_int) -> Vec<AclEntry> {
        let mut entries = Vec::new();
        if (want_type & ARCHIVE_ENTRY_ACL_TYPE_ACCESS) != 0 {
            entries.push(AclEntry {
                entry_type: ARCHIVE_ENTRY_ACL_TYPE_ACCESS,
                permset: mode_to_acl_perm((mode >> 6) & 0o7),
                tag: ARCHIVE_ENTRY_ACL_USER_OBJ,
                qual: -1,
                name: None,
            });
            entries.push(AclEntry {
                entry_type: ARCHIVE_ENTRY_ACL_TYPE_ACCESS,
                permset: mode_to_acl_perm((mode >> 3) & 0o7),
                tag: ARCHIVE_ENTRY_ACL_GROUP_OBJ,
                qual: -1,
                name: None,
            });
            entries.push(AclEntry {
                entry_type: ARCHIVE_ENTRY_ACL_TYPE_ACCESS,
                permset: mode_to_acl_perm(mode & 0o7),
                tag: ARCHIVE_ENTRY_ACL_OTHER,
                qual: -1,
                name: None,
            });
        }
        entries.extend(
            self.entries
                .iter()
                .filter(|entry| (entry.entry_type & want_type) != 0)
                .cloned(),
        );
        entries
    }

    fn format_mode_bits(permset: c_int) -> String {
        let mut text = String::with_capacity(3);
        text.push(if (permset & ARCHIVE_ENTRY_ACL_READ) != 0 {
            'r'
        } else {
            '-'
        });
        text.push(if (permset & ARCHIVE_ENTRY_ACL_WRITE) != 0 {
            'w'
        } else {
            '-'
        });
        text.push(if (permset & ARCHIVE_ENTRY_ACL_EXECUTE) != 0 {
            'x'
        } else {
            '-'
        });
        text
    }

    fn format_nfs4_bits(permset: c_int, compact: bool) -> (String, String) {
        let perm_map = [
            (ARCHIVE_ENTRY_ACL_READ_DATA, 'r'),
            (ARCHIVE_ENTRY_ACL_WRITE_DATA, 'w'),
            (ARCHIVE_ENTRY_ACL_EXECUTE, 'x'),
            (ARCHIVE_ENTRY_ACL_APPEND_DATA, 'p'),
            (ARCHIVE_ENTRY_ACL_DELETE, 'd'),
            (ARCHIVE_ENTRY_ACL_DELETE_CHILD, 'D'),
            (ARCHIVE_ENTRY_ACL_READ_ATTRIBUTES, 'a'),
            (ARCHIVE_ENTRY_ACL_WRITE_ATTRIBUTES, 'A'),
            (ARCHIVE_ENTRY_ACL_READ_NAMED_ATTRS, 'R'),
            (ARCHIVE_ENTRY_ACL_WRITE_NAMED_ATTRS, 'W'),
            (ARCHIVE_ENTRY_ACL_READ_ACL, 'c'),
            (ARCHIVE_ENTRY_ACL_WRITE_ACL, 'C'),
            (ARCHIVE_ENTRY_ACL_WRITE_OWNER, 'o'),
            (ARCHIVE_ENTRY_ACL_SYNCHRONIZE, 's'),
        ];
        let flag_map = [
            (ARCHIVE_ENTRY_ACL_ENTRY_FILE_INHERIT, 'f'),
            (ARCHIVE_ENTRY_ACL_ENTRY_DIRECTORY_INHERIT, 'd'),
            (ARCHIVE_ENTRY_ACL_ENTRY_INHERIT_ONLY, 'i'),
            (ARCHIVE_ENTRY_ACL_ENTRY_NO_PROPAGATE_INHERIT, 'n'),
            (ARCHIVE_ENTRY_ACL_ENTRY_SUCCESSFUL_ACCESS, 'S'),
            (ARCHIVE_ENTRY_ACL_ENTRY_FAILED_ACCESS, 'F'),
            (ARCHIVE_ENTRY_ACL_ENTRY_INHERITED, 'I'),
        ];

        let mut perms = String::new();
        for (bit, ch) in perm_map {
            if (permset & bit) != 0 {
                perms.push(ch);
            } else if !compact {
                perms.push('-');
            }
        }
        let mut flags = String::new();
        for (bit, ch) in flag_map {
            if (permset & bit) != 0 {
                flags.push(ch);
            } else if !compact {
                flags.push('-');
            }
        }
        (perms, flags)
    }

    pub(crate) fn to_text(&mut self, mode: mode_t, flags: c_int) -> Option<String> {
        let want_type = self.text_want_type(flags);
        if want_type == 0 {
            return None;
        }
        let separator = if (flags & ARCHIVE_ENTRY_ACL_STYLE_SEPARATOR_COMMA) != 0 {
            ","
        } else {
            "\n"
        };

        let text = if want_type == ARCHIVE_ENTRY_ACL_TYPE_NFS4 {
            let compact = (flags & ARCHIVE_ENTRY_ACL_STYLE_COMPACT) != 0;
            let mut lines = Vec::new();
            for entry in self
                .entries
                .iter()
                .filter(|entry| is_nfs4_type(entry.entry_type))
            {
                let tag = match entry.tag {
                    ARCHIVE_ENTRY_ACL_USER => {
                        format!("user:{}", entry.name.clone().unwrap_or_default())
                    }
                    ARCHIVE_ENTRY_ACL_GROUP => {
                        format!("group:{}", entry.name.clone().unwrap_or_default())
                    }
                    ARCHIVE_ENTRY_ACL_USER_OBJ => "owner@".to_string(),
                    ARCHIVE_ENTRY_ACL_GROUP_OBJ => "group@".to_string(),
                    ARCHIVE_ENTRY_ACL_EVERYONE => "everyone@".to_string(),
                    _ => continue,
                };
                let (perm_text, flag_text) = Self::format_nfs4_bits(entry.permset, compact);
                let entry_type = match entry.entry_type {
                    ARCHIVE_ENTRY_ACL_TYPE_ALLOW => "allow",
                    ARCHIVE_ENTRY_ACL_TYPE_DENY => "deny",
                    ARCHIVE_ENTRY_ACL_TYPE_AUDIT => "audit",
                    ARCHIVE_ENTRY_ACL_TYPE_ALARM => "alarm",
                    _ => continue,
                };
                let mut line = format!("{tag}:{perm_text}:{flag_text}:{entry_type}");
                if (flags & ARCHIVE_ENTRY_ACL_STYLE_EXTRA_ID) != 0
                    && matches!(entry.tag, ARCHIVE_ENTRY_ACL_USER | ARCHIVE_ENTRY_ACL_GROUP)
                {
                    line.push(':');
                    line.push_str(&entry.qual.to_string());
                }
                lines.push(line);
            }
            if lines.is_empty() {
                return None;
            }
            lines.join(separator)
        } else {
            let entries = self.posix_entries_for_text(mode, want_type);
            let include_default_prefix = (want_type & ARCHIVE_ENTRY_ACL_TYPE_ACCESS) != 0
                && (want_type & ARCHIVE_ENTRY_ACL_TYPE_DEFAULT) != 0
                || (flags & ARCHIVE_ENTRY_ACL_STYLE_MARK_DEFAULT) != 0;
            let mut lines = Vec::new();
            for entry in entries {
                let mut prefix = String::new();
                if entry.entry_type == ARCHIVE_ENTRY_ACL_TYPE_DEFAULT && include_default_prefix {
                    prefix.push_str("default:");
                }
                let tag = match entry.tag {
                    ARCHIVE_ENTRY_ACL_USER_OBJ => "user",
                    ARCHIVE_ENTRY_ACL_GROUP_OBJ => "group",
                    ARCHIVE_ENTRY_ACL_OTHER => "other",
                    ARCHIVE_ENTRY_ACL_MASK => "mask",
                    ARCHIVE_ENTRY_ACL_USER => "user",
                    ARCHIVE_ENTRY_ACL_GROUP => "group",
                    _ => continue,
                };
                let name = entry.name.unwrap_or_else(|| {
                    if matches!(entry.tag, ARCHIVE_ENTRY_ACL_USER | ARCHIVE_ENTRY_ACL_GROUP) {
                        entry.qual.to_string()
                    } else {
                        String::new()
                    }
                });
                let perm_text = Self::format_mode_bits(entry.permset);
                let mut line =
                    if matches!(entry.tag, ARCHIVE_ENTRY_ACL_USER | ARCHIVE_ENTRY_ACL_GROUP) {
                        format!("{prefix}{tag}:{name}:{perm_text}")
                    } else {
                        format!("{prefix}{tag}::{perm_text}")
                    };
                if (flags & ARCHIVE_ENTRY_ACL_STYLE_EXTRA_ID) != 0
                    && matches!(entry.tag, ARCHIVE_ENTRY_ACL_USER | ARCHIVE_ENTRY_ACL_GROUP)
                {
                    line.push(':');
                    line.push_str(&entry.qual.to_string());
                }
                lines.push(line);
            }
            if lines.is_empty() {
                return None;
            }
            lines.join(separator)
        };

        self.last_text_flags = Some(flags);
        self.text_cache = Some(CString::new(text.clone()).expect("ACL text cache"));
        self.text_w_cache = Some(to_wide_null(&text));
        Some(text)
    }

    pub(crate) fn text_ptr(&mut self, mode: mode_t, flags: c_int) -> *const c_char {
        if self.last_text_flags != Some(flags) {
            let _ = self.to_text(mode, flags);
        }
        empty_if_none(self.text_cache.as_ref())
    }

    pub(crate) fn text_w_ptr(&mut self, mode: mode_t, flags: c_int) -> *const wchar_t {
        if self.last_text_flags != Some(flags) {
            let _ = self.to_text(mode, flags);
        }
        empty_if_none_wide(self.text_w_cache.as_ref())
    }

    pub(crate) unsafe fn to_text_malloc(
        &mut self,
        mode: mode_t,
        flags: c_int,
        text_len: *mut isize,
    ) -> *mut c_char {
        let Some(text) = self.to_text(mode, flags) else {
            if !text_len.is_null() {
                *text_len = 0;
            }
            return ptr::null_mut();
        };
        if !text_len.is_null() {
            *text_len = text.len() as isize;
        }
        let mut bytes = text.into_bytes();
        bytes.push(0);
        malloc_bytes(&bytes)
    }

    pub(crate) unsafe fn to_text_w_malloc(
        &mut self,
        mode: mode_t,
        flags: c_int,
        text_len: *mut isize,
    ) -> *mut wchar_t {
        let Some(text) = self.to_text(mode, flags) else {
            if !text_len.is_null() {
                *text_len = 0;
            }
            return ptr::null_mut();
        };
        let wide = to_wide_null(&text);
        if !text_len.is_null() {
            *text_len = (wide.len() - 1) as isize;
        }
        malloc_wide(&wide)
    }

    pub(crate) fn from_text(&mut self, mode: &mut mode_t, text: &str, want_type: c_int) -> c_int {
        let mut status = ARCHIVE_OK;
        let separator = if text.contains('\n') { '\n' } else { ',' };
        for raw_line in text.split(separator) {
            let line = raw_line.trim();
            if line.is_empty() {
                continue;
            }

            if want_type == ARCHIVE_ENTRY_ACL_TYPE_NFS4 {
                let parts: Vec<_> = line.split(':').collect();
                if parts.len() < 4 {
                    status = ARCHIVE_WARN;
                    continue;
                }
                let (tag, name, offset) = match parts[0] {
                    "owner@" => (ARCHIVE_ENTRY_ACL_USER_OBJ, None, 0),
                    "group@" => (ARCHIVE_ENTRY_ACL_GROUP_OBJ, None, 0),
                    "everyone@" => (ARCHIVE_ENTRY_ACL_EVERYONE, None, 0),
                    "user" if parts.len() >= 5 => {
                        (ARCHIVE_ENTRY_ACL_USER, Some(parts[1].to_string()), 1)
                    }
                    "group" if parts.len() >= 5 => {
                        (ARCHIVE_ENTRY_ACL_GROUP, Some(parts[1].to_string()), 1)
                    }
                    _ => {
                        status = ARCHIVE_WARN;
                        continue;
                    }
                };
                let perm_index = 1 + offset;
                let flag_index = 2 + offset;
                let type_index = 3 + offset;
                let id_index = 4 + offset;
                let mut permset = 0;
                for ch in parts[perm_index].chars() {
                    permset |= match ch {
                        'r' => ARCHIVE_ENTRY_ACL_READ_DATA,
                        'w' => ARCHIVE_ENTRY_ACL_WRITE_DATA,
                        'x' => ARCHIVE_ENTRY_ACL_EXECUTE,
                        'p' => ARCHIVE_ENTRY_ACL_APPEND_DATA,
                        'd' => ARCHIVE_ENTRY_ACL_DELETE,
                        'D' => ARCHIVE_ENTRY_ACL_DELETE_CHILD,
                        'a' => ARCHIVE_ENTRY_ACL_READ_ATTRIBUTES,
                        'A' => ARCHIVE_ENTRY_ACL_WRITE_ATTRIBUTES,
                        'R' => ARCHIVE_ENTRY_ACL_READ_NAMED_ATTRS,
                        'W' => ARCHIVE_ENTRY_ACL_WRITE_NAMED_ATTRS,
                        'c' => ARCHIVE_ENTRY_ACL_READ_ACL,
                        'C' => ARCHIVE_ENTRY_ACL_WRITE_ACL,
                        'o' => ARCHIVE_ENTRY_ACL_WRITE_OWNER,
                        's' => ARCHIVE_ENTRY_ACL_SYNCHRONIZE,
                        '-' => 0,
                        _ => {
                            status = ARCHIVE_WARN;
                            0
                        }
                    };
                }
                for ch in parts[flag_index].chars() {
                    permset |= match ch {
                        'f' => ARCHIVE_ENTRY_ACL_ENTRY_FILE_INHERIT,
                        'd' => ARCHIVE_ENTRY_ACL_ENTRY_DIRECTORY_INHERIT,
                        'i' => ARCHIVE_ENTRY_ACL_ENTRY_INHERIT_ONLY,
                        'n' => ARCHIVE_ENTRY_ACL_ENTRY_NO_PROPAGATE_INHERIT,
                        'S' => ARCHIVE_ENTRY_ACL_ENTRY_SUCCESSFUL_ACCESS,
                        'F' => ARCHIVE_ENTRY_ACL_ENTRY_FAILED_ACCESS,
                        'I' => ARCHIVE_ENTRY_ACL_ENTRY_INHERITED,
                        '-' => 0,
                        _ => {
                            status = ARCHIVE_WARN;
                            0
                        }
                    };
                }
                let entry_type = match parts[type_index] {
                    "allow" => ARCHIVE_ENTRY_ACL_TYPE_ALLOW,
                    "deny" => ARCHIVE_ENTRY_ACL_TYPE_DENY,
                    "audit" => ARCHIVE_ENTRY_ACL_TYPE_AUDIT,
                    "alarm" => ARCHIVE_ENTRY_ACL_TYPE_ALARM,
                    _ => {
                        status = ARCHIVE_WARN;
                        continue;
                    }
                };
                let qual = parts
                    .get(id_index)
                    .and_then(|part| part.parse::<c_int>().ok())
                    .unwrap_or(-1);
                let result = self.add_entry(mode, entry_type, permset, tag, qual, name);
                if result != ARCHIVE_OK {
                    status = result;
                }
                continue;
            }

            let (default_prefix, remainder) = if let Some(rest) = line.strip_prefix("default:") {
                (true, rest)
            } else if let Some(rest) = line.strip_prefix("d:") {
                (true, rest)
            } else {
                (false, line)
            };
            let entry_type = if default_prefix {
                ARCHIVE_ENTRY_ACL_TYPE_DEFAULT
            } else if want_type == ARCHIVE_ENTRY_ACL_TYPE_DEFAULT {
                ARCHIVE_ENTRY_ACL_TYPE_DEFAULT
            } else {
                ARCHIVE_ENTRY_ACL_TYPE_ACCESS
            };
            let parts: Vec<_> = remainder.split(':').collect();
            if parts.len() < 3 {
                status = ARCHIVE_WARN;
                continue;
            }
            let tag = match parts[0] {
                "u" | "user" => {
                    if parts[1].is_empty() {
                        ARCHIVE_ENTRY_ACL_USER_OBJ
                    } else {
                        ARCHIVE_ENTRY_ACL_USER
                    }
                }
                "g" | "group" => {
                    if parts[1].is_empty() {
                        ARCHIVE_ENTRY_ACL_GROUP_OBJ
                    } else {
                        ARCHIVE_ENTRY_ACL_GROUP
                    }
                }
                "o" | "other" => ARCHIVE_ENTRY_ACL_OTHER,
                "m" | "mask" => ARCHIVE_ENTRY_ACL_MASK,
                _ => {
                    status = ARCHIVE_WARN;
                    continue;
                }
            };
            let name = if matches!(tag, ARCHIVE_ENTRY_ACL_USER | ARCHIVE_ENTRY_ACL_GROUP) {
                Some(parts[1].to_string())
            } else {
                None
            };
            let perm_part = parts.get(2).copied().unwrap_or("---");
            let mut permset = 0;
            for ch in perm_part.chars() {
                permset |= match ch {
                    'r' | 'R' => ARCHIVE_ENTRY_ACL_READ,
                    'w' | 'W' => ARCHIVE_ENTRY_ACL_WRITE,
                    'x' | 'X' => ARCHIVE_ENTRY_ACL_EXECUTE,
                    '-' => 0,
                    _ => {
                        status = ARCHIVE_WARN;
                        0
                    }
                };
            }
            let qual = parts
                .get(3)
                .and_then(|part| part.parse::<c_int>().ok())
                .unwrap_or(
                    if matches!(tag, ARCHIVE_ENTRY_ACL_USER | ARCHIVE_ENTRY_ACL_GROUP) {
                        0
                    } else {
                        -1
                    },
                );
            let result = self.add_entry(mode, entry_type, permset, tag, qual, name);
            if result != ARCHIVE_OK {
                status = result;
            }
        }
        status
    }
}

pub(crate) fn clear_acl(entry: &mut ArchiveEntryData) {
    entry.acl.clear();
    entry.strmode_cache = None;
}

pub(crate) fn entry_has_acl(entry: &ArchiveEntryData) -> bool {
    !entry.acl.entries.is_empty()
}

pub(crate) fn copy_stat(entry: &mut ArchiveEntryData, st: &stat) {
    entry.stat_cache = *st;
    entry.stat_dirty = false;
    entry.atime = EntryTime {
        sec: st.st_atime,
        nsec: 0,
        set: true,
    };
    entry.birthtime = EntryTime::default();
    entry.ctime = EntryTime {
        sec: st.st_ctime,
        nsec: 0,
        set: true,
    };
    entry.mtime = EntryTime {
        sec: st.st_mtime,
        nsec: 0,
        set: true,
    };
    entry.dev = st.st_dev;
    entry.dev_set = true;
    entry.gid = st.st_gid as i64;
    entry.ino = st.st_ino as i64;
    entry.ino_set = true;
    entry.mode = st.st_mode;
    entry.nlink = st.st_nlink as u32;
    entry.size = st.st_size;
    entry.size_set = true;
    entry.uid = st.st_uid as i64;
}

pub(crate) fn materialize_stat(entry: &mut ArchiveEntryData) -> &stat {
    if entry.stat_dirty {
        let mut st: stat = unsafe { mem::zeroed() };
        st.st_atime = entry.atime.sec;
        st.st_ctime = entry.ctime.sec;
        st.st_mtime = entry.mtime.sec;
        st.st_dev = entry.dev;
        st.st_gid = entry.gid as _;
        st.st_ino = entry.ino as _;
        st.st_mode = entry.mode;
        st.st_nlink = entry.nlink as _;
        st.st_rdev = entry.rdev;
        st.st_size = if entry.size_set { entry.size } else { 0 };
        st.st_uid = entry.uid as _;
        entry.stat_cache = st;
        entry.stat_dirty = false;
    }
    &entry.stat_cache
}

pub(crate) fn strmode(entry: &mut ArchiveEntryData) -> *const c_char {
    if entry.strmode_cache.is_none() {
        let mut buffer = ['-'; 11];
        buffer[0] = match entry.mode & AE_IFMT {
            AE_IFREG => '-',
            AE_IFDIR => 'd',
            AE_IFBLK => 'b',
            AE_IFCHR => 'c',
            AE_IFLNK => 'l',
            AE_IFSOCK => 's',
            AE_IFIFO => 'p',
            _ if entry.hardlink.get_str().is_some() => 'h',
            _ => '-',
        };

        let perm = entry.mode & 0o7777;
        buffer[1] = if (perm & 0o400) != 0 { 'r' } else { '-' };
        buffer[2] = if (perm & 0o200) != 0 { 'w' } else { '-' };
        buffer[3] = match ((perm & 0o100) != 0, (perm & 0o4000) != 0) {
            (true, true) => 's',
            (false, true) => 'S',
            (true, false) => 'x',
            (false, false) => '-',
        };
        buffer[4] = if (perm & 0o040) != 0 { 'r' } else { '-' };
        buffer[5] = if (perm & 0o020) != 0 { 'w' } else { '-' };
        buffer[6] = match ((perm & 0o010) != 0, (perm & 0o2000) != 0) {
            (true, true) => 's',
            (false, true) => 'S',
            (true, false) => 'x',
            (false, false) => '-',
        };
        buffer[7] = if (perm & 0o004) != 0 { 'r' } else { '-' };
        buffer[8] = if (perm & 0o002) != 0 { 'w' } else { '-' };
        buffer[9] = match ((perm & 0o001) != 0, (perm & 0o1000) != 0) {
            (true, true) => 't',
            (false, true) => 'T',
            (true, false) => 'x',
            (false, false) => '-',
        };
        buffer[10] = if entry_has_acl(entry) { '+' } else { ' ' };
        let text: String = buffer.iter().collect();
        entry.strmode_cache = Some(CString::new(text).expect("strmode"));
    }
    entry.strmode_cache.as_ref().unwrap().as_ptr()
}

#[derive(Default)]
pub(crate) struct LinkEntry {
    pub(crate) canonical: *mut archive_entry,
    pub(crate) entry: *mut archive_entry,
    pub(crate) links: u32,
}

#[repr(C)]
pub(crate) struct LinkResolverData {
    pub(crate) strategy: c_int,
    pub(crate) entries: std::collections::HashMap<(dev_t, i64), LinkEntry>,
}

pub(crate) unsafe fn free_linkresolver(resolver: *mut archive_entry_linkresolver) {
    if resolver.is_null() {
        return;
    }
    let mut resolver = Box::from_raw(resolver.cast::<LinkResolverData>());
    for (_, link) in resolver.entries.drain() {
        free_raw_entry(link.canonical);
        free_raw_entry(link.entry);
    }
}

pub(crate) unsafe fn partial_links(
    resolver: &mut LinkResolverData,
    links: *mut u32,
) -> *mut archive_entry {
    let key = resolver
        .entries
        .iter()
        .find(|(_, value)| value.entry.is_null() && !value.canonical.is_null())
        .map(|(key, _)| *key);
    let Some(key) = key else {
        if !links.is_null() {
            *links = 0;
        }
        return ptr::null_mut();
    };
    let mut entry = resolver.entries.remove(&key).unwrap();
    if !links.is_null() {
        *links = entry.links;
    }
    let result = entry.canonical;
    if !entry.entry.is_null() {
        free_raw_entry(entry.entry);
    }
    result
}

pub(crate) unsafe fn linkify(
    resolver: &mut LinkResolverData,
    entry: *mut *mut archive_entry,
    spare: *mut *mut archive_entry,
) {
    if !spare.is_null() {
        *spare = ptr::null_mut();
    }
    if entry.is_null() || (*entry).is_null() {
        return;
    }

    let current = &mut *from_raw(*entry).unwrap();
    if current.nlink <= 1 {
        return;
    }
    if matches!(current.mode & AE_IFMT, AE_IFDIR | AE_IFBLK | AE_IFCHR) {
        return;
    }

    let key = (current.dev, current.ino);
    match resolver.strategy {
        crate::ffi::archive_common::ARCHIVE_FORMAT_TAR_USTAR
        | crate::ffi::archive_common::ARCHIVE_FORMAT_TAR => {
            if let Some(link) = resolver.entries.get_mut(&key) {
                let canonical = from_raw(link.canonical).unwrap();
                current.size = 0;
                current.size_set = false;
                current
                    .hardlink
                    .set_bytes(canonical.pathname.get_bytes().map(|value| value.to_vec()));
            } else {
                resolver.entries.insert(
                    key,
                    LinkEntry {
                        canonical: archive_entry_clone_raw(*entry),
                        entry: ptr::null_mut(),
                        links: current.nlink.saturating_sub(1),
                    },
                );
            }
        }
        crate::ffi::archive_common::ARCHIVE_FORMAT_CPIO_SVR4_NOCRC => {
            if let Some(link) = resolver.entries.get_mut(&key) {
                let previous = link.entry;
                link.entry = *entry;
                *entry = previous;
                if let Some(previous) = from_raw(*entry) {
                    let canonical = from_raw(link.canonical).unwrap();
                    previous.size = 0;
                    previous.size_set = false;
                    previous
                        .hardlink
                        .set_bytes(canonical.pathname.get_bytes().map(|value| value.to_vec()));
                    if link.links > 0 {
                        link.links -= 1;
                    }
                    if link.links == 0 && !spare.is_null() {
                        *spare = link.entry;
                        link.entry = ptr::null_mut();
                    }
                }
            } else {
                resolver.entries.insert(
                    key,
                    LinkEntry {
                        canonical: archive_entry_clone_raw(*entry),
                        entry: *entry,
                        links: current.nlink.saturating_sub(1),
                    },
                );
                *entry = ptr::null_mut();
            }
        }
        _ => {}
    }
}

pub(crate) unsafe fn archive_entry_clone_raw(entry: *mut archive_entry) -> *mut archive_entry {
    let Some(entry_data) = from_raw(entry) else {
        return ptr::null_mut();
    };
    Box::into_raw(Box::new(clone_entry(entry_data))) as *mut archive_entry
}
