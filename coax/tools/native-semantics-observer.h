#ifndef COAX_VERIFICATION_TRAP_HANDLER_V1_H
#define COAX_VERIFICATION_TRAP_HANDLER_V1_H

#include <stdint.h>

/* Optional verification-only callback. The runtime still aborts normally after
 * the handler returns; no hook is installed unless a host explicitly sets it.
 * Byte spans are immutable for the duration of the call. */
typedef void (*Coax_VerificationTrapHandler_v1)(const char *what, int64_t what_count,
    const char *source, int64_t source_count, int64_t line);

#ifdef __cplusplus
extern "C" {
#endif
void coax_verification_set_trap_v1(Coax_VerificationTrapHandler_v1 handler);
#ifdef __cplusplus
}
#endif

#endif
