use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::fs::{self, File};
use std::io::Read;
use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path, PathBuf};
use std::ptr;

use libc::{mode_t, size_t, stat, timespec};

use crate::common::error::{ARCHIVE_EOF, ARCHIVE_FAILED, ARCHIVE_FATAL, ARCHIVE_OK, ARCHIVE_WARN};
use crate::common::helpers::from_optional_c_str;
use crate::common::state::{
    clear_error, set_error_string, ReadDiskArchiveHandle, ReadDiskAtimeRestore, ReadDiskNode,
    ReadDiskSymlinkMode, WriteDiskArchiveHandle, WriteDiskCurrentState, WriteDiskPendingFixup,
};
use crate::entry::internal::{
    add_sparse, clear_entry, copy_stat, from_raw as entry_from_raw, AclState, SparseEntry,
    AE_IFDIR, AE_IFIFO, AE_IFLNK, AE_IFMT, AE_IFREG, ARCHIVE_ENTRY_ACL_EXECUTE,
    ARCHIVE_ENTRY_ACL_GROUP, ARCHIVE_ENTRY_ACL_GROUP_OBJ, ARCHIVE_ENTRY_ACL_MASK,
    ARCHIVE_ENTRY_ACL_OTHER, ARCHIVE_ENTRY_ACL_READ, ARCHIVE_ENTRY_ACL_TYPE_ACCESS,
    ARCHIVE_ENTRY_ACL_TYPE_DEFAULT, ARCHIVE_ENTRY_ACL_USER, ARCHIVE_ENTRY_ACL_USER_OBJ,
    ARCHIVE_ENTRY_ACL_WRITE,
};
use crate::ffi::archive_entry;

const ARCHIVE_EXTRACT_OWNER: c_int = 0x0001;
const ARCHIVE_EXTRACT_PERM: c_int = 0x0002;
const ARCHIVE_EXTRACT_TIME: c_int = 0x0004;
const ARCHIVE_EXTRACT_NO_OVERWRITE: c_int = 0x0008;
const ARCHIVE_EXTRACT_UNLINK: c_int = 0x0010;
const ARCHIVE_EXTRACT_ACL: c_int = 0x0020;
const ARCHIVE_EXTRACT_XATTR: c_int = 0x0080;
const ARCHIVE_EXTRACT_SECURE_SYMLINKS: c_int = 0x0100;
const ARCHIVE_EXTRACT_SECURE_NODOTDOT: c_int = 0x0200;
const ARCHIVE_EXTRACT_NO_AUTODIR: c_int = 0x0400;
const ARCHIVE_EXTRACT_NO_OVERWRITE_NEWER: c_int = 0x0800;
const ARCHIVE_EXTRACT_SPARSE: c_int = 0x1000;
const ARCHIVE_EXTRACT_SECURE_NOABSOLUTEPATHS: c_int = 0x10000;
const ARCHIVE_EXTRACT_SAFE_WRITES: c_int = 0x40000;

const ARCHIVE_READDISK_RESTORE_ATIME: c_int = 0x0001;
const ARCHIVE_READDISK_HONOR_NODUMP: c_int = 0x0002;
const ARCHIVE_READDISK_NO_XATTR: c_int = 0x0010;
const ARCHIVE_READDISK_NO_ACL: c_int = 0x0020;
const ARCHIVE_READDISK_NO_SPARSE: c_int = 0x0080;

const ACL_FIRST_ENTRY: c_int = 0;
const ACL_NEXT_ENTRY: c_int = 1;
const ACL_EXECUTE: c_int = 0x01;
const ACL_WRITE: c_int = 0x02;
const ACL_READ: c_int = 0x04;
const ACL_USER_OBJ: c_int = 0x01;
const ACL_USER: c_int = 0x02;
const ACL_GROUP_OBJ: c_int = 0x04;
const ACL_GROUP: c_int = 0x08;
const ACL_MASK: c_int = 0x10;
const ACL_OTHER: c_int = 0x20;
const ACL_TYPE_ACCESS: c_int = 0x8000;
const ACL_TYPE_DEFAULT: c_int = 0x4000;
const FS_IOC_GETFLAGS: libc::c_ulong = 0x8008_6601;
const FS_NODUMP_FL: libc::c_long = 0x0000_0040;

unsafe extern "C" {
    fn acl_get_fd(fd: c_int) -> *mut c_void;
    fn acl_get_file(path_p: *const c_char, type_: c_int) -> *mut c_void;
    fn acl_get_entry(acl: *mut c_void, entry_id: c_int, entry_p: *mut *mut c_void) -> c_int;
    fn acl_get_tag_type(entry_d: *mut c_void, tag_type_p: *mut c_int) -> c_int;
    fn acl_get_qualifier(entry_d: *mut c_void) -> *mut c_void;
    fn acl_get_permset(entry_d: *mut c_void, permset_p: *mut *mut c_void) -> c_int;
    fn acl_get_perm(permset_d: *mut c_void, perm: c_int) -> c_int;
    fn acl_init(count: c_int) -> *mut c_void;
    fn acl_create_entry(acl_p: *mut *mut c_void, entry_p: *mut *mut c_void) -> c_int;
    fn acl_set_tag_type(entry_d: *mut c_void, tag_type: c_int) -> c_int;
    fn acl_set_qualifier(entry_d: *mut c_void, tag_qualifier_p: *const c_void) -> c_int;
    fn acl_clear_perms(permset_d: *mut c_void) -> c_int;
    fn acl_add_perm(permset_d: *mut c_void, perm: c_int) -> c_int;
    fn acl_set_fd(fd: c_int, acl: *mut c_void) -> c_int;
    fn acl_set_file(path_p: *const c_char, type_: c_int, acl: *mut c_void) -> c_int;
    fn acl_free(obj_p: *mut c_void) -> c_int;
}

fn last_errno() -> c_int {
    std::io::Error::last_os_error()
        .raw_os_error()
        .unwrap_or(libc::EINVAL)
}

fn c_path(path: &Path) -> Result<CString, c_int> {
    CString::new(path.as_os_str().as_bytes()).map_err(|_| libc::EINVAL)
}

fn path_stat(path: &Path, follow: bool) -> Result<stat, c_int> {
    let c_path = c_path(path)?;
    let mut st = unsafe { std::mem::zeroed::<stat>() };
    let rc = unsafe {
        if follow {
            libc::stat(c_path.as_ptr(), &mut st)
        } else {
            libc::lstat(c_path.as_ptr(), &mut st)
        }
    };
    if rc == 0 {
        Ok(st)
    } else {
        Err(last_errno())
    }
}

fn join_display_path(parent: &str, child: &str) -> String {
    if parent.is_empty() {
        child.to_string()
    } else if parent == "." {
        format!("./{child}")
    } else if parent.ends_with('/') {
        format!("{parent}{child}")
    } else {
        format!("{parent}/{child}")
    }
}

fn read_link_text(path: &Path) -> Option<String> {
    fs::read_link(path)
        .ok()
        .map(|target| target.to_string_lossy().into_owned())
}

fn filetype_from_mode(mode: mode_t) -> mode_t {
    mode & AE_IFMT
}

fn record_error(
    core: &mut crate::common::state::ArchiveCore,
    errno: c_int,
    message: impl Into<String>,
) -> c_int {
    set_error_string(core, errno, message.into());
    ARCHIVE_FAILED
}

fn should_follow_root(mode: ReadDiskSymlinkMode) -> bool {
    matches!(
        mode,
        ReadDiskSymlinkMode::Logical | ReadDiskSymlinkMode::Hybrid
    )
}

fn should_follow_descendant(mode: ReadDiskSymlinkMode) -> bool {
    matches!(mode, ReadDiskSymlinkMode::Logical)
}

fn reset_read_data(handle: &mut ReadDiskArchiveHandle) {
    handle.traversal.current_data.clear();
    handle.traversal.current_data_cursor = 0;
    handle.traversal.current_data_eof = false;
    handle.traversal.current_data_offset = 0;
    handle.traversal.current_size = 0;
    handle.traversal.current_sparse.clear();
    handle.traversal.current_sparse_index = 0;
    handle.traversal.current_fully_sparse = false;
}

struct SparseLayout {
    extents: Vec<SparseEntry>,
    fully_sparse: bool,
}

fn load_sparse_layout(path: &Path, size: i64) -> Option<SparseLayout> {
    if size <= 0 {
        return Some(SparseLayout {
            extents: Vec::new(),
            fully_sparse: false,
        });
    }

    let c_path = c_path(path).ok()?;
    let fd = unsafe { libc::open(c_path.as_ptr(), libc::O_RDONLY | libc::O_CLOEXEC) };
    if fd < 0 {
        return None;
    }

    let mut result = SparseLayout {
        extents: Vec::new(),
        fully_sparse: false,
    };
    let mut offset = 0i64;
    while offset < size {
        let data_offset = unsafe { libc::lseek(fd, offset, libc::SEEK_DATA) };
        if data_offset < 0 {
            let errno = last_errno();
            if errno == libc::ENXIO {
                result.fully_sparse = offset == 0;
                break;
            }
            unsafe {
                libc::close(fd);
            }
            return None;
        }

        let data_offset = data_offset.min(size);
        let hole_offset = unsafe { libc::lseek(fd, data_offset, libc::SEEK_HOLE) };
        if hole_offset < 0 {
            unsafe {
                libc::close(fd);
            }
            return None;
        }
        let hole_offset = hole_offset.min(size);
        if hole_offset > data_offset {
            result.extents.push(SparseEntry {
                offset: data_offset,
                length: hole_offset - data_offset,
            });
        }
        if hole_offset <= data_offset {
            break;
        }
        offset = hole_offset;
    }

    unsafe {
        libc::close(fd);
    }
    Some(result)
}

fn restore_atime(spec: &ReadDiskAtimeRestore) {
    let Ok(c_path) = c_path(&spec.path) else {
        return;
    };
    let times = [spec.atime, spec.mtime];
    unsafe {
        let flags = if spec.follow_symlink {
            0
        } else {
            libc::AT_SYMLINK_NOFOLLOW
        };
        let _ = libc::utimensat(libc::AT_FDCWD, c_path.as_ptr(), times.as_ptr(), flags);
    }
}

fn is_nodump(path: &Path) -> bool {
    let Ok(c_path) = c_path(path) else {
        return false;
    };
    let fd = unsafe { libc::open(c_path.as_ptr(), libc::O_RDONLY | libc::O_CLOEXEC) };
    if fd < 0 {
        return false;
    }
    let mut flags: libc::c_long = 0;
    let rc = unsafe { libc::ioctl(fd, FS_IOC_GETFLAGS, &mut flags) };
    unsafe {
        libc::close(fd);
    }
    rc == 0 && (flags & FS_NODUMP_FL) != 0
}

