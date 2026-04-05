use std::collections::{BTreeSet, HashMap};
use std::ffi::{c_int, CString};
use std::ptr;
use std::time::{SystemTime, UNIX_EPOCH};

use libc::{stat, wchar_t};

use crate::common::error::{
    ARCHIVE_EOF, ARCHIVE_FAILED, ARCHIVE_FATAL, ARCHIVE_MATCH_MAGIC, ARCHIVE_OK, ARCHIVE_STATE_ANY,
    ARCHIVE_STATE_NEW, ARCHIVE_WARN,
};
use crate::common::helpers::{from_optional_c_str, from_optional_wide, to_wide_null};
use crate::common::state::{
    archive_check_magic, clear_error, core_from_archive, set_error_string, ArchiveCore, ArchiveKind,
};
use crate::entry::internal::from_raw as entry_from_raw;
use crate::ffi::{archive, archive_entry};

const PATHMATCH_NO_ANCHOR_START: c_int = 1;
const PATHMATCH_NO_ANCHOR_END: c_int = 2;

#[derive(Clone)]
pub(crate) struct Pattern {
    pub(crate) text: String,
    pub(crate) wide: Vec<wchar_t>,
    pub(crate) matches: c_int,
}

impl Pattern {
    pub(crate) fn new(text: String) -> Self {
        Self {
            wide: to_wide_null(&text),
            text,
            matches: 0,
        }
    }
}

#[derive(Default)]
pub(crate) struct MatchList {
    pub(crate) patterns: Vec<Pattern>,
    pub(crate) unmatched_next: usize,
    pub(crate) unmatched_eof: bool,
}

impl MatchList {
    fn add(&mut self, pattern: String) {
        self.patterns.push(Pattern::new(pattern));
    }

    pub(crate) fn unmatched_count(&self) -> c_int {
        self.patterns
            .iter()
            .filter(|pattern| pattern.matches == 0)
            .count() as c_int
    }

    pub(crate) fn unmatched_next(&mut self, wide: bool) -> Option<(*const i8, *const wchar_t)> {
        if self.unmatched_eof {
            self.unmatched_eof = false;
            return None;
        }

        while self.unmatched_next < self.patterns.len() {
            let pattern = &self.patterns[self.unmatched_next];
            self.unmatched_next += 1;
            if pattern.matches != 0 {
                continue;
            }
            if self.unmatched_next == self.patterns.len() {
                self.unmatched_eof = true;
            }
            let c_ptr = CString::new(pattern.text.as_str())
                .expect("pattern")
                .into_raw()
                .cast_const();
            let w_ptr = pattern.wide.as_ptr();
            return Some(if wide {
                (ptr::null(), w_ptr)
            } else {
                (c_ptr, ptr::null())
            });
        }

        self.unmatched_next = 0;
        None
    }
}

#[derive(Clone, Copy)]
pub(crate) struct TimeFilter {
    pub(crate) flag: c_int,
    pub(crate) sec: i64,
    pub(crate) nsec: i64,
}

#[derive(Clone, Copy)]
pub(crate) struct PathTimeFilter {
    pub(crate) flag: c_int,
    pub(crate) mtime_sec: i64,
    pub(crate) mtime_nsec: i64,
    pub(crate) ctime_sec: i64,
    pub(crate) ctime_nsec: i64,
}

#[repr(C)]
pub(crate) struct MatchArchive {
    pub(crate) core: ArchiveCore,
    pub(crate) recursive_include: bool,
    pub(crate) inclusions: MatchList,
    pub(crate) exclusions: MatchList,
    pub(crate) newer_mtime: Option<TimeFilter>,
    pub(crate) older_mtime: Option<TimeFilter>,
    pub(crate) newer_ctime: Option<TimeFilter>,
    pub(crate) older_ctime: Option<TimeFilter>,
    pub(crate) path_time_filters: HashMap<String, PathTimeFilter>,
    pub(crate) inclusion_uids: BTreeSet<i64>,
    pub(crate) inclusion_gids: BTreeSet<i64>,
    pub(crate) inclusion_unames: Vec<Pattern>,
    pub(crate) inclusion_gnames: Vec<Pattern>,
    pub(crate) now: i64,
}

