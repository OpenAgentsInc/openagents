/*
 * The provider Yasmarang instance (openagents #9297, OFR-015).
 *
 * A second verbatim compilation of the same pinned `ngu/random.c`, used only
 * for its Yasmarang transition. On the vulnerable firmware the chip TRNG hook
 * resolves to MicroPython's `ports/stm32/rng.c` deterministic fallback, whose
 * `pyb_rng_yasmarang` is the same Ilya Levin generator, statement for
 * statement, as the one in libngu. The two differ only in seeding: the
 * fallback takes its state from the unique id exclusive-ored with SysTick and
 * from two RTC registers, where libngu uses fixed public constants.
 *
 * That MicroPython file cannot be compiled off the target — it dereferences
 * STM32 peripheral registers — so this instance supplies the same transition
 * from the pinned source that can be, and the seed is set explicitly per
 * vector. No arithmetic is transcribed here; a test refuses this file if it
 * ever carries the transition's own shift constants.
 *
 * The four external symbols of the target source are renamed at compile time
 * (see the driver's compile flags) so this object can link beside oa_libngu.o.
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

/* The provider instance is stepped directly. Nothing in it may reach a chip
 * TRNG hook, so the hook traps instead of returning a value. */
#define CHIP_TRNG_SETUP()
#define CHIP_TRNG_32() (oa_trap("provider instance reached a chip TRNG hook"), 0u)

#include OA_LIBNGU_RANDOM_C

void oa_prov_set_state(uint32_t pad, uint32_t n, uint32_t d, uint8_t dat) {
  yasmarang_pad = pad;
  yasmarang_n = n;
  yasmarang_d = d;
  yasmarang_dat = dat;
}

void oa_prov_get_state(uint32_t *pad, uint32_t *n, uint32_t *d, uint8_t *dat) {
  *pad = yasmarang_pad;
  *n = yasmarang_n;
  *d = yasmarang_d;
  *dat = yasmarang_dat;
}

uint32_t oa_prov_yasmarang(void) {
  return my_yasmarang();
}
