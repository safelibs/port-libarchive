/*-
 * Copyright (c) 2011 Tim Kientzle
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
make_test_string(char *buff, size_t buffsize, char seed)
{
	size_t i;

	for (i = 0; i + 1 < buffsize; i++)
		buff[i] = (char)(seed + (i % 23));
	buff[buffsize - 1] = '\0';
}

DEFINE_TEST(test_archive_string)
{
	struct archive_entry *entry, *clone;
	char long_group[96];
	char long_link[192];
	char long_path[256];
	char long_user[96];

	make_test_string(long_path, sizeof(long_path), 'A');
	make_test_string(long_link, sizeof(long_link), 'a');
	make_test_string(long_user, sizeof(long_user), '0');
	make_test_string(long_group, sizeof(long_group), 'K');

	assert((entry = archive_entry_new()) != NULL);
	archive_entry_copy_pathname(entry, long_path);
	archive_entry_copy_hardlink(entry, long_link);
	archive_entry_copy_uname(entry, long_user);
	archive_entry_copy_gname(entry, long_group);
	archive_entry_copy_sourcepath(entry, long_path);

	memset(long_path, 0, sizeof(long_path));
	memset(long_link, 0, sizeof(long_link));
	memset(long_user, 0, sizeof(long_user));
	memset(long_group, 0, sizeof(long_group));

	assert(NULL != archive_entry_pathname(entry));
	assert(NULL != archive_entry_hardlink(entry));
	assert(NULL != archive_entry_uname(entry));
	assert(NULL != archive_entry_gname(entry));
	assert(NULL != archive_entry_sourcepath(entry));
	assertEqualInt(255, (int)strlen(archive_entry_pathname(entry)));
	assertEqualInt(191, (int)strlen(archive_entry_hardlink(entry)));
	assertEqualInt(95, (int)strlen(archive_entry_uname(entry)));
	assertEqualInt(95, (int)strlen(archive_entry_gname(entry)));
	assertEqualInt(255, (int)strlen(archive_entry_sourcepath(entry)));

	assert((clone = archive_entry_clone(entry)) != NULL);
	assertEqualString(archive_entry_pathname(entry),
	    archive_entry_pathname(clone));
	assertEqualString(archive_entry_hardlink(entry),
	    archive_entry_hardlink(clone));
	assertEqualString(archive_entry_uname(entry),
	    archive_entry_uname(clone));
	assertEqualString(archive_entry_gname(entry),
	    archive_entry_gname(clone));
	assertEqualString(archive_entry_sourcepath(entry),
	    archive_entry_sourcepath(clone));

	archive_entry_copy_pathname(entry, "short");
	archive_entry_copy_hardlink(entry, "");
	archive_entry_copy_uname(entry, NULL);
	archive_entry_copy_gname(entry, "group");
	archive_entry_copy_sourcepath(entry, "src");

	assertEqualString("short", archive_entry_pathname(entry));
	assertEqualString("", archive_entry_hardlink(entry));
	assertEqualString(NULL, archive_entry_uname(entry));
	assertEqualString("group", archive_entry_gname(entry));
	assertEqualString("src", archive_entry_sourcepath(entry));

	assertEqualInt(255, (int)strlen(archive_entry_pathname(clone)));
	assertEqualInt(191, (int)strlen(archive_entry_hardlink(clone)));
	assertEqualInt(95, (int)strlen(archive_entry_uname(clone)));
	assertEqualInt(95, (int)strlen(archive_entry_gname(clone)));
	assertEqualInt(255, (int)strlen(archive_entry_sourcepath(clone)));

	archive_entry_clear(entry);
	assertEqualString(NULL, archive_entry_pathname(entry));
	assertEqualString(NULL, archive_entry_hardlink(entry));
	assertEqualString(NULL, archive_entry_uname(entry));
	assertEqualString(NULL, archive_entry_gname(entry));
	assertEqualString(NULL, archive_entry_sourcepath(entry));

	archive_entry_free(clone);
	archive_entry_free(entry);
}

static const char *strings[] =
{
  "dir/path",
  "dir/path2",
  "dir/path3",
  "dir/path4",
  "dir/path5",
  "dir/path6",
  "dir/path7",
  "dir/path8",
  "dir/path9",
  "dir/subdir/path",
  "dir/subdir/path2",
  "dir/subdir/path3",
  "dir/subdir/path4",
  "dir/subdir/path5",
  "dir/subdir/path6",
  "dir/subdir/path7",
  "dir/subdir/path8",
  "dir/subdir/path9",
  "dir2/path",
  "dir2/path2",
  "dir2/path3",
  "dir2/path4",
  "dir2/path5",
  "dir2/path6",
  "dir2/path7",
  "dir2/path8",
  "dir2/path9",
  NULL
};

DEFINE_TEST(test_archive_string_sort)
{
  unsigned int i, j, size;
  char **test_strings, *tmp;

  srand((unsigned int)time(NULL));
  size = sizeof(strings) / sizeof(char *);
  assert((test_strings = (char **)calloc(1, sizeof(strings))) != NULL);
  for (i = 0; i < (size - 1); i++)
    assert((test_strings[i] = strdup(strings[i])) != NULL);

  /* Shuffle the test strings */
  for (i = 0; i < (size - 1); i++)
  {
    j = rand() % ((size - 1) - i);
    j += i;
    tmp = test_strings[i];
    test_strings[i] = test_strings[j];
    test_strings[j] = tmp;
  }

  /* Sort and test */
  assertEqualInt(ARCHIVE_OK, archive_utility_string_sort(test_strings));
  for (i = 0; i < (size - 1); i++)
    assertEqualString(test_strings[i], strings[i]);

  for (i = 0; i < (size - 1); i++)
    free(test_strings[i]);
  free(test_strings);
}
