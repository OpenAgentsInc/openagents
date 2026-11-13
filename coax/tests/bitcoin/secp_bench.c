// Times Coax's secp256k1 against libsecp256k1 in one process (issue #13450).
// secp_bench.sh builds and runs it. libsecp256k1 makes the keys and
// signatures; both libraries must then decide every signature the same way,
// valid and with one bit changed, before any time is reported. Test-only.
#define _POSIX_C_SOURCE 200809L
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
// Slot N is scratch for the `agree` mode.
static uint8_t secret[N + 1][32], hash[N + 1][32], pub33[N + 1][33], xonly[N + 1][32], ecdsa[N + 1][64], schnorr[N + 1][64], aux[N + 1][32];
static secp256k1_context *ctx;
static uint64_t seed = 0x13450;
static clockid_t bench_clock = CLOCK_MONOTONIC;
static uint8_t next_byte(void) { seed = seed * 6364136223846793005ULL + 1442695040888963407ULL; return (uint8_t)(seed >> 56); }
static double now(void) { struct timespec t; clock_gettime(bench_clock, &t); return t.tv_sec * 1e6 + t.tv_nsec / 1e3; }
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
static int phase_call(int i, int mode) { uint8_t out[32]; return bench_phase(v(hash[i],32),v(xonly[i],32),v(schnorr[i],64),v(out,32),mode); }
static int phase0(int i,const uint8_t *s) { (void)s; return phase_call(i,0); }
static int phase1(int i,const uint8_t *s) { (void)s; return phase_call(i,1); }
static int phase2(int i,const uint8_t *s) { (void)s; return phase_call(i,2); }
static int phase3(int i,const uint8_t *s) { (void)s; return phase_call(i,3); }
static int phase4(int i,const uint8_t *s) { (void)s; return phase_call(i,4); }
static int phase5(int i,const uint8_t *s) { (void)s; return phase_call(i,5); }
static int phase6(int i,const uint8_t *s) { (void)s; return phase_call(i,6); }
static int phase7(int i,const uint8_t *s) { (void)s; return phase_call(i,7); }
static int phase8(int i,const uint8_t *s) { (void)s; return phase_call(i,8); }
static int inverse0(int i, const uint8_t *unused) { uint8_t out[32]; (void)unused; return bench_inverse(v(hash[i],32),v(out,32),0); }
static int inverse1(int i, const uint8_t *unused) { uint8_t out[32]; (void)unused; return bench_inverse(v(hash[i],32),v(out,32),1); }
static int inverse2(int i, const uint8_t *unused) { uint8_t out[32]; (void)unused; return bench_inverse(v(hash[i],32),v(out,32),2); }
static int inverse3(int i, const uint8_t *unused) { uint8_t out[32]; (void)unused; return bench_inverse(v(hash[i],32),v(out,32),3); }
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
  if (getenv("SECP_BENCH_CPU_TIME")) bench_clock = CLOCK_PROCESS_CPUTIME_ID;
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
  if (argc > 2 && !strcmp(argv[2], "phases")) {
    printf("lift %.3f; challenge %.3f; mul %.3f; square %.3f; sqrt %.3f us\n",rate(phase0,NULL,N),rate(phase1,NULL,N),rate(phase2,NULL,N),rate(phase3,NULL,N),rate(phase4,NULL,N));
    printf("msm %.3f; affine %.3f; table %.3f; doubles %.3f us\n",rate(phase5,NULL,N),rate(phase6,NULL,N),rate(phase7,NULL,N),rate(phase8,NULL,N));
    return 0;
  }
  if (argc > 2 && !strcmp(argv[2], "inverses")) {
    printf("field inverse: divsteps %.2f us; chain %.2f us\n", rate(inverse0,NULL,N),rate(inverse1,NULL,N));
    printf("scalar inverse: divsteps %.2f us; binary %.2f us\n", rate(inverse2,NULL,N),rate(inverse3,NULL,N));
    return 0;
  }
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
  // `secp_bench N agree COUNT`: COUNT more seeded cases, each a signature with one of nine
  // changes (none; a bit of the hash, r, s or the key; the key's prefix; n - s; r or s set to
  // 0, n, n - 1, p or all ones). Both libraries must decide every case the same way.
  if (argc > 3 && !strcmp(argv[2], "agree")) {
    static const uint8_t order[32] = {255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,254,186,174,220,230,175,72,160,59,191,210,94,140,208,54,65,65};
    static const uint8_t prime[32] = {255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,254,255,255,252,47};
    long count = atol(argv[3]), accepted[2] = {0, 0};
    for (long c = 0; c < count; c++) {
      int i = next_byte(), kind = next_byte() % 9, at = next_byte() % 32, bit = 1 << (next_byte() % 8);
      memcpy(hash[N], hash[i], 32); memcpy(pub33[N], pub33[i], 33); memcpy(xonly[N], xonly[i], 32);
      memcpy(ecdsa[N], ecdsa[i], 64); memcpy(schnorr[N], schnorr[i], 64);
      if (kind == 1) hash[N][at] ^= bit;
      if (kind == 2) { ecdsa[N][at] ^= bit; schnorr[N][at] ^= bit; }
      if (kind == 3) { ecdsa[N][32 + at] ^= bit; schnorr[N][32 + at] ^= bit; }
      if (kind == 4) { pub33[N][1 + at] ^= bit; xonly[N][at] ^= bit; }
      if (kind == 5) pub33[N][0] = (uint8_t)(next_byte() % 8);
      if (kind == 6) { int borrow = 0; for (int j = 31; j >= 0; j--) { int d = order[j] - ecdsa[i][32 + j] - borrow; borrow = d < 0; ecdsa[N][32 + j] = (uint8_t)(d + 256 * borrow); } }
      if (kind == 7 || kind == 8) {
        uint8_t value[32] = {0}; int which = next_byte() % 5, half = kind == 7 ? 0 : 32;
        if (which == 1 || which == 2) memcpy(value, order, 32);
        if (which == 2) value[31] -= 1;
        if (which == 3) memcpy(value, prime, 32);
        if (which == 4) memset(value, 255, 32);
        memcpy(ecdsa[N] + half, value, 32); memcpy(schnorr[N] + half, value, 32);
      }
      int e = ref_ecdsa(N, ecdsa[N]), s = ref_schnorr(N, schnorr[N]);
      if (coax_ecdsa(N, ecdsa[N]) != e || coax_schnorr(N, schnorr[N]) != s) { printf("FAIL: disagreement at case %ld (key %d, change %d)\n", c, i, kind); return 1; }
      accepted[0] += e; accepted[1] += s;
    }
    printf("agreement: %ld more cases, ECDSA and BIP-340 each; libsecp256k1 accepts %ld and %ld of them\n", count, accepted[0], accepted[1]);
    return 0;
  }
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