pub(crate) unsafe fn from_archive<'a>(a: *mut archive) -> Option<&'a mut MatchArchive> {
    a.cast::<MatchArchive>().as_mut()
}

pub(crate) fn new_archive() -> *mut archive {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    Box::into_raw(Box::new(MatchArchive {
        core: ArchiveCore::new(ArchiveKind::Match),
        recursive_include: true,
        inclusions: MatchList::default(),
        exclusions: MatchList::default(),
        newer_mtime: None,
        older_mtime: None,
        newer_ctime: None,
        older_ctime: None,
        path_time_filters: HashMap::new(),
        inclusion_uids: BTreeSet::new(),
        inclusion_gids: BTreeSet::new(),
        inclusion_unames: Vec::new(),
        inclusion_gnames: Vec::new(),
        now,
    })) as *mut archive
}

pub(crate) unsafe fn free_match_archive(a: *mut archive) {
    if !a.is_null() {
        drop(Box::from_raw(a.cast::<MatchArchive>()));
    }
}

pub(crate) unsafe fn validate_match_archive(a: *mut archive, function: &str) -> c_int {
    archive_check_magic(a, ARCHIVE_MATCH_MAGIC, ARCHIVE_STATE_NEW, function)
}

pub(crate) fn add_pattern(list: &mut MatchList, pattern: String) {
    let normalized = pattern.trim_end_matches('/').to_string();
    list.add(normalized);
}

pub(crate) fn add_pattern_from_file(
    list: &mut MatchList,
    path: &str,
    null_separator: c_int,
) -> c_int {
    let Ok(bytes) = std::fs::read(path) else {
        return ARCHIVE_FAILED;
    };
    let parts = if null_separator != 0 {
        bytes
            .split(|byte| *byte == 0)
            .map(|part| part.to_vec())
            .collect::<Vec<_>>()
    } else {
        bytes
            .split(|byte| *byte == b'\n' || *byte == b'\r')
            .map(|part| part.to_vec())
            .collect::<Vec<_>>()
    };
    for part in parts {
        if part.is_empty() {
            continue;
        }
        add_pattern(list, String::from_utf8_lossy(&part).into_owned());
    }
    ARCHIVE_OK
}

fn byte_at(bytes: &[u8], index: usize) -> u8 {
    bytes.get(index).copied().unwrap_or(0)
}

fn pm_list(list: &[u8], mut index: usize, end: usize, value: u8) -> bool {
    let mut range_start = 0u8;
    let mut matched = true;
    let mut nomatch = false;
    if index < end && matches!(list[index], b'!' | b'^') {
        matched = false;
        nomatch = true;
        index += 1;
    }

    while index < end {
        let mut next_range_start = 0;
        match list[index] {
            b'-' => {
                if range_start == 0 || index == end - 1 {
                    if list[index] == value {
                        return matched;
                    }
                } else {
                    index += 1;
                    let mut range_end = list[index];
                    if range_end == b'\\' && index + 1 < end {
                        index += 1;
                        range_end = list[index];
                    }
                    if range_start <= value && value <= range_end {
                        return matched;
                    }
                }
            }
            b'\\' => {
                index += 1;
                if index < end && list[index] == value {
                    return matched;
                }
                if index < end {
                    next_range_start = list[index];
                }
            }
            byte => {
                if byte == value {
                    return matched;
                }
                next_range_start = byte;
            }
        }
        range_start = next_range_start;
        index += 1;
    }
    nomatch
}

fn pm_slashskip(bytes: &[u8], mut index: usize) -> usize {
    loop {
        let current = byte_at(bytes, index);
        let next = byte_at(bytes, index + 1);
        if current == b'/' || (current == b'.' && next == b'/') || (current == b'.' && next == 0) {
            index += 1;
        } else {
            return index;
        }
    }
}

