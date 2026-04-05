pub const SECURITY_RELEVANT_BACKENDS: &[&str] = &[
    "archive_ppmd7",
    "archive_ppmd8",
    "archive_blake2s_ref",
    "archive_blake2sp_ref",
    "xxhash",
];

pub fn kib_rounded_allocation(bytes: u64, usize_max: u64) -> Option<usize> {
    let rounded = bytes.checked_shr(10)?.checked_add(1)?.checked_shl(10)?;
    if rounded > usize_max {
        None
    } else {
        usize::try_from(rounded).ok()
    }
}
