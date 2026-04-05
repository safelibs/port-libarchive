/*-
 * Copyright (c) 2003-2007 Tim Kientzle
 * All rights reserved.
 *
 * Redistribution and use in source and binary forms, with or without
 * modification, are permitted provided that the following conditions
 * are met:
 * 1. Redistributions of source code must retain the above copyright
 *    notice, this list of conditions and the following disclaimer.
 * 2. Redistributions in binary form must reproduce the above copyright
 *    notice, this list of conditions and the following disclaimer in the
 *    documentation and/or other materials provided with the distribution.
 *
 * THIS SOFTWARE IS PROVIDED BY THE AUTHOR(S) ``AS IS'' AND ANY EXPRESS OR
 * IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES
 * OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED.
 * IN NO EVENT SHALL THE AUTHOR(S) BE LIABLE FOR ANY DIRECT, INDIRECT,
 * INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT
 * NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
 * DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
 * THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
 * (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF
 * THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
 */
#include "test.h"
__FBSDID("$FreeBSD$");

static void
assert_normalized_pattern(const char *pattern, const char *unmatched)
{
	size_t len;

	assert(unmatched != NULL);
	len = strlen(pattern);
	if (len > 0 && pattern[len - 1] == '/')
		--len;
	assertEqualInt((int)len, (int)strlen(unmatched));
	assertEqualMem(pattern, unmatched, len);
}

static void
assert_invalid_include_pattern(const char *pattern)
{
	struct archive *m;

	if (!assert((m = archive_match_new()) != NULL))
		return;

	assertEqualIntA(m, ARCHIVE_FAILED,
	    archive_match_include_pattern(m, pattern));
	assertEqualInt(0, archive_match_path_unmatched_inclusions(m));
	assertEqualInt(ARCHIVE_OK, archive_match_free(m));
}

static void
assert_include_match(const char *pattern, const char *pathname,
    int recursive, int expected_match)
{
	struct archive *m;
	struct archive_entry *ae;
	const char *unmatched;

	if (!assert((m = archive_match_new()) != NULL))
		return;
	if (!assert((ae = archive_entry_new()) != NULL)) {
		archive_match_free(m);
		return;
	}

	assertEqualIntA(m, ARCHIVE_OK,
	    archive_match_set_inclusion_recursion(m, recursive));
	assertEqualIntA(m, ARCHIVE_OK,
	    archive_match_include_pattern(m, pattern));
	archive_entry_copy_pathname(ae, pathname);

	failure("include pattern '%s' against '%s'", pattern, pathname);
	assertEqualInt(expected_match ? 0 : 1,
	    archive_match_path_excluded(m, ae));

	assertEqualInt(expected_match ? 0 : 1,
	    archive_match_path_unmatched_inclusions(m));
	if (expected_match) {
		assertEqualIntA(m, ARCHIVE_EOF,
		    archive_match_path_unmatched_inclusions_next(m,
			&unmatched));
	} else {
		assertEqualIntA(m, ARCHIVE_OK,
		    archive_match_path_unmatched_inclusions_next(m,
			&unmatched));
		assert_normalized_pattern(pattern, unmatched);
		assertEqualIntA(m, ARCHIVE_EOF,
		    archive_match_path_unmatched_inclusions_next(m,
			&unmatched));
	}

	archive_entry_free(ae);
	archive_match_free(m);
}

static void
assert_include_match_w(const wchar_t *pattern, const wchar_t *pathname,
    int recursive, int expected_match)
{
	struct archive *m;
	struct archive_entry *ae;
	const wchar_t *unmatched;

	if (!assert((m = archive_match_new()) != NULL))
		return;
	if (!assert((ae = archive_entry_new()) != NULL)) {
		archive_match_free(m);
		return;
	}

	assertEqualIntA(m, ARCHIVE_OK,
	    archive_match_set_inclusion_recursion(m, recursive));
	assertEqualIntA(m, ARCHIVE_OK,
	    archive_match_include_pattern_w(m, pattern));
	archive_entry_copy_pathname_w(ae, pathname);

	assertEqualInt(expected_match ? 0 : 1,
	    archive_match_path_excluded(m, ae));
	assertEqualInt(expected_match ? 0 : 1,
	    archive_match_path_unmatched_inclusions(m));
	if (expected_match) {
		assertEqualIntA(m, ARCHIVE_EOF,
		    archive_match_path_unmatched_inclusions_next_w(m,
			&unmatched));
	} else {
		assertEqualIntA(m, ARCHIVE_OK,
		    archive_match_path_unmatched_inclusions_next_w(m,
			&unmatched));
		assertEqualWString(pattern, unmatched);
		assertEqualIntA(m, ARCHIVE_EOF,
		    archive_match_path_unmatched_inclusions_next_w(m,
			&unmatched));
	}

	archive_entry_free(ae);
	archive_match_free(m);
}

