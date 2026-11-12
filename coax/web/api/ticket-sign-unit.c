// ticket-sign.coax's C (ticket-sign.generated.c: aethyr/session/seat-ticket.coax, the one seat ticket
// v5 writer, #13260) as one unit of the API. It loads Coax libraries that nip98.generated.c,
// progress-policy.generated.c and others load too (bytes, SHA-256, secp256k1, the resume payload),
// and those export their procedures: the same names twice in one binary. Here every export of this
// unit but its own (seat_ticket_v5_*, room_write_*) gets a name of its own. A library export added later fails the
// link loudly; regenerate this list:
//   nm -g --defined-only ticket-sign.o | awk '{print $3}' | sed 's/^_//' | grep -v '^seat_ticket_v5_\|^room_write_'
#define cb_ct_equal stk_x_cb_ct_equal
#define cb_ct_select32 stk_x_cb_ct_select32
#define cb_ct_select64 stk_x_cb_ct_select64
#define cb_load_be32 stk_x_cb_load_be32
#define cb_load_be64 stk_x_cb_load_be64
#define cb_load_le32 stk_x_cb_load_le32
#define cb_load_le64 stk_x_cb_load_le64
#define cb_mask stk_x_cb_mask
#define cb_span stk_x_cb_span
#define cb_store_be32 stk_x_cb_store_be32
#define cb_store_be64 stk_x_cb_store_be64
#define cb_store_le32 stk_x_cb_store_le32
#define cb_store_le64 stk_x_cb_store_le64
#define checkpoint2_input stk_x_checkpoint2_input
#define ecmult_glv_vartime stk_x_ecmult_glv_vartime
#define ecmult_vartime stk_x_ecmult_vartime
#define hex_decode_size stk_x_hex_decode_size
#define hex_encode_size stk_x_hex_encode_size
#define NC_count stk_x_NC_count
#define nostr_fast_verify_vartime stk_x_nostr_fast_verify_vartime
#define nostr_sign_step stk_x_nostr_sign_step
#define nostr_verify_vartime stk_x_nostr_verify_vartime
#define resume_ticket_prefix stk_x_resume_ticket_prefix
#define S_hash stk_x_S_hash
#define S_hex stk_x_S_hex
#define sc_add stk_x_sc_add
#define sc_equal stk_x_sc_equal
#define sc_evaluate stk_x_sc_evaluate
#define sc_inverse stk_x_sc_inverse
#define sc_is_zero stk_x_sc_is_zero
#define sc_load stk_x_sc_load
#define sc_mul stk_x_sc_mul
#define sc_negate stk_x_sc_negate
#define sc_normalize stk_x_sc_normalize
#define sc_overflow stk_x_sc_overflow
#define sc_reduce_bytes stk_x_sc_reduce_bytes
#define sc_secret_valid stk_x_sc_secret_valid
#define sc_store stk_x_sc_store
#define sc_sub stk_x_sc_sub
#define sc_window stk_x_sc_window
#define schnorr_fast_verify_vartime stk_x_schnorr_fast_verify_vartime
#define schnorr_sign_step stk_x_schnorr_sign_step
#define schnorr_verify_vartime stk_x_schnorr_verify_vartime
#define sf_add stk_x_sf_add
#define sf_debug_add stk_x_sf_debug_add
#define sf_debug_negate stk_x_sf_debug_negate
#define sf_debug_normalize stk_x_sf_debug_normalize
#define sf_debug_valid stk_x_sf_debug_valid
#define sf_equal_mask stk_x_sf_equal_mask
#define sf_inverse stk_x_sf_inverse
#define sf_inverse_vartime stk_x_sf_inverse_vartime
#define sf_load stk_x_sf_load
#define sf_load_strict stk_x_sf_load_strict
#define sf_multiply stk_x_sf_multiply
#define sf_negate stk_x_sf_negate
#define sf_normalize stk_x_sf_normalize
#define sf_sqrt stk_x_sf_sqrt
#define sf_square stk_x_sf_square
#define sf_store stk_x_sf_store
#define sf_subtract stk_x_sf_subtract
#define sg_add stk_x_sg_add
#define sg_decode_public stk_x_sg_decode_public
#define sg_double stk_x_sg_double
#define sg_encode_public stk_x_sg_encode_public
#define sg_equal_mask stk_x_sg_equal_mask
#define sg_from_affine stk_x_sg_from_affine
#define sg_generator stk_x_sg_generator
#define sg_infinity stk_x_sg_infinity
#define sg_lift_x_public stk_x_sg_lift_x_public
#define sg_mixed_add stk_x_sg_mixed_add
#define sg_negate stk_x_sg_negate
#define sg_pick stk_x_sg_pick
#define sg_to_affine stk_x_sg_to_affine
#define sg_valid_mask stk_x_sg_valid_mask
#define sha256_info stk_x_sha256_info
#define sha256_init stk_x_sha256_init
#define sha256_state_size stk_x_sha256_state_size
#define sha256_update_limit stk_x_sha256_update_limit
#define sk_prepare stk_x_sk_prepare
#define sm_equation stk_x_sm_equation
#define sm_wnaf_vartime stk_x_sm_wnaf_vartime
#define smb_generate stk_x_smb_generate
#define smb_multiply_web stk_x_smb_multiply_web
#define smc_multiply stk_x_smc_multiply
#define sn_nonce stk_x_sn_nonce
#define snp_prepare stk_x_snp_prepare
#define so_finish stk_x_so_finish
#define so_init stk_x_so_init
#define so_update stk_x_so_update
#define sp_base stk_x_sp_base
#define sp_x stk_x_sp_x
#define ss_key stk_x_ss_key
#define ssv_nostr_step stk_x_ssv_nostr_step
#define ssv_step stk_x_ssv_step
// coax/lib's generated C keeps a few written-but-unread locals and unused parameters.
#define hmac_sha256_info stk_x_hmac_sha256_info
#define hmac_sha256_state_size stk_x_hmac_sha256_state_size
#if defined(__clang__)
#pragma clang diagnostic ignored "-Wunused-but-set-variable"
#pragma clang diagnostic ignored "-Wunused-parameter"
#pragma clang diagnostic ignored "-Wunused-variable"
#pragma clang diagnostic ignored "-Wunused-function"
#elif defined(__GNUC__)
#pragma GCC diagnostic ignored "-Wunused-but-set-variable"
#pragma GCC diagnostic ignored "-Wunused-parameter"
#pragma GCC diagnostic ignored "-Wunused-variable"
#pragma GCC diagnostic ignored "-Wunused-function"
#endif
#include "ticket-sign.generated.c"
#include <stdbool.h>
#include <string.h>
#include <openssl/rand.h>

