/* A realm script's C runtime: see stdio.h. The byte routines clang may call for a struct copy or a
   zeroed array, written as plain loops so the module needs no bulk-memory instruction. */
#include <stddef.h>
void *memcpy(void *to, const void *from, size_t count) {
  unsigned char *a = to; const unsigned char *b = from;
  while (count--) *a++ = *b++;
  return to;
}
void *memmove(void *to, const void *from, size_t count) {
  unsigned char *a = to; const unsigned char *b = from;
  if (a < b) { while (count--) *a++ = *b++; }
  else { while (count--) a[count] = b[count]; }
  return to;
}
void *memset(void *to, int value, size_t count) {
  unsigned char *a = to;
  while (count--) *a++ = (unsigned char)value;
  return to;
}
int memcmp(const void *a, const void *b, size_t count) {
  const unsigned char *x = a, *y = b;
  for (size_t i = 0; i < count; i++) if (x[i] != y[i]) return x[i] < y[i] ? -1 : 1;
  return 0;
}
