// wallet-vault.coax's C (wallet-vault.generated.c) as one unit of the API (#13217). It loads Coax
// libraries that other generated units load too (bytes, SHA-256, JSON, base64, BIP-39, scrypt),
// and those export their procedures: the same names twice in one binary. Here every export of
// this unit but its own (wv_slots, wv_vault, wv_count, wv_object, wv_slot, wv_credential, wv_b64,
// wv_unb64) gets a name of its own. A library export added later fails the link loudly;
// regenerate this list:
//   nm -g --defined-only wallet-vault.o | awk '{print $3}' | sed 's/^_//' | grep -v '^wv_'
#define base64_encode_size wvu_x_base64_encode_size
#define base64_text_count wvu_x_base64_text_count
#define bip39_decode_ct wvu_x_bip39_decode_ct
#define bip39_encode_ct wvu_x_bip39_encode_ct
#define bip39_entropy wvu_x_bip39_entropy
#define bip39_entropy_len wvu_x_bip39_entropy_len
#define bip39_normal24 wvu_x_bip39_normal24
#define bip39_seed wvu_x_bip39_seed
#define bip39_seed_ct wvu_x_bip39_seed_ct
#define bip39_words wvu_x_bip39_words
#define blockmix_owned wvu_x_blockmix_owned
#define cb_ct_equal wvu_x_cb_ct_equal
#define cb_ct_select32 wvu_x_cb_ct_select32
#define cb_ct_select64 wvu_x_cb_ct_select64
#define cb_load_be32 wvu_x_cb_load_be32
#define cb_load_be64 wvu_x_cb_load_be64
#define cb_load_le32 wvu_x_cb_load_le32
#define cb_load_le64 wvu_x_cb_load_le64
#define cb_mask wvu_x_cb_mask
#define cb_span wvu_x_cb_span
#define cb_store_be32 wvu_x_cb_store_be32
#define cb_store_be64 wvu_x_cb_store_be64
#define cb_store_le32 wvu_x_cb_store_le32
#define cb_store_le64 wvu_x_cb_store_le64
#define cc_admit wvu_x_cc_admit
#define chacha20_block_checked wvu_x_chacha20_block_checked
#define chacha20_xor_checked wvu_x_chacha20_xor_checked
#define coax_default_t_WbSpan wvu_x_coax_default_t_WbSpan
#define coax_default_t_WvSlot wvu_x_coax_default_t_WvSlot
#define hmac_owned wvu_x_hmac_owned
#define hmac_sha256_info wvu_x_hmac_sha256_info
#define hmac_sha256_state_size wvu_x_hmac_sha256_state_size
#define hmac_sha512 wvu_x_hmac_sha512
#define ja_integer wvu_x_ja_integer
#define ja_policy wvu_x_ja_policy
#define ja_policy_valid wvu_x_ja_policy_valid
#define jcs_admit_counts wvu_x_jcs_admit_counts
#define jcs_canonicalize_checked wvu_x_jcs_canonicalize_checked
#define NC_count wvu_x_NC_count
#define nip44_calc_padded_len wvu_x_nip44_calc_padded_len
#define nip44_decrypt_work_size wvu_x_nip44_decrypt_work_size
#define nip44_encrypt_request wvu_x_nip44_encrypt_request
#define nip44_encrypt_work_size wvu_x_nip44_encrypt_work_size
#define nip44_max_payload wvu_x_nip44_max_payload
#define nip44_payload_size wvu_x_nip44_payload_size
#define nip44_profile_max wvu_x_nip44_profile_max
#define nip44_raw_size wvu_x_nip44_raw_size
#define nip49_scrypt_finish wvu_x_nip49_scrypt_finish
#define nip49_scrypt_start wvu_x_nip49_scrypt_start
#define nip49_scrypt_step wvu_x_nip49_scrypt_step
#define pbkdf_owned wvu_x_pbkdf_owned
#define pbkdf2_hmac_sha512 wvu_x_pbkdf2_hmac_sha512
#define romix_finish wvu_x_romix_finish
#define romix_start wvu_x_romix_start
#define romix_step wvu_x_romix_step
#define S_hash wvu_x_S_hash
#define S_hex wvu_x_S_hex
#define S512_hash wvu_x_S512_hash
#define salsa20_8_core wvu_x_salsa20_8_core
#define scrypt_blockmix wvu_x_scrypt_blockmix
#define scrypt_pbkdf2_one wvu_x_scrypt_pbkdf2_one
#define sha256_info wvu_x_sha256_info
#define sha256_init wvu_x_sha256_init
#define sha256_state_size wvu_x_sha256_state_size
#define sha256_update_limit wvu_x_sha256_update_limit
#define sha512 wvu_x_sha512
#define sha512_info wvu_x_sha512_info
#define sha512_init wvu_x_sha512_init
#define sha512_state_size wvu_x_sha512_state_size
#define sha512_two wvu_x_sha512_two
#define sha512_update_limit wvu_x_sha512_update_limit
#define so_finish wvu_x_so_finish
#define so_init wvu_x_so_init
#define so_update wvu_x_so_update
#define wb_code_normal wvu_x_wb_code_normal
#define wb_code_words wvu_x_wb_code_words
#define wb_enough wvu_x_wb_enough
#define wb_make wvu_x_wb_make
#define wb_object wvu_x_wb_object
#define wb_open wvu_x_wb_open
#define wb_pick wvu_x_wb_pick
#define wb_prf_input wvu_x_wb_prf_input
#define wb_recovery_params wvu_x_wb_recovery_params
#define wb_recovery_secret wvu_x_wb_recovery_secret
#define wb_slot_nostr wvu_x_wb_slot_nostr
#define wb_slot_prf wvu_x_wb_slot_prf
#define wb_slot_recovery wvu_x_wb_slot_recovery
#define wb_slots_check wvu_x_wb_slots_check
#define wb_vmk wvu_x_wb_vmk
// coax/lib's generated C keeps a few written-but-unread locals and an unread parameter.
#if defined(__clang__)
#pragma clang diagnostic ignored "-Wunused-but-set-variable"
#pragma clang diagnostic ignored "-Wunused-parameter"
#elif defined(__GNUC__)
#pragma GCC diagnostic ignored "-Wunused-but-set-variable"
#pragma GCC diagnostic ignored "-Wunused-parameter"
#endif
#include "wallet-vault.generated.c"
