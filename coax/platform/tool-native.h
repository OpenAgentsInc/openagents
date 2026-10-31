#ifndef COAX_TOOL_NATIVE_H
#define COAX_TOOL_NATIVE_H
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
/* The host of a command-line tool written in Coax (coax/lib/sys/): effects
   only. The tool is a Coax library whose `tool_main :: () -> int #export_c`
   is the program; this file's main() saves argv and runs tool_main on a
   thread with a large stack (COAX_TOOL_STACK, 1 GiB of address space,
   committed only as it is touched), because a Coax program keeps its
   buffers in locals and a browser frame is megabytes. tool_main's result is
   the exit status.

   cxargs_count() is argc; cxargs_len(i) argument i's length (-1 past the
   end); cxargs_byte(i, j) its byte j (-1 past it). cxtool_err(byte) buffers
   one byte for stderr and cxtool_err_flush() writes them. cxtool_os() is 1
   on macOS, 2 on Linux, 0 elsewhere. */
#define COAX_TOOL_STACK ((size_t)1 << 30)
int64_t tool_main(void);
int64_t cxargs_count(void);
int64_t cxargs_len(int64_t i);
int64_t cxargs_byte(int64_t i, int64_t j);
int64_t cxtool_err(int64_t byte);
int64_t cxtool_err_flush(void);
int64_t cxtool_os(void);
#ifdef __cplusplus
}
#endif
#endif
