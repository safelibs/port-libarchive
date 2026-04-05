use crate::ffi::{archive, archive_entry};

const ARCHIVE_MAGIC: u32 = 0x4152_4348;
const ENTRY_MAGIC: u32 = 0x454e_5452;

#[derive(Clone, Copy, Debug)]
pub(crate) enum ArchiveKind {
    Read,
    Write,
    ReadDisk,
    WriteDisk,
    Match,
}

struct ArchiveHandle {
    magic: u32,
    _kind: ArchiveKind,
}

struct EntryHandle {
    magic: u32,
    _origin_archive: usize,
}

pub(crate) fn alloc_archive(kind: ArchiveKind) -> *mut archive {
    let handle = ArchiveHandle {
        magic: ARCHIVE_MAGIC,
        _kind: kind,
    };
    Box::into_raw(Box::new(handle)) as *mut archive
}

pub(crate) fn alloc_entry(origin_archive: *mut archive) -> *mut archive_entry {
    let handle = EntryHandle {
        magic: ENTRY_MAGIC,
        _origin_archive: origin_archive.cast::<()>() as usize,
    };
    Box::into_raw(Box::new(handle)) as *mut archive_entry
}

pub(crate) unsafe fn free_archive(ptr: *mut archive) {
    if ptr.is_null() {
        return;
    }

    let handle = ptr as *mut ArchiveHandle;
    if (*handle).magic == ARCHIVE_MAGIC {
        drop(Box::from_raw(handle));
    }
}

pub(crate) unsafe fn free_entry(ptr: *mut archive_entry) {
    if ptr.is_null() {
        return;
    }

    let handle = ptr as *mut EntryHandle;
    if (*handle).magic == ENTRY_MAGIC {
        drop(Box::from_raw(handle));
    }
}
