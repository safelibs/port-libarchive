use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::os::fd::IntoRawFd;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt, symlink};
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
    clear_entry, copy_stat, from_raw as entry_from_raw, AclState, AE_IFDIR, AE_IFIFO, AE_IFLNK,
    AE_IFMT, AE_IFREG,
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

const ACL_TYPE_ACCESS: c_int = 0x8000;
const ACL_TYPE_DEFAULT: c_int = 0x4000;
const FS_IOC_GETFLAGS: libc::c_ulong = 0x8008_6601;
const FS_NODUMP_FL: libc::c_long = 0x0000_0040;

unsafe extern "C" {
    fn acl_get_file(path_p: *const c_char, type_: c_int) -> *mut c_void;
    fn acl_to_text(acl: *mut c_void, len_p: *mut libc::ssize_t) -> *mut c_char;
    fn acl_free(obj_p: *mut c_void) -> c_int;
}

fn last_errno() -> c_int {
    std::io::Error::last_os_error().raw_os_error().unwrap_or(libc::EINVAL)
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

fn record_error(core: &mut crate::common::state::ArchiveCore, errno: c_int, message: impl Into<String>) -> c_int {
    set_error_string(core, errno, message.into());
    ARCHIVE_FAILED
}

fn should_follow_root(mode: ReadDiskSymlinkMode) -> bool {
    matches!(mode, ReadDiskSymlinkMode::Logical | ReadDiskSymlinkMode::Hybrid)
}

fn should_follow_descendant(mode: ReadDiskSymlinkMode) -> bool {
    matches!(mode, ReadDiskSymlinkMode::Logical)
}

fn reset_read_data(handle: &mut ReadDiskArchiveHandle) {
    handle.traversal.current_data.clear();
    handle.traversal.current_data_cursor = 0;
    handle.traversal.current_data_eof = false;
    handle.traversal.current_data_offset = 0;
    handle.traversal.restore_atime = None;
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

fn load_acl(path: &Path, entry_acl: &mut AclState, mode: &mut mode_t) {
    let Ok(c_path) = c_path(path) else {
        return;
    };
    for acl_type in [ACL_TYPE_ACCESS, ACL_TYPE_DEFAULT] {
        let acl = unsafe { acl_get_file(c_path.as_ptr(), acl_type) };
        if acl.is_null() {
            continue;
        }
        let mut text_len = 0isize;
        let text = unsafe { acl_to_text(acl, &mut text_len) };
        if !text.is_null() {
            let acl_text = unsafe { CStr::from_ptr(text).to_string_lossy().into_owned() };
            let _ = entry_acl.from_text(mode, &acl_text, acl_type);
            unsafe {
                let _ = acl_free(text.cast());
            }
        }
        unsafe {
            let _ = acl_free(acl);
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
            let candidate = fs::canonicalize(filesystem_path).unwrap_or_else(|_| filesystem_path.to_path_buf());
            let candidate_key = (target_stat.st_dev, target_stat.st_ino);
            let is_dir = filetype_from_mode(target_stat.st_mode) == AE_IFDIR;
            if !is_dir || !handle.traversal.visited_dirs.contains(&candidate_key) {
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
            entry_data.xattrs.push(crate::entry::internal::XattrEntry { name, value });
        }
    }
    if (handle.behavior_flags & ARCHIVE_READDISK_NO_ACL) == 0 {
        load_acl(&effective_path, &mut entry_data.acl, &mut entry_data.mode);
    }
    if let Some(uname) = resolve_uname(handle, entry_data.uid) {
        entry_data.uname.set(Some(uname));
    }
    if let Some(gname) = resolve_gname(handle, entry_data.gid) {
        entry_data.gname.set(Some(gname));
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

    let key = if let Some(st) = handle.traversal.current_stat {
        Some((st.st_dev, st.st_ino))
    } else {
        None
    };
    if let Some(key) = key {
        handle.traversal.visited_dirs.insert(key);
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
    handle.traversal.visited_dirs.clear();
    reset_read_data(handle);
    handle.traversal.pending.push(ReadDiskNode {
        display_path: path.to_string(),
        filesystem_path: PathBuf::from(path),
        follow_final_symlink: should_follow_root(handle.symlink_mode),
    });
    handle.backend_opened = true;
    ARCHIVE_OK
}

pub(crate) unsafe fn read_disk_next_header(
    handle: &mut ReadDiskArchiveHandle,
    entry: *mut archive_entry,
) -> c_int {
    reset_read_data(handle);

    while let Some(node) = handle.traversal.pending.pop() {
        if (handle.behavior_flags & ARCHIVE_READDISK_HONOR_NODUMP) != 0 && is_nodump(&node.filesystem_path) {
            continue;
        }

        let (resolved_path, st, can_descend) = match populate_entry_from_path(
            handle,
            entry,
            &node.display_path,
            &node.filesystem_path,
            node.follow_final_symlink,
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
    file.read_to_end(&mut handle.traversal.current_data).map_err(|error| {
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
    if handle.traversal.current_data_eof || handle.traversal.current_data_cursor >= handle.traversal.current_data.len() {
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
    let display_path = entry_data.pathname.get_str().unwrap_or_default().to_string();
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
    handle.traversal.visited_dirs.clear();
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

fn effective_root(handle: &mut WriteDiskArchiveHandle) -> Result<PathBuf, c_int> {
    if let Some(root) = handle.extraction.cwd_root.clone() {
        return Ok(root);
    }
    let root = std::env::current_dir().map_err(|error| {
        record_error(
            &mut handle.core,
            error.raw_os_error().unwrap_or(libc::EINVAL),
            "failed to capture current directory",
        )
    })?;
    handle.extraction.cwd_root = Some(root.clone());
    Ok(root)
}

fn has_dotdot(path: &Path) -> bool {
    path.components().any(|component| matches!(component, Component::ParentDir))
}

fn maybe_remove(path: &Path) -> Result<(), c_int> {
    match fs::symlink_metadata(path) {
        Ok(meta) => {
            if meta.is_dir() && !meta.file_type().is_symlink() {
                fs::remove_dir_all(path).map_err(|error| error.raw_os_error().unwrap_or(libc::EINVAL))?;
            } else {
                fs::remove_file(path).map_err(|error| error.raw_os_error().unwrap_or(libc::EINVAL))?;
            }
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.raw_os_error().unwrap_or(libc::EINVAL)),
    }
}

fn lexical_parent(path: &Path) -> Option<PathBuf> {
    path.parent().map(Path::to_path_buf)
}

fn ensure_intermediate_dirs(
    handle: &mut WriteDiskArchiveHandle,
    full_path: &Path,
) -> Result<(), c_int> {
    let mut current = if full_path.is_absolute() {
        PathBuf::from("/")
    } else {
        effective_root(handle)?
    };
    let mut components = full_path.components().peekable();
    while let Some(component) = components.next() {
        match component {
            Component::RootDir => {
                current = PathBuf::from("/");
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if (handle.options & ARCHIVE_EXTRACT_SECURE_NODOTDOT) != 0 {
                    return Err(record_error(
                        &mut handle.core,
                        libc::EINVAL,
                        "path contains '..' and secure nodotdot is enabled",
                    ));
                }
                if let Some(parent) = lexical_parent(&current) {
                    current = parent;
                }
            }
            Component::Normal(name) => {
                if components.peek().is_none() {
                    break;
                }
                current.push(name);
                match fs::symlink_metadata(&current) {
                    Ok(meta) if meta.file_type().is_symlink() => {
                        if (handle.options & ARCHIVE_EXTRACT_SECURE_SYMLINKS) != 0 {
                            if (handle.options & ARCHIVE_EXTRACT_UNLINK) == 0 {
                                return Err(record_error(
                                    &mut handle.core,
                                    libc::ELOOP,
                                    format!("path traverses an existing symlink: {}", current.display()),
                                ));
                            }
                            maybe_remove(&current)?;
                            fs::create_dir(&current).map_err(|error| {
                                record_error(
                                    &mut handle.core,
                                    error.raw_os_error().unwrap_or(libc::EINVAL),
                                    format!("failed to create directory {}", current.display()),
                                )
                            })?;
                        } else if let Ok(target) = fs::canonicalize(&current) {
                            current = target;
                        } else if (handle.options & ARCHIVE_EXTRACT_UNLINK) != 0 {
                            maybe_remove(&current)?;
                            fs::create_dir(&current).map_err(|error| {
                                record_error(
                                    &mut handle.core,
                                    error.raw_os_error().unwrap_or(libc::EINVAL),
                                    format!("failed to create directory {}", current.display()),
                                )
                            })?;
                        } else {
                            return Err(record_error(
                                &mut handle.core,
                                libc::ENOENT,
                                format!("broken symlink in path {}", current.display()),
                            ));
                        }
                    }
                    Ok(meta) if meta.is_dir() => {}
                    Ok(_) => {
                        if (handle.options & ARCHIVE_EXTRACT_UNLINK) != 0 {
                            maybe_remove(&current)?;
                            fs::create_dir(&current).map_err(|error| {
                                record_error(
                                    &mut handle.core,
                                    error.raw_os_error().unwrap_or(libc::EINVAL),
                                    format!("failed to create directory {}", current.display()),
                                )
                            })?;
                        } else {
                            return Err(record_error(
                                &mut handle.core,
                                libc::ENOTDIR,
                                format!("path component is not a directory: {}", current.display()),
                            ));
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        fs::create_dir(&current).map_err(|create_error| {
                            record_error(
                                &mut handle.core,
                                create_error.raw_os_error().unwrap_or(libc::EINVAL),
                                format!("failed to create directory {}", current.display()),
                            )
                        })?;
                        let mode = (0o777 & !process_umask()) as u32;
                        let _ = fs::set_permissions(&current, fs::Permissions::from_mode(mode));
                    }
                    Err(error) => {
                        return Err(record_error(
                            &mut handle.core,
                            error.raw_os_error().unwrap_or(libc::EINVAL),
                            format!("failed to inspect {}", current.display()),
                        ));
                    }
                }
            }
            Component::Prefix(_) => {}
        }
    }
    Ok(())
}

fn resolve_final_path(
    handle: &mut WriteDiskArchiveHandle,
    raw_path: &Path,
) -> Result<PathBuf, c_int> {
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
    if raw_path.is_absolute() {
        Ok(raw_path.to_path_buf())
    } else {
        Ok(effective_root(handle)?.join(raw_path))
    }
}

fn entry_uid(handle: &WriteDiskArchiveHandle, entry: &crate::entry::internal::ArchiveEntryData) -> i64 {
    if let Some(lookup) = handle.user_lookup {
        if let Some(name) = entry.uname.get_str() {
            if let Ok(name) = CString::new(name) {
                return unsafe { lookup(handle.user_lookup_private_data, name.as_ptr(), entry.uid) };
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

fn entry_gid(handle: &WriteDiskArchiveHandle, entry: &crate::entry::internal::ArchiveEntryData) -> i64 {
    if let Some(lookup) = handle.group_lookup {
        if let Some(name) = entry.gname.get_str() {
            if let Ok(name) = CString::new(name) {
                return unsafe { lookup(handle.group_lookup_private_data, name.as_ptr(), entry.gid) };
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
    path: PathBuf,
    entry: &crate::entry::internal::ArchiveEntryData,
) -> WriteDiskPendingFixup {
    let uid = entry_uid(handle, entry);
    let gid = entry_gid(handle, entry);
    WriteDiskPendingFixup {
        path,
        mode: desired_file_mode(handle, entry.mode, uid, gid),
        uid,
        gid,
        atime: entry.atime.set.then_some((entry.atime.sec, entry.atime.nsec)),
        mtime: entry.mtime.set.then_some((entry.mtime.sec, entry.mtime.nsec)),
        apply_perm: (handle.options & ARCHIVE_EXTRACT_PERM) != 0 || filetype_from_mode(entry.mode) == AE_IFDIR,
        apply_owner: (handle.options & ARCHIVE_EXTRACT_OWNER) != 0,
        apply_time: (handle.options & ARCHIVE_EXTRACT_TIME) != 0,
        xattrs: entry
            .xattrs
            .iter()
            .map(|xattr| (xattr.name.clone(), xattr.value.clone()))
            .collect(),
    }
}

fn apply_fixup(handle: &mut WriteDiskArchiveHandle, fixup: &WriteDiskPendingFixup) -> c_int {
    let mut status = ARCHIVE_OK;
    let follow = filetype_from_mode(fixup.mode) != AE_IFLNK;
    let Ok(c_path) = c_path(&fixup.path) else {
        return ARCHIVE_WARN;
    };
    let fd = unsafe {
        libc::open(
            c_path.as_ptr(),
            if follow {
                libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW
            } else {
                libc::O_RDONLY | libc::O_CLOEXEC
            },
        )
    };
    if fixup.apply_owner {
        let rc = unsafe {
            if fd >= 0 && follow {
                libc::fchown(fd, fixup.uid as _, fixup.gid as _)
            } else {
                libc::lchown(c_path.as_ptr(), fixup.uid as _, fixup.gid as _)
            }
        };
        if rc != 0 {
            status = ARCHIVE_WARN;
        }
    }
    if fixup.apply_perm {
        let rc = unsafe {
            if fd >= 0 && follow {
                libc::fchmod(fd, fixup.mode & 0o7777)
            } else {
                libc::chmod(c_path.as_ptr(), fixup.mode & 0o7777)
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
            libc::utimensat(
                libc::AT_FDCWD,
                c_path.as_ptr(),
                times.as_ptr(),
                if follow { 0 } else { libc::AT_SYMLINK_NOFOLLOW },
            )
        };
        if rc != 0 {
            status = ARCHIVE_WARN;
        }
    }
    if (handle.options & ARCHIVE_EXTRACT_XATTR) != 0 {
        for (name, value) in &fixup.xattrs {
            let rc = unsafe {
                if follow {
                    libc::setxattr(
                        c_path.as_ptr(),
                        name.as_ptr(),
                        value.as_ptr().cast(),
                        value.len(),
                        0,
                    )
                } else {
                    libc::lsetxattr(
                        c_path.as_ptr(),
                        name.as_ptr(),
                        value.as_ptr().cast(),
                        value.len(),
                        0,
                    )
                }
            };
            if rc != 0 {
                status = ARCHIVE_WARN;
            }
        }
    }
    if fd >= 0 {
        unsafe {
            libc::close(fd);
        }
    }
    status
}

fn close_current_file(current: &mut WriteDiskCurrentState) {
    if current.close_fd_on_finish && current.fd >= 0 {
        unsafe {
            libc::close(current.fd);
        }
        current.fd = -1;
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
    let full_path = match resolve_final_path(handle, raw_path) {
        Ok(path) => path,
        Err(status) => {
            handle.extraction.last_header_failed = true;
            return status;
        }
    };
    if let Some((skip_dev, skip_ino)) = handle.skip_file {
        if let Ok(st) = path_stat(&full_path, false) {
            if st.st_dev == skip_dev as _ && st.st_ino == skip_ino as _ {
                handle.extraction.last_header_failed = true;
                return record_error(&mut handle.core, libc::EEXIST, "refusing to overwrite skipped file");
            }
        }
    }
    if ensure_intermediate_dirs(handle, raw_path).is_err() {
        handle.extraction.last_header_failed = true;
        return ARCHIVE_FAILED;
    }

    let filetype = filetype_from_mode(entry_data.mode);
    if (handle.options & ARCHIVE_EXTRACT_NO_OVERWRITE) != 0 && fs::symlink_metadata(&full_path).is_ok() {
        handle.extraction.current = Some(WriteDiskCurrentState {
            path: full_path,
            final_path: None,
            fd: -1,
            size_limit: Some(0),
            written: 0,
            accept_data: false,
            close_fd_on_finish: false,
            fixup: None,
        });
        return ARCHIVE_OK;
    }
    if (handle.options & ARCHIVE_EXTRACT_NO_OVERWRITE_NEWER) != 0 {
        if let (Ok(existing), true) = (fs::metadata(&full_path), entry_data.mtime.set) {
            if existing.mtime() > entry_data.mtime.sec {
                handle.extraction.current = Some(WriteDiskCurrentState {
                    path: full_path,
                    final_path: None,
                    fd: -1,
                    size_limit: Some(0),
                    written: 0,
                    accept_data: false,
                    close_fd_on_finish: false,
                    fixup: None,
                });
                return ARCHIVE_OK;
            }
        }
    }

    if filetype == AE_IFDIR {
        match fs::symlink_metadata(&full_path) {
            Ok(meta) if meta.file_type().is_symlink() => {
                if let Ok(target_meta) = fs::metadata(&full_path) {
                    if target_meta.is_dir() && (handle.options & ARCHIVE_EXTRACT_SECURE_SYMLINKS) == 0 {
                        handle.extraction.current = Some(WriteDiskCurrentState {
                            path: full_path.clone(),
                            final_path: None,
                            fd: -1,
                            size_limit: None,
                            written: 0,
                            accept_data: false,
                            close_fd_on_finish: false,
                            fixup: Some(make_fixup(handle, full_path, entry_data)),
                        });
                        return ARCHIVE_OK;
                    }
                }
                maybe_remove(&full_path).map_err(|errno| {
                    record_error(
                        &mut handle.core,
                        errno,
                        format!("failed to replace {}", full_path.display()),
                    )
                }).ok();
            }
            Ok(meta) if meta.is_dir() => {}
            Ok(_) => {
                if (handle.options & ARCHIVE_EXTRACT_UNLINK) != 0 {
                    let _ = maybe_remove(&full_path);
                }
            }
            Err(_) => {}
        }
        if fs::symlink_metadata(&full_path).is_err() {
            fs::create_dir_all(&full_path).map_err(|error| {
                record_error(
                    &mut handle.core,
                    error.raw_os_error().unwrap_or(libc::EINVAL),
                    format!("failed to create directory {}", full_path.display()),
                )
            }).ok();
        }
        handle.extraction.current = Some(WriteDiskCurrentState {
            path: full_path.clone(),
            final_path: None,
            fd: -1,
            size_limit: None,
            written: 0,
            accept_data: false,
            close_fd_on_finish: false,
            fixup: Some(make_fixup(handle, full_path, entry_data)),
        });
        return ARCHIVE_OK;
    }

    if filetype == AE_IFLNK {
        if let Some(target) = entry_data.symlink.get_str() {
            if fs::symlink_metadata(&full_path).is_ok() {
                let _ = maybe_remove(&full_path);
            }
            symlink(target, &full_path).map_err(|error| {
                record_error(
                    &mut handle.core,
                    error.raw_os_error().unwrap_or(libc::EINVAL),
                    format!("failed to create symlink {}", full_path.display()),
                )
            }).ok();
            handle.extraction.current = Some(WriteDiskCurrentState {
                path: full_path.clone(),
                final_path: None,
                fd: -1,
                size_limit: None,
                written: 0,
                accept_data: false,
                close_fd_on_finish: false,
                fixup: Some(make_fixup(handle, full_path, entry_data)),
            });
            return ARCHIVE_OK;
        }
    }

    if let Some(target) = entry_data.hardlink.get_str() {
        let target_raw = Path::new(target);
        let target_full = match resolve_final_path(handle, target_raw) {
            Ok(path) => path,
            Err(status) => {
                handle.extraction.last_header_failed = true;
                return status;
            }
        };
        if (handle.options & ARCHIVE_EXTRACT_SECURE_NODOTDOT) != 0 && has_dotdot(target_raw) {
            handle.extraction.last_header_failed = true;
            return record_error(
                &mut handle.core,
                libc::EINVAL,
                "hardlink target contains '..' and secure nodotdot is enabled",
            );
        }
        if (handle.options & ARCHIVE_EXTRACT_SECURE_SYMLINKS) != 0 {
            let mut cursor = target_full.as_path();
            while let Some(parent) = cursor.parent() {
                if let Ok(meta) = fs::symlink_metadata(parent) {
                    if meta.file_type().is_symlink() {
                        handle.extraction.last_header_failed = true;
                        return record_error(
                            &mut handle.core,
                            libc::ELOOP,
                            "hardlink target traverses an existing symlink",
                        );
                    }
                }
                if parent == cursor {
                    break;
                }
                cursor = parent;
            }
        }
        if fs::symlink_metadata(&full_path).is_ok() {
            let _ = maybe_remove(&full_path);
        }
        if let Err(error) = fs::hard_link(&target_full, &full_path) {
            handle.extraction.last_header_failed = true;
            return record_error(
                &mut handle.core,
                error.raw_os_error().unwrap_or(libc::EINVAL),
                format!("failed to create hardlink {}", full_path.display()),
            );
        }
        let authoritative = entry_data.size_set && entry_data.size > 0;
        let fd = if authoritative {
            let options = OpenOptions::new()
                .write(true)
                .truncate(true)
                .custom_flags(libc::O_CLOEXEC)
                .open(&full_path);
            match options {
                Ok(file) => {
                    let fd = file.into_raw_fd();
                    fd
                }
                Err(error) => {
                    handle.extraction.last_header_failed = true;
                    return record_error(
                        &mut handle.core,
                        error.raw_os_error().unwrap_or(libc::EINVAL),
                        format!("failed to open {}", full_path.display()),
                    );
                }
            }
        } else {
            -1
        };
        handle.extraction.current = Some(WriteDiskCurrentState {
            path: full_path.clone(),
            final_path: None,
            fd,
            size_limit: entry_data.size_set.then_some(entry_data.size),
            written: 0,
            accept_data: authoritative,
            close_fd_on_finish: fd >= 0,
            fixup: authoritative.then(|| make_fixup(handle, full_path, entry_data)),
        });
        return ARCHIVE_OK;
    }

    if (handle.options & ARCHIVE_EXTRACT_SECURE_SYMLINKS) != 0 {
        let mut cursor = full_path.as_path();
        while let Some(parent) = cursor.parent() {
            if let Ok(meta) = fs::symlink_metadata(parent) {
                if meta.file_type().is_symlink() {
                    if (handle.options & ARCHIVE_EXTRACT_UNLINK) == 0 {
                        handle.extraction.last_header_failed = true;
                        return record_error(
                            &mut handle.core,
                            libc::ELOOP,
                            format!("path traverses an existing symlink: {}", parent.display()),
                        );
                    }
                    let _ = maybe_remove(parent);
                    let _ = fs::create_dir_all(parent);
                }
            }
            if parent == cursor {
                break;
            }
            cursor = parent;
        }
    }

    let path_to_open = if (handle.options & ARCHIVE_EXTRACT_SAFE_WRITES) != 0 {
        let temp_path = full_path.with_extension("safe-tmp");
        let _ = maybe_remove(&temp_path);
        temp_path
    } else {
        full_path.clone()
    };
    if fs::symlink_metadata(&path_to_open).is_ok() {
        let _ = maybe_remove(&path_to_open);
    }
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true).mode(0o600).custom_flags(libc::O_CLOEXEC);
    let file = match options.open(&path_to_open) {
        Ok(file) => file,
        Err(error) => {
            handle.extraction.last_header_failed = true;
            return record_error(
                &mut handle.core,
                error.raw_os_error().unwrap_or(libc::EINVAL),
                format!("failed to open {}", path_to_open.display()),
            );
        }
    };
    handle.extraction.current = Some(WriteDiskCurrentState {
        path: path_to_open,
        final_path: ((handle.options & ARCHIVE_EXTRACT_SAFE_WRITES) != 0).then_some(full_path.clone()),
        fd: file.into_raw_fd(),
        size_limit: entry_data.size_set.then_some(entry_data.size),
        written: 0,
        accept_data: true,
        close_fd_on_finish: true,
        fixup: Some(make_fixup(handle, full_path, entry_data)),
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
            return 0;
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
    rc as isize
}

pub(crate) fn write_disk_finish_entry(handle: &mut WriteDiskArchiveHandle) -> c_int {
    let Some(mut current) = handle.extraction.current.take() else {
        return if handle.extraction.last_header_failed {
            ARCHIVE_OK
        } else {
            ARCHIVE_OK
        };
    };

    close_current_file(&mut current);
    let mut status = ARCHIVE_OK;
    if let Some(final_path) = &current.final_path {
        let _ = maybe_remove(final_path);
        if let Err(error) = fs::rename(&current.path, final_path) {
            return record_error(
                &mut handle.core,
                error.raw_os_error().unwrap_or(libc::EINVAL),
                format!("failed to rename {} to {}", current.path.display(), final_path.display()),
            );
        }
        current.path = final_path.clone();
    }
    if let Some(fixup) = current.fixup.take() {
        if filetype_from_mode(fixup.mode) == AE_IFDIR {
            handle.extraction.deferred_dirs.push(fixup);
        } else {
            status = apply_fixup(handle, &fixup);
        }
    }
    status
}

pub(crate) fn write_disk_close(handle: &mut WriteDiskArchiveHandle) -> c_int {
    let mut status = ARCHIVE_OK;
    if handle.extraction.current.is_some() {
        status = write_disk_finish_entry(handle);
    }
    while let Some(fixup) = handle.extraction.deferred_dirs.pop() {
        if apply_fixup(handle, &fixup) != ARCHIVE_OK {
            status = ARCHIVE_WARN;
        }
    }
    if handle.core.state == crate::common::error::ARCHIVE_STATE_FATAL {
        ARCHIVE_FATAL
    } else {
        status
    }
}
