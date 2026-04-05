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

struct callback_state {
	int count;
	int return_once;
};

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
create_encrypted_zip(const char *passphrase, int entries, char *buff,
    size_t buffsize, size_t *used)
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
	assertEqualIntA(a, ARCHIVE_OK, archive_write_set_passphrase(a, passphrase));
	assertEqualIntA(a, ARCHIVE_OK,
	    archive_write_open_memory(a, buff, buffsize, used));
	write_zip_entry(a, "first.txt", "first");
	if (entries > 1)
		write_zip_entry(a, "second.txt", "second");
	assertEqualIntA(a, ARCHIVE_OK, archive_write_close(a));
	assertEqualInt(ARCHIVE_OK, archive_write_free(a));
}

static struct archive *
new_reader(void)
{
	struct archive *a;

	assert((a = archive_read_new()) != NULL);
	assertEqualIntA(a, ARCHIVE_OK, archive_read_support_filter_all(a));
	assertEqualIntA(a, ARCHIVE_OK, archive_read_support_format_all(a));
	return (a);
}

static void
assert_first_entry_data_fails(struct archive *a)
{
	struct archive_entry *ae;
	char data[16];

	assertEqualIntA(a, ARCHIVE_OK, archive_read_next_header(a, &ae));
	assertEqualString("first.txt", archive_entry_pathname(ae));
	assertEqualInt(ARCHIVE_FAILED, archive_read_data(a, data, sizeof(data)));
}

static void
assert_entries_read(struct archive *a, int entries)
{
	struct archive_entry *ae;
	char data[16];

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
}

static void
validate_add_passphrase(int pristine)
{
	struct archive *a = archive_read_new();

	if (!pristine) {
		archive_read_support_filter_all(a);
		archive_read_support_format_all(a);
	}

	assertEqualInt(ARCHIVE_OK, archive_read_add_passphrase(a, "pass1"));
	assertEqualInt(ARCHIVE_FAILED, archive_read_add_passphrase(a, ""));
	assertEqualInt(ARCHIVE_FAILED, archive_read_add_passphrase(a, NULL));
	archive_read_free(a);
}

static const char *
callback_constant(struct archive *a, void *_client_data)
{
	struct callback_state *state = (struct callback_state *)_client_data;

	(void)a;
	state->count += 1;
	return ("passCallBack");
}

static const char *
callback_once(struct archive *a, void *_client_data)
{
	struct callback_state *state = (struct callback_state *)_client_data;

	(void)a;
	state->count += 1;
	if (state->return_once) {
		state->return_once = 0;
		return ("passCallBack");
	}
	return (NULL);
}

DEFINE_TEST(test_archive_read_add_passphrase)
{
	validate_add_passphrase(1);
	validate_add_passphrase(0);
}

DEFINE_TEST(test_archive_read_add_passphrase_incorrect_sequance)
{
	char buff[ZIP_BUFFER_SIZE];
	size_t used;
	struct archive *a;

	if (!zip_encryption_supported()) {
		skipping("Traditional ZIP encryption is unsupported on this "
		    "platform");
		return;
	}

	create_encrypted_zip("pass1", 1, buff, sizeof(buff), &used);
	a = new_reader();
	assertEqualIntA(a, ARCHIVE_OK, archive_read_open_memory(a, buff, used));
	assert_first_entry_data_fails(a);
	assertEqualInt(ARCHIVE_OK, archive_read_free(a));
}

DEFINE_TEST(test_archive_read_add_passphrase_single)
{
	char buff[ZIP_BUFFER_SIZE];
	size_t used;
	struct archive *a;

	if (!zip_encryption_supported()) {
		skipping("Traditional ZIP encryption is unsupported on this "
		    "platform");
		return;
	}

	create_encrypted_zip("pass1", 1, buff, sizeof(buff), &used);
	a = new_reader();
	assertEqualIntA(a, ARCHIVE_OK, archive_read_add_passphrase(a, "pass1"));
	assertEqualIntA(a, ARCHIVE_OK, archive_read_open_memory(a, buff, used));
	assert_entries_read(a, 1);
	assertEqualInt(ARCHIVE_OK, archive_read_free(a));
}

DEFINE_TEST(test_archive_read_add_passphrase_multiple)
{
	char buff[ZIP_BUFFER_SIZE];
	size_t used;
	struct archive *a;

	if (!zip_encryption_supported()) {
		skipping("Traditional ZIP encryption is unsupported on this "
		    "platform");
		return;
	}

	create_encrypted_zip("pass2", 1, buff, sizeof(buff), &used);
	a = new_reader();
	assertEqualIntA(a, ARCHIVE_OK,
	    archive_read_add_passphrase(a, "invalid"));
	assertEqualIntA(a, ARCHIVE_OK, archive_read_add_passphrase(a, "pass2"));
	assertEqualIntA(a, ARCHIVE_OK, archive_read_open_memory(a, buff, used));
	assert_entries_read(a, 1);
	assertEqualInt(ARCHIVE_OK, archive_read_free(a));
}

