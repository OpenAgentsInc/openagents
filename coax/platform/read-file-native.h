#ifndef COAX_READ_FILE_NATIVE_H
#define COAX_READ_FILE_NATIVE_H
#include <stdint.h>
#ifdef COAX_coax_v_u8_DEFINED
#ifndef COAX_SCHEMA_coax_v_u8_31343a636f61782d632d747970652d7631343a76696577393a636f61785f765f7538323a3136313a38373a75696e74385f74
#error "incompatible Coax C type coax_v_u8"
#endif
#else
#define COAX_coax_v_u8_DEFINED
#define COAX_SCHEMA_coax_v_u8_31343a636f61782d632d747970652d7631343a76696577393a636f61785f765f7538323a3136313a38373a75696e74385f74
typedef struct {uint8_t *data;int64_t count;} coax_v_u8;
#endif
#ifdef COAX_coax_v_s64_DEFINED
#ifndef COAX_SCHEMA_coax_v_s64_31343a636f61782d632d747970652d7631343a7669657731303a636f61785f765f733634323a3136313a38373a696e7436345f74
#error "incompatible Coax C type coax_v_s64"
#endif
#else
#define COAX_coax_v_s64_DEFINED
#define COAX_SCHEMA_coax_v_s64_31343a636f61782d632d747970652d7631343a7669657731303a636f61785f765f733634323a3136313a38373a696e7436345f74
typedef struct {int64_t *data;int64_t count;} coax_v_s64;
#endif
#ifdef __cplusplus
extern "C" {
#endif
/* Raw synchronous effects only. Return fd/count/zero or immediately captured
   negative errno. No retry, close-on-error, kind/capacity/error-domain policy.
   Coax validates borrowed path bytes (1..4096, no NUL) before open. This TU
   adds the terminator; its 4096 bound protects the physical stack buffer.
   stat writes raw st_mode and st_size into two caller-owned int slots.
   Valid disjoint spans, exclusive descriptor ownership, no cancellation,
   concurrent close/dup2 or longjmp across the Coax frame are preconditions. */
int64_t cxfr_open(coax_v_u8 path);
int64_t cxfr_stat(int64_t fd,coax_v_s64 metadata);
int64_t cxfr_read(int64_t fd,coax_v_u8 out);
int64_t cxfr_close(int64_t fd);
#ifdef __cplusplus
}
#endif
#endif