fn pm(pattern: &[u8], mut p: usize, source: &[u8], mut s: usize, flags: c_int) -> bool {
    if byte_at(source, s) == b'.' && byte_at(source, s + 1) == b'/' {
        s = pm_slashskip(source, s + 1);
    }
    if byte_at(pattern, p) == b'.' && byte_at(pattern, p + 1) == b'/' {
        p = pm_slashskip(pattern, p + 1);
    }

    loop {
        match byte_at(pattern, p) {
            0 => {
                if byte_at(source, s) == b'/' {
                    if (flags & PATHMATCH_NO_ANCHOR_END) != 0 {
                        return true;
                    }
                    s = pm_slashskip(source, s);
                }
                return byte_at(source, s) == 0;
            }
            b'?' => {
                if byte_at(source, s) == 0 {
                    return false;
                }
            }
            b'*' => {
                while byte_at(pattern, p) == b'*' {
                    p += 1;
                }
                if byte_at(pattern, p) == 0 {
                    return true;
                }
                let mut cursor = s;
                while byte_at(source, cursor) != 0 {
                    if archive_pathmatch_bytes(&pattern[p..], &source[cursor..], flags) {
                        return true;
                    }
                    cursor += 1;
                }
                return false;
            }
            b'[' => {
                let mut end = p + 1;
                while byte_at(pattern, end) != 0 && byte_at(pattern, end) != b']' {
                    if byte_at(pattern, end) == b'\\' && byte_at(pattern, end + 1) != 0 {
                        end += 1;
                    }
                    end += 1;
                }
                if byte_at(pattern, end) == b']' {
                    if !pm_list(pattern, p + 1, end, byte_at(source, s)) {
                        return false;
                    }
                    p = end;
                } else if byte_at(pattern, p) != byte_at(source, s) {
                    return false;
                }
            }
            b'\\' => {
                if byte_at(pattern, p + 1) == 0 {
                    if byte_at(source, s) != b'\\' {
                        return false;
                    }
                } else {
                    p += 1;
                    if byte_at(pattern, p) != byte_at(source, s) {
                        return false;
                    }
                }
            }
            b'/' => {
                if byte_at(source, s) != b'/' && byte_at(source, s) != 0 {
                    return false;
                }
                p = pm_slashskip(pattern, p);
                s = pm_slashskip(source, s);
                if byte_at(pattern, p) == 0 && (flags & PATHMATCH_NO_ANCHOR_END) != 0 {
                    return true;
                }
                p = p.saturating_sub(1);
                s = s.saturating_sub(1);
            }
            b'$' => {
                if byte_at(pattern, p + 1) == 0 && (flags & PATHMATCH_NO_ANCHOR_END) != 0 {
                    return byte_at(source, pm_slashskip(source, s)) == 0;
                }
                if byte_at(pattern, p) != byte_at(source, s) {
                    return false;
                }
            }
            byte => {
                if byte != byte_at(source, s) {
                    return false;
                }
            }
        }
        p += 1;
        s += 1;
    }
}

pub(crate) fn archive_pathmatch_bytes(pattern: &[u8], source: &[u8], mut flags: c_int) -> bool {
    if pattern.is_empty() {
        return source.is_empty();
    }

    let mut pattern_index = 0;
    if byte_at(pattern, 0) == b'^' {
        pattern_index += 1;
        flags &= !PATHMATCH_NO_ANCHOR_START;
    }

    if byte_at(pattern, pattern_index) == b'/' && byte_at(source, 0) != b'/' {
        return false;
    }

    if matches!(byte_at(pattern, pattern_index), b'*' | b'/') {
        while byte_at(pattern, pattern_index) == b'/' {
            pattern_index += 1;
        }
        let mut source_index = 0;
        while byte_at(source, source_index) == b'/' {
            source_index += 1;
        }
        return pm(pattern, pattern_index, source, source_index, flags);
    }

    if (flags & PATHMATCH_NO_ANCHOR_START) != 0 {
        let mut source_index = 0;
        loop {
            if pm(pattern, pattern_index, source, source_index, flags) {
                return true;
            }
            let Some(next) = source[source_index..].iter().position(|byte| *byte == b'/') else {
                break;
            };
            source_index += next + 1;
        }
        return false;
    }

    pm(pattern, pattern_index, source, 0, flags)
}

