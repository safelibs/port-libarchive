use std::ffi::{c_char, c_int, c_void, CString};
use std::path::PathBuf;
use std::ptr;

use libc::{mode_t, size_t, stat, timespec};

use crate::common::api::ensure_variadic_shim_initialized;
use crate::common::backend::{
    api as backend_api, BackendArchive, BackendEntry, BackendReadDiskCleanupCallback,
    BackendReadDiskLookupCallback, BackendWriteDiskCleanupCallback, BackendWriteDiskLookupCallback,
};
use crate::common::error::{
    ARCHIVE_FATAL, ARCHIVE_MATCH_MAGIC, ARCHIVE_OK, ARCHIVE_READ_DISK_MAGIC, ARCHIVE_READ_MAGIC,
    ARCHIVE_STATE_CLOSED, ARCHIVE_STATE_FATAL, ARCHIVE_STATE_NEW, ARCHIVE_WRITE_DISK_MAGIC,
    ARCHIVE_WRITE_MAGIC,
};
use crate::entry::internal::AclState;
use crate::entry::internal::SparseEntry;
use crate::ffi::{archive, archive_entry};

#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ArchiveKind {
    Read = ARCHIVE_READ_MAGIC,
    Write = ARCHIVE_WRITE_MAGIC,
    ReadDisk = ARCHIVE_READ_DISK_MAGIC,
    WriteDisk = ARCHIVE_WRITE_DISK_MAGIC,
    Match = ARCHIVE_MATCH_MAGIC,
}

pub(crate) type ArchiveOpenCallback = unsafe extern "C" fn(*mut archive, *mut c_void) -> c_int;
pub(crate) type ArchiveReadCallback =
    unsafe extern "C" fn(*mut archive, *mut c_void, *mut *const c_void) -> isize;
pub(crate) type ArchiveSkipCallback = unsafe extern "C" fn(*mut archive, *mut c_void, i64) -> i64;
pub(crate) type ArchiveSeekCallback =
    unsafe extern "C" fn(*mut archive, *mut c_void, i64, c_int) -> i64;
pub(crate) type ArchiveSwitchCallback =
    unsafe extern "C" fn(*mut archive, *mut c_void, *mut c_void) -> c_int;
pub(crate) type ArchivePassphraseCallback =
    unsafe extern "C" fn(*mut archive, *mut c_void) -> *const c_char;
pub(crate) type ArchiveWriteCallback =
    unsafe extern "C" fn(*mut archive, *mut c_void, *const c_void, size_t) -> isize;
pub(crate) type ArchiveCloseCallback = unsafe extern "C" fn(*mut archive, *mut c_void) -> c_int;
pub(crate) type ArchiveFreeCallback = unsafe extern "C" fn(*mut archive, *mut c_void) -> c_int;
pub(crate) type ReadDiskExcludedCallback =
    unsafe extern "C" fn(*mut archive, *mut c_void, *mut archive_entry);
pub(crate) type ReadDiskMetadataFilterCallback =
    unsafe extern "C" fn(*mut archive, *mut c_void, *mut archive_entry) -> c_int;

#[derive(Clone, Copy)]
pub(crate) enum ReadFilterRegistration {
    All,
    None,
    Bzip2,
    Compress,
    Gzip,
    Grzip,
    Lrzip,
    Lz4,
    Lzip,
    Lzma,
    Lzop,
    Uu,
    Xz,
    Zstd,
}

#[derive(Clone, Copy)]
pub(crate) enum ReadFormatRegistration {
    All,
    Ar,
    Cpio,
    Empty,
    Raw,
    Tar,
}

#[derive(Clone)]
pub(crate) enum ReadSourceConfig {
    None,
    Memory {
        buffer: *const c_void,
        size: size_t,
    },
    Filename {
        path: String,
        block_size: size_t,
    },
    Filenames {
        paths: Vec<String>,
        block_size: size_t,
    },
    FilenameW {
        path: String,
        block_size: size_t,
    },
    Memory2 {
        buffer: *const c_void,
        size: size_t,
        read_size: size_t,
    },
    Fd {
        fd: c_int,
        block_size: size_t,
    },
    File {
        file: *mut libc::FILE,
    },
    Callbacks,
}