fn load_xattrs(path: &Path, follow: bool) -> Vec<(CString, Vec<u8>)> {
    let Ok(c_path) = c_path(path) else {
        return Vec::new();
    };
    let list_len = unsafe {
        if follow {
            libc::listxattr(c_path.as_ptr(), ptr::null_mut(), 0)
        } else {
            libc::llistxattr(c_path.as_ptr(), ptr::null_mut(), 0)
        }
    };
    if list_len <= 0 {
        return Vec::new();
    }

    let mut names = vec![0u8; list_len as usize];
    let rc = unsafe {
        if follow {
            libc::listxattr(c_path.as_ptr(), names.as_mut_ptr().cast(), names.len())
        } else {
            libc::llistxattr(c_path.as_ptr(), names.as_mut_ptr().cast(), names.len())
        }
    };
    if rc <= 0 {
        return Vec::new();
    }

    let mut result = Vec::new();
    let mut start = 0usize;
    for index in 0..names.len() {
        if names[index] != 0 {
            continue;
        }
        if index == start {
            start = index + 1;
            continue;
        }
        let Ok(name) = CString::new(&names[start..index]) else {
            start = index + 1;
            continue;
        };
        let size = unsafe {
            if follow {
                libc::getxattr(c_path.as_ptr(), name.as_ptr(), ptr::null_mut(), 0)
            } else {
                libc::lgetxattr(c_path.as_ptr(), name.as_ptr(), ptr::null_mut(), 0)
            }
        };
        if size >= 0 {
            let mut value = vec![0u8; size as usize];
            let read_size = unsafe {
                if follow {
                    libc::getxattr(
                        c_path.as_ptr(),
                        name.as_ptr(),
                        value.as_mut_ptr().cast(),
                        value.len(),
                    )
                } else {
                    libc::lgetxattr(
                        c_path.as_ptr(),
                        name.as_ptr(),
                        value.as_mut_ptr().cast(),
                        value.len(),
                    )
                }
            };
            if read_size >= 0 {
                value.truncate(read_size as usize);
                result.push((name, value));
            }
        }
        start = index + 1;
    }
    result
}

fn acl_tag_to_entry(tag: c_int) -> Option<c_int> {
    match tag {
        ACL_USER => Some(ARCHIVE_ENTRY_ACL_USER),
        ACL_GROUP => Some(ARCHIVE_ENTRY_ACL_GROUP),
        ACL_MASK => Some(ARCHIVE_ENTRY_ACL_MASK),
        ACL_USER_OBJ => Some(ARCHIVE_ENTRY_ACL_USER_OBJ),
        ACL_GROUP_OBJ => Some(ARCHIVE_ENTRY_ACL_GROUP_OBJ),
        ACL_OTHER => Some(ARCHIVE_ENTRY_ACL_OTHER),
        _ => None,
    }
}

fn acl_entry_to_tag(tag: c_int) -> Option<c_int> {
    match tag {
        ARCHIVE_ENTRY_ACL_USER => Some(ACL_USER),
        ARCHIVE_ENTRY_ACL_GROUP => Some(ACL_GROUP),
        ARCHIVE_ENTRY_ACL_MASK => Some(ACL_MASK),
        ARCHIVE_ENTRY_ACL_USER_OBJ => Some(ACL_USER_OBJ),
        ARCHIVE_ENTRY_ACL_GROUP_OBJ => Some(ACL_GROUP_OBJ),
        ARCHIVE_ENTRY_ACL_OTHER => Some(ACL_OTHER),
        _ => None,
    }
}

fn acl_permset_to_permset(permset: *mut c_void) -> Result<c_int, c_int> {
    let mut ae_perm = 0;
    for (archive_perm, acl_perm) in [
        (ARCHIVE_ENTRY_ACL_READ, ACL_READ),
        (ARCHIVE_ENTRY_ACL_WRITE, ACL_WRITE),
        (ARCHIVE_ENTRY_ACL_EXECUTE, ACL_EXECUTE),
    ] {
        let rc = unsafe { acl_get_perm(permset, acl_perm) };
        if rc < 0 {
            return Err(last_errno());
        }
        if rc != 0 {
            ae_perm |= archive_perm;
        }
    }
    Ok(ae_perm)
}

fn add_acl_entries_from_system_acl(
    handle: &mut ReadDiskArchiveHandle,
    acl: *mut c_void,
    entry_type: c_int,
    entry_acl: &mut AclState,
    mode: &mut mode_t,
) {
    let mut entry_ptr = ptr::null_mut();
    let mut status = unsafe { acl_get_entry(acl, ACL_FIRST_ENTRY, &mut entry_ptr) };
    while status == 1 {
        let mut tag = 0;
        if unsafe { acl_get_tag_type(entry_ptr, &mut tag) } != 0 {
            break;
        }
        let Some(ae_tag) = acl_tag_to_entry(tag) else {
            status = unsafe { acl_get_entry(acl, ACL_NEXT_ENTRY, &mut entry_ptr) };
            continue;
        };

        let mut ae_id = -1;
        let ae_name = match tag {
            ACL_USER => {
                let qualifier = unsafe { acl_get_qualifier(entry_ptr) };
                if qualifier.is_null() {
                    None
                } else {
                    ae_id = unsafe { *(qualifier.cast::<libc::uid_t>()) as c_int };
                    let name = resolve_uname(handle, ae_id as i64);
                    unsafe {
                        let _ = acl_free(qualifier);
                    }
                    name
                }
            }
            ACL_GROUP => {
                let qualifier = unsafe { acl_get_qualifier(entry_ptr) };
                if qualifier.is_null() {
                    None
                } else {
                    ae_id = unsafe { *(qualifier.cast::<libc::gid_t>()) as c_int };
                    let name = resolve_gname(handle, ae_id as i64);
                    unsafe {
                        let _ = acl_free(qualifier);
                    }
                    name
                }
            }
            _ => None,
        };

        let mut permset = ptr::null_mut();
        if unsafe { acl_get_permset(entry_ptr, &mut permset) } != 0 {
            break;
        }
        let Ok(ae_perm) = acl_permset_to_permset(permset) else {
            break;
        };
        let _ = entry_acl.add_entry(mode, entry_type, ae_perm, ae_tag, ae_id, ae_name);
        status = unsafe { acl_get_entry(acl, ACL_NEXT_ENTRY, &mut entry_ptr) };
    }
}

fn load_acl(
    handle: &mut ReadDiskArchiveHandle,
    path: &Path,
    is_directory: bool,
    follow: bool,
    entry_acl: &mut AclState,
    mode: &mut mode_t,
) {
    if !follow && !is_directory {
        return;
    }
    let Ok(c_path) = c_path(path) else {
        return;
    };

    let access_acl = unsafe {
        if follow {
            acl_get_file(c_path.as_ptr(), ACL_TYPE_ACCESS)
        } else {
            ptr::null_mut()
        }
    };
    if !access_acl.is_null() {
        add_acl_entries_from_system_acl(
            handle,
            access_acl,
            ARCHIVE_ENTRY_ACL_TYPE_ACCESS,
            entry_acl,
            mode,
        );
        unsafe {
            let _ = acl_free(access_acl);
        }
    }

    if !is_directory {
        return;
    }
    let default_acl = unsafe { acl_get_file(c_path.as_ptr(), ACL_TYPE_DEFAULT) };
    if !default_acl.is_null() {
        add_acl_entries_from_system_acl(
            handle,
            default_acl,
            ARCHIVE_ENTRY_ACL_TYPE_DEFAULT,
            entry_acl,
            mode,
        );
        unsafe {
            let _ = acl_free(default_acl);
        }
    }
}

