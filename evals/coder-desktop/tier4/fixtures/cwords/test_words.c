#include <stdio.h>
#include "words.h"

int main(void) {
  struct { const char *s; int n; } cases[] = {
    {"", 0}, {"one", 1}, {"  two  words ", 2}, {"a\tb\nc", 3}, {"\n\n", 0}};
  int bad = 0;
  for (unsigned i = 0; i < sizeof cases / sizeof cases[0]; i++)
    if (count_words(cases[i].s) != cases[i].n) { printf("FAIL case %u\n", i); bad = 1; }
  if (!bad) printf("ok\n");
  return bad;
}
