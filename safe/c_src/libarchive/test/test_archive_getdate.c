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

#include <time.h>

static struct archive *
new_date_matcher(const char *date_string)
{
	struct archive *m;

	assert((m = archive_match_new()) != NULL);
	assertEqualIntA(m, ARCHIVE_OK, archive_match_include_date(m,
	    ARCHIVE_MATCH_MTIME | ARCHIVE_MATCH_NEWER | ARCHIVE_MATCH_EQUAL,
	    date_string));
	return (m);
}

static int
mtime_excluded(struct archive *m, time_t t)
{
	struct archive_entry *ae;
	int excluded;

	assert((ae = archive_entry_new()) != NULL);
	archive_entry_set_mtime(ae, t, 0);
	excluded = archive_match_time_excluded(m, ae);
	archive_entry_free(ae);
	return (excluded);
}

static void
assert_boundary(const char *date_string, time_t boundary)
{
	struct archive *m;

	m = new_date_matcher(date_string);
	assertEqualInt(1, mtime_excluded(m, boundary - 1));
	assertEqualInt(0, mtime_excluded(m, boundary));
	assertEqualInt(0, mtime_excluded(m, boundary + 1));
	archive_match_free(m);
}

static void
assert_equivalent(const char *date_string1, const char *date_string2,
    time_t older, time_t newer)
{
	struct archive *m1, *m2;

	m1 = new_date_matcher(date_string1);
	m2 = new_date_matcher(date_string2);
	assertEqualInt(mtime_excluded(m1, older), mtime_excluded(m2, older));
	assertEqualInt(mtime_excluded(m1, newer), mtime_excluded(m2, newer));
	archive_match_free(m2);
	archive_match_free(m1);
}

DEFINE_TEST(test_archive_getdate)
{
	time_t now = time(NULL);

	assert_boundary("Jan 1, 1970 UTC", 0);
	assert_boundary("7:12:18-0530 4 May 1983", 420900138);
	assert_boundary("2004/01/29 513 mest", 1075345980);
	assert_boundary("99/02/17 7pm utc", 919278000);
	assert_boundary("02/17/99 7:11am est", 919253460);
	assert_boundary("Sun Feb 22 17:38:26 PST 2009", 1235353106);

	assert_equivalent("now - 2 hours", "2 hours ago",
	    now - 3 * 60 * 60, now - 60 * 60);
	assert_equivalent("2 hours ago", "+2 hours ago",
	    now - 3 * 60 * 60, now - 60 * 60);
	assert_equivalent("now - 2 hours", "-2 hours",
	    now - 3 * 60 * 60, now - 60 * 60);

	assert_equivalent("tomorrow", "now + 24 hours",
	    now + 23 * 60 * 60, now + 25 * 60 * 60);
	assert_equivalent("yesterday", "now - 24 hours",
	    now - 25 * 60 * 60, now - 23 * 60 * 60);
	assert_equivalent("now + 1 hour", "now + 60 minutes",
	    now + 30 * 60, now + 90 * 60);
	assert_equivalent("now + 1 hour + 1 minute", "now + 61 minutes",
	    now + 30 * 60, now + 2 * 60 * 60);
}
