// coax/lib/workos/adapters/c_unit.coax's C (workos-sdk.generated.c) as one unit of oa-api.
// Like passkey-verify-unit.c and auth-seal-unit.c, this renames shared Coax stdlib exports
// (bytes, SHA-256, HMAC, PBKDF2, RSA, base64, hex, UTF-8) so they do not collide with
// nip98.generated.c, auth-seal-unit.c, and passkey-verify-unit.c in the same binary, while
// leaving every wos_* export untouched.
#define NC_count wos_x_NC_count
#define S_hash wos_x_S_hash
#define S_hex wos_x_S_hex
#define base64_encode_size wos_x_base64_encode_size
#define base64_text_count wos_x_base64_text_count
#define cb_ct_equal wos_x_cb_ct_equal
#define cb_ct_select32 wos_x_cb_ct_select32
#define cb_ct_select64 wos_x_cb_ct_select64
#define cb_load_be32 wos_x_cb_load_be32
#define cb_load_be64 wos_x_cb_load_be64
#define cb_load_le32 wos_x_cb_load_le32
#define cb_load_le64 wos_x_cb_load_le64
#define cb_mask wos_x_cb_mask
#define cb_span wos_x_cb_span
#define cb_store_be32 wos_x_cb_store_be32
#define cb_store_be64 wos_x_cb_store_be64
#define cb_store_le32 wos_x_cb_store_le32
#define cb_store_le64 wos_x_cb_store_le64
#define hex_decode_size wos_x_hex_decode_size
#define hex_encode_size wos_x_hex_encode_size
#define hmac_sha256_info wos_x_hmac_sha256_info
#define hmac_sha256_state_size wos_x_hmac_sha256_state_size
#define pbkdf2_info wos_x_pbkdf2_info
#define pbkdf2_state_size wos_x_pbkdf2_state_size
#define pbkdf2_worker_budget wos_x_pbkdf2_worker_budget
#define pbkdf2_worker_request wos_x_pbkdf2_worker_request
#define rsa_verify_sha256 wos_x_rsa_verify_sha256
#define sha256_info wos_x_sha256_info
#define sha256_init wos_x_sha256_init
#define sha256_state_size wos_x_sha256_state_size
#define sha256_update_limit wos_x_sha256_update_limit
#if defined(__clang__)
#pragma clang diagnostic ignored "-Wunused-but-set-variable"
#pragma clang diagnostic ignored "-Wunused-parameter"
#pragma clang diagnostic ignored "-Wunused-function"
#elif defined(__GNUC__)
#pragma GCC diagnostic ignored "-Wunused-but-set-variable"
#pragma GCC diagnostic ignored "-Wunused-parameter"
#pragma GCC diagnostic ignored "-Wunused-function"
#endif
#include "workos-sdk.generated.c"
