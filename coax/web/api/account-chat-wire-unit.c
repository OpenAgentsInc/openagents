#define pcb_context acw_pcb_context
#define cb_load_be32 acw_cb_load_be32
#define cb_load_be64 acw_cb_load_be64
#define cb_load_le32 acw_cb_load_le32
#define cb_load_le64 acw_cb_load_le64
#define cb_store_be32 acw_cb_store_be32
#define cb_store_be64 acw_cb_store_be64
#define cb_store_le32 acw_cb_store_le32
#define cb_store_le64 acw_cb_store_le64
#define cb_ct_select32 acw_cb_ct_select32
#define cb_ct_select64 acw_cb_ct_select64
#define cb_ct_equal acw_cb_ct_equal
#define cb_mask acw_cb_mask
#define cb_span acw_cb_span
#define S_hash acw_S_hash
#define S_hex acw_S_hex
#define sha256_state_size acw_sha256_state_size
#define sha256_update_limit acw_sha256_update_limit
#define sha256_info acw_sha256_info
#define sha256_init acw_sha256_init
// Generated gateway helpers retain their source signature even when a parameter
// is unused. Keep the SDK build's warnings scoped to this generated unit.
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wunused-parameter"
#include "account-chat-wire.generated.c"
#pragma GCC diagnostic pop