fn resolve_uname(handle: &mut ReadDiskArchiveHandle, uid: i64) -> Option<String> {
    unsafe {
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
}

fn resolve_gname(handle: &mut ReadDiskArchiveHandle, gid: i64) -> Option<String> {
    unsafe {
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
}

fn populate_entry_from_path(
    handle: &mut ReadDiskArchiveHandle,
    entry: *mut archive_entry,
    display_path: &str,
    filesystem_path: &Path,
    follow_final_symlink: bool,
    ancestor_dirs: &[(u64, u64)],
    provided_stat: Option<stat>,
) -> Result<(PathBuf, stat, bool), c_int> {
    let Some(entry_data) = (unsafe { entry_from_raw(entry) }) else {
        return Err(libc::EINVAL);
    };
    clear_entry(entry_data);
    entry_data.pathname.set(Some(display_path.to_string()));
    entry_data
        .sourcepath
        .set(Some(filesystem_path.to_string_lossy().into_owned()));

    let lstat_info = path_stat(filesystem_path, false)?;
    let mut effective_stat = provided_stat.unwrap_or(lstat_info);
    let mut effective_path = filesystem_path.to_path_buf();
    let mut followed = false;

    if filetype_from_mode(lstat_info.st_mode) == AE_IFLNK && follow_final_symlink {
        if let Ok(target_stat) = path_stat(filesystem_path, true) {
            let candidate =
                fs::canonicalize(filesystem_path).unwrap_or_else(|_| filesystem_path.to_path_buf());
            let candidate_key = (target_stat.st_dev, target_stat.st_ino);
            let is_dir = filetype_from_mode(target_stat.st_mode) == AE_IFDIR;
            if !is_dir || !ancestor_dirs.contains(&candidate_key) {
                effective_stat = target_stat;
                effective_path = candidate;
                followed = true;
            }
        }
    }

    copy_stat(entry_data, &effective_stat);
    entry_data.pathname.set(Some(display_path.to_string()));
    entry_data
        .sourcepath
        .set(Some(filesystem_path.to_string_lossy().into_owned()));
    if filetype_from_mode(lstat_info.st_mode) == AE_IFLNK && !followed {
        entry_data.symlink.set(read_link_text(filesystem_path));
    }

    if (handle.behavior_flags & ARCHIVE_READDISK_NO_XATTR) == 0 {
        entry_data.xattrs.clear();
        for (name, value) in load_xattrs(&effective_path, followed) {
            entry_data
                .xattrs
                .push(crate::entry::internal::XattrEntry { name, value });
        }
    }
    if (handle.behavior_flags & ARCHIVE_READDISK_NO_ACL) == 0 {
        load_acl(
            handle,
            &effective_path,
            filetype_from_mode(effective_stat.st_mode) == AE_IFDIR,
            followed || filetype_from_mode(lstat_info.st_mode) != AE_IFLNK,
            &mut entry_data.acl,
            &mut entry_data.mode,
        );
    }
    if let Some(uname) = resolve_uname(handle, entry_data.uid) {
        entry_data.uname.set(Some(uname));
    }
    if let Some(gname) = resolve_gname(handle, entry_data.gid) {
        entry_data.gname.set(Some(gname));
    }
    entry_data.sparse.clear();
    entry_data.sparse_iter = 0;
    if filetype_from_mode(effective_stat.st_mode) == AE_IFREG
        && (handle.behavior_flags & ARCHIVE_READDISK_NO_SPARSE) == 0
    {
        if let Some(layout) = load_sparse_layout(&effective_path, effective_stat.st_size) {
            for extent in layout.extents {
                add_sparse(entry_data, extent.offset, extent.length);
            }
        }
    }

    Ok((
        effective_path,
        effective_stat,
        filetype_from_mode(effective_stat.st_mode) == AE_IFDIR,
    ))
}

fn push_children(handle: &mut ReadDiskArchiveHandle) -> c_int {
    if !handle.traversal.current_can_descend {
        return ARCHIVE_OK;
    }
    let Some(current_path) = handle.traversal.current_resolved_path.clone() else {
        return ARCHIVE_OK;
    };
    let Some(current) = handle.traversal.current.clone() else {
        return ARCHIVE_OK;
    };
    let mut ancestor_dirs = current.ancestor_dirs.clone();
    if let Some(st) = handle.traversal.current_stat {
        ancestor_dirs.push((st.st_dev, st.st_ino));
    }
    if (handle.behavior_flags & ARCHIVE_READDISK_RESTORE_ATIME) != 0 {
        if let Ok(atime_stat) = path_stat(&current_path, current.follow_final_symlink) {
            handle.traversal.restore_atime = Some(ReadDiskAtimeRestore {
                path: current_path.clone(),
                atime: timespec {
                    tv_sec: atime_stat.st_atime,
                    tv_nsec: 0,
                },
                mtime: timespec {
                    tv_sec: atime_stat.st_mtime,
                    tv_nsec: 0,
                },
                follow_symlink: current.follow_final_symlink,
            });
        }
    }

    let entries = match fs::read_dir(&current_path) {
        Ok(entries) => entries,
        Err(error) => {
            return record_error(
                &mut handle.core,
                error.raw_os_error().unwrap_or(libc::EINVAL),
                format!("failed to read directory {}", current_path.display()),
            );
        }
    };

    let mut children = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            (!name.is_empty()).then(|| {
                (
                    name.to_string(),
                    ReadDiskNode {
                        display_path: join_display_path(&current.display_path, &name),
                        filesystem_path: current_path.join(&*name),
                        follow_final_symlink: should_follow_descendant(handle.symlink_mode),
                        ancestor_dirs: ancestor_dirs.clone(),
                    },
                )
            })
        })
        .collect::<Vec<_>>();
    children.sort_by(|left, right| left.0.cmp(&right.0));
    for (_, child) in children.into_iter().rev() {
        handle.traversal.pending.push(child);
    }
    handle.traversal.current_can_descend = false;
    ARCHIVE_OK
}

pub(crate) fn read_disk_open_path(handle: &mut ReadDiskArchiveHandle, path: &str) -> c_int {
    clear_error(&mut handle.core);
    handle.traversal.pending.clear();
    handle.traversal.current = None;
    handle.traversal.current_resolved_path = None;
    handle.traversal.current_stat = None;
    handle.traversal.restore_atime = None;
    reset_read_data(handle);
    handle.traversal.pending.push(ReadDiskNode {
        display_path: path.to_string(),
        filesystem_path: PathBuf::from(path),
        follow_final_symlink: should_follow_root(handle.symlink_mode),
        ancestor_dirs: Vec::new(),
    });
    handle.backend_opened = true;
    ARCHIVE_OK
}

pub(crate) unsafe fn read_disk_next_header(
    handle: &mut ReadDiskArchiveHandle,
    entry: *mut archive_entry,
) -> c_int {
    if let Some(spec) = handle.traversal.restore_atime.take() {
        restore_atime(&spec);
    }
    reset_read_data(handle);

    while let Some(node) = handle.traversal.pending.pop() {
        if (handle.behavior_flags & ARCHIVE_READDISK_HONOR_NODUMP) != 0
            && is_nodump(&node.filesystem_path)
        {
            continue;
        }

        let (resolved_path, st, can_descend) = match populate_entry_from_path(
            handle,
            entry,
            &node.display_path,
            &node.filesystem_path,
            node.follow_final_symlink,
            &node.ancestor_dirs,
            None,
        ) {
            Ok(result) => result,
            Err(errno) => {
                return record_error(
                    &mut handle.core,
                    errno,
                    format!("failed to stat {}", node.filesystem_path.display()),
                );
            }
        };

        handle.traversal.current = Some(node.clone());
        handle.traversal.current_resolved_path = Some(resolved_path.clone());
        handle.traversal.current_can_descend = can_descend;
        handle.traversal.current_stat = Some(st);
        handle.traversal.current_size = st.st_size;
        if let Some(entry_data) = entry_from_raw(entry) {
            handle.traversal.current_sparse = entry_data.sparse.clone();
            handle.traversal.current_sparse_index = 0;
            handle.traversal.current_fully_sparse = st.st_size > 0
                && filetype_from_mode(st.st_mode) == AE_IFREG
                && (handle.behavior_flags & ARCHIVE_READDISK_NO_SPARSE) == 0
                && handle.traversal.current_sparse.is_empty()
                && load_sparse_layout(&resolved_path, st.st_size)
                    .map(|layout| layout.fully_sparse)
                    .unwrap_or(false);
        }

        if !handle.matching.is_null()
            && crate::r#match::api::archive_match_excluded(handle.matching, entry) != 0
        {
            if let Some(callback) = handle.excluded_cb {
                callback(
                    (handle as *mut ReadDiskArchiveHandle).cast(),
                    handle.excluded_client_data,
                    entry,
                );
            }
            handle.traversal.current = None;
            handle.traversal.current_resolved_path = None;
            handle.traversal.current_can_descend = false;
            handle.traversal.current_stat = None;
            continue;
        }

        if let Some(callback) = handle.metadata_filter_cb {
            let keep = callback(
                (handle as *mut ReadDiskArchiveHandle).cast(),
                handle.metadata_filter_client_data,
                entry,
            );
            if keep == 0 {
                handle.traversal.current = None;
                handle.traversal.current_resolved_path = None;
                handle.traversal.current_can_descend = false;
                handle.traversal.current_stat = None;
                continue;
            }
        }

        return ARCHIVE_OK;
    }

    handle.traversal.current = None;
    handle.traversal.current_resolved_path = None;
    handle.traversal.current_can_descend = false;
    handle.traversal.current_stat = None;
    ARCHIVE_EOF
}

fn ensure_current_data(handle: &mut ReadDiskArchiveHandle) -> Result<(), c_int> {
    if !handle.traversal.current_data.is_empty() || handle.traversal.current_data_eof {
        return Ok(());
    }
    let Some(current) = handle.traversal.current.clone() else {
        return Err(ARCHIVE_FATAL);
    };
    let Some(st) = handle.traversal.current_stat else {
        return Err(ARCHIVE_FATAL);
    };
    if filetype_from_mode(st.st_mode) != AE_IFREG && filetype_from_mode(st.st_mode) != AE_IFIFO {
        handle.traversal.current_data_eof = true;
        handle.traversal.current_data_offset = 0;
        return Ok(());
    }

    let Some(path) = handle.traversal.current_resolved_path.clone() else {
        return Err(ARCHIVE_FATAL);
    };
    if (handle.behavior_flags & ARCHIVE_READDISK_RESTORE_ATIME) != 0 {
        if let Ok(atime_stat) = path_stat(&path, current.follow_final_symlink) {
            handle.traversal.restore_atime = Some(ReadDiskAtimeRestore {
                path: path.clone(),
                atime: timespec {
                    tv_sec: atime_stat.st_atime,
                    tv_nsec: 0,
                },
                mtime: timespec {
                    tv_sec: atime_stat.st_mtime,
                    tv_nsec: 0,
                },
                follow_symlink: current.follow_final_symlink,
            });
        }
    }

    let mut file = File::open(&path).map_err(|error| {
        record_error(
            &mut handle.core,
            error.raw_os_error().unwrap_or(libc::EINVAL),
            format!("failed to open {}", path.display()),
        )
    })?;
    file.read_to_end(&mut handle.traversal.current_data)
        .map_err(|error| {
            record_error(
                &mut handle.core,
                error.raw_os_error().unwrap_or(libc::EINVAL),
                format!("failed to read {}", path.display()),
            )
        })?;
    handle.traversal.current_data_offset = handle.traversal.current_data.len() as i64;
    Ok(())
}

pub(crate) unsafe fn read_disk_data(
    handle: &mut ReadDiskArchiveHandle,
    buffer: *mut c_void,
    size: size_t,
) -> isize {
    if ensure_current_data(handle).is_err() {
        return ARCHIVE_FATAL as isize;
    }
    if handle.traversal.current_data_cursor >= handle.traversal.current_data.len() {
        if let Some(spec) = handle.traversal.restore_atime.take() {
            restore_atime(&spec);
        }
        return 0;
    }
    let remaining = &handle.traversal.current_data[handle.traversal.current_data_cursor..];
    let count = remaining.len().min(size);
    if !buffer.is_null() && count != 0 {
        ptr::copy_nonoverlapping(remaining.as_ptr(), buffer.cast::<u8>(), count);
    }
    handle.traversal.current_data_cursor += count;
    if handle.traversal.current_data_cursor >= handle.traversal.current_data.len() {
        if let Some(spec) = handle.traversal.restore_atime.take() {
            restore_atime(&spec);
        }
    }
    count as isize
}

