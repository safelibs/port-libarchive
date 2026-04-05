/*-
 * Copyright (c) 2011 Tim Kientzle
 * Copyright (c) 2014 Michihiro NAKAJIMA
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

#define ZIP_BUFFER_SIZE 65536

static int
zip_encryption_supported(void)
{
	struct archive *a;
	int r;

	assert((a = archive_write_new()) != NULL);
	assertEqualIntA(a, ARCHIVE_OK, archive_write_set_format_zip(a));
	assertEqualIntA(a, ARCHIVE_OK, archive_write_add_filter_none(a));
	assertEqualIntA(a, ARCHIVE_OK,
	    archive_write_set_options(a, "zip:compression=store"));
	r = archive_write_set_options(a, "zip:encryption=traditional");
	assertEqualInt(ARCHIVE_OK, archive_write_free(a));
	return (r == ARCHIVE_OK);
}

static void
write_zip_entry(struct archive *a, const char *pathname, const char *contents)
{
	struct archive_entry *ae;
	size_t size = strlen(contents);

	assert((ae = archive_entry_new()) != NULL);
	archive_entry_copy_pathname(ae, pathname);
	archive_entry_set_mode(ae, AE_IFREG | 0644);
	archive_entry_set_size(ae, size);
	assertEqualIntA(a, ARCHIVE_OK, archive_write_header(a, ae));
	assertEqualInt((int)size, archive_write_data(a, contents, size));
	archive_entry_free(ae);
}

static void
create_encrypted_zip_with_passphrase(const char *passphrase, int pristine,
    char *buff, size_t buffsize, size_t *used)
{
	struct archive *a;

	assert((a = archive_write_new()) != NULL);
	if (pristine)
		assertEqualIntA(a, ARCHIVE_OK,
		    archive_write_set_passphrase(a, passphrase));
	assertEqualIntA(a, ARCHIVE_OK, archive_write_set_format_zip(a));
	assertEqualIntA(a, ARCHIVE_OK, archive_write_add_filter_none(a));
	assertEqualIntA(a, ARCHIVE_OK,
	    archive_write_set_options(a, "zip:compression=store"));
	assertEqualIntA(a, ARCHIVE_OK,
	    archive_write_set_options(a, "zip:encryption=traditional"));
	assertEqualIntA(a, ARCHIVE_OK,
	    archive_write_set_options(a, "zip:experimental"));
	if (!pristine)
		assertEqualIntA(a, ARCHIVE_OK,
		    archive_write_set_passphrase(a, passphrase));
	assertEqualIntA(a, ARCHIVE_OK,
	    archive_write_open_memory(a, buff, buffsize, used));
	write_zip_entry(a, "first.txt", "first");
	assertEqualIntA(a, ARCHIVE_OK, archive_write_close(a));
	assertEqualInt(ARCHIVE_OK, archive_write_free(a));
}

static void
create_encrypted_zip_with_callback(archive_passphrase_callback *callback,
    void *client_data, char *buff, size_t buffsize, size_t *used)
{
	struct archive *a;

	assert((a = archive_write_new()) != NULL);
	assertEqualIntA(a, ARCHIVE_OK, archive_write_set_format_zip(a));
	assertEqualIntA(a, ARCHIVE_OK, archive_write_add_filter_none(a));
	assertEqualIntA(a, ARCHIVE_OK,
	    archive_write_set_options(a, "zip:compression=store"));
	assertEqualIntA(a, ARCHIVE_OK,
	    archive_write_set_options(a, "zip:encryption=traditional"));
	assertEqualIntA(a, ARCHIVE_OK,
	    archive_write_set_options(a, "zip:experimental"));
	assertEqualIntA(a, ARCHIVE_OK,
	    archive_write_set_passphrase_callback(a, client_data, callback));
	assertEqualIntA(a, ARCHIVE_OK,
	    archive_write_open_memory(a, buff, buffsize, used));
	write_zip_entry(a, "first.txt", "first");
	write_zip_entry(a, "second.txt", "second");
	assertEqualIntA(a, ARCHIVE_OK, archive_write_close(a));
	assertEqualInt(ARCHIVE_OK, archive_write_free(a));
}

static void
assert_zip_data_fails(const char *buff, size_t used, const char *passphrase)
{
	struct archive *a;
	struct archive_entry *ae;
	char data[16];

	assert((a = archive_read_new()) != NULL);
	assertEqualIntA(a, ARCHIVE_OK, archive_read_support_filter_all(a));
	assertEqualIntA(a, ARCHIVE_OK, archive_read_support_format_all(a));
	if (passphrase != NULL) {
		assertEqualIntA(a, ARCHIVE_OK,
		    archive_read_add_passphrase(a, passphrase));
	}
	assertEqualIntA(a, ARCHIVE_OK, archive_read_open_memory(a, buff, used));
	assertEqualIntA(a, ARCHIVE_OK, archive_read_next_header(a, &ae));
	assertEqualString("first.txt", archive_entry_pathname(ae));
	assertEqualInt(ARCHIVE_FAILED, archive_read_data(a, data, sizeof(data)));
	assertEqualInt(ARCHIVE_OK, archive_read_free(a));
}

static void
assert_zip_data_succeeds(const char *buff, size_t used, const char *passphrase,
    int entries)
{
	struct archive *a;
	struct archive_entry *ae;
	char data[16];

	assert((a = archive_read_new()) != NULL);
	assertEqualIntA(a, ARCHIVE_OK, archive_read_support_filter_all(a));
	assertEqualIntA(a, ARCHIVE_OK, archive_read_support_format_all(a));
	assertEqualIntA(a, ARCHIVE_OK,
	    archive_read_add_passphrase(a, passphrase));
	assertEqualIntA(a, ARCHIVE_OK, archive_read_open_memory(a, buff, used));

	assertEqualIntA(a, ARCHIVE_OK, archive_read_next_header(a, &ae));
	assertEqualString("first.txt", archive_entry_pathname(ae));
	assertEqualInt(5, archive_read_data(a, data, sizeof(data)));
	assertEqualMem("first", data, 5);

	if (entries > 1) {
		assertEqualIntA(a, ARCHIVE_OK, archive_read_next_header(a, &ae));
		assertEqualString("second.txt", archive_entry_pathname(ae));
		assertEqualInt(6, archive_read_data(a, data, sizeof(data)));
		assertEqualMem("second", data, 6);
	}

	assertEqualIntA(a, ARCHIVE_EOF, archive_read_next_header(a, &ae));
	assertEqualInt(ARCHIVE_OK, archive_read_free(a));
}

static void
test_set_passphrase(int pristine)
{
	char buff[ZIP_BUFFER_SIZE];
	size_t used;
	struct archive *a;

	assert((a = archive_write_new()) != NULL);
	if (!pristine) {
		assertEqualIntA(a, ARCHIVE_OK, archive_write_set_format_zip(a));
		assertEqualIntA(a, ARCHIVE_OK, archive_write_add_filter_none(a));
	}
	assertEqualIntA(a, ARCHIVE_OK, archive_write_set_passphrase(a, "pass1"));
	assertEqualIntA(a, ARCHIVE_FAILED, archive_write_set_passphrase(a, ""));
	assertEqualIntA(a, ARCHIVE_FAILED, archive_write_set_passphrase(a, NULL));
	assertEqualIntA(a, ARCHIVE_OK, archive_write_set_passphrase(a, "pass2"));
	assertEqualInt(ARCHIVE_OK, archive_write_free(a));

	create_encrypted_zip_with_passphrase("pass2", pristine, buff, sizeof(buff),
	    &used);
	assert_zip_data_fails(buff, used, NULL);
	assert_zip_data_fails(buff, used, "pass1");
	assert_zip_data_succeeds(buff, used, "pass2", 1);
}

static const char *
callback1(struct archive *a, void *_client_data)
{
	int *cnt = (int *)_client_data;

	(void)a;
	*cnt += 1;
	return ("passCallBack");
}

DEFINE_TEST(test_archive_write_set_passphrase)
{
	if (!zip_encryption_supported()) {
		skipping("Traditional ZIP encryption is unsupported on this "
		    "platform");
		return;
	}

	test_set_passphrase(1);
	test_set_passphrase(0);
}

DEFINE_TEST(test_archive_write_set_passphrase_callback)
{
	char buff[ZIP_BUFFER_SIZE];
	size_t used;
	int cnt = 0;

	if (!zip_encryption_supported()) {
		skipping("Traditional ZIP encryption is unsupported on this "
		    "platform");
		return;
	}

	create_encrypted_zip_with_callback(callback1, &cnt, buff, sizeof(buff),
	    &used);
	assertEqualInt(1, cnt);
	assert_zip_data_succeeds(buff, used, "passCallBack", 2);
}
