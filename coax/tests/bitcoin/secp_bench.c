// Times Coax's secp256k1 against libsecp256k1 in one process (issue #13450).
// secp_bench.sh builds and runs it. libsecp256k1 makes the keys and
// signatures; both libraries must then decide every signature the same way,
// valid and with one bit changed, before any time is reported. Test-only.
#include "secp_bench.h"
#include "secp256k1.h"
#include "secp256k1_extrakeys.h"
#include "secp256k1_schnorrsig.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#define N 256
#define PASSES 7
static uint8_t secret[N][32], hash[N][32], pub33[N][33], xonly[N][32], ecdsa[N][64], schnorr[N][64], aux[N][32];
static secp256k1_context *ctx;
static uint64_t seed = 0x13450;
static uint8_t next_byte(void) { seed = seed * 6364136223846793005ULL + 1442695040888963407ULL; return (uint8_t)(seed >> 56); }
static double now(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); return t.tv_sec * 1e6 + t.tv_nsec / 1e3; }
static coax_v_u8 v(uint8_t *p, int64_t n) { return (coax_v_u8){p, n}; }

static int ref_ecdsa(int i, const uint8_t *sig) {
  secp256k1_pubkey key; secp256k1_ecdsa_signature s;
  if (!secp256k1_ec_pubkey_parse(ctx, &key, pub33[i], 33)) return 0;
  if (!secp256k1_ecdsa_signature_parse_compact(ctx, &s, sig)) return 0;
  return secp256k1_ecdsa_verify(ctx, &s, hash[i], &key);
}
static int ref_schnorr(int i, const uint8_t *sig) {
  secp256k1_xonly_pubkey key;
  if (!secp256k1_xonly_pubkey_parse(ctx, &key, xonly[i])) return 0;
  return secp256k1_schnorrsig_verify(ctx, sig, hash[i], 32, &key);
}
static int coax_ecdsa(int i, const uint8_t *sig) { return ecdsa_verify_vartime(v(hash[i], 32), v(pub33[i], 33), v((uint8_t *)sig, 64), 1); }
static int coax_schnorr(int i, const uint8_t *sig) { return schnorr_verify_vartime(v(hash[i], 32), v(xonly[i], 32), v((uint8_t *)sig, 64)); }
static int ref_ecdsa_sign(int i, const uint8_t *unused) {
  secp256k1_ecdsa_signature s; uint8_t out[64]; (void)unused;
  if (!secp256k1_ecdsa_sign(ctx, &s, hash[i], secret[i], NULL, NULL)) return 0;
  secp256k1_ecdsa_signature_serialize_compact(ctx, out, &s);
  return !memcmp(out, ecdsa[i], 64);
}
static int ref_schnorr_sign(int i, const uint8_t *unused) {
  secp256k1_keypair pair; uint8_t out[64]; (void)unused;
  if (!secp256k1_keypair_create(ctx, &pair, secret[i])) return 0;
  if (!secp256k1_schnorrsig_sign32(ctx, out, hash[i], &pair, aux[i])) return 0;
  return !memcmp(out, schnorr[i], 64);
}
static coax_t_EcdsaWork ew; static coax_t_SignWork sw;
static int coax_ecdsa_sign(int i, const uint8_t *unused) {
  uint8_t out[64]; (void)unused; memset(&ew, 0, sizeof ew);
  if (!ecdsa_sign(v(secret[i], 32), v(hash[i], 32), (coax_v_t_EcdsaWork){&ew, 1}, v(out, 64), 0)) return 0;
  return !memcmp(out, ecdsa[i], 64);
}
static int coax_schnorr_sign(int i, const uint8_t *unused) {
  uint8_t out[64]; int64_t status, calls = 0; (void)unused; memset(&sw, 0, sizeof sw);
  do { status = schnorr_sign_step(v(secret[i], 32), v(aux[i], 32), v(hash[i], 32), (coax_v_t_SignWork){&sw, 1}, v(out, 64), 0); calls++; } while (status == 1 && calls < 16);
  return status == 2 && !memcmp(out, schnorr[i], 64);
}
// Microseconds for one call: the best of PASSES passes over count inputs.
static double rate(int (*f)(int, const uint8_t *), uint8_t (*sig)[64], int count) {
  double best = 1e18;
  for (int pass = 0; pass < PASSES; pass++) {
    double start = now(); int ok = 0;
    for (int i = 0; i < count; i++) ok += f(i, sig ? sig[i] : NULL);
    double each = (now() - start) / count;
    if (ok != count) { printf("FAIL: %d of %d calls gave the expected result\n", ok, count); exit(1); }
    if (each < best) best = each;
  }
  return best;
}
static void hexline(const uint8_t *b, int n) { for (int i = 0; i < n; i++) printf("%02x", b[i]); printf("\n"); }
static void row(const char *name, double coax, double ref) { printf("%-16s %10.2f %10.2f %8.2f\n", name, coax, ref, coax / ref); }
int main(int argc, char **argv) {
  int sign_count = argc > 1 ? atoi(argv[1]) : N;
  ctx = secp256k1_context_create(SECP256K1_CONTEXT_NONE);
  for (int i = 0; i < N; i++) {
    secp256k1_pubkey key; secp256k1_keypair pair; secp256k1_xonly_pubkey x; secp256k1_ecdsa_signature s; size_t n = 33;
    for (int j = 0; j < 32; j++) { secret[i][j] = next_byte(); hash[i][j] = next_byte(); aux[i][j] = next_byte(); }
    if (!secp256k1_ec_pubkey_create(ctx, &key, secret[i]) || !secp256k1_keypair_create(ctx, &pair, secret[i])) return 2;
    secp256k1_ec_pubkey_serialize(ctx, pub33[i], &n, &key, SECP256K1_EC_COMPRESSED);
    secp256k1_keypair_xonly_pub(ctx, &x, NULL, &pair); secp256k1_xonly_pubkey_serialize(ctx, xonly[i], &x);
    if (!secp256k1_ecdsa_sign(ctx, &s, hash[i], secret[i], NULL, NULL)) return 2;
    secp256k1_ecdsa_signature_serialize_compact(ctx, ecdsa[i], &s);
    if (!secp256k1_schnorrsig_sign32(ctx, schnorr[i], hash[i], &pair, aux[i])) return 2;
  }
  // `secp_bench N dump` prints the keys and signatures for secp_bench_wasm.cjs.
  if (argc > 2 && !strcmp(argv[2], "dump")) {
    for (int i = 0; i < N; i++) {
      hexline(hash[i], 32); hexline(pub33[i], 33); hexline(xonly[i], 32); hexline(ecdsa[i], 64); hexline(schnorr[i], 64);
    }
    return 0;
  }
  // Agreement first: each signature, and each with one bit of r or s changed.
  int checked = 0;
  for (int i = 0; i < N; i++) {
    for (int k = 0; k < 3; k++) {
      uint8_t e[64], s[64]; memcpy(e, ecdsa[i], 64); memcpy(s, schnorr[i], 64);
      if (k) { int at = k == 1 ? next_byte() % 32 : 32 + next_byte() % 32; e[at] ^= 1 << (next_byte() % 8); s[at] ^= 1 << (next_byte() % 8); }
      int want = k == 0;
      if (ref_ecdsa(i, e) != want && k == 0) return 3;
      if (coax_ecdsa(i, e) != ref_ecdsa(i, e) || coax_schnorr(i, s) != ref_schnorr(i, s)) { printf("FAIL: disagreement at %d/%d\n", i, k); return 1; }
      if (ref_schnorr(i, s) != want) { printf("FAIL: libsecp256k1 decided %d/%d unexpectedly\n", i, k); return 1; }
      checked += 2;
    }
  }
  printf("agreement: %d decisions equal libsecp256k1's\n", checked);
  // `secp_bench N ecdsa|schnorr SECONDS` only loops one Coax verifier, for a profiler.
  if (argc > 3) {
    double until = now() + atof(argv[3]) * 1e6; long done = 0;
    while (now() < until) for (int i = 0; i < N; i++) done += argv[2][0] == 'e' ? coax_ecdsa(i, ecdsa[i]) : coax_schnorr(i, schnorr[i]);
    printf("%ld verifications\n", done); return 0;
  }
  printf("%-16s %10s %10s %8s\n", "microseconds", "Coax", "libsecp", "ratio");
  row("ECDSA verify", rate(coax_ecdsa, ecdsa, N), rate(ref_ecdsa, ecdsa, N));
  row("BIP-340 verify", rate(coax_schnorr, schnorr, N), rate(ref_schnorr, schnorr, N));
  row("ECDSA sign", rate(coax_ecdsa_sign, NULL, sign_count), rate(ref_ecdsa_sign, NULL, sign_count));
  row("BIP-340 sign", rate(coax_schnorr_sign, NULL, sign_count), rate(ref_schnorr_sign, NULL, sign_count));
  return 0;
}