#[derive(Clone)]
pub(crate) enum WriteFilterConfig {
    Code(c_int),
    Name(String),
    Program(String),
    B64Encode,
    Bzip2,
    Compress,
    Grzip,
    Gzip,
    Lrzip,
    Lz4,
    Lzip,
    Lzma,
    Lzop,
    None,
    Uuencode,
    Xz,
    Zstd,
}

#[derive(Clone)]
pub(crate) enum WriteFormatConfig {
    SevenZip,
    ArBsd,
    ArSvr4,
    Cpio,
    CpioBin,
    CpioNewc,
    CpioOdc,
    CpioPwb,
    Gnutar,
    Iso9660,
    Mtree,
    MtreeClassic,
    Pax,
    PaxRestricted,
    Raw,
    Shar,
    SharDump,
    Ustar,
    V7tar,
    Warc,
    Xar,
    Zip,
}

#[derive(Clone)]
pub(crate) enum WriteOptionConfig {
    FilterOption {
        module: Option<String>,
        option: Option<String>,
        value: Option<String>,
    },
    FormatOption {
        module: Option<String>,
        option: Option<String>,
        value: Option<String>,
    },
    Option {
        module: Option<String>,
        option: Option<String>,
        value: Option<String>,
    },
    Options(String),
    Passphrase(String),
}

#[derive(Clone)]
pub(crate) enum WriteOpenConfig {
    None,
    Callbacks,
    Memory {
        buffer: *mut c_void,
        size: size_t,
        used: *mut size_t,
    },
    Fd(c_int),
    Filename(String),
    FilenameW(String),
    File(*mut c_void),
}

#[derive(Clone, Copy)]
pub(crate) enum ReadDiskSymlinkMode {
    Logical,
    Physical,
    Hybrid,
}

#[derive(Clone)]
pub(crate) enum ReadDiskOpenPath {
    None,
    Utf8(String),
    Wide(String),
}

#[derive(Clone)]
pub(crate) struct ReadDiskNode {
    pub(crate) display_path: String,
    pub(crate) filesystem_path: PathBuf,
    pub(crate) follow_final_symlink: bool,
    pub(crate) ancestor_dirs: Vec<(u64, u64)>,
}

pub(crate) struct ReadDiskAtimeRestore {
    pub(crate) path: PathBuf,
    pub(crate) atime: timespec,
    pub(crate) mtime: timespec,
    pub(crate) follow_symlink: bool,
}

#[derive(Default)]
pub(crate) struct ReadDiskTraversalState {
    pub(crate) pending: Vec<ReadDiskNode>,
    pub(crate) current: Option<ReadDiskNode>,
    pub(crate) current_resolved_path: Option<PathBuf>,
    pub(crate) current_data: Vec<u8>,
    pub(crate) current_data_cursor: usize,
    pub(crate) current_data_eof: bool,
    pub(crate) current_data_offset: i64,
    pub(crate) current_size: i64,
    pub(crate) current_sparse: Vec<SparseEntry>,
    pub(crate) current_sparse_index: usize,
    pub(crate) current_fully_sparse: bool,
    pub(crate) current_can_descend: bool,
    pub(crate) restore_atime: Option<ReadDiskAtimeRestore>,
    pub(crate) current_stat: Option<stat>,
}

pub(crate) struct WriteDiskPendingFixup {
    pub(crate) display_path: PathBuf,
    pub(crate) mode: mode_t,
    pub(crate) uid: i64,
    pub(crate) gid: i64,
    pub(crate) fflags_set: libc::c_ulong,
    pub(crate) fflags_clear: libc::c_ulong,
    pub(crate) atime: Option<(i64, i64)>,
    pub(crate) mtime: Option<(i64, i64)>,
    pub(crate) apply_perm: bool,
    pub(crate) apply_owner: bool,
    pub(crate) apply_time: bool,
    pub(crate) acl: AclState,
    pub(crate) xattrs: Vec<(CString, Vec<u8>)>,
    pub(crate) target_fd: c_int,
    pub(crate) parent_fd: c_int,
    pub(crate) name: Option<CString>,
    pub(crate) follow: bool,
}

