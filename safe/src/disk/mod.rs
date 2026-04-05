use std::ffi::{c_char, c_int, c_long, c_void, CStr, CString};
use std::ptr;
use std::slice;

use libc::{mode_t, size_t, stat, wchar_t};

use crate::common::backend::{api as backend_api, BackendArchive, BackendEntry};
use crate::common::error::{ARCHIVE_EOF, ARCHIVE_FATAL, ARCHIVE_OK};
use crate::common::helpers::from_optional_c_str;
use crate::common::panic_boundary::{ffi_const_ptr, ffi_int};
use crate::common::state::{
    archive_check_magic, archive_magic, clear_error, read_disk_from_archive, read_from_archive,
    set_error_string, sync_backend_core, write_disk_from_archive, ArchiveKind,
};
use crate::entry::internal::{
    clear_entry, from_raw, AclEntry, AclState, ArchiveEntryData, SparseEntry, XattrEntry,
};
use crate::ffi::{archive, archive_entry};
use crate::r#match::internal::{from_archive as match_from_archive, MatchArchive};

fn c_string_opt(value: Option<&str>) -> Option<CString> {
    value.map(|value| CString::new(value).expect("string must not contain NUL"))
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

    if let Some(pathname) = c_string_opt(src_data.pathname.get_str()) {
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
    if let Some(uname) = c_string_opt(src_data.uname.get_str()) {
        (api.archive_entry_copy_uname)(dst, uname.as_ptr());
    }
    if let Some(gname) = c_string_opt(src_data.gname.get_str()) {
        (api.archive_entry_copy_gname)(dst, gname.as_ptr());
    }
    if let Some(hardlink) = c_string_opt(src_data.hardlink.get_str()) {
        (api.archive_entry_copy_hardlink)(dst, hardlink.as_ptr());
    }
    if let Some(symlink) = c_string_opt(src_data.symlink.get_str()) {
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
        .set((!((api.archive_entry_pathname)(src)).is_null()).then(|| {
            CStr::from_ptr((api.archive_entry_pathname)(src))
                .to_string_lossy()
                .into_owned()
        }));
    dst_data
        .uname
        .set((!((api.archive_entry_uname)(src)).is_null()).then(|| {
            CStr::from_ptr((api.archive_entry_uname)(src))
                .to_string_lossy()
                .into_owned()
        }));
    dst_data
        .gname
        .set((!((api.archive_entry_gname)(src)).is_null()).then(|| {
            CStr::from_ptr((api.archive_entry_gname)(src))
                .to_string_lossy()
                .into_owned()
        }));
    dst_data
        .hardlink
        .set((!((api.archive_entry_hardlink)(src)).is_null()).then(|| {
            CStr::from_ptr((api.archive_entry_hardlink)(src))
                .to_string_lossy()
                .into_owned()
        }));
    dst_data
        .symlink
        .set((!((api.archive_entry_symlink)(src)).is_null()).then(|| {
            CStr::from_ptr((api.archive_entry_symlink)(src))
                .to_string_lossy()
                .into_owned()
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

unsafe fn clone_match_to_backend(matching: *mut archive) -> *mut BackendArchive {
    let Some(source) = match_from_archive(matching) else {
        return ptr::null_mut();
    };
    let api = backend_api();
    let backend = (api.archive_match_new)();
    if backend.is_null() {
        return ptr::null_mut();
    }

    let _ =
        (api.archive_match_set_inclusion_recursion)(backend, i32::from(source.recursive_include));
    for pattern in &source.exclusions.patterns {
        let text = CString::new(pattern.text.as_str()).expect("pattern");
        let _ = (api.archive_match_exclude_pattern)(backend, text.as_ptr());
    }
    for pattern in &source.inclusions.patterns {
        let text = CString::new(pattern.text.as_str()).expect("pattern");
        let _ = (api.archive_match_include_pattern)(backend, text.as_ptr());
    }
    if let Some(filter) = source.newer_mtime {
        let _ = (api.archive_match_include_time)(
            backend,
            filter.flag,
            filter.sec,
            filter.nsec as c_long,
        );
    }
    if let Some(filter) = source.older_mtime {
        let _ = (api.archive_match_include_time)(
            backend,
            filter.flag,
            filter.sec,
            filter.nsec as c_long,
        );
    }
    if let Some(filter) = source.newer_ctime {
        let _ = (api.archive_match_include_time)(
            backend,
            filter.flag,
            filter.sec,
            filter.nsec as c_long,
        );
    }
    if let Some(filter) = source.older_ctime {
        let _ = (api.archive_match_include_time)(
            backend,
            filter.flag,
            filter.sec,
            filter.nsec as c_long,
        );
    }
    for uid in &source.inclusion_uids {
        let _ = (api.archive_match_include_uid)(backend, *uid);
    }
    for gid in &source.inclusion_gids {
        let _ = (api.archive_match_include_gid)(backend, *gid);
    }
    for uname in &source.inclusion_unames {
        let text = CString::new(uname.text.as_str()).expect("uname");
        let _ = (api.archive_match_include_uname)(backend, text.as_ptr());
    }
    for gname in &source.inclusion_gnames {
        let text = CString::new(gname.text.as_str()).expect("gname");
        let _ = (api.archive_match_include_gname)(backend, text.as_ptr());
    }
    backend
}

unsafe extern "C" fn excluded_callback_shim(
    _backend: *mut BackendArchive,
    client_data: *mut c_void,
    entry: *mut BackendEntry,
) {
    let handle = &mut *(client_data as *mut crate::common::state::ReadDiskArchiveHandle);
    let Some(callback) = handle.excluded_cb else {
        return;
    };
    let custom = crate::entry::api::archive_entry_new();
    if custom.is_null() {
        return;
    }
    if backend_entry_to_custom(entry, custom) == ARCHIVE_OK {
        callback(
            (handle as *mut crate::common::state::ReadDiskArchiveHandle).cast(),
            handle.excluded_client_data,
            custom,
        );
    }
    crate::entry::api::archive_entry_free(custom);
}

unsafe extern "C" fn metadata_filter_callback_shim(
    _backend: *mut BackendArchive,
    client_data: *mut c_void,
    entry: *mut BackendEntry,
) -> c_int {
    let handle = &mut *(client_data as *mut crate::common::state::ReadDiskArchiveHandle);
    let Some(callback) = handle.metadata_filter_cb else {
        return 1;
    };
    let custom = crate::entry::api::archive_entry_new();
    if custom.is_null() {
        return 0;
    }
    let status = if backend_entry_to_custom(entry, custom) == ARCHIVE_OK {
        callback(
            (handle as *mut crate::common::state::ReadDiskArchiveHandle).cast(),
            handle.metadata_filter_client_data,
            custom,
        )
    } else {
        0
    };
    crate::entry::api::archive_entry_free(custom);
    status
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

#[no_mangle]
pub extern "C" fn archive_read_disk_set_symlink_logical(a: *mut archive) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_read_disk(a, "archive_read_disk_set_symlink_logical") else {
            return ARCHIVE_FATAL;
        };
        clear_error(&mut handle.core);
        let status = (backend_api().archive_read_disk_set_symlink_logical)(handle.backend);
        sync_backend_core(a);
        status
    })
}

#[no_mangle]
pub extern "C" fn archive_read_disk_set_symlink_physical(a: *mut archive) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_read_disk(a, "archive_read_disk_set_symlink_physical") else {
            return ARCHIVE_FATAL;
        };
        clear_error(&mut handle.core);
        let status = (backend_api().archive_read_disk_set_symlink_physical)(handle.backend);
        sync_backend_core(a);
        status
    })
}

#[no_mangle]
pub extern "C" fn archive_read_disk_set_symlink_hybrid(a: *mut archive) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_read_disk(a, "archive_read_disk_set_symlink_hybrid") else {
            return ARCHIVE_FATAL;
        };
        clear_error(&mut handle.core);
        let status = (backend_api().archive_read_disk_set_symlink_hybrid)(handle.backend);
        sync_backend_core(a);
        status
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
        let backend_entry = (backend_api().archive_entry_new)();
        if backend_entry.is_null() {
            return ARCHIVE_FATAL;
        }
        let status = if custom_entry_to_backend(entry, backend_entry) != ARCHIVE_OK {
            ARCHIVE_FATAL
        } else {
            let status = (backend_api().archive_read_disk_entry_from_file)(
                handle.backend,
                backend_entry,
                fd,
                st.cast(),
            );
            if status == ARCHIVE_OK {
                backend_entry_to_custom(backend_entry, entry)
            } else {
                status
            }
        };
        (backend_api().archive_entry_free)(backend_entry);
        sync_backend_core(a);
        status
    })
}

