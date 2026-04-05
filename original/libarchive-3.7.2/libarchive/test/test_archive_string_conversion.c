/*-
 * Copyright (c) 2011-2012 Michihiro NAKAJIMA
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

#include <locale.h>

static void
test_archive_entry_conversion_matrix(void)
{
	struct archive_entry *entry;

	assert((entry = archive_entry_new()) != NULL);

	archive_entry_set_pathname_utf8(entry, "\xD0\xBF\xD1\x80\xD0\xB8");
	assertEqualUTF8String("\xD0\xBF\xD1\x80\xD0\xB8",
	    archive_entry_pathname_utf8(entry));
	assertEqualWString(L"\x043f\x0440\x0438", archive_entry_pathname_w(entry));
	archive_entry_copy_pathname_w(entry, L"\x65e5\x672c.txt");
	assertEqualUTF8String("\xE6\x97\xA5\xE6\x9C\xAC.txt",
	    archive_entry_pathname_utf8(entry));
	assertEqualWString(L"\x65e5\x672c.txt", archive_entry_pathname_w(entry));
	archive_entry_update_pathname_utf8(entry,
	    "renamed/\xE6\x97\xA5.txt");
	assertEqualUTF8String("renamed/\xE6\x97\xA5.txt",
	    archive_entry_pathname_utf8(entry));
	assertEqualWString(L"renamed/\x65e5.txt",
	    archive_entry_pathname_w(entry));

	archive_entry_set_uname_utf8(entry, "\xD0\xB8\xD0\xBC\xD1\x8F");
	assertEqualUTF8String("\xD0\xB8\xD0\xBC\xD1\x8F",
	    archive_entry_uname_utf8(entry));
	assertEqualWString(L"\x0438\x043c\x044f", archive_entry_uname_w(entry));
	archive_entry_copy_uname_w(entry, L"\x65e5\x672c");
	assertEqualUTF8String("\xE6\x97\xA5\xE6\x9C\xAC",
	    archive_entry_uname_utf8(entry));
	assertEqualWString(L"\x65e5\x672c", archive_entry_uname_w(entry));
	archive_entry_update_uname_utf8(entry,
	    "user-\xE6\x97\xA5");
	assertEqualUTF8String("user-\xE6\x97\xA5",
	    archive_entry_uname_utf8(entry));
	assertEqualWString(L"user-\x65e5", archive_entry_uname_w(entry));

	archive_entry_set_gname_utf8(entry, "\xD0\xB3\xD1\x80\xD1\x83\xD0\xBF\xD0\xBF\xD0\xB0");
	assertEqualUTF8String(
	    "\xD0\xB3\xD1\x80\xD1\x83\xD0\xBF\xD0\xBF\xD0\xB0",
	    archive_entry_gname_utf8(entry));
	assertEqualWString(L"\x0433\x0440\x0443\x043f\x043f\x0430",
	    archive_entry_gname_w(entry));
	archive_entry_copy_gname_w(entry, L"\x7d44");
	assertEqualUTF8String("\xE7\xB5\x84", archive_entry_gname_utf8(entry));
	assertEqualWString(L"\x7d44", archive_entry_gname_w(entry));
	archive_entry_update_gname_utf8(entry,
	    "group-\xE6\x97\xA5");
	assertEqualUTF8String("group-\xE6\x97\xA5",
	    archive_entry_gname_utf8(entry));
	assertEqualWString(L"group-\x65e5", archive_entry_gname_w(entry));

	archive_entry_set_hardlink_utf8(entry, "\xD1\x86\xD0\xB5\xD0\xBB\xD1\x8C");
	assertEqualUTF8String("\xD1\x86\xD0\xB5\xD0\xBB\xD1\x8C",
	    archive_entry_hardlink_utf8(entry));
	assertEqualWString(L"\x0446\x0435\x043b\x044c",
	    archive_entry_hardlink_w(entry));
	archive_entry_copy_hardlink_w(entry, L"\x65e5\x672c-link");
	assertEqualUTF8String("\xE6\x97\xA5\xE6\x9C\xAC-link",
	    archive_entry_hardlink_utf8(entry));
	assertEqualWString(L"\x65e5\x672c-link",
	    archive_entry_hardlink_w(entry));
	archive_entry_update_hardlink_utf8(entry,
	    "hard-\xE6\x97\xA5");
	assertEqualUTF8String("hard-\xE6\x97\xA5",
	    archive_entry_hardlink_utf8(entry));
	assertEqualWString(L"hard-\x65e5",
	    archive_entry_hardlink_w(entry));

	archive_entry_set_symlink_utf8(entry, "\xD0\xBF\xD1\x83\xD1\x82\xD1\x8C");
	assertEqualUTF8String("\xD0\xBF\xD1\x83\xD1\x82\xD1\x8C",
	    archive_entry_symlink_utf8(entry));
	assertEqualWString(L"\x043f\x0443\x0442\x044c",
	    archive_entry_symlink_w(entry));
	archive_entry_copy_symlink_w(entry, L"\x65e5\x672c-path");
	assertEqualUTF8String("\xE6\x97\xA5\xE6\x9C\xAC-path",
	    archive_entry_symlink_utf8(entry));
	assertEqualWString(L"\x65e5\x672c-path",
	    archive_entry_symlink_w(entry));
	archive_entry_update_symlink_utf8(entry,
	    "sym-\xE6\x97\xA5");
	assertEqualUTF8String("sym-\xE6\x97\xA5",
	    archive_entry_symlink_utf8(entry));
	assertEqualWString(L"sym-\x65e5",
	    archive_entry_symlink_w(entry));

	archive_entry_free(entry);
}

static const char *
expected_o_umlaut_pathname(void)
{
#if defined(__APPLE__)
	return ("norm-o\xCC\x88.txt");
#else
	return ("norm-\xC3\xB6.txt");
#endif
}

static const char *
expected_a_umlaut_pathname(void)
{
#if defined(__APPLE__)
	return ("norm-a\xCC\x88.txt");
#else
	return ("norm-\xC3\xA4.txt");
#endif
}

static void
test_archive_pax_roundtrip_conversion(void)
{
	struct archive *a;
	struct archive_entry *entry;
	char buff[16384];
	size_t used;
	const char *expected_path;

	assert((a = archive_write_new()) != NULL);
	assertEqualInt(ARCHIVE_OK, archive_write_set_format_pax(a));
	assertEqualInt(ARCHIVE_OK, archive_write_add_filter_none(a));
	assertEqualInt(ARCHIVE_OK, archive_write_set_bytes_per_block(a, 0));
	assertEqualInt(ARCHIVE_OK,
	    archive_write_open_memory(a, buff, sizeof(buff), &used));

	assert((entry = archive_entry_new2(a)) != NULL);
	archive_entry_copy_pathname_w(entry,
	    L"wide-\x65e5\x672c/\x043f\x0440\x0438.txt");
	archive_entry_copy_uname_w(entry, L"\x0438\x043c\x044f");
	archive_entry_copy_gname_w(entry, L"\x7d44");
	archive_entry_set_mode(entry, AE_IFREG | 0644);
	archive_entry_set_size(entry, 0);
	assertEqualInt(ARCHIVE_OK, archive_write_header(a, entry));
	archive_entry_free(entry);

	assert((entry = archive_entry_new2(a)) != NULL);
	archive_entry_set_pathname(entry, "hardlink-entry");
	archive_entry_copy_hardlink_w(entry, L"target-\x65e5\x672c");
	archive_entry_set_mode(entry, AE_IFREG | 0644);
	archive_entry_set_size(entry, 0);
	assertEqualInt(ARCHIVE_OK, archive_write_header(a, entry));
	archive_entry_free(entry);

	assert((entry = archive_entry_new2(a)) != NULL);
	archive_entry_set_pathname(entry, "symlink-entry");
	archive_entry_copy_symlink_w(entry, L"link-\x043f\x0443\x0442\x044c");
	archive_entry_set_mode(entry, AE_IFLNK | 0755);
	archive_entry_set_size(entry, 0);
	assertEqualInt(ARCHIVE_OK, archive_write_header(a, entry));
	archive_entry_free(entry);

	assert((entry = archive_entry_new2(a)) != NULL);
	archive_entry_set_pathname_utf8(entry, "norm-o\xCC\x88.txt");
	archive_entry_set_mode(entry, AE_IFREG | 0644);
	archive_entry_set_size(entry, 0);
	assertEqualInt(ARCHIVE_OK, archive_write_header(a, entry));
	archive_entry_free(entry);

	assert((entry = archive_entry_new2(a)) != NULL);
	archive_entry_copy_pathname_w(entry, L"norm-\x00e4.txt");
	archive_entry_set_mode(entry, AE_IFREG | 0644);
	archive_entry_set_size(entry, 0);
	assertEqualInt(ARCHIVE_OK, archive_write_header(a, entry));
	archive_entry_free(entry);

	assertEqualInt(ARCHIVE_OK, archive_write_close(a));
	assertEqualInt(ARCHIVE_OK, archive_write_free(a));

	assert((a = archive_read_new()) != NULL);
	assertEqualInt(ARCHIVE_OK, archive_read_support_filter_all(a));
	assertEqualInt(ARCHIVE_OK, archive_read_support_format_all(a));
	assertEqualInt(ARCHIVE_OK, archive_read_open_memory(a, buff, used));

	assertEqualIntA(a, ARCHIVE_OK, archive_read_next_header(a, &entry));
	assertEqualUTF8String("wide-\xE6\x97\xA5\xE6\x9C\xAC/\xD0\xBF\xD1\x80\xD0\xB8.txt",
	    archive_entry_pathname_utf8(entry));
	assertEqualWString(L"wide-\x65e5\x672c/\x043f\x0440\x0438.txt",
	    archive_entry_pathname_w(entry));
	assertEqualUTF8String("\xD0\xB8\xD0\xBC\xD1\x8F",
	    archive_entry_uname_utf8(entry));
	assertEqualWString(L"\x0438\x043c\x044f", archive_entry_uname_w(entry));
	assertEqualUTF8String("\xE7\xB5\x84", archive_entry_gname_utf8(entry));
	assertEqualWString(L"\x7d44", archive_entry_gname_w(entry));

	assertEqualIntA(a, ARCHIVE_OK, archive_read_next_header(a, &entry));
	assertEqualString("hardlink-entry", archive_entry_pathname(entry));
	assertEqualUTF8String("target-\xE6\x97\xA5\xE6\x9C\xAC",
	    archive_entry_hardlink_utf8(entry));
	assertEqualWString(L"target-\x65e5\x672c",
	    archive_entry_hardlink_w(entry));

	assertEqualIntA(a, ARCHIVE_OK, archive_read_next_header(a, &entry));
	assertEqualString("symlink-entry", archive_entry_pathname(entry));
	assertEqualUTF8String("link-\xD0\xBF\xD1\x83\xD1\x82\xD1\x8C",
	    archive_entry_symlink_utf8(entry));
	assertEqualWString(L"link-\x043f\x0443\x0442\x044c",
	    archive_entry_symlink_w(entry));

	expected_path = expected_o_umlaut_pathname();
	assertEqualIntA(a, ARCHIVE_OK, archive_read_next_header(a, &entry));
	assertEqualUTF8String(expected_path, archive_entry_pathname_utf8(entry));

	expected_path = expected_a_umlaut_pathname();
	assertEqualIntA(a, ARCHIVE_OK, archive_read_next_header(a, &entry));
	assertEqualUTF8String(expected_path, archive_entry_pathname_utf8(entry));

	assertEqualIntA(a, ARCHIVE_EOF, archive_read_next_header(a, &entry));
	assertEqualInt(ARCHIVE_OK, archive_read_free(a));
}

static int
have_zip_hdrcharset_utf8(void)
{
	struct archive *a;
	int r;

	assert((a = archive_write_new()) != NULL);
	assertEqualInt(ARCHIVE_OK, archive_write_set_format_zip(a));
	r = archive_write_set_options(a, "hdrcharset=UTF-8");
	assertEqualInt(ARCHIVE_OK, archive_write_free(a));
	return (r == ARCHIVE_OK);
}

static void
assert_zip_path_roundtrip(const char *options, const wchar_t *wpathname,
    const char *utf8_pathname, int expected_flag,
    const char *expected_raw_path, size_t expected_raw_length,
    const char *expected_utf8_path, const wchar_t *expected_wpath)
{
	struct archive *a;
	struct archive_entry *entry;
	char buff[4096];
	size_t used;

	assert((a = archive_write_new()) != NULL);
	assertEqualInt(ARCHIVE_OK, archive_write_set_format_zip(a));
	if (options != NULL) {
		assertEqualInt(ARCHIVE_OK, archive_write_set_options(a, options));
	}
	assertEqualInt(ARCHIVE_OK,
	    archive_write_open_memory(a, buff, sizeof(buff), &used));

	assert((entry = archive_entry_new2(a)) != NULL);
	if (wpathname != NULL)
		archive_entry_copy_pathname_w(entry, wpathname);
	else
		archive_entry_set_pathname_utf8(entry, utf8_pathname);
	archive_entry_set_mode(entry, AE_IFREG | 0644);
	archive_entry_set_size(entry, 0);
	assertEqualInt(ARCHIVE_OK, archive_write_header(a, entry));
	archive_entry_free(entry);
	assertEqualInt(ARCHIVE_OK, archive_write_close(a));
	assertEqualInt(ARCHIVE_OK, archive_write_free(a));

	assertEqualInt(expected_flag, buff[7]);
	assertEqualMem(buff + 30, expected_raw_path, expected_raw_length);

	assert((a = archive_read_new()) != NULL);
	assertEqualInt(ARCHIVE_OK, archive_read_support_filter_all(a));
	assertEqualInt(ARCHIVE_OK, archive_read_support_format_all(a));
	assertEqualInt(ARCHIVE_OK, archive_read_open_memory(a, buff, used));
	assertEqualIntA(a, ARCHIVE_OK, archive_read_next_header(a, &entry));
	assertEqualUTF8String(expected_utf8_path, archive_entry_pathname_utf8(entry));
	assertEqualWString(expected_wpath, archive_entry_pathname_w(entry));
	assertEqualIntA(a, ARCHIVE_EOF, archive_read_next_header(a, &entry));
	assertEqualInt(ARCHIVE_OK, archive_read_free(a));
}

static void
test_archive_zip_roundtrip_conversion(void)
{
	int have_hdrcharset_utf8;

	have_hdrcharset_utf8 = have_zip_hdrcharset_utf8();
	if (!have_hdrcharset_utf8) {
		skipping("UTF-8 header conversion is unsupported on this platform");
	} else {
		assert_zip_path_roundtrip("hdrcharset=UTF-8",
		    L"\x043f\x0440\x0438.txt", NULL, 0x08,
		    "\xD0\xBF\xD1\x80\xD0\xB8.txt", 10,
		    "\xD0\xBF\xD1\x80\xD0\xB8.txt",
		    L"\x043f\x0440\x0438.txt");
		assert_zip_path_roundtrip("hdrcharset=UTF-8",
		    L"ascii.txt", NULL, 0,
		    "ascii.txt", 9, "ascii.txt", L"ascii.txt");
	}

	assert_zip_path_roundtrip(NULL, NULL,
	    "\xD0\xBF\xD1\x80\xD0\xB8.txt", 0x08,
	    "\xD0\xBF\xD1\x80\xD0\xB8.txt", 10,
	    "\xD0\xBF\xD1\x80\xD0\xB8.txt", L"\x043f\x0440\x0438.txt");
}

DEFINE_TEST(test_archive_string_conversion)
{
	if (NULL == setlocale(LC_ALL, "en_US.UTF-8")) {
		skipping("en_US.UTF-8 locale not available on this system.");
		return;
	}

	test_archive_entry_conversion_matrix();
	test_archive_pax_roundtrip_conversion();
	test_archive_zip_roundtrip_conversion();
}
