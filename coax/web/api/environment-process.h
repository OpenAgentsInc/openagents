// The API's background routes ignore SIGCHLD. A waited broker must retain
// its exit status; workers serve one request at a time, so restore the old
// disposition after this synchronous call.
#ifndef OA_ENVIRONMENT_PROCESS_H
#define OA_ENVIRONMENT_PROCESS_H
#include "environment-wire.generated.h"
int64_t environment_proxy(coax_string host,coax_string grant,coax_string epoch,
  coax_string relay,coax_string keyfile,coax_string input,coax_string program,coax_v_u8 out);
#endif
