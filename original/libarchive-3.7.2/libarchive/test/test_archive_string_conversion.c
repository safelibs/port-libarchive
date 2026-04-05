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
test_archive_entry_conversion(void)
{
	struct archive_entry *entry;
	const char *utf8 = "\xD0\xBF\xD1\x80\xD0\xB8";

	assert((entry = archive_entry_new()) != NULL);
	archive_entry_set_pathname_utf8(entry, utf8);
	assertEqualUTF8String(utf8, archive_entry_pathname_utf8(entry));
	assertEqualWString(L"\x043f\x0440\x0438", archive_entry_pathname_w(entry));

	archive_entry_copy_pathname_w(entry, L"\x65e5\x672c.txt");
	assertEqualUTF8String("\xE6\x97\xA5\xE6\x9C\xAC.txt",
	    archive_entry_pathname_utf8(entry));
	assertEqualWString(L"\x65e5\x672c.txt", archive_entry_pathname_w(entry));

	archive_entry_copy_uname_w(entry, L"\x0438\x043c\x044f");
	assertEqualUTF8String("\xD0\xB8\xD0\xBC\xD1\x8F",
	    archive_entry_uname_utf8(entry));
	assertEqualWString(L"\x0438\x043c\x044f", archive_entry_uname_w(entry));
	archive_entry_free(entry);
}

static void
test_archive_write_conversion(void)
{
	struct archive *a;
	struct archive_entry *entry;
	char buff[4096];
	size_t used;

	assert((a = archive_write_new()) != NULL);
	assertEqualInt(ARCHIVE_OK, archive_write_set_format_zip(a));
	if (archive_write_set_options(a, "hdrcharset=UTF-8") != ARCHIVE_OK) {
		skipping("UTF-8 header conversion is unsupported on this platform");
		archive_write_free(a);
		return;
	}
	assertEqualInt(ARCHIVE_OK,
	    archive_write_open_memory(a, buff, sizeof(buff), &used));

	assert((entry = archive_entry_new2(a)) != NULL);
	archive_entry_copy_pathname_w(entry, L"\x043f\x0440\x0438.txt");
	archive_entry_set_filetype(entry, AE_IFREG);
	archive_entry_set_size(entry, 0);
	assertEqualInt(ARCHIVE_OK, archive_write_header(a, entry));
	archive_entry_free(entry);
	assertEqualInt(ARCHIVE_OK, archive_write_free(a));

	assertEqualInt(0x08, buff[7]);
	assertEqualMem(buff + 30, "\xD0\xBF\xD1\x80\xD0\xB8.txt", 10);
}

DEFINE_TEST(test_archive_string_conversion)
{
	if (NULL == setlocale(LC_ALL, "en_US.UTF-8")) {
		skipping("en_US.UTF-8 locale not available on this system.");
		return;
	}

	test_archive_entry_conversion();
	test_archive_write_conversion();
}