pub(crate) fn archive_pathmatch(pattern: &str, source: &str, flags: c_int) -> bool {
    archive_pathmatch_bytes(pattern.as_bytes(), source.as_bytes(), flags)
}

fn match_path_exclusion(pattern: &str, path: &str) -> bool {
    archive_pathmatch(
        pattern,
        path,
        PATHMATCH_NO_ANCHOR_START | PATHMATCH_NO_ANCHOR_END,
    )
}

fn match_path_inclusion(pattern: &str, path: &str, recursive: bool) -> bool {
    let flags = if recursive {
        PATHMATCH_NO_ANCHOR_END
    } else {
        0
    };
    archive_pathmatch(pattern, path, flags)
}

pub(crate) fn path_excluded(matcher: &mut MatchArchive, path: &str) -> c_int {
    let mut matched_index = None;
    for (index, inclusion) in matcher.inclusions.patterns.iter_mut().enumerate() {
        if inclusion.matches == 0
            && match_path_inclusion(&inclusion.text, path, matcher.recursive_include)
        {
            inclusion.matches += 1;
            matched_index = Some(index);
            break;
        }
    }

    for exclusion in &matcher.exclusions.patterns {
        if match_path_exclusion(&exclusion.text, path) {
            return 1;
        }
    }

    if matched_index.is_some() {
        return 0;
    }

    for inclusion in &mut matcher.inclusions.patterns {
        if inclusion.matches > 0
            && match_path_inclusion(&inclusion.text, path, matcher.recursive_include)
        {
            inclusion.matches += 1;
            return 0;
        }
    }

    if matcher.inclusions.patterns.is_empty() {
        0
    } else {
        1
    }
}

fn compare_time(actual_sec: i64, actual_nsec: i64, filter: TimeFilter, newer: bool) -> bool {
    if newer {
        if actual_sec < filter.sec {
            return true;
        }
        if actual_sec == filter.sec {
            if actual_nsec < filter.nsec {
                return true;
            }
            if actual_nsec == filter.nsec
                && (filter.flag & crate::ffi::archive_common::ARCHIVE_MATCH_EQUAL) == 0
            {
                return true;
            }
        }
    } else {
        if actual_sec > filter.sec {
            return true;
        }
        if actual_sec == filter.sec {
            if actual_nsec > filter.nsec {
                return true;
            }
            if actual_nsec == filter.nsec
                && (filter.flag & crate::ffi::archive_common::ARCHIVE_MATCH_EQUAL) == 0
            {
                return true;
            }
        }
    }
    false
}