DEFINE_TEST(test_archive_read_add_passphrase_set_callback1)
{
	char buff[ZIP_BUFFER_SIZE];
	size_t used;
	struct archive *a;
	struct callback_state state;

	if (!zip_encryption_supported()) {
		skipping("Traditional ZIP encryption is unsupported on this "
		    "platform");
		return;
	}

	state.count = 0;
	state.return_once = 0;
	create_encrypted_zip("passCallBack", 2, buff, sizeof(buff), &used);
	a = new_reader();
	assertEqualIntA(a, ARCHIVE_OK,
	    archive_read_set_passphrase_callback(a, &state, callback_constant));
	assertEqualIntA(a, ARCHIVE_OK, archive_read_open_memory(a, buff, used));
	assert_entries_read(a, 2);
	assertEqualInt(1, state.count);
	assertEqualInt(ARCHIVE_OK, archive_read_free(a));
}

DEFINE_TEST(test_archive_read_add_passphrase_set_callback2)
{
	char buff[ZIP_BUFFER_SIZE];
	size_t used;
	struct archive *a;
	struct callback_state state;

	if (!zip_encryption_supported()) {
		skipping("Traditional ZIP encryption is unsupported on this "
		    "platform");
		return;
	}

	state.count = 0;
	state.return_once = 1;
	create_encrypted_zip("passCallBack", 1, buff, sizeof(buff), &used);
	a = new_reader();
	assertEqualIntA(a, ARCHIVE_OK,
	    archive_read_set_passphrase_callback(a, &state, callback_once));
	assertEqualIntA(a, ARCHIVE_OK, archive_read_open_memory(a, buff, used));
	assert_entries_read(a, 1);
	assertEqualInt(1, state.count);
	assertEqualInt(ARCHIVE_OK, archive_read_free(a));
}

DEFINE_TEST(test_archive_read_add_passphrase_set_callback3)
{
	char buff[ZIP_BUFFER_SIZE];
	size_t used;
	struct archive *a;
	struct callback_state state;

	if (!zip_encryption_supported()) {
		skipping("Traditional ZIP encryption is unsupported on this "
		    "platform");
		return;
	}

	state.count = 0;
	state.return_once = 1;
	create_encrypted_zip("passCallBack", 2, buff, sizeof(buff), &used);
	a = new_reader();
	assertEqualIntA(a, ARCHIVE_OK,
	    archive_read_set_passphrase_callback(a, &state, callback_once));
	assertEqualIntA(a, ARCHIVE_OK, archive_read_open_memory(a, buff, used));
	assert_entries_read(a, 2);
	assertEqualInt(1, state.count);
	assertEqualInt(ARCHIVE_OK, archive_read_free(a));
}

DEFINE_TEST(test_archive_read_add_passphrase_multiple_with_callback)
{
	char buff[ZIP_BUFFER_SIZE];
	size_t used;
	struct archive *a;
	struct callback_state state;

	if (!zip_encryption_supported()) {
		skipping("Traditional ZIP encryption is unsupported on this "
		    "platform");
		return;
	}

	state.count = 0;
	state.return_once = 0;
	create_encrypted_zip("passCallBack", 1, buff, sizeof(buff), &used);
	a = new_reader();
	assertEqualIntA(a, ARCHIVE_OK,
	    archive_read_add_passphrase(a, "invalid1"));
	assertEqualIntA(a, ARCHIVE_OK,
	    archive_read_add_passphrase(a, "invalid2"));
	assertEqualIntA(a, ARCHIVE_OK,
	    archive_read_set_passphrase_callback(a, &state, callback_constant));
	assertEqualIntA(a, ARCHIVE_OK, archive_read_open_memory(a, buff, used));
	assert_entries_read(a, 1);
	assertEqualInt(1, state.count);
	assertEqualInt(ARCHIVE_OK, archive_read_free(a));
}

DEFINE_TEST(test_archive_read_add_passphrase_multiple_with_callback2)
{
	char buff[ZIP_BUFFER_SIZE];
	size_t used;
	struct archive *a;
	struct callback_state state;

	if (!zip_encryption_supported()) {
		skipping("Traditional ZIP encryption is unsupported on this "
		    "platform");
		return;
	}

	state.count = 0;
	state.return_once = 1;
	create_encrypted_zip("passCallBack", 2, buff, sizeof(buff), &used);
	a = new_reader();
	assertEqualIntA(a, ARCHIVE_OK,
	    archive_read_add_passphrase(a, "invalid1"));
	assertEqualIntA(a, ARCHIVE_OK,
	    archive_read_add_passphrase(a, "invalid2"));
	assertEqualIntA(a, ARCHIVE_OK,
	    archive_read_set_passphrase_callback(a, &state, callback_once));
	assertEqualIntA(a, ARCHIVE_OK, archive_read_open_memory(a, buff, used));
	assert_entries_read(a, 2);
	assertEqualInt(1, state.count);
	assertEqualInt(ARCHIVE_OK, archive_read_free(a));
}
