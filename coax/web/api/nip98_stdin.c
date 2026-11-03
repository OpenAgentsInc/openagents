// The API's NIP-98 check (nostr.c over nip98.generated.c, the code oa-api
// runs) on lines from stdin: "METHOD URL<TAB>Nostr <base64>". Prints one
// verdict a line ("ok PUBKEY" or "refused: WHY") and exits 1 when any line
// was refused. For signers checked outside the API, such as the hosted
// runner's (coax/web/agent/runner_sign_check.c, #12630).
//   cc -std=c11 -D_DEFAULT_SOURCE nip98_stdin.c nostr.c nip98.generated.c -lm
#include <stdio.h>
#include <string.h>
#include <time.h>

#include "nostr.h"

int main(void) {
  static char line[16384];
  int bad = 0;
  while (fgets(line, sizeof line, stdin)) {
    size_t n = strlen(line);
    while (n && (line[n - 1] == '\n' || line[n - 1] == '\r')) line[--n] = 0;
    char *tab = strchr(line, '\t'), *sp = strchr(line, ' ');
    if (!tab || !sp || sp > tab) continue;
    *sp = 0; *tab = 0;
    nostr_event ev; char err[256] = "";
    if (nip98_check(tab + 1, line, sp + 1, NULL, 0, (long long)time(NULL), &ev, err, sizeof err)) printf("ok %s\n", ev.pubkey);
    else { printf("refused: %s\n", err); bad++; }
  }
  return bad ? 1 : 0;
}
