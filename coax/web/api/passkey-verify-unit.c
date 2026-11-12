// passkey-verify.coax's C (passkey-verify.generated.c) as one unit of the API. It loads Coax
// libraries that nip98.generated.c, auth-seal.generated.c and auth-recover-crypto.generated.c load
// too (bytes, SHA-256, JSON), and those export their procedures: the same names twice in one
// binary. Here every export of this unit but its own four (pv_register, pv_assert, pv_b64url,
// pv_unb64url) gets a name of its own. A library export added later fails the link loudly;
// regenerate this list:
//   nm -g --defined-only passkey-verify.o | awk '{print $3}' | sed 's/^_//' | grep -v '^pv_'
#define cb_ct_equal pkv_x_cb_ct_equal
#define cb_ct_select32 pkv_x_cb_ct_select32
#define cb_ct_select64 pkv_x_cb_ct_select64
#define cb_load_be32 pkv_x_cb_load_be32
#define cb_load_be64 pkv_x_cb_load_be64
#define cb_load_le32 pkv_x_cb_load_le32
#define cb_load_le64 pkv_x_cb_load_le64
#define cb_mask pkv_x_cb_mask
#define cb_span pkv_x_cb_span
#define cb_store_be32 pkv_x_cb_store_be32
#define cb_store_be64 pkv_x_cb_store_be64
#define cb_store_le32 pkv_x_cb_store_le32
#define cb_store_le64 pkv_x_cb_store_le64
#define hmac_sha256_info pkv_x_hmac_sha256_info
#define hmac_sha256_state_size pkv_x_hmac_sha256_state_size
#define hmac_sha512 pkv_x_hmac_sha512
#define p256_der_read pkv_x_p256_der_read
#define p256_public_key pkv_x_p256_public_key
#define p256_sign pkv_x_p256_sign
#define p256_verify pkv_x_p256_verify
#define p256_verify_der pkv_x_p256_verify_der
#define pbkdf2_hmac_sha512 pkv_x_pbkdf2_hmac_sha512
#define rsa_verify_sha256 pkv_x_rsa_verify_sha256
#define S_hash pkv_x_S_hash
#define S_hex pkv_x_S_hex
#define S512_hash pkv_x_S512_hash
#define sha256_info pkv_x_sha256_info
#define sha256_init pkv_x_sha256_init
#define sha256_state_size pkv_x_sha256_state_size
#define sha256_update_limit pkv_x_sha256_update_limit
#define sha512 pkv_x_sha512
#define sha512_info pkv_x_sha512_info
#define sha512_init pkv_x_sha512_init
#define sha512_state_size pkv_x_sha512_state_size
#define sha512_two pkv_x_sha512_two
#define sha512_update_limit pkv_x_sha512_update_limit
// coax/lib's generated C keeps a few written-but-unread locals and an unread parameter (cbor.coax).
#if defined(__clang__)
#pragma clang diagnostic ignored "-Wunused-but-set-variable"
#pragma clang diagnostic ignored "-Wunused-parameter"
#elif defined(__GNUC__)
#pragma GCC diagnostic ignored "-Wunused-but-set-variable"
#pragma GCC diagnostic ignored "-Wunused-parameter"
#endif
#include "passkey-verify.generated.c"
