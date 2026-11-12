// auth-device-crypto.coax's C (auth-device-crypto.generated.c) as one unit of the API. It loads
// Coax libraries that nip98.generated.c, auth-seal.generated.c and others load too (bytes,
// SHA-256, HMAC) and P-256, and those export their procedures: the same names twice in one
// binary. Here every export of this unit but its own (adc_*) gets a name of its own. A library
// export added later fails the link loudly; regenerate this list:
//   nm -g --defined-only auth-device-crypto.o | awk '{print $3}' | sed 's/^_//' | grep -v '^adc_'
#define cb_ct_equal adc_x_cb_ct_equal
#define cb_ct_select32 adc_x_cb_ct_select32
#define cb_ct_select64 adc_x_cb_ct_select64
#define cb_load_be32 adc_x_cb_load_be32
#define cb_load_be64 adc_x_cb_load_be64
#define cb_load_le32 adc_x_cb_load_le32
#define cb_load_le64 adc_x_cb_load_le64
#define cb_mask adc_x_cb_mask
#define cb_span adc_x_cb_span
#define cb_store_be32 adc_x_cb_store_be32
#define cb_store_be64 adc_x_cb_store_be64
#define cb_store_le32 adc_x_cb_store_le32
#define cb_store_le64 adc_x_cb_store_le64
#define hmac_sha256_info adc_x_hmac_sha256_info
#define hmac_sha256_state_size adc_x_hmac_sha256_state_size
#define p256_der_read adc_x_p256_der_read
#define p256_public_key adc_x_p256_public_key
#define p256_sign adc_x_p256_sign
#define p256_verify adc_x_p256_verify
#define p256_verify_der adc_x_p256_verify_der
#define S_hash adc_x_S_hash
#define S_hex adc_x_S_hex
#define sha256_info adc_x_sha256_info
#define sha256_init adc_x_sha256_init
#define sha256_state_size adc_x_sha256_state_size
#define sha256_update_limit adc_x_sha256_update_limit
// coax/lib's generated C keeps a few written-but-unread locals.
#if defined(__clang__)
#pragma clang diagnostic ignored "-Wunused-but-set-variable"
#elif defined(__GNUC__)
#pragma GCC diagnostic ignored "-Wunused-but-set-variable"
#endif
#include "auth-device-crypto.generated.c"
#include <stdbool.h>
#include <string.h>

// What auth-device.c calls: C strings in, the same answers out.
typedef struct { int64_t ts_ms; bool well_formed; bool verified; } adc_proof;
bool adc_key_c(const char *text, size_t n, unsigned char out[65]);
adc_proof adc_check_c(const char *header, const char *method, const char *url, const char *binding, const unsigned char key[65], unsigned char digest[32]);

static coax_v_u8 adc_view(const char *s) { return (coax_v_u8){(uint8_t *)s, (int64_t)strlen(s)}; }
bool adc_key_c(const char *text, size_t n, unsigned char out[65]) {
  return adc_key((coax_v_u8){(uint8_t *)text, (int64_t)n}, (coax_v_u8){out, 65});
}
adc_proof adc_check_c(const char *header, const char *method, const char *url, const char *binding, const unsigned char key[65], unsigned char digest[32]) {
  coax_t_AdcProof p = adc_check(adc_view(header), adc_view(method), adc_view(url), adc_view(binding), (coax_v_u8){(uint8_t *)key, 65}, (coax_v_u8){digest, 32});
  return (adc_proof){p.ts_ms, p.well_formed, p.verified};
}
