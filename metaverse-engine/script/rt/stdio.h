/* A realm script's C runtime (GitHub #13467). A script module has no libc: its only imports are the
   script API (metaverse-engine/script/api-table.coax). These headers stand where the C that coax emits
   includes the standard ones. Printing is nothing (a script cannot print: its effects exclude io), and
   the one message the emitted runtime prints, a failed check's "path:line: what", becomes the import
   sa_trap(line), so the engine's diagnostic names the source line. */
#ifndef RS_STDIO_H
#define RS_STDIO_H
#include <stdint.h>
typedef struct rs_file FILE;
#define stdout ((FILE *)0)
#define stderr ((FILE *)0)
__attribute__((import_module("env"), import_name("sa_trap"))) void sa_trap(int32_t line);
#define fflush(stream) ((void)0)
#define fprintf(stream, format, path, line, what) sa_trap(line)
#define printf(...) ((void)0)
#define snprintf(buffer, size, ...) ((buffer)[0] = 0, 0)
#define fputs(text, stream) ((void)0)
#define fputc(c, stream) ((void)0)
#define putchar(c) ((void)0)
#define fwrite(data, size, count, stream) ((void)0)
#endif
