pub const ADVANCED_WRITE_FORMAT_NAMES: &[&str] = &[
    "7zip",
    "cd9660",
    "iso",
    "iso9660",
    "mtree",
    "mtree-classic",
    "warc",
    "xar",
    "zip",
];

pub const ADVANCED_WRITE_EXTENSIONS: &[&str] = &[".7z", ".iso", ".zip", ".jar"];

pub fn checked_iso9660_name_len(base_len: usize, extension_len: usize, limit: usize) -> bool {
    base_len
        .checked_add(extension_len)
        .is_some_and(|total| total <= limit)
}

pub fn checked_zip_entry_size(size: i64) -> Option<u64> {
    u64::try_from(size).ok()
}

pub fn zstd_long_window_limit(pointer_width_bits: u32) -> u32 {
    if pointer_width_bits <= 32 {
        26
    } else {
        27
    }
}