pub(crate) unsafe fn read_disk_data_block(
    handle: &mut ReadDiskArchiveHandle,
    buffer: *mut *const c_void,
    size: *mut size_t,
    offset: *mut i64,
) -> c_int {
    if ensure_current_data(handle).is_err() {
        return ARCHIVE_FATAL;
    }
    if handle.traversal.current_sparse_index < handle.traversal.current_sparse.len() {
        let extent = handle.traversal.current_sparse[handle.traversal.current_sparse_index];
        handle.traversal.current_sparse_index += 1;
        if !buffer.is_null() {
            *buffer = handle
                .traversal
                .current_data
                .as_ptr()
                .add(extent.offset as usize)
                .cast();
        }
        if !size.is_null() {
            *size = extent.length as usize;
        }
        if !offset.is_null() {
            *offset = extent.offset;
        }
        handle.traversal.current_data_cursor = handle.traversal.current_data.len();
        return ARCHIVE_OK;
    }
    if handle.traversal.current_fully_sparse {
        handle.traversal.current_fully_sparse = false;
        handle.traversal.current_data_cursor = handle.traversal.current_data.len();
        if !buffer.is_null() {
            *buffer = handle.traversal.current_data.as_ptr().cast();
        }
        if !size.is_null() {
            *size = 0;
        }
        if !offset.is_null() {
            *offset = handle.traversal.current_size;
        }
        return ARCHIVE_OK;
    }
    if handle.traversal.current_data_eof
        || handle.traversal.current_data_cursor >= handle.traversal.current_data.len()
    {
        if !size.is_null() {
            *size = 0;
        }
        if !offset.is_null() {
            *offset = handle.traversal.current_data_offset;
        }
        if !buffer.is_null() {
            *buffer = ptr::null();
        }
        if !handle.traversal.current_data_eof {
            handle.traversal.current_data_eof = true;
            if let Some(spec) = handle.traversal.restore_atime.take() {
                restore_atime(&spec);
            }
        }
        return ARCHIVE_EOF;
    }
    if !buffer.is_null() {
        *buffer = handle.traversal.current_data.as_ptr().cast();
    }
    if !size.is_null() {
        *size = handle.traversal.current_data.len();
    }
    if !offset.is_null() {
        *offset = 0;
    }
    handle.traversal.current_data_cursor = handle.traversal.current_data.len();
    ARCHIVE_OK
}

pub(crate) fn read_disk_can_descend(handle: &ReadDiskArchiveHandle) -> c_int {
    i32::from(handle.traversal.current_can_descend)
}

pub(crate) fn read_disk_descend(handle: &mut ReadDiskArchiveHandle) -> c_int {
    push_children(handle)
}

pub(crate) unsafe fn read_disk_entry_from_file(
    handle: &mut ReadDiskArchiveHandle,
    entry: *mut archive_entry,
    fd: c_int,
    st: *const stat,
) -> c_int {
    let Some(entry_data) = entry_from_raw(entry) else {
        return ARCHIVE_FATAL;
    };
    let path = entry_data
        .sourcepath
        .get_str()
        .or_else(|| entry_data.pathname.get_str())
        .map(PathBuf::from);
    let display_path = entry_data
        .pathname
        .get_str()
        .unwrap_or_default()
        .to_string();
    let follow = should_follow_root(handle.symlink_mode);
    let provided_stat = if let Some(st) = st.as_ref() {
        Some(*st)
    } else if fd >= 0 {
        let mut local = std::mem::zeroed::<stat>();
        if libc::fstat(fd, &mut local) == 0 {
            Some(local)
        } else {
            None
        }
    } else {
        None
    };

    if let Some(path) = path {
        match populate_entry_from_path(
            handle,
            entry,
            &display_path,
            &path,
            follow,
            &[],
            provided_stat,
        ) {
            Ok(_) => ARCHIVE_OK,
            Err(errno) => record_error(
                &mut handle.core,
                errno,
                format!("failed to stat {}", path.display()),
            ),
        }
    } else if let Some(st) = provided_stat {
        clear_entry(entry_data);
        copy_stat(entry_data, &st);
        if let Some(uname) = resolve_uname(handle, entry_data.uid) {
            entry_data.uname.set(Some(uname));
        }
        if let Some(gname) = resolve_gname(handle, entry_data.gid) {
            entry_data.gname.set(Some(gname));
        }
        ARCHIVE_OK
    } else {
        record_error(
            &mut handle.core,
            libc::EINVAL,
            "entry_from_file requires a pathname or stat information",
        )
    }
}

pub(crate) fn read_disk_close(handle: &mut ReadDiskArchiveHandle) -> c_int {
    if let Some(spec) = handle.traversal.restore_atime.take() {
        restore_atime(&spec);
    }
    handle.traversal.pending.clear();
    handle.traversal.current = None;
    handle.traversal.current_resolved_path = None;
    handle.traversal.current_stat = None;
    reset_read_data(handle);
    handle.backend_opened = false;
    ARCHIVE_OK
}

fn process_umask() -> mode_t {
    fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|contents| {
            contents
                .lines()
                .find_map(|line| line.strip_prefix("Umask:\t"))
                .and_then(|value| u32::from_str_radix(value.trim(), 8).ok())
        })
        .unwrap_or(0o022) as mode_t
}

fn current_time_pair() -> (i64, i64) {
    let mut ts = timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    unsafe {
        if libc::clock_gettime(libc::CLOCK_REALTIME, &mut ts) == 0 {
            (ts.tv_sec, ts.tv_nsec as i64)
        } else {
            (0, 0)
        }
    }
}

fn has_dotdot(path: &Path) -> bool {
    path.components()
        .any(|component| matches!(component, Component::ParentDir))
}

struct WriteDiskResolvedTarget {
    parent_fd: c_int,
    name: CString,
    display_path: PathBuf,
}

fn close_fd_if_valid(fd: c_int) {
    if fd >= 0 {
        unsafe {
            libc::close(fd);
        }
    }
}

fn dup_fd(fd: c_int) -> Result<c_int, c_int> {
    let duplicated = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 0) };
    if duplicated >= 0 {
        Ok(duplicated)
    } else {
        Err(last_errno())
    }
}

fn open_root_fd(handle: &mut WriteDiskArchiveHandle, absolute: bool) -> Result<c_int, c_int> {
    let slot = if absolute {
        &mut handle.extraction.absolute_root_fd
    } else {
        &mut handle.extraction.cwd_root_fd
    };
    if let Some(fd) = *slot {
        return Ok(fd);
    }
    let base = if absolute { b"/\0" } else { b".\0" };
    let fd = unsafe {
        libc::open(
            base.as_ptr().cast(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        Err(last_errno())
    } else {
        *slot = Some(fd);
        Ok(fd)
    }
}

fn open_dir_at(dir_fd: c_int, name: &CStr, nofollow: bool) -> Result<c_int, c_int> {
    let mut flags = libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC;
    if nofollow {
        flags |= libc::O_NOFOLLOW;
    }
    let fd = unsafe { libc::openat(dir_fd, name.as_ptr(), flags, 0) };
    if fd >= 0 {
        Ok(fd)
    } else {
        Err(last_errno())
    }
}

fn stat_at(dir_fd: c_int, name: &CStr, flags: c_int) -> Result<stat, c_int> {
    let mut st = unsafe { std::mem::zeroed::<stat>() };
    let rc = unsafe { libc::fstatat(dir_fd, name.as_ptr(), &mut st, flags) };
    if rc == 0 {
        Ok(st)
    } else {
        Err(last_errno())
    }
}

fn unlink_entry_at(dir_fd: c_int, name: &CStr, flags: c_int) -> Result<(), c_int> {
    let rc = unsafe { libc::unlinkat(dir_fd, name.as_ptr(), flags) };
    if rc == 0 {
        Ok(())
    } else {
        Err(last_errno())
    }
}

fn remove_entry_at(dir_fd: c_int, name: &CStr) -> Result<(), c_int> {
    let st = match stat_at(dir_fd, name, libc::AT_SYMLINK_NOFOLLOW) {
        Ok(st) => st,
        Err(errno) if errno == libc::ENOENT => return Ok(()),
        Err(errno) => return Err(errno),
    };
    if filetype_from_mode(st.st_mode) != AE_IFDIR {
        return match unlink_entry_at(dir_fd, name, 0) {
            Ok(()) => Ok(()),
            Err(errno) if errno == libc::ENOENT => Ok(()),
            Err(errno) => Err(errno),
        };
    }

    let child_fd = open_dir_at(dir_fd, name, true)?;
    let iter_fd = dup_fd(child_fd)?;
    let dir = unsafe { libc::fdopendir(iter_fd) };
    if dir.is_null() {
        close_fd_if_valid(iter_fd);
        close_fd_if_valid(child_fd);
        return Err(last_errno());
    }

    let mut walk_error = None;
    loop {
        unsafe {
            *libc::__errno_location() = 0;
        }
        let entry = unsafe { libc::readdir(dir) };
        if entry.is_null() {
            let errno = last_errno();
            if errno != 0 {
                walk_error = Some(errno);
            }
            break;
        }
        let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) };
        let bytes = name.to_bytes();
        if bytes == b"." || bytes == b".." {
            continue;
        }
        if let Err(errno) = remove_entry_at(child_fd, name) {
            walk_error = Some(errno);
            break;
        }
    }

    unsafe {
        libc::closedir(dir);
    }
    close_fd_if_valid(child_fd);

    if let Some(errno) = walk_error {
        return Err(errno);
    }
    unlink_entry_at(dir_fd, name, libc::AT_REMOVEDIR)
}

fn advance_parent_fd(
    handle: &mut WriteDiskArchiveHandle,
    current_fd: c_int,
    component: &CStr,
    create_intermediate: bool,
    display_path: &Path,
) -> Result<c_int, c_int> {
    let nofollow = (handle.options & ARCHIVE_EXTRACT_SECURE_SYMLINKS) != 0;
    match open_dir_at(current_fd, component, nofollow) {
        Ok(fd) => Ok(fd),
        Err(errno) if errno == libc::ENOENT => {
            if !create_intermediate || (handle.options & ARCHIVE_EXTRACT_NO_AUTODIR) != 0 {
                Err(record_error(
                    &mut handle.core,
                    errno,
                    format!("missing parent directory for {}", display_path.display()),
                ))
            } else {
                let mode = 0o777 & !process_umask();
                let created = unsafe { libc::mkdirat(current_fd, component.as_ptr(), mode) };
                if created != 0 {
                    let create_errno = last_errno();
                    if create_errno != libc::EEXIST {
                        return Err(record_error(
                            &mut handle.core,
                            create_errno,
                            format!("failed to create directory for {}", display_path.display()),
                        ));
                    }
                }
                open_dir_at(current_fd, component, true).map_err(|open_errno| {
                    record_error(
                        &mut handle.core,
                        open_errno,
                        format!("failed to open directory for {}", display_path.display()),
                    )
                })
            }
        }
        Err(errno) if errno == libc::ELOOP && (handle.options & ARCHIVE_EXTRACT_UNLINK) == 0 => {
            Err(record_error(
                &mut handle.core,
                errno,
                format!(
                    "path traverses an existing symlink: {}",
                    display_path.display()
                ),
            ))
        }
        Err(errno)
            if create_intermediate
                && (handle.options & ARCHIVE_EXTRACT_UNLINK) != 0
                && matches!(errno, libc::ELOOP | libc::ENOTDIR | libc::ENOENT) =>
        {
            remove_entry_at(current_fd, component).map_err(|remove_errno| {
                record_error(
                    &mut handle.core,
                    remove_errno,
                    format!(
                        "failed to replace path component for {}",
                        display_path.display()
                    ),
                )
            })?;
            let mode = 0o777 & !process_umask();
            if unsafe { libc::mkdirat(current_fd, component.as_ptr(), mode) } != 0 {
                return Err(record_error(
                    &mut handle.core,
                    last_errno(),
                    format!("failed to create directory for {}", display_path.display()),
                ));
            }
            open_dir_at(current_fd, component, true).map_err(|open_errno| {
                record_error(
                    &mut handle.core,
                    open_errno,
                    format!("failed to open directory for {}", display_path.display()),
                )
            })
        }
        Err(errno) => Err(record_error(
            &mut handle.core,
            errno,
            format!(
                "path component is not a directory: {}",
                display_path.display()
            ),
        )),
    }
}