pub(crate) fn time_excluded(matcher: &MatchArchive, entry: *mut archive_entry) -> c_int {
    let Some(entry) = (unsafe { entry_from_raw(entry) }) else {
        return ARCHIVE_FAILED;
    };

    let ctime_sec = if entry.ctime.set {
        entry.ctime.sec
    } else {
        entry.mtime.sec
    };
    let ctime_nsec = if entry.ctime.set {
        entry.ctime.nsec as i64
    } else {
        entry.mtime.nsec as i64
    };
    let mtime_sec = entry.mtime.sec;
    let mtime_nsec = entry.mtime.nsec as i64;

    if let Some(filter) = matcher.newer_ctime {
        if compare_time(ctime_sec, ctime_nsec, filter, true) {
            return 1;
        }
    }
    if let Some(filter) = matcher.older_ctime {
        if compare_time(ctime_sec, ctime_nsec, filter, false) {
            return 1;
        }
    }
    if let Some(filter) = matcher.newer_mtime {
        if compare_time(mtime_sec, mtime_nsec, filter, true) {
            return 1;
        }
    }
    if let Some(filter) = matcher.older_mtime {
        if compare_time(mtime_sec, mtime_nsec, filter, false) {
            return 1;
        }
    }

    if let Some(path) = entry.pathname.get_str() {
        if let Some(filter) = matcher.path_time_filters.get(path) {
            if (filter.flag & crate::ffi::archive_common::ARCHIVE_MATCH_CTIME) != 0 {
                if filter.ctime_sec > ctime_sec {
                    if (filter.flag & crate::ffi::archive_common::ARCHIVE_MATCH_OLDER) != 0 {
                        return 1;
                    }
                } else if filter.ctime_sec < ctime_sec {
                    if (filter.flag & crate::ffi::archive_common::ARCHIVE_MATCH_NEWER) != 0 {
                        return 1;
                    }
                } else if filter.ctime_nsec > ctime_nsec {
                    if (filter.flag & crate::ffi::archive_common::ARCHIVE_MATCH_OLDER) != 0 {
                        return 1;
                    }
                } else if filter.ctime_nsec < ctime_nsec {
                    if (filter.flag & crate::ffi::archive_common::ARCHIVE_MATCH_NEWER) != 0 {
                        return 1;
                    }
                } else if (filter.flag & crate::ffi::archive_common::ARCHIVE_MATCH_EQUAL) != 0 {
                    return 1;
                }
            }
            if (filter.flag & crate::ffi::archive_common::ARCHIVE_MATCH_MTIME) != 0 {
                if filter.mtime_sec > mtime_sec {
                    if (filter.flag & crate::ffi::archive_common::ARCHIVE_MATCH_OLDER) != 0 {
                        return 1;
                    }
                } else if filter.mtime_sec < mtime_sec {
                    if (filter.flag & crate::ffi::archive_common::ARCHIVE_MATCH_NEWER) != 0 {
                        return 1;
                    }
                } else if filter.mtime_nsec > mtime_nsec {
                    if (filter.flag & crate::ffi::archive_common::ARCHIVE_MATCH_OLDER) != 0 {
                        return 1;
                    }
                } else if filter.mtime_nsec < mtime_nsec {
                    if (filter.flag & crate::ffi::archive_common::ARCHIVE_MATCH_NEWER) != 0 {
                        return 1;
                    }
                } else if (filter.flag & crate::ffi::archive_common::ARCHIVE_MATCH_EQUAL) != 0 {
                    return 1;
                }
            }
        }
    }
    0
}

pub(crate) fn owner_excluded(matcher: &mut MatchArchive, entry: *mut archive_entry) -> c_int {
    let Some(entry) = (unsafe { entry_from_raw(entry) }) else {
        return ARCHIVE_FAILED;
    };

    if !matcher.inclusion_uids.is_empty() && !matcher.inclusion_uids.contains(&entry.uid) {
        return 1;
    }
    if !matcher.inclusion_gids.is_empty() && !matcher.inclusion_gids.contains(&entry.gid) {
        return 1;
    }
    if !matcher.inclusion_unames.is_empty() {
        let Some(name) = entry.uname.get_str() else {
            return 1;
        };
        if let Some(pattern) = matcher
            .inclusion_unames
            .iter_mut()
            .find(|pattern| pattern.text == name)
        {
            pattern.matches += 1;
        } else {
            return 1;
        }
    }
    if !matcher.inclusion_gnames.is_empty() {
        let Some(name) = entry.gname.get_str() else {
            return 1;
        };
        if let Some(pattern) = matcher
            .inclusion_gnames
            .iter_mut()
            .find(|pattern| pattern.text == name)
        {
            pattern.matches += 1;
        } else {
            return 1;
        }
    }
    0
}

