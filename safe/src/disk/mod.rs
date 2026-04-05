mod native;
pub(crate) use native::{
    read_disk_can_descend as native_read_disk_can_descend,
    read_disk_close as native_read_disk_close, read_disk_data as native_read_disk_data,
    read_disk_data_block as native_read_disk_data_block,
    read_disk_descend as native_read_disk_descend,
    read_disk_entry_from_file as native_read_disk_entry_from_file,
    read_disk_next_header as native_read_disk_next_header,
    read_disk_open_path as native_read_disk_open_path, write_disk_close as native_write_disk_close,
    write_disk_data as native_write_disk_data,
    write_disk_data_block as native_write_disk_data_block,
    write_disk_finish_entry as native_write_disk_finish_entry,
    write_disk_header as native_write_disk_header,
};

use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::ptr;
use std::slice;

use libc::{stat, wchar_t};

use crate::common::backend::{api as backend_api, BackendArchive, BackendEntry};
use crate::common::error::{ARCHIVE_EOF, ARCHIVE_FATAL, ARCHIVE_OK};
use crate::common::helpers::{from_optional_c_str, from_optional_wide};
use crate::common::panic_boundary::{ffi_const_ptr, ffi_int};
use crate::common::state::{
    archive_check_magic, clear_error, read_disk_from_archive, write_disk_from_archive,
    ReadDiskOpenPath, ReadDiskSymlinkMode,
};
use crate::entry::internal::{clear_entry, from_raw, CachedText, SparseEntry, XattrEntry};
use crate::ffi::{archive, archive_entry};

fn c_string_opt(value: Option<&str>) -> Option<CString> {
    value.map(|value| CString::new(value).expect("string must not contain NUL"))
}

fn cached_c_string_opt(value: &CachedText) -> Option<CString> {
    value.to_cstring()
}

pub(crate) unsafe fn custom_entry_to_backend(
    src: *mut archive_entry,
    dst: *mut BackendEntry,
) -> c_int {
    let Some(src_data) = from_raw(src) else {
        return ARCHIVE_FATAL;
    };
    let api = backend_api();
    (api.archive_entry_clear)(dst);

    if let Some(pathname) = cached_c_string_opt(&src_data.pathname) {
        (api.archive_entry_copy_pathname)(dst, pathname.as_ptr());
    }
    (api.archive_entry_set_mode)(dst, src_data.mode);
    if src_data.size_set {
        (api.archive_entry_set_size)(dst, src_data.size);
    } else {
        (api.archive_entry_unset_size)(dst);
    }
    (api.archive_entry_set_uid)(dst, src_data.uid);
    (api.archive_entry_set_gid)(dst, src_data.gid);
    if let Some(uname) = cached_c_string_opt(&src_data.uname) {
        (api.archive_entry_copy_uname)(dst, uname.as_ptr());
    }
    if let Some(gname) = cached_c_string_opt(&src_data.gname) {
        (api.archive_entry_copy_gname)(dst, gname.as_ptr());
    }
    if let Some(hardlink) = cached_c_string_opt(&src_data.hardlink) {
        (api.archive_entry_copy_hardlink)(dst, hardlink.as_ptr());
    }
    if let Some(symlink) = cached_c_string_opt(&src_data.symlink) {
        (api.archive_entry_copy_symlink)(dst, symlink.as_ptr());
    }
    (api.archive_entry_set_symlink_type)(dst, src_data.symlink_type);
    (api.archive_entry_set_nlink)(dst, src_data.nlink);
    if src_data.ino_set {
        (api.archive_entry_set_ino)(dst, src_data.ino);
    }
    if src_data.dev_set {
        (api.archive_entry_set_dev)(dst, src_data.dev);
    }
    (api.archive_entry_set_rdev)(dst, src_data.rdev);
    if src_data.atime.set {
        (api.archive_entry_set_atime)(dst, src_data.atime.sec, src_data.atime.nsec);
    } else {
        (api.archive_entry_unset_atime)(dst);
    }
    if src_data.birthtime.set {
        (api.archive_entry_set_birthtime)(dst, src_data.birthtime.sec, src_data.birthtime.nsec);
    } else {
        (api.archive_entry_unset_birthtime)(dst);
    }
    if src_data.ctime.set {
        (api.archive_entry_set_ctime)(dst, src_data.ctime.sec, src_data.ctime.nsec);
    } else {
        (api.archive_entry_unset_ctime)(dst);
    }
    if src_data.mtime.set {
        (api.archive_entry_set_mtime)(dst, src_data.mtime.sec, src_data.mtime.nsec);
    } else {
        (api.archive_entry_unset_mtime)(dst);
    }
    (api.archive_entry_set_fflags)(dst, src_data.fflags_set, src_data.fflags_clear);
    if !src_data.mac_metadata.is_empty() {
        (api.archive_entry_copy_mac_metadata)(
            dst,
            src_data.mac_metadata.as_ptr().cast(),
            src_data.mac_metadata.len(),
        );
    }
    (api.archive_entry_set_is_data_encrypted)(dst, i8::from(src_data.data_encrypted));
    (api.archive_entry_set_is_metadata_encrypted)(dst, i8::from(src_data.metadata_encrypted));
    (api.archive_entry_acl_clear)(dst);
    for acl in &src_data.acl.entries {
        let name = c_string_opt(acl.name.as_deref());
        (api.archive_entry_acl_add_entry)(
            dst,
            acl.entry_type,
            acl.permset,
            acl.tag,
            acl.qual,
            name.as_ref().map_or(ptr::null(), |value| value.as_ptr()),
        );
    }
    for xattr in &src_data.xattrs {
        (api.archive_entry_xattr_add_entry)(
            dst,
            xattr.name.as_ptr(),
            xattr.value.as_ptr().cast(),
            xattr.value.len(),
        );
    }
    for sparse in &src_data.sparse {
        (api.archive_entry_sparse_add_entry)(dst, sparse.offset, sparse.length);
    }
    ARCHIVE_OK
}

