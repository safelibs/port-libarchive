/*-
 * Copyright (c) 2003-2007 Tim Kientzle
 * Copyright (c) 2011 Andres Mejia
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

static const char mtree_digest_archive[] =
    "#mtree\n"
    "md5file type=file md5digest=93b885adfe0da089cdf634904fd59f71\n"
    "rmd160file type=file rmd160digest="
    "c81b94933420221a7ac004a90242d8b1d3e5070d\n"
    "sha1file type=file sha1digest="
    "5ba93c9db0cff93f52b521d7420e43f6eda2784f\n"
    "sha256file type=file sha256digest="
    "6e340b9cffb37a989ca544e6bb780a2c78901d3fb33738768511a30617afa01d\n"
    "sha384file type=file sha384digest="
    "bec021b4f368e3069134e012c2b4307083d3a9bdd206e24e5f0d86e13d663665"
    "5933ec2b413465966817a9c208a11717\n"
    "sha512file type=file sha512digest="
    "b8244d028981d693af7b456af8efa4cad63d282e19ff14942c246e50d9351d22"
    "704a802a71c3580b6370de4ceb293c324a8423342557d4e5c38438f0e36910ee\n";

static void
assert_digest_for_entry(const char *pathname, int digest_type,
    const void *expected, size_t expected_size)
{
	struct archive *a;
	struct archive_entry *ae;
	int found = 0;

	assert((a = archive_read_new()) != NULL);
	assertEqualIntA(a, ARCHIVE_OK, archive_read_support_filter_none(a));
	assertEqualIntA(a, ARCHIVE_OK, archive_read_support_format_mtree(a));
	assertEqualIntA(a, ARCHIVE_OK, archive_read_open_memory(a,
	    mtree_digest_archive, strlen(mtree_digest_archive)));

	while (archive_read_next_header(a, &ae) == ARCHIVE_OK) {
		if (strcmp(pathname, archive_entry_pathname(ae)) != 0)
			continue;
		assertEqualMem(expected, archive_entry_digest(ae, digest_type),
		    expected_size);
		found = 1;
		break;
	}

	assert(found);
	assertEqualInt(ARCHIVE_OK, archive_read_free(a));
}

DEFINE_TEST(test_archive_md5)
{
	static const unsigned char expected[] =
	    "\x93\xb8\x85\xad\xfe\x0d\xa0\x89"
	    "\xcd\xf6\x34\x90\x4f\xd5\x9f\x71";

	assert_digest_for_entry("md5file", ARCHIVE_ENTRY_DIGEST_MD5,
	    expected, sizeof(expected) - 1);
}

DEFINE_TEST(test_archive_rmd160)
{
	static const unsigned char expected[] =
	    "\xc8\x1b\x94\x93\x34\x20\x22\x1a\x7a\xc0"
	    "\x04\xa9\x02\x42\xd8\xb1\xd3\xe5\x07\x0d";

	assert_digest_for_entry("rmd160file", ARCHIVE_ENTRY_DIGEST_RMD160,
	    expected, sizeof(expected) - 1);
}

DEFINE_TEST(test_archive_sha1)
{
	static const unsigned char expected[] =
	    "\x5b\xa9\x3c\x9d\xb0\xcf\xf9\x3f\x52\xb5"
	    "\x21\xd7\x42\x0e\x43\xf6\xed\xa2\x78\x4f";

	assert_digest_for_entry("sha1file", ARCHIVE_ENTRY_DIGEST_SHA1,
	    expected, sizeof(expected) - 1);
}

DEFINE_TEST(test_archive_sha256)
{
	static const unsigned char expected[] =
	    "\x6e\x34\x0b\x9c\xff\xb3\x7a\x98"
	    "\x9c\xa5\x44\xe6\xbb\x78\x0a\x2c"
	    "\x78\x90\x1d\x3f\xb3\x37\x38\x76"
	    "\x85\x11\xa3\x06\x17\xaf\xa0\x1d";

	assert_digest_for_entry("sha256file", ARCHIVE_ENTRY_DIGEST_SHA256,
	    expected, sizeof(expected) - 1);
}

DEFINE_TEST(test_archive_sha384)
{
	static const unsigned char expected[] =
	    "\xbe\xc0\x21\xb4\xf3\x68\xe3\x06"
	    "\x91\x34\xe0\x12\xc2\xb4\x30\x70"
	    "\x83\xd3\xa9\xbd\xd2\x06\xe2\x4e"
	    "\x5f\x0d\x86\xe1\x3d\x66\x36\x65"
	    "\x59\x33\xec\x2b\x41\x34\x65\x96"
	    "\x68\x17\xa9\xc2\x08\xa1\x17\x17";

	assert_digest_for_entry("sha384file", ARCHIVE_ENTRY_DIGEST_SHA384,
	    expected, sizeof(expected) - 1);
}

DEFINE_TEST(test_archive_sha512)
{
	static const unsigned char expected[] =
	    "\xb8\x24\x4d\x02\x89\x81\xd6\x93"
	    "\xaf\x7b\x45\x6a\xf8\xef\xa4\xca"
	    "\xd6\x3d\x28\x2e\x19\xff\x14\x94"
	    "\x2c\x24\x6e\x50\xd9\x35\x1d\x22"
	    "\x70\x4a\x80\x2a\x71\xc3\x58\x0b"
	    "\x63\x70\xde\x4c\xeb\x29\x3c\x32"
	    "\x4a\x84\x23\x34\x25\x57\xd4\xe5"
	    "\xc3\x84\x38\xf0\xe3\x69\x10\xee";

	assert_digest_for_entry("sha512file", ARCHIVE_ENTRY_DIGEST_SHA512,
	    expected, sizeof(expected) - 1);
}