pub(crate) fn validate_time_flag(matcher: &mut MatchArchive, flag: c_int) -> c_int {
    let time_bits = crate::ffi::archive_common::ARCHIVE_MATCH_MTIME
        | crate::ffi::archive_common::ARCHIVE_MATCH_CTIME;
    let comparison_bits = crate::ffi::archive_common::ARCHIVE_MATCH_NEWER
        | crate::ffi::archive_common::ARCHIVE_MATCH_OLDER
        | crate::ffi::archive_common::ARCHIVE_MATCH_EQUAL;

    if (flag & !(time_bits | comparison_bits)) != 0 {
        set_error_string(
            &mut matcher.core,
            libc::EINVAL,
            "Invalid time flag".to_string(),
        );
        return ARCHIVE_FAILED;
    }
    if (flag & time_bits) == 0 {
        set_error_string(&mut matcher.core, libc::EINVAL, "No time flag".to_string());
        return ARCHIVE_FAILED;
    }
    if (flag & comparison_bits) == 0 {
        set_error_string(
            &mut matcher.core,
            libc::EINVAL,
            "No comparison flag".to_string(),
        );
        return ARCHIVE_FAILED;
    }
    ARCHIVE_OK
}

pub(crate) fn set_timefilter(matcher: &mut MatchArchive, flag: c_int, sec: i64, nsec: i64) {
    let filter = TimeFilter { flag, sec, nsec };
    let equal_only = (flag
        & (crate::ffi::archive_common::ARCHIVE_MATCH_EQUAL
            | crate::ffi::archive_common::ARCHIVE_MATCH_NEWER
            | crate::ffi::archive_common::ARCHIVE_MATCH_OLDER))
        == crate::ffi::archive_common::ARCHIVE_MATCH_EQUAL;
    if (flag & crate::ffi::archive_common::ARCHIVE_MATCH_MTIME) != 0 {
        if (flag & crate::ffi::archive_common::ARCHIVE_MATCH_NEWER) != 0 || equal_only {
            matcher.newer_mtime = Some(filter);
        }
        if (flag & crate::ffi::archive_common::ARCHIVE_MATCH_OLDER) != 0 || equal_only {
            matcher.older_mtime = Some(filter);
        }
    }
    if (flag & crate::ffi::archive_common::ARCHIVE_MATCH_CTIME) != 0 {
        if (flag & crate::ffi::archive_common::ARCHIVE_MATCH_NEWER) != 0 || equal_only {
            matcher.newer_ctime = Some(filter);
        }
        if (flag & crate::ffi::archive_common::ARCHIVE_MATCH_OLDER) != 0 || equal_only {
            matcher.older_ctime = Some(filter);
        }
    }
}

fn month_number(month: &str) -> Option<u32> {
    match month.to_ascii_lowercase().as_str() {
        "jan" | "january" => Some(1),
        "feb" | "february" => Some(2),
        "mar" | "march" => Some(3),
        "apr" | "april" => Some(4),
        "may" => Some(5),
        "jun" | "june" => Some(6),
        "jul" | "july" => Some(7),
        "aug" | "august" => Some(8),
        "sep" | "sept" | "september" => Some(9),
        "oct" | "october" => Some(10),
        "nov" | "november" => Some(11),
        "dec" | "december" => Some(12),
        _ => None,
    }
}

fn timezone_offset(zone: &str) -> Option<i64> {
    match zone.to_ascii_uppercase().as_str() {
        "UTC" | "GMT" => Some(0),
        "EST" => Some(-5 * 3600),
        "PST" => Some(-8 * 3600),
        "MEST" => Some(2 * 3600),
        _ => None,
    }
}

fn parse_signed_hhmm(zone: &str) -> Option<i64> {
    if zone.len() != 5 {
        return None;
    }
    let sign = match &zone[0..1] {
        "+" => 1,
        "-" => -1,
        _ => return None,
    };
    let hour: i64 = zone[1..3].parse().ok()?;
    let minute: i64 = zone[3..5].parse().ok()?;
    Some(sign * (hour * 3600 + minute * 60))
}

