/* For generated C built for Windows (GitHub #13782): MinGW's stdio.h makes stdin, stdout and
   stderr macros, and a Coax struct may have fields with those names (coax/lib/agent/claude's
   hook events). After this header the three are plain variables, which a field may share a
   name with; tool-win32.c defines them and fills them before main. Pass it with -include. */
#ifndef COAX_WIN32_STDIO_H
#define COAX_WIN32_STDIO_H
#include <stdio.h>
#undef stdin
#undef stdout
#undef stderr
extern FILE *stdin, *stdout, *stderr;
#endif
