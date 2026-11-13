/* A realm script's C runtime: see stdio.h. abort is the WebAssembly trap instruction; nothing allocates. */
#ifndef RS_STDLIB_H
#define RS_STDLIB_H
#include <stddef.h>
#define abort() __builtin_trap()
#define malloc(size) ((void *)0)
#define calloc(count, size) ((void *)0)
#define realloc(pointer, size) ((void *)0)
#define free(pointer) ((void)0)
/* Named only by the emitted print helpers, which a script never reaches; left undefined on purpose:
   a module that did reach one would import it, and the module check refuses that import. */
double strtod(const char *text, char **end);
int atoi(const char *text);
#endif