fn normalize_year(year: i64) -> i64 {
    if year < 100 {
        if year >= 69 {
            1900 + year
        } else {
            2000 + year
        }
    } else {
        year
    }
}

fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let year = year - i64::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let yoe = year - era * 400;
    let month = month as i64;
    let day = day as i64;
    let doy = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn epoch_from_components(
    year: i64,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
    offset: i64,
) -> i64 {
    days_from_civil(year, month, day) * 86_400
        + hour as i64 * 3600
        + minute as i64 * 60
        + second as i64
        - offset
}

fn parse_relative_date(now: i64, text: &str) -> Option<i64> {
    let lower = text.trim().to_ascii_lowercase();
    if lower == "tomorrow" {
        return Some(now + 24 * 3600);
    }
    if lower == "yesterday" {
        return Some(now - 24 * 3600);
    }

    let mut rest = lower.as_str();
    if let Some(value) = rest.strip_prefix("now") {
        rest = value.trim();
    }
    if rest.is_empty() {
        return Some(now);
    }

    let ago = rest.contains("ago");
    let tokens: Vec<_> = rest
        .split_whitespace()
        .filter(|token| *token != "ago")
        .collect();
    let mut index = 0;
    let mut total = 0i64;
    while index < tokens.len() {
        let mut sign = 1i64;
        let token = tokens[index];
        let number = if token == "+" || token == "-" {
            sign = if token == "-" { -1 } else { 1 };
            index += 1;
            tokens.get(index)?.parse::<i64>().ok()?
        } else {
            token.parse::<i64>().ok().or_else(|| {
                if let Some(stripped) = token.strip_prefix('+') {
                    stripped.parse::<i64>().ok()
                } else if let Some(stripped) = token.strip_prefix('-') {
                    sign = -1;
                    stripped.parse::<i64>().ok()
                } else {
                    None
                }
            })?
        };
        index += 1;
        let unit = *tokens.get(index)?;
        index += 1;
        let seconds = match unit {
            "hour" | "hours" => number * 3600,
            "minute" | "minutes" => number * 60,
            "day" | "days" => number * 86_400,
            _ => return None,
        };
        total += sign * seconds;
    }

    if ago {
        Some(now - total.abs())
    } else {
        Some(now + total)
    }
}

fn parse_time_token(token: &str) -> Option<(u32, u32, u32, Option<i64>)> {
    let token = token.trim();
    if let Some(value) = token
        .strip_suffix("am")
        .or_else(|| token.strip_suffix("pm"))
    {
        let pm = token.ends_with("pm");
        let (mut hour, minute, second, _) = parse_time_token(value)?;
        if hour == 12 {
            hour = 0;
        }
        if pm {
            hour += 12;
        }
        return Some((hour, minute, second, None));
    }

    if let Some(offset) = token
        .char_indices()
        .skip(1)
        .find(|(_, ch)| *ch == '+' || *ch == '-')
        .and_then(|(index, _)| parse_signed_hhmm(&token[index..]).map(|offset| (index, offset)))
    {
        let (index, offset) = offset;
        let (hour, minute, second, _) = parse_time_token(&token[..index])?;
        return Some((hour, minute, second, Some(offset)));
    }

    if token.contains(':') {
        let parts: Vec<_> = token.split(':').collect();
        let hour: u32 = parts.get(0)?.parse().ok()?;
        let minute: u32 = parts.get(1)?.parse().ok()?;
        let second: u32 = parts.get(2).and_then(|part| part.parse().ok()).unwrap_or(0);
        return Some((hour, minute, second, None));
    }

    if token.len() == 3 || token.len() == 4 {
        let hour: u32 = token[..token.len() - 2].parse().ok()?;
        let minute: u32 = token[token.len() - 2..].parse().ok()?;
        return Some((hour, minute, 0, None));
    }

    None
}

