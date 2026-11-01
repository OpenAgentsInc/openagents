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
#ifdef COAX_coax_v_u8_DEFINED
#ifndef COAX_SCHEMA_coax_v_u8_31343a636f61782d632d747970652d7631343a76696577393a636f61785f765f7538323a3136313a38373a75696e74385f74
#error "incompatible Coax C type coax_v_u8"
#endif
#else
#define COAX_coax_v_u8_DEFINED
#define COAX_SCHEMA_coax_v_u8_31343a636f61782d632d747970652d7631343a76696577393a636f61785f765f7538323a3136313a38373a75696e74385f74
typedef struct { uint8_t *data; int64_t count; } coax_v_u8;
#endif
/* The terminal, for a full-screen tool. cxterm_raw(1) turns off echo and line
   buffering on stdin (stty -echo -icanon min 0 time 0) and cxterm_raw(0) puts
   back what was there; cxterm_size() is rows * 65536 + columns, or -1 off a
   terminal; cxterm_byte(ms) is one byte from stdin within ms, -1 for none, -2
   at its end; cxterm_write writes bytes to stdout at once. cxterm_watch()
   catches SIGINT, SIGTERM and SIGHUP (cxterm_stopped() then names the signal)
   and SIGWINCH (cxterm_resized() says once that the size changed). */
int64_t cxterm_raw(int64_t on);
int64_t cxterm_size(void);
int64_t cxterm_byte(int64_t timeout_ms);
int64_t cxterm_write(coax_v_u8 s);
int64_t cxterm_watch(void);
int64_t cxterm_stopped(void);
int64_t cxterm_resized(void);
#ifdef __cplusplus
}
#endif
#endif
