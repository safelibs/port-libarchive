#ifndef TEST_CRC32_H
#define TEST_CRC32_H

#include <stddef.h>
#include <stdint.h>

/*
 * Test-local CRC32 helper.
 * This preserves the existing data-integrity assertions without relying on
 * libarchive's internal archive_crc32.h header.
 */
static uint32_t
test_crc32(uint32_t crc, const void *_p, size_t len)
{
	uint32_t crc2, b, i;
	const uint8_t *p = (const uint8_t *)_p;
	static volatile int crc_tbl_inited = 0;
	static uint32_t crc_tbl[256];

	if (!crc_tbl_inited) {
		for (b = 0; b < 256; ++b) {
			crc2 = b;
			for (i = 8; i > 0; --i) {
				if (crc2 & 1)
					crc2 = (crc2 >> 1) ^ 0xedb88320U;
				else
					crc2 >>= 1;
			}
			crc_tbl[b] = crc2;
		}
		crc_tbl_inited = 1;
	}

	crc ^= 0xffffffffU;
	for (; len >= 8; len -= 8) {
		crc = crc_tbl[(crc ^ *p++) & 0xff] ^ (crc >> 8);
		crc = crc_tbl[(crc ^ *p++) & 0xff] ^ (crc >> 8);
		crc = crc_tbl[(crc ^ *p++) & 0xff] ^ (crc >> 8);
		crc = crc_tbl[(crc ^ *p++) & 0xff] ^ (crc >> 8);
		crc = crc_tbl[(crc ^ *p++) & 0xff] ^ (crc >> 8);
		crc = crc_tbl[(crc ^ *p++) & 0xff] ^ (crc >> 8);
		crc = crc_tbl[(crc ^ *p++) & 0xff] ^ (crc >> 8);
		crc = crc_tbl[(crc ^ *p++) & 0xff] ^ (crc >> 8);
	}
	while (len--)
		crc = crc_tbl[(crc ^ *p++) & 0xff] ^ (crc >> 8);
	return (crc ^ 0xffffffffU);
}

#endif
