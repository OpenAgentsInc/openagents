// agent-attest.coax's C (agent-attest.generated.c) as one unit of the API. It loads the Coax
// Nostr and crypto libraries (NIP-OA, BIP-340 verify, SHA-256), and nip98.generated.c loads the
// same ones: the same names twice in one binary. Here this unit's copies get names of their own
// (as auth-seal-unit.c). A library export added later that both use fails the link loudly; add it here.
#define cb_ct_equal aa_cb_ct_equal
#define cb_ct_select32 aa_cb_ct_select32
#define cb_ct_select64 aa_cb_ct_select64
#define cb_load_be32 aa_cb_load_be32
#define cb_load_be64 aa_cb_load_be64
#define cb_load_le32 aa_cb_load_le32
#define cb_load_le64 aa_cb_load_le64
#define cb_mask aa_cb_mask
#define cb_span aa_cb_span
#define cb_store_be32 aa_cb_store_be32
#define cb_store_be64 aa_cb_store_be64
#define cb_store_le32 aa_cb_store_le32
#define cb_store_le64 aa_cb_store_le64
#define ecmult_glv_vartime aa_ecmult_glv_vartime
#define ecmult_vartime aa_ecmult_vartime
#define nostr_verify_vartime aa_nostr_verify_vartime
#define S_hash aa_S_hash
#define S_hex aa_S_hex
#define sc_add aa_sc_add
#define sc_equal aa_sc_equal
#define sc_evaluate aa_sc_evaluate
#define sc_inverse aa_sc_inverse
#define sc_is_zero aa_sc_is_zero
#define sc_load aa_sc_load
#define sc_mul aa_sc_mul
#define sc_negate aa_sc_negate
#define sc_normalize aa_sc_normalize
#define sc_overflow aa_sc_overflow
#define sc_reduce_bytes aa_sc_reduce_bytes
#define sc_secret_valid aa_sc_secret_valid
#define sc_store aa_sc_store
#define sc_sub aa_sc_sub
#define sc_window aa_sc_window
#define schnorr_verify_vartime aa_schnorr_verify_vartime
#define sf_add aa_sf_add
#define sf_debug_add aa_sf_debug_add
#define sf_debug_negate aa_sf_debug_negate
#define sf_debug_normalize aa_sf_debug_normalize
#define sf_debug_valid aa_sf_debug_valid
#define sf_equal_mask aa_sf_equal_mask
#define sf_inverse aa_sf_inverse
#define sf_inverse_vartime aa_sf_inverse_vartime
#define sf_load aa_sf_load
#define sf_load_strict aa_sf_load_strict
#define sf_multiply aa_sf_multiply
#define sf_negate aa_sf_negate
#define sf_normalize aa_sf_normalize
#define sf_sqrt aa_sf_sqrt
#define sf_square aa_sf_square
#define sf_store aa_sf_store
#define sf_subtract aa_sf_subtract
#define sg_add aa_sg_add
#define sg_decode_public aa_sg_decode_public
#define sg_double aa_sg_double
#define sg_encode_public aa_sg_encode_public
#define sg_equal_mask aa_sg_equal_mask
#define sg_from_affine aa_sg_from_affine
#define sg_generator aa_sg_generator
#define sg_infinity aa_sg_infinity
#define sg_lift_x_public aa_sg_lift_x_public
#define sg_mixed_add aa_sg_mixed_add
#define sg_negate aa_sg_negate
#define sg_pick aa_sg_pick
#define sg_to_affine aa_sg_to_affine
#define sg_valid_mask aa_sg_valid_mask
#define sha256_info aa_sha256_info
#define sha256_init aa_sha256_init
#define sha256_state_size aa_sha256_state_size
#define sha256_update_limit aa_sha256_update_limit
#define sm_wnaf_vartime aa_sm_wnaf_vartime
// The generated protobuf codec and library code leave parameters and values unused in places; they
// are the generator's, not ours, so those warnings stay quiet for this unit only (-Werror in test.sh).
#pragma GCC diagnostic ignored "-Wunused-parameter"
#pragma GCC diagnostic ignored "-Wunused-variable"
#pragma GCC diagnostic ignored "-Wunused-but-set-variable"
#pragma GCC diagnostic ignored "-Wunused-function"
#include "agent-attest.generated.c"