pub(crate) unsafe fn backend_entry_to_custom(
    src: *mut BackendEntry,
    dst: *mut archive_entry,
) -> c_int {
    let Some(dst_data) = from_raw(dst) else {
        return ARCHIVE_FATAL;
    };
    clear_entry(dst_data);
    let api = backend_api();

    dst_data.mode = (api.archive_entry_mode)(src);
    dst_data.uid = (api.archive_entry_uid)(src);
    dst_data.gid = (api.archive_entry_gid)(src);
    dst_data.nlink = (api.archive_entry_nlink)(src);
    dst_data.ino = (api.archive_entry_ino)(src);
    dst_data.ino_set = dst_data.ino != 0;
    dst_data.dev = (api.archive_entry_dev)(src);
    dst_data.dev_set = dst_data.dev != 0;
    dst_data.rdev = (api.archive_entry_rdev)(src);
    dst_data
        .pathname
        .set_bytes((!((api.archive_entry_pathname)(src)).is_null()).then(|| {
            CStr::from_ptr((api.archive_entry_pathname)(src))
                .to_bytes()
                .to_vec()
        }));
    dst_data
        .uname
        .set_bytes((!((api.archive_entry_uname)(src)).is_null()).then(|| {
            CStr::from_ptr((api.archive_entry_uname)(src))
                .to_bytes()
                .to_vec()
        }));
    dst_data
        .gname
        .set_bytes((!((api.archive_entry_gname)(src)).is_null()).then(|| {
            CStr::from_ptr((api.archive_entry_gname)(src))
                .to_bytes()
                .to_vec()
        }));
    dst_data
        .hardlink
        .set_bytes((!((api.archive_entry_hardlink)(src)).is_null()).then(|| {
            CStr::from_ptr((api.archive_entry_hardlink)(src))
                .to_bytes()
                .to_vec()
        }));
    dst_data
        .symlink
        .set_bytes((!((api.archive_entry_symlink)(src)).is_null()).then(|| {
            CStr::from_ptr((api.archive_entry_symlink)(src))
                .to_bytes()
                .to_vec()
        }));
    dst_data.symlink_type = (api.archive_entry_symlink_type)(src);
    if (api.archive_entry_size_is_set)(src) != 0 {
        dst_data.size = (api.archive_entry_size)(src);
        dst_data.size_set = true;
    }
    if (api.archive_entry_atime_is_set)(src) != 0 {
        dst_data.atime.set(
            (api.archive_entry_atime)(src),
            (api.archive_entry_atime_nsec)(src) as i64,
        );
    }
    if (api.archive_entry_birthtime_is_set)(src) != 0 {
        dst_data.birthtime.set(
            (api.archive_entry_birthtime)(src),
            (api.archive_entry_birthtime_nsec)(src) as i64,
        );
    }
    if (api.archive_entry_ctime_is_set)(src) != 0 {
        dst_data.ctime.set(
            (api.archive_entry_ctime)(src),
            (api.archive_entry_ctime_nsec)(src) as i64,
        );
    }
    if (api.archive_entry_mtime_is_set)(src) != 0 {
        dst_data.mtime.set(
            (api.archive_entry_mtime)(src),
            (api.archive_entry_mtime_nsec)(src) as i64,
        );
    }
    (api.archive_entry_fflags)(src, &mut dst_data.fflags_set, &mut dst_data.fflags_clear);
    let mut mac_size = 0usize;
    let mac_ptr = (api.archive_entry_mac_metadata)(src, &mut mac_size);
    if !mac_ptr.is_null() && mac_size != 0 {
        dst_data.mac_metadata = slice::from_raw_parts(mac_ptr.cast::<u8>(), mac_size).to_vec();
    }
    dst_data.data_encrypted = (api.archive_entry_is_data_encrypted)(src) != 0;
    dst_data.metadata_encrypted = (api.archive_entry_is_metadata_encrypted)(src) != 0;

    let acl_types = (api.archive_entry_acl_types)(src);
    if acl_types != 0 {
        dst_data.acl.clear();
        let _ = (api.archive_entry_acl_reset)(src, acl_types);
        loop {
            let mut entry_type = 0;
            let mut permset = 0;
            let mut tag = 0;
            let mut qual = -1;
            let mut name = ptr::null();
            let status = (api.archive_entry_acl_next)(
                src,
                acl_types,
                &mut entry_type,
                &mut permset,
                &mut tag,
                &mut qual,
                &mut name,
            );
            if status == ARCHIVE_EOF {
                break;
            }
            if status != ARCHIVE_OK {
                return status;
            }
            let name =
                (!name.is_null()).then(|| CStr::from_ptr(name).to_string_lossy().into_owned());
            let _ =
                dst_data
                    .acl
                    .add_entry(&mut dst_data.mode, entry_type, permset, tag, qual, name);
        }
    }

    dst_data.xattrs.clear();
    let _ = (api.archive_entry_xattr_reset)(src);
    loop {
        let mut name = ptr::null();
        let mut value = ptr::null();
        let mut size = 0usize;
        let status = (api.archive_entry_xattr_next)(src, &mut name, &mut value, &mut size);
        if status == crate::common::error::ARCHIVE_WARN {
            break;
        }
        if status != ARCHIVE_OK {
            return status;
        }
        if !name.is_null() {
            dst_data.xattrs.push(XattrEntry {
                name: CString::new(CStr::from_ptr(name).to_bytes()).expect("xattr name"),
                value: slice::from_raw_parts(value.cast::<u8>(), size).to_vec(),
            });
        }
    }

    dst_data.sparse.clear();
    let _ = (api.archive_entry_sparse_reset)(src);
    loop {
        let mut offset = 0;
        let mut length = 0;
        let status = (api.archive_entry_sparse_next)(src, &mut offset, &mut length);
        if status == crate::common::error::ARCHIVE_WARN {
            break;
        }
        if status != ARCHIVE_OK {
            return status;
        }
        dst_data.sparse.push(SparseEntry { offset, length });
    }

    dst_data.stat_dirty = true;
    dst_data.strmode_cache = None;
    dst_data.xattr_iter = 0;
    dst_data.sparse_iter = 0;
    ARCHIVE_OK
}