#[no_mangle]
pub extern "C" fn archive_read_disk_gname(a: *mut archive, gid: i64) -> *const c_char {
    ffi_const_ptr(|| unsafe {
        let Some(handle) = validate_read_disk(a, "archive_read_disk_gname") else {
            return ptr::null();
        };
        (backend_api().archive_read_disk_gname)(handle.backend, gid)
    })
}

#[no_mangle]
pub extern "C" fn archive_read_disk_uname(a: *mut archive, uid: i64) -> *const c_char {
    ffi_const_ptr(|| unsafe {
        let Some(handle) = validate_read_disk(a, "archive_read_disk_uname") else {
            return ptr::null();
        };
        (backend_api().archive_read_disk_uname)(handle.backend, uid)
    })
}

#[no_mangle]
pub extern "C" fn archive_read_disk_set_standard_lookup(a: *mut archive) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_read_disk(a, "archive_read_disk_set_standard_lookup") else {
            return ARCHIVE_FATAL;
        };
        (backend_api().archive_read_disk_set_standard_lookup)(handle.backend)
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
        (backend_api().archive_read_disk_set_gname_lookup)(
            handle.backend,
            private_data,
            lookup,
            cleanup,
        )
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
        (backend_api().archive_read_disk_set_uname_lookup)(
            handle.backend,
            private_data,
            lookup,
            cleanup,
        )
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
        let status = (backend_api().archive_read_disk_open)(handle.backend, path);
        sync_backend_core(a);
        status
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
        let status = (backend_api().archive_read_disk_open_w)(handle.backend, path);
        sync_backend_core(a);
        status
    })
}

