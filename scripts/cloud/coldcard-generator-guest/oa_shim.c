/*
 * Trap implementations for the MicroPython interface the pinned libngu
 * `ngu/random.c` compiles against (openagents #9297, OFR-015).
 *
 * Every function here aborts. None of them can contribute a value to a
 * generator vector: the harness either produces its numbers from the target
 * source's own arithmetic or it exits non-zero.
 *
 * `mp_raise_OSError` is the one trap the target source can legitimately reach:
 * `my_random_bytes` calls it when two consecutive chip TRNG reads are equal,
 * which is the health check the reproduction models. A vector that trips it is
 * a failed run, not a vector.
 */

#include <stdio.h>
#include <stdlib.h>

#include "py/runtime.h"

#include "oa_harness.h"

const int mp_type_module = 0;
const int mp_type_bytes = 0;
const mp_obj_t mp_const_none = (const void *)0;

void oa_trap(const char *reason) {
  fprintf(stderr, "oa-coldcard-generator trap: %s\n", reason);
  exit(9);
}

void _ngu_assert(const char *fname, int line_num) {
  fprintf(stderr, "oa-coldcard-generator libngu assert at %s:%d\n", fname, line_num);
  exit(10);
}

void mp_raise_OSError(int code) {
  fprintf(stderr, "oa-coldcard-generator target raised OSError %d\n", code);
  exit(11);
}

void mp_raise_ValueError(const char *message) {
  fprintf(stderr, "oa-coldcard-generator target raised ValueError %s\n", message);
  exit(12);
}

mp_obj_t mp_obj_new_int_from_uint(uint32_t value) {
  (void)value;
  oa_trap("mp_obj_new_int_from_uint reached");
}

mp_obj_t mp_obj_new_str_from_vstr(const void *type, vstr_t *vstr) {
  (void)type;
  (void)vstr;
  oa_trap("mp_obj_new_str_from_vstr reached");
}

int mp_obj_get_int_truncated(mp_obj_t value) {
  (void)value;
  oa_trap("mp_obj_get_int_truncated reached");
}

void vstr_init_len(vstr_t *vstr, size_t len) {
  (void)vstr;
  (void)len;
  oa_trap("vstr_init_len reached");
}
