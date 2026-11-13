/* A realm script's C runtime: see stdio.h. The byte routines are rt.c's. */
#ifndef RS_STRING_H
#define RS_STRING_H
#include <stddef.h>
void *memcpy(void *to, const void *from, size_t count);
void *memmove(void *to, const void *from, size_t count);
void *memset(void *to, int value, size_t count);
int memcmp(const void *a, const void *b, size_t count);
size_t strlen(const char *text);
char *strchr(const char *text, int c);
#endif
