/*-
 * Copyright (c) 2003-2009 Tim Kientzle
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
__FBSDID("$FreeBSD: head/lib/libarchive/test/test_read_file_nonexistent.c 189473 2009-03-07 02:09:21Z kientzle $");

static void read_test(const char *name);
static void write_test(void);

static void
read_test(const char *name)
{
	struct archive* a = archive_read_new();
	int r;

	r = archive_read_support_filter_by_code(a, ARCHIVE_FILTER_BZIP2);
	if((ARCHIVE_WARN == r && !canBzip2()) || ARCHIVE_WARN > r) {
		skipping("bzip2 unsupported");
		assertEqualInt(ARCHIVE_OK, archive_read_free(a));
		return;
	}

	assertEqualIntA(a, ARCHIVE_OK, archive_read_support_format_all(a));

	extract_reference_file(name);
	assertEqualIntA(a, ARCHIVE_OK, archive_read_open_filename(a, name, 2));
	/* bzip2 and none */
	assertEqualInt(2, archive_filter_count(a));
	assertEqualInt(ARCHIVE_FILTER_BZIP2, archive_compression(a));
	assertEqualString("bzip2", archive_compression_name(a));

	assertEqualInt(ARCHIVE_OK, archive_read_free(a));
}

static void
write_test(void)
{
	char buff[4096];
	size_t used;
	struct archive* a = archive_write_new();
	int r;

	assertEqualInt(10240, archive_write_get_bytes_per_block(a));
	assertEqualIntA(a, ARCHIVE_OK,
	    archive_write_set_format(a, ARCHIVE_FORMAT_TAR_USTAR));
	assertEqualInt(ARCHIVE_FORMAT_TAR_USTAR & ARCHIVE_FORMAT_BASE_MASK,
	    archive_format(a) & ARCHIVE_FORMAT_BASE_MASK);
	assertEqualIntA(a, ARCHIVE_OK, archive_write_set_bytes_per_block(a, 10));
	assertEqualInt(10, archive_write_get_bytes_per_block(a));
	assertEqualIntA(a, ARCHIVE_OK,
	    archive_write_add_filter(a, ARCHIVE_FILTER_NONE));
	used = 0;
	assertEqualIntA(a, ARCHIVE_OK,
	    archive_write_open_memory(a, buff, sizeof(buff), &used));
	assertEqualInt(1, archive_filter_count(a));
	assertEqualInt(ARCHIVE_FILTER_NONE, archive_compression(a));
	assertEqualString(NULL, archive_compression_name(a));
	assertEqualInt(ARCHIVE_OK, archive_write_free(a));

	assert((a = archive_write_new()) != NULL);
	assertEqualIntA(a, ARCHIVE_OK,
	    archive_write_set_format(a, ARCHIVE_FORMAT_TAR_USTAR));
	assertEqualIntA(a, ARCHIVE_OK, archive_write_set_bytes_per_block(a, 10));
	assertEqualInt(10, archive_write_get_bytes_per_block(a));
	r = archive_write_add_filter(a, ARCHIVE_FILTER_BZIP2);
	if((ARCHIVE_WARN == r && !canBzip2()) || ARCHIVE_WARN > r) {
		skipping("bzip2 unsupported");
		assertEqualInt(ARCHIVE_OK, archive_write_free(a));
		return;
	}
	used = 0;
	assertEqualIntA(a, ARCHIVE_OK,
	    archive_write_open_memory(a, buff, sizeof(buff), &used));
	/* bzip2 and none */
	assertEqualInt(2, archive_filter_count(a));
	assertEqualInt(ARCHIVE_FILTER_BZIP2, archive_compression(a));
	assertEqualString("bzip2", archive_compression_name(a));
	assertEqualInt(ARCHIVE_OK, archive_write_free(a));
}

DEFINE_TEST(test_filter_count)
{
	read_test("test_compat_bzip2_1.tbz");
	write_test();
}