// What player.c calls: C strings in, the ticket out. These move bytes only; every rule is the Coax writer's.
bool stk_secret_c(const char *text, unsigned char out[32]);
size_t stk_kid_c(const unsigned char secret[32], char out[9]);
size_t stk_write_c(const unsigned char secret[32], const char *pid, int acct, const char *name, const char *room, int shard,
                   long long expires, int hp, int xp, int wanted, long long owner, const char *ext, int profile, const char *saved,
                   char *out, size_t cap);
int stk_room_write_c(const char *masters, const char *signer, const char *purpose, const char *body, size_t n, const char *mac);

static coax_string stk_text(const char *s) { return (coax_string){s ? s : "", s ? (int64_t)strlen(s) : 0}; }
bool stk_secret_c(const char *text, unsigned char out[32]) {
  return seat_ticket_v5_secret(stk_text(text), (coax_v_u8){out, 32});
}
size_t stk_kid_c(const unsigned char secret[32], char out[9]) {
  coax_string k = seat_ticket_v5_kid((coax_v_u8){(uint8_t *)secret, 32}, (coax_v_u8){(uint8_t *)out, 8});
  out[k.count == 8 ? 8 : 0] = 0;
  return k.count == 8 ? 8 : 0;
}
// The ticket into out (cap at least 4097: 4096 bytes and a NUL), its length; 0 when the writer refused.
// aux is BIP-340's fresh auxiliary randomness, from the platform.
size_t stk_write_c(const unsigned char secret[32], const char *pid, int acct, const char *name, const char *room, int shard,
                   long long expires, int hp, int xp, int wanted, long long owner, const char *ext, int profile, const char *saved,
                   char *out, size_t cap) {
  unsigned char aux[32];
  if (cap < 4097 || RAND_bytes(aux, sizeof aux) != 1) return 0;
  coax_string t = seat_ticket_v5_write((coax_v_u8){(uint8_t *)secret, 32}, (coax_v_u8){aux, 32}, stk_text(pid), acct, stk_text(name),
                                       stk_text(room), shard, expires, hp, xp, wanted, owner, stk_text(ext), profile, stk_text(saved),
                                       (coax_v_u8){(uint8_t *)out, 4096});
  if (t.count <= 0 || t.count > 4096) { out[0] = 0; return 0; }
  out[t.count] = 0;
  return (size_t)t.count;
}
// A room shard's write checked against the API's write masters (room-write.coax room_write_check): the
// shard (1 to 99) that signed it, or 0.
int stk_room_write_c(const char *masters, const char *signer, const char *purpose, const char *body, size_t n, const char *mac) {
  if (!body || n > 32768) return 0;
  return (int)room_write_check(stk_text(masters), stk_text(signer), stk_text(purpose), (coax_string){body, (int64_t)n}, stk_text(mac));
}
