// Generated Coax request validation and bounded child-process adapter.
// The primary execution codec and Projects share this exported budget rule.
// Keep this unit's generated instance private to its C namespace, as the
// Projects wrapper already does for shared cryptographic exports.
#define pcb_context environment_pcb_context
#define cb_load_be32 environment_cb_load_be32
#define cb_load_be64 environment_cb_load_be64
#define cb_load_le32 environment_cb_load_le32
#define cb_load_le64 environment_cb_load_le64
#define cb_store_be32 environment_cb_store_be32
#define cb_store_be64 environment_cb_store_be64
#define cb_store_le32 environment_cb_store_le32
#define cb_store_le64 environment_cb_store_le64
#define cb_ct_select32 environment_cb_ct_select32
#define cb_ct_select64 environment_cb_ct_select64
#define cb_ct_equal environment_cb_ct_equal
#define cb_mask environment_cb_mask
#define cb_span environment_cb_span
#define S_hash environment_S_hash
#define S_hex environment_S_hex
#define sha256_state_size environment_sha256_state_size
#define sha256_update_limit environment_sha256_update_limit
#define sha256_info environment_sha256_info
#define sha256_init environment_sha256_init
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wunused-parameter"
#include "environment-wire.generated.c"
#pragma GCC diagnostic pop
#include <signal.h>
#include <stdbool.h>
static int64_t environment_proxy_kind(int accounts,coax_string host,coax_string grant,coax_string epoch,
  coax_string relay,coax_string keyfile,coax_string input,coax_string program,coax_v_u8 out) {
  struct sigaction before,waiting = {0};
  waiting.sa_handler = SIG_DFL;
  sigemptyset(&waiting.sa_mask);
  if (sigaction(SIGCHLD,&waiting,&before) != 0) return -5000;
  int64_t n=accounts==2 ? ta_proxy(host,grant,epoch,relay,keyfile,input,program,out):accounts ? pa_proxy(host,grant,epoch,relay,keyfile,input,program,out):ea_proxy(host,grant,epoch,relay,keyfile,input,program,out);
  if (sigaction(SIGCHLD,&before,NULL) != 0) return -5001;
  return n;
}

int64_t environment_station_proxy(coax_string host,coax_string grant,coax_string epoch,coax_string relay,coax_string keyfile,bool refresh,coax_string program,coax_v_u8 out) {
  struct sigaction before,waiting = {0};
  waiting.sa_handler = SIG_DFL;
  sigemptyset(&waiting.sa_mask);
  if (sigaction(SIGCHLD,&waiting,&before) != 0) return -5000;
  int64_t n=st_proxy(host,grant,epoch,relay,keyfile,refresh,program,out);
  if (sigaction(SIGCHLD,&before,NULL) != 0) return -5001;
  return n;
}
int64_t environment_proxy(coax_string host,coax_string grant,coax_string epoch,coax_string relay,coax_string keyfile,coax_string input,coax_string program,coax_v_u8 out) {return environment_proxy_kind(0,host,grant,epoch,relay,keyfile,input,program,out);}
int64_t environment_accounts_proxy(coax_string host,coax_string grant,coax_string epoch,coax_string relay,coax_string keyfile,coax_string input,coax_string program,coax_v_u8 out) {return environment_proxy_kind(1,host,grant,epoch,relay,keyfile,input,program,out);}
int64_t environment_task_proxy(coax_string host,coax_string grant,coax_string epoch,coax_string relay,coax_string keyfile,coax_string input,coax_string program,coax_v_u8 out) {return environment_proxy_kind(2,host,grant,epoch,relay,keyfile,input,program,out);}