pub(crate) fn parse_date(now: i64, text: &str) -> Option<i64> {
    if let Some(relative) = parse_relative_date(now, text) {
        return Some(relative);
    }

    let cleaned = text.replace(',', "");
    let tokens: Vec<_> = cleaned.split_whitespace().collect();
    if tokens.len() == 4 && month_number(tokens[0]).is_some() {
        return Some(epoch_from_components(
            normalize_year(tokens[2].parse().ok()?),
            month_number(tokens[0])?,
            tokens[1].parse().ok()?,
            0,
            0,
            0,
            timezone_offset(tokens[3])?,
        ));
    }

    if tokens.len() == 3 && tokens[0].contains('/') {
        let date_parts: Vec<_> = tokens[0].split('/').collect();
        let time = parse_time_token(tokens[1])?;
        let offset = time.3.or_else(|| timezone_offset(tokens[2]))?;
        let year0: i64 = date_parts[0].parse().ok()?;
        let year2: i64 = date_parts[2].parse().ok()?;
        let (year, month, day) = if year0 >= 13 {
            (
                normalize_year(year0),
                date_parts[1].parse().ok()?,
                date_parts[2].parse().ok()?,
            )
        } else {
            (
                normalize_year(year2),
                date_parts[0].parse().ok()?,
                date_parts[1].parse().ok()?,
            )
        };
        return Some(epoch_from_components(
            year, month, day, time.0, time.1, time.2, offset,
        ));
    }

    if tokens.len() == 4 && tokens[0].contains('/') {
        let date_parts: Vec<_> = tokens[0].split('/').collect();
        let time = parse_time_token(tokens[1])?;
        let offset = time.3.or_else(|| timezone_offset(tokens[2]))?;
        let year0: i64 = date_parts[0].parse().ok()?;
        let year2: i64 = date_parts[2].parse().ok()?;
        let (year, month, day) = if year0 >= 13 {
            (
                normalize_year(year0),
                date_parts[1].parse().ok()?,
                date_parts[2].parse().ok()?,
            )
        } else {
            (
                normalize_year(year2),
                date_parts[0].parse().ok()?,
                date_parts[1].parse().ok()?,
            )
        };
        return Some(epoch_from_components(
            year, month, day, time.0, time.1, time.2, offset,
        ));
    }

    if tokens.len() == 5 && month_number(tokens[1]).is_some() {
        let time = parse_time_token(tokens[2])?;
        return Some(epoch_from_components(
            normalize_year(tokens[4].parse().ok()?),
            month_number(tokens[1])?,
            tokens[2 - 1].parse().ok()?,
            time.0,
            time.1,
            time.2,
            time.3.or_else(|| timezone_offset(tokens[3]))?,
        ));
    }

    if tokens.len() == 4 && tokens[2].contains(':') {
        return Some(epoch_from_components(
            normalize_year(tokens[3].parse().ok()?),
            month_number(tokens[0])?,
            tokens[1].parse().ok()?,
            parse_time_token(tokens[2])?.0,
            parse_time_token(tokens[2])?.1,
            parse_time_token(tokens[2])?.2,
            timezone_offset(tokens[3])?,
        ));
    }

    if tokens.len() == 5 && tokens[0].len() == 3 && month_number(tokens[1]).is_some() {
        let time = parse_time_token(tokens[3])?;
        return Some(epoch_from_components(
            normalize_year(tokens[4].parse().ok()?),
            month_number(tokens[1])?,
            tokens[2].parse().ok()?,
            time.0,
            time.1,
            time.2,
            timezone_offset(tokens[4 - 1])?,
        ));
    }

    None
}

pub(crate) fn file_times_from_path(path: &str) -> Option<PathTimeFilter> {
    let mut st: stat = unsafe { std::mem::zeroed() };
    let path = std::ffi::CString::new(path).ok()?;
    if unsafe { libc::stat(path.as_ptr(), &mut st) } != 0 {
        return None;
    }
    Some(PathTimeFilter {
        flag: 0,
        mtime_sec: i64::from(st.st_mtime),
        mtime_nsec: 0,
        ctime_sec: i64::from(st.st_ctime),
        ctime_nsec: 0,
    })
}