fn validate_read_disk(
    a: *mut archive,
    function: &str,
) -> Option<&'static mut crate::common::state::ReadDiskArchiveHandle> {
    unsafe {
        if archive_check_magic(
            a,
            crate::common::error::ARCHIVE_READ_DISK_MAGIC,
            crate::common::error::ARCHIVE_STATE_ANY,
            function,
        ) == ARCHIVE_FATAL
        {
            return None;
        }
        read_disk_from_archive(a)
    }
}

fn validate_write_disk(
    a: *mut archive,
    function: &str,
) -> Option<&'static mut crate::common::state::WriteDiskArchiveHandle> {
    unsafe {
        if archive_check_magic(
            a,
            crate::common::error::ARCHIVE_WRITE_DISK_MAGIC,
            crate::common::error::ARCHIVE_STATE_ANY,
            function,
        ) == ARCHIVE_FATAL
        {
            return None;
        }
        write_disk_from_archive(a)
    }
}

unsafe fn cleanup_read_disk_gname_lookup(handle: &mut crate::common::state::ReadDiskArchiveHandle) {
    if let Some(cleanup) = handle.gname_lookup_cleanup.take() {
        cleanup(handle.gname_lookup_private_data);
    }
    handle.gname_lookup_private_data = ptr::null_mut();
    handle.gname_lookup = None;
}