fn resolve_write_target(
    handle: &mut WriteDiskArchiveHandle,
    raw_path: &Path,
    create_intermediate: bool,
) -> Result<WriteDiskResolvedTarget, c_int> {
    if (handle.options & ARCHIVE_EXTRACT_SECURE_NOABSOLUTEPATHS) != 0 && raw_path.is_absolute() {
        return Err(record_error(
            &mut handle.core,
            libc::EINVAL,
            "absolute paths are not permitted",
        ));
    }
    if (handle.options & ARCHIVE_EXTRACT_SECURE_NODOTDOT) != 0 && has_dotdot(raw_path) {
        return Err(record_error(
            &mut handle.core,
            libc::EINVAL,
            "path contains '..' and secure nodotdot is enabled",
        ));
    }

    let Some(file_name) = raw_path.file_name() else {
        return Err(record_error(
            &mut handle.core,
            libc::EINVAL,
            format!("entry path has no final component: {}", raw_path.display()),
        ));
    };
    let name = CString::new(file_name.as_bytes()).map_err(|_| {
        record_error(
            &mut handle.core,
            libc::EINVAL,
            format!("entry path contains NUL bytes: {}", raw_path.display()),
        )
    })?;

    let base_fd = open_root_fd(handle, raw_path.is_absolute()).map_err(|errno| {
        record_error(
            &mut handle.core,
            errno,
            format!("failed to open extraction root for {}", raw_path.display()),
        )
    })?;
    let mut current_fd = dup_fd(base_fd).map_err(|errno| {
        record_error(
            &mut handle.core,
            errno,
            format!(
                "failed to duplicate extraction root for {}",
                raw_path.display()
            ),
        )
    })?;

    let mut components = raw_path.components().peekable();
    while let Some(component) = components.next() {
        match component {
            Component::RootDir => {
                close_fd_if_valid(current_fd);
                current_fd = dup_fd(base_fd).map_err(|errno| {
                    record_error(
                        &mut handle.core,
                        errno,
                        format!(
                            "failed to duplicate extraction root for {}",
                            raw_path.display()
                        ),
                    )
                })?;
            }
            Component::CurDir => {}
            Component::ParentDir => {
                let parent = unsafe {
                    libc::openat(
                        current_fd,
                        b"..\0".as_ptr().cast(),
                        libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
                        0,
                    )
                };
                if parent < 0 {
                    close_fd_if_valid(current_fd);
                    return Err(record_error(
                        &mut handle.core,
                        last_errno(),
                        format!(
                            "failed to traverse parent directory for {}",
                            raw_path.display()
                        ),
                    ));
                }
                close_fd_if_valid(current_fd);
                current_fd = parent;
            }
            Component::Normal(component_name) => {
                if components.peek().is_none() {
                    break;
                }
                let component = CString::new(component_name.as_bytes()).map_err(|_| {
                    close_fd_if_valid(current_fd);
                    record_error(
                        &mut handle.core,
                        libc::EINVAL,
                        format!("entry path contains NUL bytes: {}", raw_path.display()),
                    )
                })?;
                let next_fd = advance_parent_fd(
                    handle,
                    current_fd,
                    component.as_c_str(),
                    create_intermediate,
                    raw_path,
                )?;
                close_fd_if_valid(current_fd);
                current_fd = next_fd;
            }
            Component::Prefix(_) => {}
        }
    }

    Ok(WriteDiskResolvedTarget {
        parent_fd: current_fd,
        name,
        display_path: raw_path.to_path_buf(),
    })
}

fn entry_uid(
    handle: &WriteDiskArchiveHandle,
    entry: &crate::entry::internal::ArchiveEntryData,
) -> i64 {
    if let Some(lookup) = handle.user_lookup {
        if let Some(name) = entry.uname.get_str() {
            if let Ok(name) = CString::new(name) {
                return unsafe {
                    lookup(handle.user_lookup_private_data, name.as_ptr(), entry.uid)
                };
            }
        }
    }
    if handle.use_standard_lookup {
        if let Some(name) = entry.uname.get_str() {
            if let Ok(name) = CString::new(name) {
                unsafe {
                    let user = libc::getpwnam(name.as_ptr());
                    if !user.is_null() {
                        return (*user).pw_uid as i64;
                    }
                }
            }
        }
    }
    entry.uid
}

fn entry_gid(
    handle: &WriteDiskArchiveHandle,
    entry: &crate::entry::internal::ArchiveEntryData,
) -> i64 {
    if let Some(lookup) = handle.group_lookup {
        if let Some(name) = entry.gname.get_str() {
            if let Ok(name) = CString::new(name) {
                return unsafe {
                    lookup(handle.group_lookup_private_data, name.as_ptr(), entry.gid)
                };
            }
        }
    }
    if handle.use_standard_lookup {
        if let Some(name) = entry.gname.get_str() {
            if let Ok(name) = CString::new(name) {
                unsafe {
                    let group = libc::getgrnam(name.as_ptr());
                    if !group.is_null() {
                        return (*group).gr_gid as i64;
                    }
                }
            }
        }
    }
    entry.gid
}

fn lookup_write_uid(handle: &WriteDiskArchiveHandle, name: Option<&str>, uid: i64) -> i64 {
    let Some(name) = name.filter(|name| !name.is_empty()) else {
        return uid;
    };
    if let Some(lookup) = handle.user_lookup {
        if let Ok(name) = CString::new(name) {
            return unsafe { lookup(handle.user_lookup_private_data, name.as_ptr(), uid) };
        }
    }
    if handle.use_standard_lookup {
        if let Ok(name) = CString::new(name) {
            unsafe {
                let user = libc::getpwnam(name.as_ptr());
                if !user.is_null() {
                    return (*user).pw_uid as i64;
                }
            }
        }
    }
    uid
}

fn lookup_write_gid(handle: &WriteDiskArchiveHandle, name: Option<&str>, gid: i64) -> i64 {
    let Some(name) = name.filter(|name| !name.is_empty()) else {
        return gid;
    };
    if let Some(lookup) = handle.group_lookup {
        if let Ok(name) = CString::new(name) {
            return unsafe { lookup(handle.group_lookup_private_data, name.as_ptr(), gid) };
        }
    }
    if handle.use_standard_lookup {
        if let Ok(name) = CString::new(name) {
            unsafe {
                let group = libc::getgrnam(name.as_ptr());
                if !group.is_null() {
                    return (*group).gr_gid as i64;
                }
            }
        }
    }
    gid
}

fn proc_fd_path(fd: c_int) -> CString {
    CString::new(format!("/proc/self/fd/{fd}")).expect("proc fd path")
}

fn build_posix_acl(
    handle: &WriteDiskArchiveHandle,
    acl_state: &AclState,
    mode: mode_t,
    want_type: c_int,
) -> Result<*mut c_void, c_int> {
    let mut acl_state = acl_state.clone();
    let entry_count = acl_state.count(want_type);
    if entry_count == 0 {
        return Ok(ptr::null_mut());
    }

    let mut acl = unsafe { acl_init(entry_count) };
    if acl.is_null() {
        return Err(last_errno());
    }

    let iter_count = acl_state.reset(mode, want_type);
    if iter_count <= 0 {
        unsafe {
            let _ = acl_free(acl);
        }
        return Ok(ptr::null_mut());
    }

    loop {
        let mut entry_type = 0;
        let mut permset = 0;
        let mut tag = 0;
        let mut qual = -1;
        let mut name = ptr::null();
        let status = unsafe {
            acl_state.next(
                &mut entry_type,
                &mut permset,
                &mut tag,
                &mut qual,
                &mut name,
            )
        };
        if status == ARCHIVE_EOF {
            break;
        }
        if status != ARCHIVE_OK {
            unsafe {
                let _ = acl_free(acl);
            }
            return Err(libc::EINVAL);
        }

        let Some(tag_type) = acl_entry_to_tag(tag) else {
            unsafe {
                let _ = acl_free(acl);
            }
            return Err(libc::EINVAL);
        };

        let mut acl_entry = ptr::null_mut();
        if unsafe { acl_create_entry(&mut acl, &mut acl_entry) } != 0 {
            let errno = last_errno();
            unsafe {
                let _ = acl_free(acl);
            }
            return Err(errno);
        }
        if unsafe { acl_set_tag_type(acl_entry, tag_type) } != 0 {
            let errno = last_errno();
            unsafe {
                let _ = acl_free(acl);
            }
            return Err(errno);
        }

        match tag {
            ARCHIVE_ENTRY_ACL_USER => {
                let resolved =
                    lookup_write_uid(handle, from_optional_c_str(name).as_deref(), qual as i64)
                        as libc::uid_t;
                if unsafe { acl_set_qualifier(acl_entry, ptr::addr_of!(resolved).cast::<c_void>()) }
                    != 0
                {
                    let errno = last_errno();
                    unsafe {
                        let _ = acl_free(acl);
                    }
                    return Err(errno);
                }
            }
            ARCHIVE_ENTRY_ACL_GROUP => {
                let resolved =
                    lookup_write_gid(handle, from_optional_c_str(name).as_deref(), qual as i64)
                        as libc::gid_t;
                if unsafe { acl_set_qualifier(acl_entry, ptr::addr_of!(resolved).cast::<c_void>()) }
                    != 0
                {
                    let errno = last_errno();
                    unsafe {
                        let _ = acl_free(acl);
                    }
                    return Err(errno);
                }
            }
            _ => {}
        }

        let mut acl_permset = ptr::null_mut();
        if unsafe { acl_get_permset(acl_entry, &mut acl_permset) } != 0 {
            let errno = last_errno();
            unsafe {
                let _ = acl_free(acl);
            }
            return Err(errno);
        }
        if unsafe { acl_clear_perms(acl_permset) } != 0 {
            let errno = last_errno();
            unsafe {
                let _ = acl_free(acl);
            }
            return Err(errno);
        }
        for (archive_perm, acl_perm) in [
            (ARCHIVE_ENTRY_ACL_READ, ACL_READ),
            (ARCHIVE_ENTRY_ACL_WRITE, ACL_WRITE),
            (ARCHIVE_ENTRY_ACL_EXECUTE, ACL_EXECUTE),
        ] {
            if (permset & archive_perm) != 0 && unsafe { acl_add_perm(acl_permset, acl_perm) } != 0
            {
                let errno = last_errno();
                unsafe {
                    let _ = acl_free(acl);
                }
                return Err(errno);
            }
        }
        let _ = entry_type;
    }

    Ok(acl)
}