#[no_mangle]
pub extern "C" fn archive_read_disk_descend(a: *mut archive) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_read_disk(a, "archive_read_disk_descend") else {
            return ARCHIVE_FATAL;
        };
        (backend_api().archive_read_disk_descend)(handle.backend)
    })
}

#[no_mangle]
pub extern "C" fn archive_read_disk_can_descend(a: *mut archive) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_read_disk(a, "archive_read_disk_can_descend") else {
            return ARCHIVE_FATAL;
        };
        (backend_api().archive_read_disk_can_descend)(handle.backend)
    })
}

#[no_mangle]
pub extern "C" fn archive_read_disk_current_filesystem(a: *mut archive) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_read_disk(a, "archive_read_disk_current_filesystem") else {
            return ARCHIVE_FATAL;
        };
        (backend_api().archive_read_disk_current_filesystem)(handle.backend)
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
        (backend_api().archive_read_disk_current_filesystem_is_synthetic)(handle.backend)
    })
}

#[no_mangle]
pub extern "C" fn archive_read_disk_current_filesystem_is_remote(a: *mut archive) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_read_disk(a, "archive_read_disk_current_filesystem_is_remote")
        else {
            return ARCHIVE_FATAL;
        };
        (backend_api().archive_read_disk_current_filesystem_is_remote)(handle.backend)
    })
}

#[no_mangle]
pub extern "C" fn archive_read_disk_set_atime_restored(a: *mut archive) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_read_disk(a, "archive_read_disk_set_atime_restored") else {
            return ARCHIVE_FATAL;
        };
        (backend_api().archive_read_disk_set_atime_restored)(handle.backend)
    })
}

