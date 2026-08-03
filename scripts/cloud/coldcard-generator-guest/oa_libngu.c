/*
 * The libngu Yasmarang instance (openagents #9297, OFR-015).
 *
 * This file compiles the pinned `ngu/random.c` VERBATIM. Its path arrives as
 * OA_LIBNGU_RANDOM_C at compile time and points into the read-only Coldcard
 * tree baked into the admitted guest image; the driver records that file's
 * sha256 next to every value this harness produces.
 *
 * The system headers are pulled in first, then the platform macros the target
 * source keys off are undefined so it takes none of its built-in TRNG paths
 * and uses the hook below instead. Nothing in the target source is edited.
 */

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "oa_harness.h"

#undef __linux__
#undef __APPLE__
#undef __FreeBSD__
#undef ESP_PLATFORM
#undef MICROPY_PY_STM

#define CHIP_TRNG_SETUP()
#define CHIP_TRNG_32() oa_provider32()

#include OA_LIBNGU_RANDOM_C

void oa_lib_set_state(uint32_t pad, uint32_t n, uint32_t d, uint8_t dat) {
  yasmarang_pad = pad;
  yasmarang_n = n;
  yasmarang_d = d;
  yasmarang_dat = dat;
}

void oa_lib_get_state(uint32_t *pad, uint32_t *n, uint32_t *d, uint8_t *dat) {
  *pad = yasmarang_pad;
  *n = yasmarang_n;
  *d = yasmarang_d;
  *dat = yasmarang_dat;
}

uint32_t oa_lib_yasmarang(void) {
  return my_yasmarang();
}

void oa_lib_random_bytes(uint8_t *dest, uint32_t count) {
  my_random_bytes(dest, count);
}

int oa_lib_rand_below(int mx) {
  return _rand_below(mx);
}

int oa_lib_bit_length(uint32_t value) {
  return _bit_length(value);
}
