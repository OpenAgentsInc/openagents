/* NIP-49's published ncryptsec vector at its real cost, log_n 16, through
 * the Coax scrypt (coax/lib/crypto/scrypt_nip49.coax) with its 64 MiB
 * workspace on the heap. Coax has no heap, so the host allocates V
 * (128 * r * N bytes) and passes it in as a view; the library refuses a
 * view that is too small instead of writing past it. Run under ASan/UBSan
 * by ncryptsec_vector.sh. Every input here is public test data. */
#include "ncryptsec_vector.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* The vector's 91 payload bytes: 02, 10 (log_n 16), salt, nonce, 00, box. */
static const char *PAYLOAD = "021052d7c3f8580e7b41953381e5bc49646bc33f02a7dcaac8bdd8da23cd449783240b6ebc12edeea7bf00b8e8803440de7b3e9519c3e734cb2ac9a211ea2dc52312e5117a11a3022d813ab438719ca0b504a1193be510c3aee776";
/* scrypt("nostr", salt, N = 2^16, r = 8, p = 1), from Python's hashlib.scrypt. */
static const char *KEY = "6d1e8b279f52b3a9ba60a2b43cab50804a67ac763ff4bafa782475efb0722aff";
/* The private key NIP-49 publishes for the vector. */
static const char *PRIVATE = "3501454135014541350145413501453fefb02227e449e57cf4d3a3ce05378683";

enum { ROMIX_DONE = 3, LOG_N = 16, WORKSPACE = 128 * 8 * (1 << LOG_N) };

static int failures;
#define CHECK(c) do { if (!(c)) { fprintf(stderr, "FAIL line %d: %s\n", __LINE__, #c); failures++; } } while (0)

static void unhex(const char *text, uint8_t *out, size_t n) {
  for (size_t i = 0; i < n; i++) { unsigned v; sscanf(text + 2 * i, "%2x", &v); out[i] = (uint8_t)v; }
}
static coax_v_u8 view(uint8_t *p, size_t n) { return (coax_v_u8){p, (int64_t)n}; }

int main(void) {
  static coax_t_Nip49Scrypt state;
  coax_v_t_Nip49Scrypt st = {&state, 1};
  uint8_t payload[91], key[32], private_key[32], out[33], secret[32], password[5];
  memcpy(password, "nostr", 5);
  unhex(PAYLOAD, payload, 91); unhex(KEY, key, 32); unhex(PRIVATE, private_key, 32);
  uint8_t *salt = payload + 2, *nonce = payload + 18, *aad = payload + 42, *box = payload + 43;
  CHECK(payload[1] == LOG_N);

  /* A workspace one byte short is refused before anything is written: the
   * heap block is exactly that size, so ASan reports any write past it. */
  uint8_t *small = malloc(WORKSPACE - 1);
  CHECK(small != NULL);
  memset(&state, 0xa5, sizeof state);
  CHECK(!nip49_scrypt_start(st, view(password, 5), view(salt, 16), view(small, WORKSPACE - 1), LOG_N, 0));
  CHECK(((uint8_t *)&state)[0] == 0xa5 && state.phase == (int64_t)0xa5a5a5a5a5a5a5a5ULL);
  free(small);

  uint8_t *workspace = malloc(WORKSPACE);
  CHECK(workspace != NULL);
  /* log_n 17 needs 128 MiB; 64 MiB is refused for it. */
  CHECK(!nip49_scrypt_start(st, view(password, 5), view(salt, 16), view(workspace, WORKSPACE), LOG_N + 1, 0));

  /* The real run. */
  CHECK(nip49_scrypt_start(st, view(password, 5), view(salt, 16), view(workspace, WORKSPACE), LOG_N, 0));
  /* A step given a shorter view of the workspace is refused too. */
  CHECK(!nip49_scrypt_step(st, view(workspace, WORKSPACE - 1), 64, 0));
  long steps = 0;
  while (state.romix.e[0].phase != ROMIX_DONE && steps < 4 * (1L << LOG_N)) {
    if (!nip49_scrypt_step(st, view(workspace, WORKSPACE), 64, 0)) break;
    steps++;
  }
  CHECK(state.romix.e[0].phase == ROMIX_DONE);
  out[32] = 0xa5;
  CHECK(nip49_scrypt_finish(st, view(out, 32), 0));
  CHECK(memcmp(out, key, 32) == 0 && out[32] == 0xa5);
  free(workspace);

  /* Open the vector with the key scrypt derived. */
  CHECK(xcp_open(view(out, 32), view(nonce, 24), view(aad, 1), view(box, 48), view(secret, 32), 0));
  CHECK(memcmp(secret, private_key, 32) == 0);

  if (failures) { fprintf(stderr, "%d failed\n", failures); return 1; }
  printf("ncryptsec vector: log_n 16 in a 64 MiB heap workspace, %ld steps, key and private key match NIP-49\n", steps);
  return 0;
}