pub(crate) struct WriteDiskCurrentState {
    pub(crate) display_path: PathBuf,
    pub(crate) current_parent_fd: c_int,
    pub(crate) current_name: Option<CString>,
    pub(crate) final_parent_fd: Option<c_int>,
    pub(crate) final_name: Option<CString>,
    pub(crate) fd: c_int,
    pub(crate) size_limit: Option<i64>,
    pub(crate) written: i64,
    pub(crate) accept_data: bool,
    pub(crate) suppress_data_warnings: bool,
    pub(crate) close_fd_on_finish: bool,
    pub(crate) fixup: Option<WriteDiskPendingFixup>,
}

#[derive(Default)]
pub(crate) struct WriteDiskExtractionState {
    pub(crate) cwd_root_fd: Option<c_int>,
    pub(crate) absolute_root_fd: Option<c_int>,
    pub(crate) temp_counter: u64,
    pub(crate) total_bytes_written: i64,
    pub(crate) current: Option<WriteDiskCurrentState>,
    pub(crate) deferred_dirs: Vec<WriteDiskPendingFixup>,
    pub(crate) last_header_failed: bool,
}

#[repr(C)]
pub(crate) struct ArchiveCore {
    pub(crate) magic: u32,
    pub(crate) state: u32,
    pub(crate) archive_format: c_int,
    pub(crate) archive_format_name: Option<CString>,
    pub(crate) file_count: c_int,
    pub(crate) archive_error_number: c_int,
    pub(crate) error_string: Option<CString>,
    pub(crate) position_compressed: i64,
    pub(crate) position_uncompressed: i64,
}

#[repr(C)]
pub(crate) struct ArchiveBaseHandle {
    pub(crate) core: ArchiveCore,
}

#[repr(C)]
pub(crate) struct ReadCallbackNode {
    pub(crate) owner: *mut archive,
    pub(crate) client_data: *mut c_void,
}

#[repr(C)]
pub(crate) struct ReadArchiveHandle {
    pub(crate) core: ArchiveCore,
    pub(crate) backend: *mut BackendArchive,
    pub(crate) entry: *mut archive_entry,
    pub(crate) current_entry: *mut BackendEntry,
    pub(crate) backend_opened: bool,
    pub(crate) open_cb: Option<ArchiveOpenCallback>,
    pub(crate) read_cb: Option<ArchiveReadCallback>,
    pub(crate) skip_cb: Option<ArchiveSkipCallback>,
    pub(crate) seek_cb: Option<ArchiveSeekCallback>,
    pub(crate) close_cb: Option<ArchiveCloseCallback>,
    pub(crate) switch_cb: Option<ArchiveSwitchCallback>,
    pub(crate) callback_nodes: Vec<Box<ReadCallbackNode>>,
    pub(crate) passphrase_cb: Option<ArchivePassphraseCallback>,
    pub(crate) passphrase_client_data: *mut c_void,
    pub(crate) extract_progress: Option<unsafe extern "C" fn(*mut c_void)>,
    pub(crate) extract_progress_user_data: *mut c_void,
    pub(crate) extract_skip_file: Option<(i64, i64)>,
    pub(crate) placeholder_formats: u32,
    pub(crate) filter_registrations: Vec<ReadFilterRegistration>,
    pub(crate) format_registrations: Vec<ReadFormatRegistration>,
    pub(crate) source: ReadSourceConfig,
}

#[repr(C)]
pub(crate) struct WriteArchiveHandle {
    pub(crate) core: ArchiveCore,
    pub(crate) backend: *mut BackendArchive,
    pub(crate) client_data: *mut c_void,
    pub(crate) open_cb: Option<ArchiveOpenCallback>,
    pub(crate) write_cb: Option<ArchiveWriteCallback>,
    pub(crate) close_cb: Option<ArchiveCloseCallback>,
    pub(crate) free_cb: Option<ArchiveFreeCallback>,
    pub(crate) passphrase_cb: Option<ArchivePassphraseCallback>,
    pub(crate) passphrase_client_data: *mut c_void,
    pub(crate) backend_opened: bool,
    pub(crate) bytes_per_block: c_int,
    pub(crate) bytes_in_last_block: c_int,
    pub(crate) skip_file: Option<(i64, i64)>,
    pub(crate) filters: Vec<WriteFilterConfig>,
    pub(crate) format: Option<WriteFormatConfig>,
    pub(crate) options: Vec<WriteOptionConfig>,
    pub(crate) open_target: WriteOpenConfig,
}

