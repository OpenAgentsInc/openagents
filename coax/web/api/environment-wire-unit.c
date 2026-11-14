// Generated Coax request validation and bounded child-process adapter.
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wunused-parameter"
#include "environment-wire.generated.c"
#pragma GCC diagnostic pop
#include <signal.h>
int64_t environment_proxy(coax_string host,coax_string grant,coax_string epoch,
  coax_string relay,coax_string keyfile,coax_string input,coax_string program,coax_v_u8 out) {
  struct sigaction before,waiting = {0};
  waiting.sa_handler = SIG_DFL;
  sigemptyset(&waiting.sa_mask);
  if (sigaction(SIGCHLD,&waiting,&before) != 0) return -5000;
  int64_t n=ea_proxy(host,grant,epoch,relay,keyfile,input,program,out);
  if (sigaction(SIGCHLD,&before,NULL) != 0) return -5001;
  return n;
}
