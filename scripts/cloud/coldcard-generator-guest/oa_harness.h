/*
 * Shared declarations for the OFR-015 Coldcard generator harness
 * (openagents #9297).
 *
 * Two translation units each compile the SAME pinned libngu `ngu/random.c`
 * verbatim, giving two independent instances of the target's Yasmarang state:
 *
 *   - oa_libngu.c   the libngu instance, the one Coldcard's `ngu.random`
 *                   module owns
 *   - oa_provider.c a second instance standing in for the chip TRNG hook
 *                   `CHIP_TRNG_32()`. On the vulnerable firmware that hook
 *                   resolves to MicroPython's `ports/stm32/rng.c` fallback,
 *                   whose `pyb_rng_yasmarang` is the same Yasmarang
 *                   transition, seeded from unique-id/SysTick/RTC values
 *                   instead of the fixed public constants.
 *
 * The provider instance's globals are renamed at compile time so both objects
 * can link; the arithmetic in both is the pinned target source.
 */

#ifndef OA_COLDCARD_GENERATOR_HARNESS_H
#define OA_COLDCARD_GENERATOR_HARNESS_H

#include <stdint.h>

/* The CHIP_TRNG_32() hook the pinned source calls. */
uint32_t oa_provider32(void);

/* libngu instance (oa_libngu.c) */
void oa_lib_set_state(uint32_t pad, uint32_t n, uint32_t d, uint8_t dat);
void oa_lib_get_state(uint32_t *pad, uint32_t *n, uint32_t *d, uint8_t *dat);
uint32_t oa_lib_yasmarang(void);
void oa_lib_random_bytes(uint8_t *dest, uint32_t count);
int oa_lib_rand_below(int mx);
int oa_lib_bit_length(uint32_t value);

/* provider instance (oa_provider.c) */
void oa_prov_set_state(uint32_t pad, uint32_t n, uint32_t d, uint8_t dat);
void oa_prov_get_state(uint32_t *pad, uint32_t *n, uint32_t *d, uint8_t *dat);
uint32_t oa_prov_yasmarang(void);

/* Harness traps (oa_shim.c). Reaching one aborts the run. */
void oa_trap(const char *reason) __attribute__((noreturn));

#endif /* OA_COLDCARD_GENERATOR_HARNESS_H */