#[repr(C)]
pub(crate) struct ReadDiskArchiveHandle {
    pub(crate) core: ArchiveCore,
    pub(crate) backend: *mut BackendArchive,
    pub(crate) entry: *mut archive_entry,
    pub(crate) current_entry: *mut BackendEntry,
    pub(crate) backend_match: *mut BackendArchive,
    pub(crate) excluded_cb: Option<ReadDiskExcludedCallback>,
    pub(crate) excluded_client_data: *mut c_void,
    pub(crate) metadata_filter_cb: Option<ReadDiskMetadataFilterCallback>,
    pub(crate) metadata_filter_client_data: *mut c_void,
    pub(crate) backend_opened: bool,
    pub(crate) symlink_mode: ReadDiskSymlinkMode,
    pub(crate) behavior_flags: c_int,
    pub(crate) open_path: ReadDiskOpenPath,
    pub(crate) matching: *mut archive,
    pub(crate) gname_lookup_private_data: *mut c_void,
    pub(crate) gname_lookup: BackendReadDiskLookupCallback,
    pub(crate) gname_lookup_cleanup: BackendReadDiskCleanupCallback,
    pub(crate) uname_lookup_private_data: *mut c_void,
    pub(crate) uname_lookup: BackendReadDiskLookupCallback,
    pub(crate) uname_lookup_cleanup: BackendReadDiskCleanupCallback,
    pub(crate) use_standard_lookup: bool,
    pub(crate) gname_cache: Option<CString>,
    pub(crate) uname_cache: Option<CString>,
    pub(crate) traversal: ReadDiskTraversalState,
}

#[repr(C)]
pub(crate) struct WriteDiskArchiveHandle {
    pub(crate) core: ArchiveCore,
    pub(crate) backend: *mut BackendArchive,
    pub(crate) options: c_int,
    pub(crate) skip_file: Option<(i64, i64)>,
    pub(crate) group_lookup_private_data: *mut c_void,
    pub(crate) group_lookup: BackendWriteDiskLookupCallback,
    pub(crate) group_lookup_cleanup: BackendWriteDiskCleanupCallback,
    pub(crate) user_lookup_private_data: *mut c_void,
    pub(crate) user_lookup: BackendWriteDiskLookupCallback,
    pub(crate) user_lookup_cleanup: BackendWriteDiskCleanupCallback,
    pub(crate) use_standard_lookup: bool,
    pub(crate) extraction: WriteDiskExtractionState,
}

impl ArchiveCore {
    pub(crate) fn new(kind: ArchiveKind) -> Self {
        Self {
            magic: kind as u32,
            state: ARCHIVE_STATE_NEW,
            archive_format: 0,
            archive_format_name: None,
            file_count: 0,
            archive_error_number: 0,
            error_string: None,
            position_compressed: 0,
            position_uncompressed: 0,
        }
    }
}

pub(crate) unsafe fn core_from_archive<'a>(a: *mut archive) -> Option<&'a mut ArchiveCore> {
    a.cast::<ArchiveCore>().as_mut()
}

pub(crate) unsafe fn read_from_archive<'a>(a: *mut archive) -> Option<&'a mut ReadArchiveHandle> {
    a.cast::<ReadArchiveHandle>().as_mut()
}

pub(crate) unsafe fn write_from_archive<'a>(a: *mut archive) -> Option<&'a mut WriteArchiveHandle> {
    a.cast::<WriteArchiveHandle>().as_mut()
}

pub(crate) unsafe fn read_disk_from_archive<'a>(
    a: *mut archive,
) -> Option<&'a mut ReadDiskArchiveHandle> {
    a.cast::<ReadDiskArchiveHandle>().as_mut()
}