unsafe fn cleanup_read_disk_uname_lookup(handle: &mut crate::common::state::ReadDiskArchiveHandle) {
    if let Some(cleanup) = handle.uname_lookup_cleanup.take() {
        cleanup(handle.uname_lookup_private_data);
    }
    handle.uname_lookup_private_data = ptr::null_mut();
    handle.uname_lookup = None;
}

unsafe fn cleanup_write_disk_group_lookup(
    handle: &mut crate::common::state::WriteDiskArchiveHandle,
) {
    if let Some(cleanup) = handle.group_lookup_cleanup.take() {
        cleanup(handle.group_lookup_private_data);
    }
    handle.group_lookup_private_data = ptr::null_mut();
    handle.group_lookup = None;
}

unsafe fn cleanup_write_disk_user_lookup(
    handle: &mut crate::common::state::WriteDiskArchiveHandle,
) {
    if let Some(cleanup) = handle.user_lookup_cleanup.take() {
        cleanup(handle.user_lookup_private_data);
    }
    handle.user_lookup_private_data = ptr::null_mut();
    handle.user_lookup = None;
}

unsafe fn resolve_read_disk_gname(
    handle: &mut crate::common::state::ReadDiskArchiveHandle,
    gid: i64,
) -> Option<String> {
    if let Some(lookup) = handle.gname_lookup {
        return from_optional_c_str(lookup(handle.gname_lookup_private_data, gid));
    }
    if handle.use_standard_lookup {
        let group = libc::getgrgid(gid as libc::gid_t);
        if !group.is_null() {
            return from_optional_c_str((*group).gr_name);
        }
    }
    None
}

unsafe fn resolve_read_disk_uname(
    handle: &mut crate::common::state::ReadDiskArchiveHandle,
    uid: i64,
) -> Option<String> {
    if let Some(lookup) = handle.uname_lookup {
        return from_optional_c_str(lookup(handle.uname_lookup_private_data, uid));
    }
    if handle.use_standard_lookup {
        let user = libc::getpwuid(uid as libc::uid_t);
        if !user.is_null() {
            return from_optional_c_str((*user).pw_name);
        }
    }
    None
}

#[no_mangle]
pub extern "C" fn archive_read_disk_set_symlink_logical(a: *mut archive) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_read_disk(a, "archive_read_disk_set_symlink_logical") else {
            return ARCHIVE_FATAL;
        };
        clear_error(&mut handle.core);
        handle.symlink_mode = ReadDiskSymlinkMode::Logical;
        ARCHIVE_OK
    })
}

#[no_mangle]
pub extern "C" fn archive_read_disk_set_symlink_physical(a: *mut archive) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_read_disk(a, "archive_read_disk_set_symlink_physical") else {
            return ARCHIVE_FATAL;
        };
        clear_error(&mut handle.core);
        handle.symlink_mode = ReadDiskSymlinkMode::Physical;
        ARCHIVE_OK
    })
}

