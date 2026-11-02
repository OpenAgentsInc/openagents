// Native driver for ecdsa_sign_libsecp.py: one request per stdin line,
//   key-hex hash-hex  ->  compact-hex der-hex, or REFUSED
// It only calls the Coax entries; every decision is theirs.
#include "es.h"
#include <stdio.h>
#include <string.h>
static void unhex(const char *s, uint8_t *out, size_t n) {
  for (size_t i = 0; i < n; i++) { unsigned v; sscanf(s + 2 * i, "%2x", &v); out[i] = (uint8_t)v; }
}
static void hex(const uint8_t *b, size_t n) { for (size_t i = 0; i < n; i++) printf("%02x", b[i]); }
static coax_t_EcdsaWork work;
int main(void) {
  char k[128], h[128];
  while (scanf("%127s %127s", k, h) == 2) {
    if (strlen(k) != 64 || strlen(h) != 64) return 2;
    uint8_t key[32], hash[32], sig[64] = {0}, der[72] = {0};
    unhex(k, key, 32); unhex(h, hash, 32);
    memset(&work, 0xa5, sizeof work);
    _Bool ok = ecdsa_sign((coax_v_u8){key, 32}, (coax_v_u8){hash, 32}, (coax_v_t_EcdsaWork){&work, 1}, (coax_v_u8){sig, 64}, 0);
    int64_t n = ok ? ecdsa_der((coax_v_u8){sig, 64}, (coax_v_u8){der, 72}) : -1;
    // The workspace must be cleared after every call.
    static const uint8_t zero[97];
    int cleared = !memcmp(&work.hk, zero, 32) && !memcmp(&work.hv, zero, 32) && !memcmp(&work.buf, zero, 97) &&
                  !memcmp(&work.x, zero, 32) && !memcmp(&work.k, zero, sizeof work.k) && work.valid == 0;
    if (!ok || n < 8 || !cleared) { printf("REFUSED\n"); continue; }
    hex(sig, 64); printf(" "); hex(der, (size_t)n); printf("\n");
    fflush(stdout);
  }
  return 0;
}
