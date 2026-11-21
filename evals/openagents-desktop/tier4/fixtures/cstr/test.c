#include <stdio.h>
#include <string.h>
#include "strrev.h"

static int check(const char *in, const char *want) {
  char buf[64];
  strcpy(buf, in);
  str_reverse(buf);
  if (strcmp(buf, want) != 0) { printf("FAIL %s -> %s (want %s)\n", in, buf, want); return 1; }
  return 0;
}

int main(void) {
  int bad = check("abc", "cba") + check("ab", "ba") + check("", "") + check("racecar!", "!racecar");
  if (!bad) printf("ok\n");
  return bad;
}