fn apply_acl_fixup(handle: &mut WriteDiskArchiveHandle, fixup: &WriteDiskPendingFixup) -> c_int {
    if (handle.options & ARCHIVE_EXTRACT_ACL) == 0 || fixup.acl.types() == 0 {
        return ARCHIVE_OK;
    }
    if filetype_from_mode(fixup.mode) == AE_IFLNK {
        return ARCHIVE_OK;
    }

    let mut status = ARCHIVE_OK;
    for want_type in [
        ARCHIVE_ENTRY_ACL_TYPE_ACCESS,
        ARCHIVE_ENTRY_ACL_TYPE_DEFAULT,
    ] {
        if want_type == ARCHIVE_ENTRY_ACL_TYPE_DEFAULT && filetype_from_mode(fixup.mode) != AE_IFDIR
        {
            continue;
        }
        let acl = match build_posix_acl(handle, &fixup.acl, fixup.mode, want_type) {
            Ok(acl) => acl,
            Err(errno) => {
                set_error_string(
                    &mut handle.core,
                    errno,
                    format!("failed to build ACL for {}", fixup.display_path.display()),
                );
                return ARCHIVE_WARN;
            }
        };
        if acl.is_null() {
            continue;
        }

        let rc = unsafe {
            if want_type == ARCHIVE_ENTRY_ACL_TYPE_ACCESS {
                if fixup.target_fd >= 0 {
                    acl_set_fd(fixup.target_fd, acl)
                } else {
                    -1
                }
            } else if fixup.target_fd >= 0 {
                acl_set_file(
                    proc_fd_path(fixup.target_fd).as_ptr(),
                    ACL_TYPE_DEFAULT,
                    acl,
                )
            } else {
                -1
            }
        };
        let errno = if rc == 0 { 0 } else { last_errno() };
        unsafe {
            let _ = acl_free(acl);
        }
        if rc != 0 && errno != libc::EOPNOTSUPP {
            set_error_string(
                &mut handle.core,
                errno,
                format!("failed to set ACL on {}", fixup.display_path.display()),
            );
            status = ARCHIVE_WARN;
        }
    }
    status
}

fn owner_mismatch(target_uid: i64, target_gid: i64) -> (bool, bool) {
    let current_uid = unsafe { libc::geteuid() as i64 };
    let current_gid = unsafe { libc::getegid() as i64 };
    (target_uid != current_uid, target_gid != current_gid)
}

fn desired_file_mode(handle: &WriteDiskArchiveHandle, mode: mode_t, uid: i64, gid: i64) -> mode_t {
    let mut result = mode;
    if (handle.options & ARCHIVE_EXTRACT_PERM) == 0 {
        result &= !process_umask();
        result &= !(libc::S_ISUID | libc::S_ISGID);
        return result;
    }
    let (uid_mismatch, gid_mismatch) = owner_mismatch(uid, gid);
    if uid_mismatch && (handle.options & ARCHIVE_EXTRACT_OWNER) == 0 {
        result &= !libc::S_ISUID;
    }
    if gid_mismatch && (handle.options & ARCHIVE_EXTRACT_OWNER) == 0 {
        result &= !libc::S_ISGID;
    }
    result
}

fn make_fixup(
    handle: &WriteDiskArchiveHandle,
    display_path: PathBuf,
    entry: &crate::entry::internal::ArchiveEntryData,
    target_fd: c_int,
    parent_fd: c_int,
    name: Option<CString>,
    follow: bool,
) -> WriteDiskPendingFixup {
    let uid = entry_uid(handle, entry);
    let gid = entry_gid(handle, entry);
    let apply_time = (handle.options & ARCHIVE_EXTRACT_TIME) != 0;
    let now = apply_time.then(current_time_pair);
    WriteDiskPendingFixup {
        display_path,
        mode: desired_file_mode(handle, entry.mode, uid, gid),
        uid,
        gid,
        atime: if entry.atime.set {
            Some((entry.atime.sec, entry.atime.nsec))
        } else {
            now
        },
        mtime: if entry.mtime.set {
            Some((entry.mtime.sec, entry.mtime.nsec))
        } else {
            now
        },
        apply_perm: (handle.options & ARCHIVE_EXTRACT_PERM) != 0
            || filetype_from_mode(entry.mode) == AE_IFDIR,
        apply_owner: (handle.options & ARCHIVE_EXTRACT_OWNER) != 0,
        apply_time,
        acl: entry.acl.clone(),
        xattrs: entry
            .xattrs
            .iter()
            .map(|xattr| (xattr.name.clone(), xattr.value.clone()))
            .collect(),
        target_fd,
        parent_fd,
        name,
        follow,
    }
}

fn close_fixup(fixup: &mut WriteDiskPendingFixup) {
    if fixup.target_fd >= 0 {
        close_fd_if_valid(fixup.target_fd);
        fixup.target_fd = -1;
    }
    if fixup.parent_fd >= 0 {
        close_fd_if_valid(fixup.parent_fd);
        fixup.parent_fd = -1;
    }
}

fn apply_fixup(handle: &mut WriteDiskArchiveHandle, mut fixup: WriteDiskPendingFixup) -> c_int {
    let mut status = ARCHIVE_OK;
    let mut mode = fixup.mode;
    if fixup.apply_owner {
        let rc = unsafe {
            if fixup.target_fd >= 0 && fixup.follow {
                libc::fchown(fixup.target_fd, fixup.uid as _, fixup.gid as _)
            } else if let Some(name) = fixup.name.as_ref() {
                libc::fchownat(
                    fixup.parent_fd,
                    name.as_ptr(),
                    fixup.uid as _,
                    fixup.gid as _,
                    libc::AT_SYMLINK_NOFOLLOW,
                )
            } else {
                -1
            }
        };
        if rc != 0 {
            status = ARCHIVE_WARN;
            mode &= !(libc::S_ISUID | libc::S_ISGID);
        }
    }
    if fixup.apply_perm {
        let rc = unsafe {
            if fixup.target_fd >= 0 && fixup.follow {
                libc::fchmod(fixup.target_fd, mode & 0o7777)
            } else {
                -1
            }
        };
        if rc != 0 {
            status = ARCHIVE_WARN;
        }
    }
    if fixup.apply_time {
        let atime = fixup.atime.unwrap_or((0, 0));
        let mtime = fixup.mtime.unwrap_or((0, 0));
        let times = [
            timespec {
                tv_sec: atime.0,
                tv_nsec: atime.1 as _,
            },
            timespec {
                tv_sec: mtime.0,
                tv_nsec: mtime.1 as _,
            },
        ];
        let rc = unsafe {
            if fixup.target_fd >= 0 && fixup.follow {
                libc::futimens(fixup.target_fd, times.as_ptr())
            } else if let Some(name) = fixup.name.as_ref() {
                libc::utimensat(
                    fixup.parent_fd,
                    name.as_ptr(),
                    times.as_ptr(),
                    libc::AT_SYMLINK_NOFOLLOW,
                )
            } else {
                -1
            }
        };
        if rc != 0 {
            status = ARCHIVE_WARN;
        }
    }
    if (handle.options & ARCHIVE_EXTRACT_XATTR) != 0 {
        for (name, value) in &fixup.xattrs {
            let rc = unsafe {
                if fixup.target_fd >= 0 {
                    libc::fsetxattr(
                        fixup.target_fd,
                        name.as_ptr(),
                        value.as_ptr().cast(),
                        value.len(),
                        0,
                    )
                } else {
                    -1
                }
            };
            if rc != 0 {
                status = ARCHIVE_WARN;
            }
        }
    }
    if apply_acl_fixup(handle, &fixup) != ARCHIVE_OK {
        status = ARCHIVE_WARN;
    }
    close_fixup(&mut fixup);
    status
}

fn close_current_state(current: &mut WriteDiskCurrentState) {
    if current.close_fd_on_finish && current.fd >= 0 {
        close_fd_if_valid(current.fd);
        current.fd = -1;
    }
    if current.current_parent_fd >= 0 {
        close_fd_if_valid(current.current_parent_fd);
        current.current_parent_fd = -1;
    }
    if let Some(fd) = current.final_parent_fd.take() {
        close_fd_if_valid(fd);
    }
}

fn skip_current_state(display_path: PathBuf) -> WriteDiskCurrentState {
    WriteDiskCurrentState {
        display_path,
        current_parent_fd: -1,
        current_name: None,
        final_parent_fd: None,
        final_name: None,
        fd: -1,
        size_limit: Some(0),
        written: 0,
        accept_data: false,
        close_fd_on_finish: false,
        fixup: None,
    }
}

fn target_exists(parent_fd: c_int, name: &CStr) -> bool {
    stat_at(parent_fd, name, libc::AT_SYMLINK_NOFOLLOW).is_ok()
}

fn open_created_file_at(parent_fd: c_int, name: &CStr, mode: mode_t) -> Result<c_int, c_int> {
    let fd = unsafe {
        libc::openat(
            parent_fd,
            name.as_ptr(),
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_CLOEXEC | libc::O_NOFOLLOW,
            mode,
        )
    };
    if fd >= 0 {
        Ok(fd)
    } else {
        Err(last_errno())
    }
}

