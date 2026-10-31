#ifndef COAX_NATIVE_CHECK_OBSERVER_ABI_V2_H
#define COAX_NATIVE_CHECK_OBSERVER_ABI_V2_H

#include <stdint.h>

/* Explicit verification ABI2. All strings and arrays are immutable borrowed
 * spans. run_values requires a tuple already admitted by the Coax FD policy;
 * it returns -1 for malformed carrier arity/storage, 0 after the body returns. */
typedef struct Coax_VerificationEnumValue_v2 {
  uint64_t bits;
  const char *label;
  int64_t label_count;
} Coax_VerificationEnumValue_v2;

typedef struct Coax_VerificationBinding_v2 {
  const char *name;
  int64_t name_count;
  const char *type_name;
  int64_t type_name_count;
  int64_t kind, width, is_signed, first;
  uint64_t count;
  const Coax_VerificationEnumValue_v2 *values;
  int64_t value_count;
} Coax_VerificationBinding_v2;

typedef struct Coax_VerificationCheck_v2 {
  int64_t declaration_index;
  const char *name;
  int64_t name_count;
  const char *source;
  int64_t source_count;
  int64_t line, column;
  int64_t kind;
  const Coax_VerificationBinding_v2 *bindings;
  int64_t binding_count;
  uint64_t case_count;
  int64_t (*run_values)(const uint64_t *values, int64_t count);
} Coax_VerificationCheck_v2;

extern const Coax_VerificationCheck_v2 *coax_verification_checks_v2(int64_t *count);

#endif
