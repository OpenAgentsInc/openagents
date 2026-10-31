#ifndef COAX_NATIVE_CHECK_OBSERVER_ABI_V1_H
#define COAX_NATIVE_CHECK_OBSERVER_ABI_V1_H

#include <stdint.h>

/* Verification-only C0 ABI. Strings are immutable byte spans, including NULs. */
typedef struct Coax_VerificationCheck_v1 {
  int64_t declaration_index;
  const char *name;
  int64_t name_count;
  const char *source;
  int64_t source_count;
  int64_t line, column;
  void (*run)(void);
} Coax_VerificationCheck_v1;

extern const Coax_VerificationCheck_v1 *coax_verification_checks_v1(int64_t *count);

#endif