#[no_mangle]
pub extern "C" fn archive_read_disk_set_symlink_hybrid(a: *mut archive) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_read_disk(a, "archive_read_disk_set_symlink_hybrid") else {
            return ARCHIVE_FATAL;
        };
        clear_error(&mut handle.core);
        handle.symlink_mode = ReadDiskSymlinkMode::Hybrid;
        ARCHIVE_OK
    })
}

#[no_mangle]
pub extern "C" fn archive_read_disk_entry_from_file(
    a: *mut archive,
    entry: *mut archive_entry,
    fd: c_int,
    st: *const stat,
) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_read_disk(a, "archive_read_disk_entry_from_file") else {
            return ARCHIVE_FATAL;
        };
        if entry.is_null() {
            return ARCHIVE_FATAL;
        }
        native_read_disk_entry_from_file(handle, entry, fd, st)
    })
}

#[no_mangle]
pub extern "C" fn archive_read_disk_gname(a: *mut archive, gid: i64) -> *const c_char {
    ffi_const_ptr(|| unsafe {
        let Some(handle) = validate_read_disk(a, "archive_read_disk_gname") else {
            return ptr::null();
        };
        if let Some(name) = resolve_read_disk_gname(handle, gid) {
            handle.gname_cache = Some(CString::new(name).expect("gname"));
            return handle
                .gname_cache
                .as_ref()
                .map_or(ptr::null(), |value| value.as_ptr());
        }
        ptr::null()
    })
}

#[no_mangle]
pub extern "C" fn archive_read_disk_uname(a: *mut archive, uid: i64) -> *const c_char {
    ffi_const_ptr(|| unsafe {
        let Some(handle) = validate_read_disk(a, "archive_read_disk_uname") else {
            return ptr::null();
        };
        if let Some(name) = resolve_read_disk_uname(handle, uid) {
            handle.uname_cache = Some(CString::new(name).expect("uname"));
            return handle
                .uname_cache
                .as_ref()
                .map_or(ptr::null(), |value| value.as_ptr());
        }
        ptr::null()
    })
}

#[no_mangle]
pub extern "C" fn archive_read_disk_set_standard_lookup(a: *mut archive) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_read_disk(a, "archive_read_disk_set_standard_lookup") else {
            return ARCHIVE_FATAL;
        };
        cleanup_read_disk_gname_lookup(handle);
        cleanup_read_disk_uname_lookup(handle);
        handle.use_standard_lookup = true;
        ARCHIVE_OK
    })
}

#[no_mangle]
pub extern "C" fn archive_read_disk_set_gname_lookup(
    a: *mut archive,
    private_data: *mut c_void,
    lookup: crate::common::backend::BackendReadDiskLookupCallback,
    cleanup: crate::common::backend::BackendReadDiskCleanupCallback,
) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_read_disk(a, "archive_read_disk_set_gname_lookup") else {
            return ARCHIVE_FATAL;
        };
        cleanup_read_disk_gname_lookup(handle);
        handle.gname_lookup_private_data = private_data;
        handle.gname_lookup = lookup;
        handle.gname_lookup_cleanup = cleanup;
        handle.use_standard_lookup = false;
        ARCHIVE_OK
    })
}

#[no_mangle]
pub extern "C" fn archive_read_disk_set_uname_lookup(
    a: *mut archive,
    private_data: *mut c_void,
    lookup: crate::common::backend::BackendReadDiskLookupCallback,
    cleanup: crate::common::backend::BackendReadDiskCleanupCallback,
) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_read_disk(a, "archive_read_disk_set_uname_lookup") else {
            return ARCHIVE_FATAL;
        };
        cleanup_read_disk_uname_lookup(handle);
        handle.uname_lookup_private_data = private_data;
        handle.uname_lookup = lookup;
        handle.uname_lookup_cleanup = cleanup;
        handle.use_standard_lookup = false;
        ARCHIVE_OK
    })
}