pub(crate) unsafe fn write_disk_from_archive<'a>(
    a: *mut archive,
) -> Option<&'a mut WriteDiskArchiveHandle> {
    a.cast::<WriteDiskArchiveHandle>().as_mut()
}

pub(crate) unsafe fn backend_archive(a: *mut archive) -> *mut BackendArchive {
    match archive_magic(a) {
        ARCHIVE_READ_MAGIC => read_from_archive(a).map_or(ptr::null_mut(), |handle| handle.backend),
        ARCHIVE_WRITE_MAGIC => {
            write_from_archive(a).map_or(ptr::null_mut(), |handle| handle.backend)
        }
        ARCHIVE_READ_DISK_MAGIC => {
            read_disk_from_archive(a).map_or(ptr::null_mut(), |handle| handle.backend)
        }
        ARCHIVE_WRITE_DISK_MAGIC => {
            write_disk_from_archive(a).map_or(ptr::null_mut(), |handle| handle.backend)
        }
        _ => ptr::null_mut(),
    }
}

pub(crate) fn alloc_archive(kind: ArchiveKind) -> *mut archive {
    ensure_variadic_shim_initialized();
    match kind {
        ArchiveKind::Read => Box::into_raw(Box::new(ReadArchiveHandle {
            core: ArchiveCore::new(kind),
            backend: ptr::null_mut(),
            entry: ptr::null_mut(),
            current_entry: ptr::null_mut(),
            backend_opened: false,
            open_cb: None,
            read_cb: None,
            skip_cb: None,
            seek_cb: None,
            close_cb: None,
            switch_cb: None,
            callback_nodes: Vec::new(),
            passphrase_cb: None,
            passphrase_client_data: ptr::null_mut(),
            extract_progress: None,
            extract_progress_user_data: ptr::null_mut(),
            extract_skip_file: None,
            placeholder_formats: 0,
            filter_registrations: Vec::new(),
            format_registrations: Vec::new(),
            source: ReadSourceConfig::None,
        })) as *mut archive,
        ArchiveKind::Write => Box::into_raw(Box::new(WriteArchiveHandle {
            core: ArchiveCore::new(kind),
            backend: ptr::null_mut(),
            client_data: ptr::null_mut(),
            open_cb: None,
            write_cb: None,
            close_cb: None,
            free_cb: None,
            passphrase_cb: None,
            passphrase_client_data: ptr::null_mut(),
            backend_opened: false,
            bytes_per_block: 10240,
            bytes_in_last_block: -1,
            skip_file: None,
            filters: Vec::new(),
            format: None,
            options: Vec::new(),
            open_target: WriteOpenConfig::None,
        })) as *mut archive,
        ArchiveKind::ReadDisk => Box::into_raw(Box::new(ReadDiskArchiveHandle {
            core: ArchiveCore::new(kind),
            backend: ptr::null_mut(),
            entry: ptr::null_mut(),
            current_entry: ptr::null_mut(),
            backend_match: ptr::null_mut(),
            excluded_cb: None,
            excluded_client_data: ptr::null_mut(),
            metadata_filter_cb: None,
            metadata_filter_client_data: ptr::null_mut(),
            backend_opened: false,
            symlink_mode: ReadDiskSymlinkMode::Physical,
            behavior_flags: 0,
            open_path: ReadDiskOpenPath::None,
            matching: ptr::null_mut(),
            gname_lookup_private_data: ptr::null_mut(),
            gname_lookup: None,
            gname_lookup_cleanup: None,
            uname_lookup_private_data: ptr::null_mut(),
            uname_lookup: None,
            uname_lookup_cleanup: None,
            use_standard_lookup: false,
            gname_cache: None,
            uname_cache: None,
            traversal: ReadDiskTraversalState::default(),
        })) as *mut archive,
        ArchiveKind::WriteDisk => Box::into_raw(Box::new(WriteDiskArchiveHandle {
            core: ArchiveCore::new(kind),
            backend: ptr::null_mut(),
            options: 0,
            skip_file: None,
            group_lookup_private_data: ptr::null_mut(),
            group_lookup: None,
            group_lookup_cleanup: None,
            user_lookup_private_data: ptr::null_mut(),
            user_lookup: None,
            user_lookup_cleanup: None,
            use_standard_lookup: false,
            extraction: WriteDiskExtractionState::default(),
        })) as *mut archive,
        ArchiveKind::Match => Box::into_raw(Box::new(ArchiveBaseHandle {
            core: ArchiveCore::new(kind),
        })) as *mut archive,
    }
}

