#define cb_load_be32 prw_cb_load_be32
#define cb_load_be64 prw_cb_load_be64
#define cb_load_le32 prw_cb_load_le32
#define cb_load_le64 prw_cb_load_le64
#define cb_store_be32 prw_cb_store_be32
#define cb_store_be64 prw_cb_store_be64
#define cb_store_le32 prw_cb_store_le32
#define cb_store_le64 prw_cb_store_le64
#define cb_ct_select32 prw_cb_ct_select32
#define cb_ct_select64 prw_cb_ct_select64
#define cb_ct_equal prw_cb_ct_equal
#define cb_mask prw_cb_mask
#define cb_span prw_cb_span
#define S_hash prw_S_hash
#define S_hex prw_S_hex
#define sha256_state_size prw_sha256_state_size
#define sha256_update_limit prw_sha256_update_limit
#define sha256_info prw_sha256_info
#define sha256_init prw_sha256_init
// Generated gateway helpers retain their source signature even when a parameter
// is unused. Keep the SDK build's warnings scoped to this generated unit.
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wunused-parameter"
#include "projects-wire.generated.c"
#pragma GCC diagnostic pop
