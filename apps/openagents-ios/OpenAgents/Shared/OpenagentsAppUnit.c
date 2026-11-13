// apps/shared/openagents-app.coax's C (Generated/OpenagentsApp.c) as one unit of the app (#13516). It
// loads the session record, which loads Coax libraries (sha256, constant-time bytes) that other
// generated units load too, and a library's exports would then be defined twice in one binary. Here
// every export of this unit but the ones Swift calls (oap_host_*) gets a name of its own. A library
// export added later fails the link loudly; regenerate this list:
//   nm -g --defined-only OpenagentsApp.o | awk '{print $3}' | sed 's/^_//' | grep -v '^oap_host'
#define S_hash oapu_x_S_hash
#define S_hex oapu_x_S_hex
#define cb_ct_equal oapu_x_cb_ct_equal
#define cb_ct_select32 oapu_x_cb_ct_select32
#define cb_ct_select64 oapu_x_cb_ct_select64
#define cb_load_be32 oapu_x_cb_load_be32
#define cb_load_be64 oapu_x_cb_load_be64
#define cb_load_le32 oapu_x_cb_load_le32
#define cb_load_le64 oapu_x_cb_load_le64
#define cb_mask oapu_x_cb_mask
#define cb_span oapu_x_cb_span
#define cb_store_be32 oapu_x_cb_store_be32
#define cb_store_be64 oapu_x_cb_store_be64
#define cb_store_le32 oapu_x_cb_store_le32
#define cb_store_le64 oapu_x_cb_store_le64
#define sha256_info oapu_x_sha256_info
#define sha256_init oapu_x_sha256_init
#define sha256_state_size oapu_x_sha256_state_size
#define sha256_update_limit oapu_x_sha256_update_limit
#pragma clang diagnostic ignored "-Wcomma"
#pragma clang diagnostic ignored "-Wunused-function"
#pragma clang diagnostic ignored "-Wunused-variable"
#pragma clang diagnostic ignored "-Wunused-but-set-variable"
#pragma clang diagnostic ignored "-Wunused-parameter"
#pragma clang diagnostic ignored "-Wshorten-64-to-32"
#include "OpenagentsApp.c"