static void
assert_exclude_match(const char *pattern, const char *pathname,
    int expected_excluded)
{
	struct archive *m;
	struct archive_entry *ae;

	if (!assert((m = archive_match_new()) != NULL))
		return;
	if (!assert((ae = archive_entry_new()) != NULL)) {
		archive_match_free(m);
		return;
	}

	assertEqualIntA(m, ARCHIVE_OK,
	    archive_match_exclude_pattern(m, pattern));
	archive_entry_copy_pathname(ae, pathname);

	failure("exclude pattern '%s' against '%s'", pattern, pathname);
	assertEqualInt(expected_excluded, archive_match_path_excluded(m, ae));

	archive_entry_free(ae);
	archive_match_free(m);
}

/*
 * archive_pathmatch() itself is not a public API, so exercise the same
 * matcher behavior through archive_match_*(), which exposes exact,
 * recursive, anchored and unanchored path matching semantics publicly.
 */
DEFINE_TEST(test_archive_pathmatch)
{
	/* Exact matches via non-recursive inclusion. */
	assert_include_match("a/b/c", "a/b/c", 0, 1);
	assert_include_match("a/b/", "a/b/c", 0, 0);
	assert_include_match("a/b", "a/b/c", 0, 0);
	assert_invalid_include_pattern("");
	assert_include_match("*", "", 0, 1);
	assert_include_match("*", "a", 0, 1);
	assert_include_match("*", "abcd/efgh/ijkl", 0, 1);
	assert_include_match("?", "", 0, 0);
	assert_include_match("?", "a", 0, 1);
	assert_include_match("?", "ab", 0, 0);
	assert_include_match("a?c", "abc", 0, 1);
	assert_include_match("a?c", "a/c", 0, 1);
	assert_include_match("a?*c*", "a/c", 0, 1);
	assert_include_match("*a*", "a/c", 0, 1);
	assert_include_match("*a*", "/a/c", 0, 1);
	assert_include_match("a*", "defghi", 0, 0);
	assert_include_match("*a*", "defghi", 0, 0);

	/* Character classes and quoting. */
	assert_include_match("abc[def", "abc[def", 0, 1);
	assert_include_match("abc[def]", "abcd", 0, 1);
	assert_include_match("abc[def]", "abcg", 0, 0);
	assert_include_match("abc[d-fh-k]", "abck", 0, 1);
	assert_include_match("abc[d-fh-k]", "abcl", 0, 0);
	assert_include_match("abc[]efg", "abcdefg", 0, 0);
	assert_include_match("abc[!]efg", "abcqefg", 0, 1);
	assert_include_match("abc[d-fh-]", "abc-", 0, 1);
	assert_include_match("abc[\\]]", "abc]", 0, 1);
	assert_include_match("abc[d\\]]", "abcd", 0, 1);
	assert_include_match("abc[d\\]e]", "abcd]e", 0, 0);
	assert_include_match("abc[\\d-f]gh", "abcegh", 0, 1);
	assert_include_match("abc[d\\-f]gh", "abc-gh", 0, 1);
	assert_include_match("abc[!d]", "abcd", 0, 0);
	assert_include_match("abc[!d]", "abce", 0, 1);
	assert_include_match("abc[!d-z]", "abcq", 0, 0);
	assert_include_match("abc\\[def]", "abc[def]", 0, 1);
	assert_include_match("abc\\\\[def]", "abc\\d", 0, 1);
	assert_include_match("abcd\\", "abcd\\", 0, 1);
	assert_include_match("abcd\\[", "abcd\\", 0, 0);

	/* Canonical path handling. */
	assert_include_match("a/b/", "a/bc", 0, 0);
	assert_include_match("a/./b", "a/b", 0, 1);
	assert_include_match("a\\/./b", "a/b", 0, 0);
	assert_include_match("a/\\./b", "a/b", 0, 0);
	assert_include_match("./abc/./def/", "abc/def/", 0, 1);
	assert_include_match("abc/def", "./././abc/./def", 0, 1);
	assert_include_match("abc/def/././//", "./././abc/./def/", 0, 1);
	assert_include_match(".////abc/.//def", "./././abc/./def", 0, 1);
	assert_include_match("./abc?def/", "abc/def/", 0, 1);
	assert_include_match("./abc?./def/", "abc/def/", 0, 0);
	assert_include_match("./abc/./def/", "abc/def", 0, 1);
	assert_include_match("./abc/./def/./", "abc/def", 0, 1);
	assert_include_match("./abc/./def/.", "abc/def", 0, 1);
	assert_include_match("./abc*/./def", "abc/def/.", 0, 1);

	/* Start-anchored, end-unanchored semantics via recursive inclusion. */
	assert_include_match("abcd", "abcd", 1, 1);
	assert_include_match("abcd", "abcd/", 1, 1);
	assert_include_match("abcd", "abcd/.", 1, 1);
	assert_include_match("abc", "abcd", 1, 0);
	assert_include_match("a/b/c", "a/b/c/d", 1, 1);
	assert_include_match("a/b/c$", "a/b/c/d", 1, 0);
	assert_include_match("a/b/c$", "a/b/c", 1, 1);
	assert_include_match("a/b/c$", "a/b/c/", 1, 1);
	assert_include_match("a/b/c/", "a/b/c/d", 1, 1);
	assert_include_match("a/b/c/$", "a/b/c/d", 1, 0);
	assert_include_match("a/b/c/$", "a/b/c/", 1, 1);
	assert_include_match("a/b/c/$", "a/b/c", 1, 1);

	/* Unanchored matching semantics via exclusion patterns. */
	assert_exclude_match("bcd$", "abcd", 0);
	assert_exclude_match("abcd$", "abcd", 1);
	assert_exclude_match("^bcd$", "abcd", 0);
	assert_exclude_match("b/c/d$", "a/b/c/d", 1);
	assert_exclude_match("^b/c/d$", "a/b/c/d", 0);
	assert_exclude_match("/b/c/d$", "a/b/c/d", 0);
	assert_exclude_match("a/b/c$", "a/b/c/d", 0);
	assert_exclude_match("a/b/c/d$", "a/b/c/d", 1);
	assert_exclude_match("b/c$", "a/b/c/d", 0);
	assert_exclude_match("^b/c$", "a/b/c/d", 0);
	assert_exclude_match("b/c/d$", "/a/b/c/d", 1);
	assert_exclude_match("b/c", "a/b/c/d", 1);
	assert_exclude_match("/b/c", "a/b/c/d", 0);
	assert_exclude_match("/a/b/c", "a/b/c/d", 0);
	assert_exclude_match("/a/b/c", "/a/b/c/d", 1);
	assert_exclude_match("/a/b/c$", "a/b/c/d", 0);
	assert_exclude_match("/a/b/c/d$", "a/b/c/d", 0);
	assert_exclude_match("/a/b/c/d$", "/a/b/c/d/e", 0);
	assert_exclude_match("/a/b/c/d$", "/a/b/c/d", 1);
	assert_exclude_match("^a/b/c", "a/b/c/d", 1);
	assert_exclude_match("^a/b/c$", "a/b/c/d", 0);
	assert_exclude_match("a/b/c$", "a/b/c/d", 0);
	assert_exclude_match("b/c/d$", "a/b/c/d", 1);

	/* Wide-character public APIs still exercise the same matcher logic. */
	assert_include_match_w(L"a?c", L"a/c", 0, 1);
	assert_include_match_w(L"a/b/c$", L"a/b/c", 1, 1);
}
