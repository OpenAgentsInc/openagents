#ifndef OPENAGENTS_MESH_PUBLIC_OUTPUT_H
#define OPENAGENTS_MESH_PUBLIC_OUTPUT_H
#include <stdint.h>
/* Public structured-output facts only. Coax decides what may be emitted.
   Host lifecycle ignores SIGPIPE (as fleet-host.c already does). */
int64_t mpub_stdout_tty(void);
int64_t mpub_stderr_byte(int64_t byte);
#endif