#[no_mangle]
pub extern "C" fn archive_read_disk_open(a: *mut archive, path: *const c_char) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_read_disk(a, "archive_read_disk_open") else {
            return ARCHIVE_FATAL;
        };
        if path.is_null() {
            return ARCHIVE_FATAL;
        }
        clear_error(&mut handle.core);
        let path = from_optional_c_str(path)
            .ok_or(ARCHIVE_FATAL)
            .unwrap_or_default();
        handle.open_path = ReadDiskOpenPath::Utf8(path.clone());
        native_read_disk_open_path(handle, &path)
    })
}

#[no_mangle]
pub extern "C" fn archive_read_disk_open_w(a: *mut archive, path: *const wchar_t) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_read_disk(a, "archive_read_disk_open_w") else {
            return ARCHIVE_FATAL;
        };
        if path.is_null() {
            return ARCHIVE_FATAL;
        }
        clear_error(&mut handle.core);
        let path = from_optional_wide(path)
            .ok_or(ARCHIVE_FATAL)
            .unwrap_or_default();
        handle.open_path = ReadDiskOpenPath::Wide(path.clone());
        native_read_disk_open_path(handle, &path)
    })
}

#[no_mangle]
pub extern "C" fn archive_read_disk_descend(a: *mut archive) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_read_disk(a, "archive_read_disk_descend") else {
            return ARCHIVE_FATAL;
        };
        native_read_disk_descend(handle)
    })
}

#[no_mangle]
pub extern "C" fn archive_read_disk_can_descend(a: *mut archive) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_read_disk(a, "archive_read_disk_can_descend") else {
            return ARCHIVE_FATAL;
        };
        native_read_disk_can_descend(handle)
    })
}

#[no_mangle]
pub extern "C" fn archive_read_disk_current_filesystem(a: *mut archive) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_read_disk(a, "archive_read_disk_current_filesystem") else {
            return ARCHIVE_FATAL;
        };
        let _ = handle;
        0
    })
}

#[no_mangle]
pub extern "C" fn archive_read_disk_current_filesystem_is_synthetic(a: *mut archive) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) =
            validate_read_disk(a, "archive_read_disk_current_filesystem_is_synthetic")
        else {
            return ARCHIVE_FATAL;
        };
        let _ = handle;
        0
    })
}

#[no_mangle]
pub extern "C" fn archive_read_disk_current_filesystem_is_remote(a: *mut archive) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_read_disk(a, "archive_read_disk_current_filesystem_is_remote")
        else {
            return ARCHIVE_FATAL;
        };
        let _ = handle;
        0
    })
}

#[no_mangle]
pub extern "C" fn archive_read_disk_set_atime_restored(a: *mut archive) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_read_disk(a, "archive_read_disk_set_atime_restored") else {
            return ARCHIVE_FATAL;
        };
        handle.behavior_flags |= 0x0001;
        ARCHIVE_OK
    })
}

#[no_mangle]
pub extern "C" fn archive_read_disk_set_behavior(a: *mut archive, flags: c_int) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_read_disk(a, "archive_read_disk_set_behavior") else {
            return ARCHIVE_FATAL;
        };
        handle.behavior_flags = flags;
        ARCHIVE_OK
    })
}

#[no_mangle]
pub extern "C" fn archive_read_disk_set_matching(
    a: *mut archive,
    matching: *mut archive,
    excluded_func: Option<crate::common::state::ReadDiskExcludedCallback>,
    client_data: *mut c_void,
) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_read_disk(a, "archive_read_disk_set_matching") else {
            return ARCHIVE_FATAL;
        };
        handle.matching = matching;
        handle.excluded_cb = excluded_func;
        handle.excluded_client_data = client_data;
        ARCHIVE_OK
    })
}

#[no_mangle]
pub extern "C" fn archive_read_disk_set_metadata_filter_callback(
    a: *mut archive,
    metadata_filter_func: Option<crate::common::state::ReadDiskMetadataFilterCallback>,
    client_data: *mut c_void,
) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_read_disk(a, "archive_read_disk_set_metadata_filter_callback")
        else {
            return ARCHIVE_FATAL;
        };
        handle.metadata_filter_cb = metadata_filter_func;
        handle.metadata_filter_client_data = client_data;
        ARCHIVE_OK
    })
}

