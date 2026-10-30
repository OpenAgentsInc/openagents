#ifndef OPENAGENTS_MESH_PUBLIC_INPUT_H
#define OPENAGENTS_MESH_PUBLIC_INPUT_H
#include <stdint.h>
#define MPUB_INPUT_MAX (2 * 1024 * 1024)
/* Public protobuf bundle only, not the private replay-witness protocol.
   Reads from exclusively owned stdin until EOF, max2MiB. Regular files,
   FIFOs and AF_UNIX sockets allowed; TTY/device/AF_INET refused. Caller
   validates all format, signatures and policy in Coax. Timeout0 performs a
   bounded nonblocking drain; 1..60000ms has one total monotonic deadline.
   Returns count>=0, -2timeout, -3I/O/unsafe/invalidtimeout, -4overlimit.
   Failure/clear wipes the owned buffer; flags are restored, fd0 stays open. */
int64_t mpub_stdin_read(int64_t timeout_ms);
int64_t mpub_stdin_byte(int64_t index);
int64_t mpub_stdin_clear(void);
#endif