fn next_safe_temp_name(handle: &mut WriteDiskArchiveHandle, name: &CStr) -> Result<CString, c_int> {
    handle.extraction.temp_counter = handle.extraction.temp_counter.wrapping_add(1);
    CString::new(format!(
        ".{}.safe-tmp-{}",
        String::from_utf8_lossy(name.to_bytes()),
        handle.extraction.temp_counter
    ))
    .map_err(|_| libc::EINVAL)
}

fn prepare_directory_fixup(
    handle: &mut WriteDiskArchiveHandle,
    target: &WriteDiskResolvedTarget,
    entry_data: &crate::entry::internal::ArchiveEntryData,
) -> Result<WriteDiskPendingFixup, c_int> {
    match stat_at(
        target.parent_fd,
        target.name.as_c_str(),
        libc::AT_SYMLINK_NOFOLLOW,
    ) {
        Ok(st) if filetype_from_mode(st.st_mode) == AE_IFDIR => {
            let fd =
                open_dir_at(target.parent_fd, target.name.as_c_str(), true).map_err(|errno| {
                    record_error(
                        &mut handle.core,
                        errno,
                        format!("failed to open directory {}", target.display_path.display()),
                    )
                })?;
            let mut fixup = make_fixup(
                handle,
                target.display_path.clone(),
                entry_data,
                fd,
                -1,
                None,
                true,
            );
            fixup.apply_perm = (handle.options & ARCHIVE_EXTRACT_PERM) != 0;
            fixup.apply_owner = false;
            Ok(fixup)
        }
        Ok(st) if filetype_from_mode(st.st_mode) == AE_IFLNK => {
            if (handle.options & ARCHIVE_EXTRACT_SECURE_SYMLINKS) == 0 {
                if let Ok(fd) = open_dir_at(target.parent_fd, target.name.as_c_str(), false) {
                    let mut fixup = make_fixup(
                        handle,
                        target.display_path.clone(),
                        entry_data,
                        fd,
                        -1,
                        None,
                        true,
                    );
                    fixup.apply_perm = (handle.options & ARCHIVE_EXTRACT_PERM) != 0;
                    fixup.apply_owner = false;
                    return Ok(fixup);
                }
            }
            remove_entry_at(target.parent_fd, target.name.as_c_str()).map_err(|errno| {
                record_error(
                    &mut handle.core,
                    errno,
                    format!("failed to replace {}", target.display_path.display()),
                )
            })?;
            let mode = 0o777 & !process_umask();
            if unsafe { libc::mkdirat(target.parent_fd, target.name.as_ptr(), mode) } != 0 {
                return Err(record_error(
                    &mut handle.core,
                    last_errno(),
                    format!(
                        "failed to create directory {}",
                        target.display_path.display()
                    ),
                ));
            }
            let fd =
                open_dir_at(target.parent_fd, target.name.as_c_str(), true).map_err(|errno| {
                    record_error(
                        &mut handle.core,
                        errno,
                        format!("failed to open directory {}", target.display_path.display()),
                    )
                })?;
            Ok(make_fixup(
                handle,
                target.display_path.clone(),
                entry_data,
                fd,
                -1,
                None,
                true,
            ))
        }
        Ok(_) => {
            remove_entry_at(target.parent_fd, target.name.as_c_str()).map_err(|errno| {
                record_error(
                    &mut handle.core,
                    errno,
                    format!("failed to replace {}", target.display_path.display()),
                )
            })?;
            let mode = 0o777 & !process_umask();
            if unsafe { libc::mkdirat(target.parent_fd, target.name.as_ptr(), mode) } != 0 {
                return Err(record_error(
                    &mut handle.core,
                    last_errno(),
                    format!(
                        "failed to create directory {}",
                        target.display_path.display()
                    ),
                ));
            }
            let fd =
                open_dir_at(target.parent_fd, target.name.as_c_str(), true).map_err(|errno| {
                    record_error(
                        &mut handle.core,
                        errno,
                        format!("failed to open directory {}", target.display_path.display()),
                    )
                })?;
            Ok(make_fixup(
                handle,
                target.display_path.clone(),
                entry_data,
                fd,
                -1,
                None,
                true,
            ))
        }
        Err(errno) if errno == libc::ENOENT => {
            let mode = 0o777 & !process_umask();
            if unsafe { libc::mkdirat(target.parent_fd, target.name.as_ptr(), mode) } != 0 {
                return Err(record_error(
                    &mut handle.core,
                    last_errno(),
                    format!(
                        "failed to create directory {}",
                        target.display_path.display()
                    ),
                ));
            }
            let fd =
                open_dir_at(target.parent_fd, target.name.as_c_str(), true).map_err(|errno| {
                    record_error(
                        &mut handle.core,
                        errno,
                        format!("failed to open directory {}", target.display_path.display()),
                    )
                })?;
            Ok(make_fixup(
                handle,
                target.display_path.clone(),
                entry_data,
                fd,
                -1,
                None,
                true,
            ))
        }
        Err(errno) => Err(record_error(
            &mut handle.core,
            errno,
            format!("failed to inspect {}", target.display_path.display()),
        )),
    }
}

pub(crate) unsafe fn write_disk_header(
    handle: &mut WriteDiskArchiveHandle,
    entry: *mut archive_entry,
) -> c_int {
    clear_error(&mut handle.core);
    handle.extraction.last_header_failed = false;
    let Some(entry_data) = entry_from_raw(entry) else {
        return ARCHIVE_FATAL;
    };
    let Some(pathname) = entry_data.pathname.get_str() else {
        return record_error(&mut handle.core, libc::EINVAL, "entry pathname is missing");
    };
    let raw_path = Path::new(pathname);
    let target = match resolve_write_target(handle, raw_path, true) {
        Ok(target) => target,
        Err(status) => {
            handle.extraction.last_header_failed = true;
            return status;
        }
    };

    if let Some((skip_dev, skip_ino)) = handle.skip_file {
        if let Ok(st) = stat_at(
            target.parent_fd,
            target.name.as_c_str(),
            libc::AT_SYMLINK_NOFOLLOW,
        ) {
            if st.st_dev == skip_dev as _ && st.st_ino == skip_ino as _ {
                close_fd_if_valid(target.parent_fd);
                handle.extraction.last_header_failed = true;
                return record_error(
                    &mut handle.core,
                    libc::EEXIST,
                    "refusing to overwrite skipped file",
                );
            }
        }
    }

    let filetype = filetype_from_mode(entry_data.mode);
    if (handle.options & ARCHIVE_EXTRACT_NO_OVERWRITE) != 0
        && target_exists(target.parent_fd, target.name.as_c_str())
    {
        handle.extraction.current = Some(skip_current_state(target.display_path.clone()));
        close_fd_if_valid(target.parent_fd);
        return ARCHIVE_OK;
    }
    if (handle.options & ARCHIVE_EXTRACT_NO_OVERWRITE_NEWER) != 0 {
        if let (Ok(existing), true) = (
            stat_at(target.parent_fd, target.name.as_c_str(), 0),
            entry_data.mtime.set,
        ) {
            if existing.st_mtime > entry_data.mtime.sec {
                handle.extraction.current = Some(skip_current_state(target.display_path.clone()));
                close_fd_if_valid(target.parent_fd);
                return ARCHIVE_OK;
            }
        }
    }

    if filetype == AE_IFDIR {
        let fixup = match prepare_directory_fixup(handle, &target, entry_data) {
            Ok(fixup) => fixup,
            Err(status) => {
                close_fd_if_valid(target.parent_fd);
                handle.extraction.last_header_failed = true;
                return status;
            }
        };
        close_fd_if_valid(target.parent_fd);
        handle.extraction.current = Some(WriteDiskCurrentState {
            display_path: target.display_path.clone(),
            current_parent_fd: -1,
            current_name: None,
            final_parent_fd: None,
            final_name: None,
            fd: -1,
            size_limit: None,
            written: 0,
            accept_data: false,
            close_fd_on_finish: false,
            fixup: Some(fixup),
        });
        return ARCHIVE_OK;
    }

    if filetype == AE_IFLNK {
        let link_target = match entry_data.symlink.get_str() {
            Some(target_value) => CString::new(target_value).map_err(|_| {
                record_error(
                    &mut handle.core,
                    libc::EINVAL,
                    format!("invalid symlink target {}", target.display_path.display()),
                )
            }),
            None => Err(record_error(
                &mut handle.core,
                libc::EINVAL,
                format!(
                    "symlink target is missing for {}",
                    target.display_path.display()
                ),
            )),
        };
        let Ok(link_target) = link_target else {
            close_fd_if_valid(target.parent_fd);
            handle.extraction.last_header_failed = true;
            return ARCHIVE_FAILED;
        };
        if target_exists(target.parent_fd, target.name.as_c_str()) {
            let _ = remove_entry_at(target.parent_fd, target.name.as_c_str());
        }
        if libc::symlinkat(link_target.as_ptr(), target.parent_fd, target.name.as_ptr()) != 0 {
            close_fd_if_valid(target.parent_fd);
            handle.extraction.last_header_failed = true;
            return record_error(
                &mut handle.core,
                last_errno(),
                format!("failed to create symlink {}", target.display_path.display()),
            );
        }
        handle.extraction.current = Some(WriteDiskCurrentState {
            display_path: target.display_path.clone(),
            current_parent_fd: -1,
            current_name: None,
            final_parent_fd: None,
            final_name: None,
            fd: -1,
            size_limit: None,
            written: 0,
            accept_data: false,
            close_fd_on_finish: false,
            fixup: Some(make_fixup(
                handle,
                target.display_path.clone(),
                entry_data,
                -1,
                target.parent_fd,
                Some(target.name.clone()),
                false,
            )),
        });
        return ARCHIVE_OK;
    }

    if let Some(link_target) = entry_data.hardlink.get_str() {
        let hardlink_target = match resolve_write_target(handle, Path::new(link_target), false) {
            Ok(target_value) => target_value,
            Err(status) => {
                close_fd_if_valid(target.parent_fd);
                handle.extraction.last_header_failed = true;
                return status;
            }
        };
        if target_exists(target.parent_fd, target.name.as_c_str()) {
            let _ = remove_entry_at(target.parent_fd, target.name.as_c_str());
        }
        if libc::linkat(
            hardlink_target.parent_fd,
            hardlink_target.name.as_ptr(),
            target.parent_fd,
            target.name.as_ptr(),
            0,
        ) != 0
        {
            close_fd_if_valid(hardlink_target.parent_fd);
            close_fd_if_valid(target.parent_fd);
            handle.extraction.last_header_failed = true;
            let errno = last_errno();
            if errno == libc::ENOENT {
                return record_error(
                    &mut handle.core,
                    errno,
                    format!("Hard-link target '{}' does not exist.", link_target),
                );
            }
            return record_error(
                &mut handle.core,
                errno,
                format!(
                    "failed to create hardlink {}",
                    target.display_path.display()
                ),
            );
        }
        close_fd_if_valid(hardlink_target.parent_fd);

        let authoritative = entry_data.size_set && entry_data.size > 0;
        let fd = if authoritative {
            let fd = libc::openat(
                target.parent_fd,
                target.name.as_ptr(),
                libc::O_WRONLY | libc::O_TRUNC | libc::O_CLOEXEC | libc::O_NOFOLLOW,
                0,
            );
            if fd < 0 {
                close_fd_if_valid(target.parent_fd);
                handle.extraction.last_header_failed = true;
                return record_error(
                    &mut handle.core,
                    last_errno(),
                    format!("failed to open {}", target.display_path.display()),
                );
            }
            fd
        } else {
            -1
        };
        if !authoritative {
            close_fd_if_valid(target.parent_fd);
        }
        handle.extraction.current = Some(WriteDiskCurrentState {
            display_path: target.display_path.clone(),
            current_parent_fd: -1,
            current_name: None,
            final_parent_fd: None,
            final_name: None,
            fd,
            size_limit: entry_data.size_set.then_some(entry_data.size),
            written: 0,
            accept_data: authoritative,
            close_fd_on_finish: fd >= 0,
            fixup: authoritative.then(|| {
                let mut fixup = make_fixup(
                    handle,
                    target.display_path.clone(),
                    entry_data,
                    fd,
                    -1,
                    None,
                    true,
                );
                fixup.apply_perm = true;
                fixup
            }),
        });
        return ARCHIVE_OK;
    }

    let use_safe_writes = (handle.options & ARCHIVE_EXTRACT_SAFE_WRITES) != 0;
    let create_mode = (desired_file_mode(
        handle,
        entry_data.mode,
        entry_uid(handle, entry_data),
        entry_gid(handle, entry_data),
    ) & 0o777) as mode_t;
    let (fd, current_name, final_name) = if use_safe_writes {
        let mut opened = None;
        let mut temp_name = None;
        for _ in 0..128 {
            let candidate = match next_safe_temp_name(handle, target.name.as_c_str()) {
                Ok(name) => name,
                Err(errno) => {
                    close_fd_if_valid(target.parent_fd);
                    handle.extraction.last_header_failed = true;
                    return record_error(
                        &mut handle.core,
                        errno,
                        format!(
                            "failed to create temp name for {}",
                            target.display_path.display()
                        ),
                    );
                }
            };
            match open_created_file_at(target.parent_fd, candidate.as_c_str(), create_mode) {
                Ok(fd) => {
                    opened = Some(fd);
                    temp_name = Some(candidate);
                    break;
                }
                Err(errno) if errno == libc::EEXIST => continue,
                Err(errno) => {
                    close_fd_if_valid(target.parent_fd);
                    handle.extraction.last_header_failed = true;
                    return record_error(
                        &mut handle.core,
                        errno,
                        format!("failed to open {}", target.display_path.display()),
                    );
                }
            }
        }
        let Some(fd) = opened else {
            close_fd_if_valid(target.parent_fd);
            handle.extraction.last_header_failed = true;
            return record_error(
                &mut handle.core,
                libc::EEXIST,
                format!(
                    "failed to allocate temp name for {}",
                    target.display_path.display()
                ),
            );
        };
        (fd, temp_name, Some(target.name.clone()))
    } else {
        if target_exists(target.parent_fd, target.name.as_c_str()) {
            let _ = remove_entry_at(target.parent_fd, target.name.as_c_str());
        }
        let fd = match open_created_file_at(target.parent_fd, target.name.as_c_str(), create_mode) {
            Ok(fd) => fd,
            Err(errno) => {
                close_fd_if_valid(target.parent_fd);
                handle.extraction.last_header_failed = true;
                return record_error(
                    &mut handle.core,
                    errno,
                    format!("failed to open {}", target.display_path.display()),
                );
            }
        };
        close_fd_if_valid(target.parent_fd);
        (fd, None, None)
    };

    handle.extraction.current = Some(WriteDiskCurrentState {
        display_path: target.display_path.clone(),
        current_parent_fd: if use_safe_writes {
            target.parent_fd
        } else {
            -1
        },
        current_name,
        final_parent_fd: None,
        final_name,
        fd,
        size_limit: entry_data.size_set.then_some(entry_data.size),
        written: 0,
        accept_data: true,
        close_fd_on_finish: true,
        fixup: Some(make_fixup(
            handle,
            target.display_path.clone(),
            entry_data,
            fd,
            -1,
            None,
            true,
        )),
    });
    ARCHIVE_OK
}

