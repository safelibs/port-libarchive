use crate::algorithms;

pub const ADVANCED_READ_SUPPORT_EXPORTS: &[&str] = &[
    "archive_read_support_format_7zip",
    "archive_read_support_format_cab",
    "archive_read_support_format_iso9660",
    "archive_read_support_format_lha",
    "archive_read_support_format_mtree",
    "archive_read_support_format_warc",
    "archive_read_support_format_xar",
];

pub fn checked_zisofs_layout(
    pz_log2_bs: u8,
    uncompressed_size: u64,
    usize_max: u64,
) -> Option<(usize, usize, usize)> {
    if !(7..=30).contains(&pz_log2_bs) {
        return None;
    }

    let block_size = 1_u64.checked_shl(u32::from(pz_log2_bs))?;
    if block_size > usize_max {
        return None;
    }

    let blocks = uncompressed_size
        .checked_add(block_size.checked_sub(1)?)?
        .checked_div(block_size)?;
    let table_bytes = blocks.checked_add(1)?.checked_mul(4)?;
    let alloc_bytes = algorithms::kib_rounded_allocation(table_bytes, usize_max)?;
    let table_bytes = usize::try_from(table_bytes).ok()?;
    let block_size = usize::try_from(block_size).ok()?;
    Some((table_bytes, alloc_bytes, block_size))
}

pub fn checked_warc_skip(content_length: i64) -> Option<i64> {
    if content_length < 0 {
        None
    } else {
        content_length.checked_add(4)
    }
}

pub fn forward_progress(
    consumed_before: u64,
    consumed_after: u64,
    produced_before: u64,
    produced_after: u64,
) -> bool {
    consumed_after > consumed_before || produced_after > produced_before
}

pub fn within_work_budget(
    consumed_bytes: u64,
    iterations: u64,
    multiplier: u64,
    slack: u64,
) -> bool {
    consumed_bytes
        .checked_mul(multiplier)
        .and_then(|budget| budget.checked_add(slack))
        .is_some_and(|budget| iterations <= budget)
}

pub fn continuation_budget_ok(queue_len: usize, new_entries: usize, max_entries: usize) -> bool {
    new_entries > 0
        && queue_len
            .checked_add(new_entries)
            .is_some_and(|count| count <= max_entries)
}

pub fn line_and_read_ahead_fit(line_len: usize, read_ahead: usize, max_len: usize) -> bool {
    line_len
        .checked_add(read_ahead)
        .is_some_and(|count| count <= max_len)
}

pub fn window_and_filter_ok(window_size: usize, filter_block_size: usize) -> bool {
    window_size > 0 && filter_block_size <= window_size
}

pub fn cursor_order_ok(src: usize, dst: usize) -> bool {
    src <= dst
}

pub fn monotonic_seek_ok(previous: u64, next: u64, upper_bound: u64) -> bool {
    next >= previous && next <= upper_bound
}

pub fn longlink_complete(payload: &[u8]) -> bool {
    payload.last().copied() == Some(0)
}

pub fn zip_extra_span_ok(
    name_len: usize,
    extra_offset: usize,
    extra_len: usize,
    total_extra: usize,
) -> bool {
    name_len > 0
        && extra_offset
            .checked_add(extra_len)
            .is_some_and(|end| end <= total_extra)
}

pub fn cpio_symlink_size_ok(declared_size: i64, available_bytes: usize) -> bool {
    declared_size >= 0 && (declared_size as u64) <= available_bytes as u64
}

pub fn substream_count_ok(file_count: usize, substreams: u64, usize_max: u64) -> bool {
    substreams <= usize_max && usize::try_from(substreams).is_ok_and(|count| count >= file_count)
}

pub fn skip_target_ok(current: u64, size: u64, block_size: u64, physical_end: u64) -> bool {
    if block_size == 0 {
        return false;
    }
    let padded = size
        .checked_add(block_size - 1)
        .and_then(|value| value.checked_div(block_size))
        .and_then(|blocks| blocks.checked_mul(block_size));
    padded
        .and_then(|padded| current.checked_add(padded))
        .is_some_and(|target| target <= physical_end)
}
