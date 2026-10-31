#ifndef COAX_NATIVE_FORALL_PROGRESS_V2_H
#define COAX_NATIVE_FORALL_PROGRESS_V2_H

#include <stddef.h>
#include <stdint.h>
#include <stdatomic.h>

/* Host layout only. Coax owns packet encoding, admission and event meaning.
 * The driver creates a new zeroed file with committed=255 for each process. */
enum {
  COAX_FORALL_SLOT_BYTES = 16384,
  COAX_FORALL_REGION_BYTES = 32769
};

typedef struct Coax_ForallProgress_v2 {
  _Atomic unsigned char committed;
  uint8_t slots[2][COAX_FORALL_SLOT_BYTES];
} Coax_ForallProgress_v2;

_Static_assert(sizeof(_Atomic unsigned char) == 1, "progress needs a one-byte atomic selector");
_Static_assert(offsetof(Coax_ForallProgress_v2, slots) == 1, "progress slot offset changed");
_Static_assert(sizeof(Coax_ForallProgress_v2) == COAX_FORALL_REGION_BYTES, "progress extent changed");

#endif