pub(crate) unsafe fn write_disk_data(
    handle: &mut WriteDiskArchiveHandle,
    buffer: *const c_void,
    size: size_t,
) -> isize {
    let Some(current) = handle.extraction.current.as_mut() else {
        handle.core.state = crate::common::error::ARCHIVE_STATE_FATAL;
        return ARCHIVE_FATAL as isize;
    };
    if !current.accept_data || current.fd < 0 {
        return ARCHIVE_WARN as isize;
    }
    let mut to_write = size as usize;
    if let Some(limit) = current.size_limit {
        let remaining = (limit - current.written).max(0) as usize;
        to_write = to_write.min(remaining);
    }
    if size != 0 && to_write == 0 {
        return ARCHIVE_WARN as isize;
    }
    let slice = std::slice::from_raw_parts(buffer.cast::<u8>(), to_write);
    let rc = libc::write(current.fd, slice.as_ptr().cast(), slice.len());
    if rc < 0 {
        handle.core.state = crate::common::error::ARCHIVE_STATE_FATAL;
        return ARCHIVE_FATAL as isize;
    }
    current.written += rc as i64;
    rc as isize
}

pub(crate) unsafe fn write_disk_data_block(
    handle: &mut WriteDiskArchiveHandle,
    buffer: *const c_void,
    size: size_t,
    offset: i64,
) -> isize {
    let Some(current) = handle.extraction.current.as_mut() else {
        handle.core.state = crate::common::error::ARCHIVE_STATE_FATAL;
        return ARCHIVE_FATAL as isize;
    };
    if !current.accept_data || current.fd < 0 {
        return ARCHIVE_WARN as isize;
    }
    let mut to_write = size as usize;
    if let Some(limit) = current.size_limit {
        if offset >= limit {
            return ARCHIVE_OK as isize;
        }
        to_write = to_write.min((limit - offset) as usize);
    }
    let slice = std::slice::from_raw_parts(buffer.cast::<u8>(), to_write);
    let rc = libc::pwrite(current.fd, slice.as_ptr().cast(), slice.len(), offset);
    if rc < 0 {
        handle.core.state = crate::common::error::ARCHIVE_STATE_FATAL;
        return ARCHIVE_FATAL as isize;
    }
    current.written = current.written.max(offset + rc as i64);
    ARCHIVE_OK as isize
}

pub(crate) fn write_disk_finish_entry(handle: &mut WriteDiskArchiveHandle) -> c_int {
    let Some(mut current) = handle.extraction.current.take() else {
        if handle.extraction.last_header_failed {
            handle.extraction.last_header_failed = false;
        }
        return ARCHIVE_OK;
    };

    let fixup = current.fixup.take();
    if let Some(ref entry_fixup) = fixup {
        if entry_fixup.target_fd >= 0 && entry_fixup.target_fd == current.fd {
            current.fd = -1;
        }
    }

    let mut status = ARCHIVE_OK;
    let truncate_fd = if current.fd >= 0 {
        current.fd
    } else {
        fixup
            .as_ref()
            .map_or(-1, |entry_fixup| entry_fixup.target_fd)
    };
    if truncate_fd >= 0 {
        if let Some(limit) = current.size_limit {
            if unsafe { libc::ftruncate(truncate_fd, limit) } != 0 {
                status = ARCHIVE_WARN;
            }
        }
    }
    if let Some(final_name) = current.final_name.as_ref() {
        let Some(current_name) = current.current_name.as_ref() else {
            if let Some(mut fixup) = fixup {
                close_fixup(&mut fixup);
            }
            close_current_state(&mut current);
            return record_error(
                &mut handle.core,
                libc::EINVAL,
                format!(
                    "missing temporary name for {}",
                    current.display_path.display()
                ),
            );
        };
        if target_exists(current.current_parent_fd, final_name.as_c_str()) {
            let _ = remove_entry_at(current.current_parent_fd, final_name.as_c_str());
        }
        let final_parent_fd = current.final_parent_fd.unwrap_or(current.current_parent_fd);
        if unsafe {
            libc::renameat(
                current.current_parent_fd,
                current_name.as_ptr(),
                final_parent_fd,
                final_name.as_ptr(),
            )
        } != 0
        {
            if let Some(mut fixup) = fixup {
                close_fixup(&mut fixup);
            }
            close_current_state(&mut current);
            return record_error(
                &mut handle.core,
                last_errno(),
                format!("failed to rename into {}", current.display_path.display()),
            );
        }
    }
    if let Some(fixup) = fixup {
        if filetype_from_mode(fixup.mode) == AE_IFDIR {
            handle.extraction.deferred_dirs.push(fixup);
        } else {
            status = apply_fixup(handle, fixup);
        }
    }
    close_current_state(&mut current);
    status
}

pub(crate) fn write_disk_close(handle: &mut WriteDiskArchiveHandle) -> c_int {
    let mut status = ARCHIVE_OK;
    if handle.extraction.current.is_some() {
        status = write_disk_finish_entry(handle);
    } else if handle.extraction.last_header_failed {
        status = ARCHIVE_FATAL;
    }
    while let Some(fixup) = handle.extraction.deferred_dirs.pop() {
        if apply_fixup(handle, fixup) != ARCHIVE_OK {
            status = ARCHIVE_WARN;
        }
    }
    if let Some(fd) = handle.extraction.cwd_root_fd.take() {
        close_fd_if_valid(fd);
    }
    if let Some(fd) = handle.extraction.absolute_root_fd.take() {
        close_fd_if_valid(fd);
    }
    handle.extraction.temp_counter = 0;
    if handle.core.state == crate::common::error::ARCHIVE_STATE_FATAL {
        ARCHIVE_FATAL
    } else {
        status
    }
}