#[no_mangle]
pub extern "C" fn archive_read_disk_set_behavior(a: *mut archive, flags: c_int) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_read_disk(a, "archive_read_disk_set_behavior") else {
            return ARCHIVE_FATAL;
        };
        (backend_api().archive_read_disk_set_behavior)(handle.backend, flags)
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

        if !handle.backend_match.is_null() {
            (backend_api().archive_match_free)(handle.backend_match);
            handle.backend_match = ptr::null_mut();
        }

        handle.excluded_cb = excluded_func;
        handle.excluded_client_data = client_data;

        if matching.is_null() {
            return (backend_api().archive_read_disk_set_matching)(
                handle.backend,
                ptr::null_mut(),
                None,
                ptr::null_mut(),
            );
        }

        handle.backend_match = clone_match_to_backend(matching);
        if handle.backend_match.is_null() {
            set_error_string(
                &mut handle.core,
                -1,
                "failed to clone archive_match".to_string(),
            );
            return ARCHIVE_FATAL;
        }

        (backend_api().archive_read_disk_set_matching)(
            handle.backend,
            handle.backend_match,
            excluded_func.map(|_| {
                excluded_callback_shim
                    as unsafe extern "C" fn(*mut BackendArchive, *mut c_void, *mut BackendEntry)
            }),
            (handle as *mut crate::common::state::ReadDiskArchiveHandle).cast(),
        )
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
        (backend_api().archive_read_disk_set_metadata_filter_callback)(
            handle.backend,
            metadata_filter_func.map(|_| {
                metadata_filter_callback_shim
                    as unsafe extern "C" fn(
                        *mut BackendArchive,
                        *mut c_void,
                        *mut BackendEntry,
                    ) -> c_int
            }),
            (handle as *mut crate::common::state::ReadDiskArchiveHandle).cast(),
        )
    })
}

#[no_mangle]
pub extern "C" fn archive_write_disk_set_skip_file(a: *mut archive, dev: i64, ino: i64) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_write_disk(a, "archive_write_disk_set_skip_file") else {
            return ARCHIVE_FATAL;
        };
        (backend_api().archive_write_disk_set_skip_file)(handle.backend, dev, ino)
    })
}

#[no_mangle]
pub extern "C" fn archive_write_disk_set_options(a: *mut archive, flags: c_int) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_write_disk(a, "archive_write_disk_set_options") else {
            return ARCHIVE_FATAL;
        };
        (backend_api().archive_write_disk_set_options)(handle.backend, flags)
    })
}

#[no_mangle]
pub extern "C" fn archive_write_disk_set_standard_lookup(a: *mut archive) -> c_int {
    ffi_int(ARCHIVE_FATAL, || unsafe {
        let Some(handle) = validate_write_disk(a, "archive_write_disk_set_standard_lookup") else {
            return ARCHIVE_FATAL;
        };
        (backend_api().archive_write_disk_set_standard_lookup)(handle.backend)
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
        (backend_api().archive_write_disk_set_group_lookup)(
            handle.backend,
            private_data,
            lookup,
            cleanup,
        )
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
        (backend_api().archive_write_disk_set_user_lookup)(
            handle.backend,
            private_data,
            lookup,
            cleanup,
        )
    })
}

#[no_mangle]
pub extern "C" fn archive_write_disk_gid(a: *mut archive, name: *const c_char, gid: i64) -> i64 {
    unsafe {
        let Some(handle) = validate_write_disk(a, "archive_write_disk_gid") else {
            return -1;
        };
        (backend_api().archive_write_disk_gid)(handle.backend, name, gid)
    }
}

#[no_mangle]
pub extern "C" fn archive_write_disk_uid(a: *mut archive, name: *const c_char, uid: i64) -> i64 {
    unsafe {
        let Some(handle) = validate_write_disk(a, "archive_write_disk_uid") else {
            return -1;
        };
        (backend_api().archive_write_disk_uid)(handle.backend, name, uid)
    }
}