#[no_mangle]
pub extern "C" fn archive_write_disk_set_skip_file(a: *mut archive, dev: i64, ino: i64) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_write_disk(a, "archive_write_disk_set_skip_file") else {
            return ARCHIVE_FATAL;
        };
        handle.skip_file = Some((dev, ino));
        ARCHIVE_OK
    })
}

#[no_mangle]
pub extern "C" fn archive_write_disk_set_options(a: *mut archive, flags: c_int) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_write_disk(a, "archive_write_disk_set_options") else {
            return ARCHIVE_FATAL;
        };
        handle.options = flags;
        ARCHIVE_OK
    })
}

#[no_mangle]
pub extern "C" fn archive_write_disk_set_standard_lookup(a: *mut archive) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_write_disk(a, "archive_write_disk_set_standard_lookup") else {
            return ARCHIVE_FATAL;
        };
        cleanup_write_disk_group_lookup(handle);
        cleanup_write_disk_user_lookup(handle);
        handle.use_standard_lookup = true;
        ARCHIVE_OK
    })
}

#[no_mangle]
pub extern "C" fn archive_write_disk_set_group_lookup(
    a: *mut archive,
    private_data: *mut c_void,
    lookup: crate::common::backend::BackendWriteDiskLookupCallback,
    cleanup: crate::common::backend::BackendWriteDiskCleanupCallback,
) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_write_disk(a, "archive_write_disk_set_group_lookup") else {
            return ARCHIVE_FATAL;
        };
        cleanup_write_disk_group_lookup(handle);
        handle.group_lookup_private_data = private_data;
        handle.group_lookup = lookup;
        handle.group_lookup_cleanup = cleanup;
        handle.use_standard_lookup = false;
        ARCHIVE_OK
    })
}

#[no_mangle]
pub extern "C" fn archive_write_disk_set_user_lookup(
    a: *mut archive,
    private_data: *mut c_void,
    lookup: crate::common::backend::BackendWriteDiskLookupCallback,
    cleanup: crate::common::backend::BackendWriteDiskCleanupCallback,
) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_write_disk(a, "archive_write_disk_set_user_lookup") else {
            return ARCHIVE_FATAL;
        };
        cleanup_write_disk_user_lookup(handle);
        handle.user_lookup_private_data = private_data;
        handle.user_lookup = lookup;
        handle.user_lookup_cleanup = cleanup;
        handle.use_standard_lookup = false;
        ARCHIVE_OK
    })
}

#[no_mangle]
pub extern "C" fn archive_write_disk_gid(a: *mut archive, name: *const c_char, gid: i64) -> i64 {
    unsafe {
        let Some(handle) = validate_write_disk(a, "archive_write_disk_gid") else {
            return -1;
        };
        let name = from_optional_c_str(name);
        let Some(name) = name.filter(|name| !name.is_empty()) else {
            return gid;
        };
        if let Some(lookup) = handle.group_lookup {
            let name = CString::new(name.as_str()).expect("group name");
            return lookup(handle.group_lookup_private_data, name.as_ptr(), gid);
        }
        if handle.use_standard_lookup {
            let name = CString::new(name.as_str()).expect("group name");
            let group = libc::getgrnam(name.as_ptr());
            if !group.is_null() {
                return (*group).gr_gid as i64;
            }
        }
        gid
    }
}

#[no_mangle]
pub extern "C" fn archive_write_disk_uid(a: *mut archive, name: *const c_char, uid: i64) -> i64 {
    unsafe {
        let Some(handle) = validate_write_disk(a, "archive_write_disk_uid") else {
            return -1;
        };
        let name = from_optional_c_str(name);
        let Some(name) = name.filter(|name| !name.is_empty()) else {
            return uid;
        };
        if let Some(lookup) = handle.user_lookup {
            let name = CString::new(name.as_str()).expect("user name");
            return lookup(handle.user_lookup_private_data, name.as_ptr(), uid);
        }
        if handle.use_standard_lookup {
            let name = CString::new(name.as_str()).expect("user name");
            let user = libc::getpwnam(name.as_ptr());
            if !user.is_null() {
                return (*user).pw_uid as i64;
            }
        }
        uid
    }
}
