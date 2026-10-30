/* Harness for coax/tests/net/tls.sh: calls the Coax driver (tls_ws.coax)
   and compares its result with what the script expects.
     tls_test connect URL EXPECT      TCP + TLS handshake only
     tls_test echo URL EXPECT         WebSocket upgrade and one echoed message
     tls_test eose URL [REQ COUNT]    a read-only REQ (subscription "coax-tls")
                                      until EOSE; COUNT events expected
   EXPECT is an integer, or "neg" for any failure code. */
#include "tls_ws.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static coax_string view(const char *s) {
  coax_string v; v.data = s; v.count = (int64_t)strlen(s); return v;
}

static int check(const char *what, const char *url, int64_t got, const char *expect) {
  int ok = strcmp(expect, "neg") == 0 ? got < 0 : got == strtoll(expect, NULL, 0);
  printf("%s %s %s: got %lld, want %s\n", ok ? "PASS" : "FAIL", what, url, (long long)got, expect);
  return ok ? 0 : 1;
}

int main(int argc, char **argv) {
  if (argc == 4 && strcmp(argv[1], "connect") == 0) return check("connect", argv[2], tlsws_connect(view(argv[2])), argv[3]);
  if (argc == 4 && strcmp(argv[1], "echo") == 0) {
    return check("echo", argv[2], tlsws_echo(view(argv[2]), view("hello over wss from coax")), argv[3]);
  }
  if ((argc == 3 || argc == 5) && strcmp(argv[1], "eose") == 0) {
    const char *req = argc == 5 ? argv[3] : "[\"REQ\",\"coax-tls\",{\"kinds\":[0],\"limit\":2}]";
    int64_t n = tlsws_req_eose(view(argv[2]), view(req));
    int ok = argc == 5 ? n == strtoll(argv[4], NULL, 0) : n >= 0;
    printf("%s eose %s: %lld events before EOSE%s%s\n", ok ? "PASS" : "FAIL", argv[2], (long long)n,
           argc == 5 ? ", want " : "", argc == 5 ? argv[4] : "");
    return ok ? 0 : 1;
  }
  fprintf(stderr, "usage: tls_test connect|echo URL EXPECT | eose URL [REQ COUNT]\n");
  return 2;
}