pub(crate) unsafe fn archive_magic(a: *mut archive) -> u32 {
    core_from_archive(a).map_or(0, |core| core.magic)
}

pub(crate) unsafe fn archive_check_magic(
    a: *mut archive,
    expected_magic: u32,
    allowed_states: u32,
    function: &str,
) -> c_int {
    let Some(core) = core_from_archive(a) else {
        return ARCHIVE_FATAL;
    };

    if core.magic != expected_magic {
        set_error_string(
            core,
            -1,
            format!("PROGRAMMER ERROR: Function '{function}' invoked on wrong archive object"),
        );
        core.state = ARCHIVE_STATE_FATAL;
        return ARCHIVE_FATAL;
    }

    if (core.state & allowed_states) == 0 {
        if core.state != ARCHIVE_STATE_FATAL {
            set_error_string(
                core,
                -1,
                format!(
                    "INTERNAL ERROR: Function '{function}' invoked with archive structure in state '{}'",
                    state_name(core.state)
                ),
            );
        }
        core.state = ARCHIVE_STATE_FATAL;
        return ARCHIVE_FATAL;
    }

    ARCHIVE_OK
}

fn state_name(state: u32) -> &'static str {
    match state {
        crate::common::error::ARCHIVE_STATE_NEW => "new",
        crate::common::error::ARCHIVE_STATE_HEADER => "header",
        crate::common::error::ARCHIVE_STATE_DATA => "data",
        crate::common::error::ARCHIVE_STATE_EOF => "eof",
        crate::common::error::ARCHIVE_STATE_CLOSED => "closed",
        crate::common::error::ARCHIVE_STATE_FATAL => "fatal",
        _ => "unknown",
    }
}

pub(crate) fn set_error_string(core: &mut ArchiveCore, errno: c_int, message: String) {
    core.archive_error_number = errno;
    core.error_string = Some(CString::new(message).expect("error message must not contain NUL"));
}

pub(crate) fn set_error_option(core: &mut ArchiveCore, errno: c_int, message: Option<String>) {
    core.archive_error_number = errno;
    core.error_string =
        message.map(|message| CString::new(message).expect("error message must not contain NUL"));
}

pub(crate) fn clear_error(core: &mut ArchiveCore) {
    core.archive_error_number = 0;
    core.error_string = None;
}

pub(crate) fn error_string_ptr(core: &ArchiveCore) -> *const c_char {
    core.error_string
        .as_ref()
        .map_or(ptr::null(), |value| value.as_ptr())
}

pub(crate) unsafe fn free_archive(a: *mut archive) -> c_int {
    if a.is_null() {
        return ARCHIVE_OK;
    }

    match archive_magic(a) {
        ARCHIVE_READ_MAGIC => {
            let handle = a.cast::<ReadArchiveHandle>();
            if !(*handle).entry.is_null() {
                crate::entry::internal::free_raw_entry((*handle).entry);
            }
            if !(*handle).backend.is_null() {
                (backend_api().archive_read_free)((*handle).backend);
            }
            drop(Box::from_raw(handle));
        }
        ARCHIVE_WRITE_MAGIC => {
            let handle = a.cast::<WriteArchiveHandle>();
            if !(*handle).backend.is_null() {
                (backend_api().archive_write_free)((*handle).backend);
            }
            drop(Box::from_raw(handle));
        }
        ARCHIVE_READ_DISK_MAGIC => {
            let handle = a.cast::<ReadDiskArchiveHandle>();
            if !(*handle).entry.is_null() {
                crate::entry::internal::free_raw_entry((*handle).entry);
            }
            if !(*handle).backend_match.is_null() {
                (backend_api().archive_match_free)((*handle).backend_match);
            }
            let _ = crate::disk::native_read_disk_close(&mut *handle);
            if let Some(cleanup) = (*handle).gname_lookup_cleanup {
                cleanup((*handle).gname_lookup_private_data);
            }
            if let Some(cleanup) = (*handle).uname_lookup_cleanup {
                cleanup((*handle).uname_lookup_private_data);
            }
            drop(Box::from_raw(handle));
        }
        ARCHIVE_WRITE_DISK_MAGIC => {
            let handle = a.cast::<WriteDiskArchiveHandle>();
            let _ = crate::disk::native_write_disk_close(&mut *handle);
            if let Some(cleanup) = (*handle).group_lookup_cleanup {
                cleanup((*handle).group_lookup_private_data);
            }
            if let Some(cleanup) = (*handle).user_lookup_cleanup {
                cleanup((*handle).user_lookup_private_data);
            }
            drop(Box::from_raw(handle));
        }
        ARCHIVE_MATCH_MAGIC => {
            crate::r#match::internal::free_match_archive(a);
        }
        _ => return ARCHIVE_FATAL,
    }

    ARCHIVE_OK
}

