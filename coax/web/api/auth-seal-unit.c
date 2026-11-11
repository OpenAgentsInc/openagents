// auth-seal.coax's C (auth-seal.generated.c) as one unit of the API. The Coax crypto libraries it
// loads (bytes, SHA-256) export their procedures, and so does nip98.generated.c, which loads the
// same ones: the same names twice in one binary. Here this unit's copies get names of their own.
// A library export added later that both use fails the link loudly; add it here.
#define cb_load_be32 as_cb_load_be32
#define cb_load_be64 as_cb_load_be64
#define cb_load_le32 as_cb_load_le32
#define cb_load_le64 as_cb_load_le64
#define cb_store_be32 as_cb_store_be32
#define cb_store_be64 as_cb_store_be64
#define cb_store_le32 as_cb_store_le32
#define cb_store_le64 as_cb_store_le64
#define cb_ct_select32 as_cb_ct_select32
#define cb_ct_select64 as_cb_ct_select64
#define cb_ct_equal as_cb_ct_equal
#define cb_mask as_cb_mask
#define cb_span as_cb_span
#define S_hash as_S_hash
#define S_hex as_S_hex
#define sha256_state_size as_sha256_state_size
#define sha256_update_limit as_sha256_update_limit
#define sha256_info as_sha256_info
#define sha256_init as_sha256_init
#include "auth-seal.generated.c"
