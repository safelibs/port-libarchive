/*-
 * Copyright (c) 2012 Michihiro NAKAJIMA
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

#include <limits.h>
#include <unistd.h>

static void
write_single_file_archive(struct archive *a)
{
	struct archive_entry *ae;

	assert((ae = archive_entry_new()) != NULL);
	archive_entry_copy_pathname(ae, "file");
	archive_entry_set_mode(ae, AE_IFREG | 0644);
	archive_entry_set_size(ae, 7);
	assertEqualIntA(a, ARCHIVE_OK, archive_write_header(a, ae));
	assertEqualInt(7, archive_write_data(a, "payload", 7));
	archive_entry_free(ae);
}

static void
verify_passthrough_archive(const char *buff, size_t used)
{
	struct archive *a;
	struct archive_entry *ae;
	char data[16];

	assert((a = archive_read_new()) != NULL);
	assertEqualIntA(a, ARCHIVE_OK, archive_read_support_filter_all(a));
	assertEqualIntA(a, ARCHIVE_OK, archive_read_support_format_all(a));
	assertEqualIntA(a, ARCHIVE_OK, archive_read_open_memory(a, buff, used));
	assertEqualIntA(a, ARCHIVE_OK, archive_read_next_header(a, &ae));
	assertEqualString("file", archive_entry_pathname(ae));
	assertEqualInt(7, archive_entry_size(ae));
	assertEqualInt(7, archive_read_data(a, data, sizeof(data)));
	assertEqualMem("payload", data, 7);
	assertEqualIntA(a, ARCHIVE_EOF, archive_read_next_header(a, &ae));
	assertEqualInt(ARCHIVE_OK, archive_read_free(a));
}

static int
write_archive_through_program(const char *cmd, char *buff, size_t buffsize,
    size_t *used)
{
	struct archive *a;
	int r;

	assert((a = archive_write_new()) != NULL);
	assertEqualIntA(a, ARCHIVE_OK, archive_write_set_format_ustar(a));
	r = archive_write_add_filter_program(a, cmd);
	if (r == ARCHIVE_FATAL) {
		assertEqualInt(ARCHIVE_OK, archive_write_free(a));
		return (ARCHIVE_FATAL);
	}
	assertEqualIntA(a, ARCHIVE_OK,
	    archive_write_open_memory(a, buff, buffsize, used));
	write_single_file_archive(a);
	assertEqualIntA(a, ARCHIVE_OK, archive_write_close(a));
	assertEqualInt(ARCHIVE_OK, archive_write_free(a));
	return (ARCHIVE_OK);
}

static void
assert_program_invocation(const char *directory, const char *script_name,
    const char *command, const char *expected_invocation)
{
	char archive_buffer[65536];
	char command_buffer[1024];
	char log_path[256];
	char script_path[256];
	char script_contents[1024];
	size_t used;
	int r;

	assertMakeDir(directory, 0755);
	assert(0 < snprintf(log_path, sizeof(log_path), "%s/invocation.txt",
	    directory));
	assert(0 < snprintf(script_path, sizeof(script_path), "%s/%s",
	    directory, script_name));
	assert(0 < snprintf(script_contents, sizeof(script_contents),
	    "#!/bin/sh\n"
	    "{\n"
	    "  printf '%%s\\n' \"$0\"\n"
	    "  for arg in \"$@\"; do printf '%%s\\n' \"$arg\"; done\n"
	    "} > \"%s\"\n"
	    "cat\n",
	    log_path));
	assertMakeFile(script_path, 0755, script_contents);

	assert(0 < snprintf(command_buffer, sizeof(command_buffer), "%s",
	    command));
	r = write_archive_through_program(command_buffer, archive_buffer,
	    sizeof(archive_buffer), &used);
	if (r == ARCHIVE_FATAL) {
		skipping("archive_write_add_filter_program() unsupported "
		    "on this platform");
		return;
	}

	verify_passthrough_archive(archive_buffer, used);
	assertTextFileContents(expected_invocation, log_path);
}

static void
assert_absolute_program_invocation(const char *directory)
{
	char absolute_script_path[PATH_MAX];
	char command[PATH_MAX + 8];
	char cwd[PATH_MAX];
	char expected_invocation[PATH_MAX + 2];

	assert(getcwd(cwd, sizeof(cwd)) != NULL);
	assert(0 < snprintf(absolute_script_path, sizeof(absolute_script_path),
	    "%s/%s/filter script", cwd, directory));
	assert(0 < snprintf(command, sizeof(command), "\"%s\"",
	    absolute_script_path));
	assert(0 < snprintf(expected_invocation, sizeof(expected_invocation),
	    "%s\n", absolute_script_path));
	assert_program_invocation(directory, "filter script", command,
	    expected_invocation);
}

DEFINE_TEST(test_archive_cmdline)
{
	assert_program_invocation("cmdline-case-1", "filter script",
	    "\"cmdline-case-1/filter script\"",
	    "cmdline-case-1/filter script\n");

	assert_program_invocation("cmdline-case-2", "filter script",
	    "\"cmdline-case-2/filter script\" ",
	    "cmdline-case-2/filter script\n");

	assert_absolute_program_invocation("cmdline-case-3");

	assert_program_invocation("cmdline case 4", "filter x",
	    "\"cmdline case 4/filter \"x",
	    "cmdline case 4/filter x\n");

	assert_program_invocation("cmdline case 5", "filter x s ",
	    "\"cmdline case 5/filter \"x\" s \"",
	    "cmdline case 5/filter x s \n");

	assert_program_invocation("cmdline case 6", "filter\" script",
	    "\"cmdline case 6/filter\\\" script\"",
	    "cmdline case 6/filter\" script\n");

	assert_program_invocation("cmdline case 7", "filter script",
	    "\"cmdline case 7/filter script\" -d",
	    "cmdline case 7/filter script\n-d\n");

	assert_program_invocation("cmdline case 8", "filter script",
	    "\"cmdline case 8/filter script\" -d -q",
	    "cmdline case 8/filter script\n-d\n-q\n");

	assert_program_invocation("cmdline case 9", "filter script",
	    "\"cmdline case 9/filter script\" \"arg with space\" plain",
	    "cmdline case 9/filter script\narg with space\nplain\n");
}