pub(crate) unsafe fn close_archive(a: *mut archive) -> c_int {
    let Some(core) = core_from_archive(a) else {
        return ARCHIVE_FATAL;
    };

    let status = match core.magic {
        ARCHIVE_READ_MAGIC | ARCHIVE_READ_DISK_MAGIC => {
            if core.magic == ARCHIVE_READ_DISK_MAGIC {
                if let Some(handle) = read_disk_from_archive(a) {
                    crate::disk::native_read_disk_close(handle)
                } else {
                    ARCHIVE_FATAL
                }
            } else {
                let backend = backend_archive(a);
                if backend.is_null() {
                    ARCHIVE_OK
                } else {
                    (backend_api().archive_read_close)(backend)
                }
            }
        }
        ARCHIVE_WRITE_MAGIC | ARCHIVE_WRITE_DISK_MAGIC => {
            if core.magic == ARCHIVE_WRITE_DISK_MAGIC {
                if let Some(handle) = write_disk_from_archive(a) {
                    crate::disk::native_write_disk_close(handle)
                } else {
                    ARCHIVE_FATAL
                }
            } else {
                let backend = backend_archive(a);
                if backend.is_null() {
                    ARCHIVE_OK
                } else {
                    (backend_api().archive_write_close)(backend)
                }
            }
        }
        _ => ARCHIVE_OK,
    };
    if status == ARCHIVE_OK {
        core.state = ARCHIVE_STATE_CLOSED;
        match core.magic {
            ARCHIVE_READ_MAGIC => {
                if let Some(handle) = read_from_archive(a) {
                    handle.backend_opened = false;
                }
            }
            ARCHIVE_WRITE_MAGIC => {
                if let Some(handle) = write_from_archive(a) {
                    handle.backend_opened = false;
                }
            }
            ARCHIVE_READ_DISK_MAGIC => {
                if let Some(handle) = read_disk_from_archive(a) {
                    handle.backend_opened = false;
                }
            }
            _ => {}
        }
    }
    status
}

pub(crate) unsafe fn sync_backend_core(a: *mut archive) {
    let Some(core) = core_from_archive(a) else {
        return;
    };
    if !matches!(core.magic, ARCHIVE_READ_MAGIC | ARCHIVE_WRITE_MAGIC) {
        return;
    }
    let backend = backend_archive(a);
    if backend.is_null() {
        return;
    }
    core.archive_format = (backend_api().archive_format)(backend);
    core.file_count = (backend_api().archive_file_count)(backend);
    core.position_compressed = (backend_api().archive_position_compressed)(backend);
    core.position_uncompressed = (backend_api().archive_position_uncompressed)(backend);
}

pub(crate) unsafe fn backend_error_number(a: *mut archive) -> c_int {
    let backend = backend_archive(a);
    if backend.is_null() {
        0
    } else {
        (backend_api().archive_errno)(backend)
    }
}

pub(crate) unsafe fn backend_error_string_ptr(a: *mut archive) -> *const c_char {
    let backend = backend_archive(a);
    if backend.is_null() {
        ptr::null()
    } else {
        (backend_api().archive_error_string)(backend)
    }
}
