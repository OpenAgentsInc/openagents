#include <string.h>
#include "strrev.h"

/* Reverse s in place. */
void str_reverse(char *s) {
  size_t n = strlen(s);
  for (size_t i = 0; i <= n / 2; i++) {
    char t = s[i];
    s[i] = s[n - i];
    s[n - i] = t;
  }
}
