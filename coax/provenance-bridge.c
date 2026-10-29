// Command-line transport for the checked Coax comparison policy.
#include "provenance.h"
#include <string.h>

int main(int argc, char **argv) {
  if (argc != 7) return 2;
  coax_string s[6];
  for (int i = 0; i < 6; ++i) {
    size_t n = strlen(argv[i+1]);
    if (n != 64) return 1;
    s[i] = (coax_string){ argv[i+1], (int64_t)n };
  }
  return PV_match(s[0],s[1],s[2],s[3],s[4],s[5]) ? 0 : 1;
}
