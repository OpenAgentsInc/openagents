// The runner's NIP-98 signing, as the runner does it, with a TEST key: runner.c
// itself (its main renamed), its load_key and nip98 (the Coax signer, its only
// one). Prints the public key, then for each URL argument
// one line "METHOD URL<TAB>Authorization header". The API side checks those
// lines with its own verifier (coax/web/api/nip98_stdin.c), a separate
// program, as oa-api does (#12630). The key is SHA-256 of a fixed public
// label: never a real key. Build (deploy.sh's runner-sign.c, runner-wire.c and headers in WORK):
//   cc -std=c11 -D_DEFAULT_SOURCE -IWORK runner_sign_check.c WORK/runner-sign.c WORK/runner-wire.c WORK/seat-runner-wire.c -lsecp256k1 -lcrypto -lcurl -lm
//   ./runner_sign_check https://example.invalid/api/private/x/state
#define main runner_main
#include "runner.c"
#undef main

int main(int argc, char **argv) {
  unsigned char k[32]; char khex[65];
  const char *label = "openagents runner-sign test key";
  SHA256((const unsigned char *)label, strlen(label), k);
  hex(k, 32, khex); memset(k, 0, sizeof k);
  setenv("OA_RUNNER_KEY", khex, 1); memset(khex, 0, sizeof khex);
  if (!load_key()) { fprintf(stderr, "runner_sign_check: load_key refused\n"); return 2; }
  printf("signer coax pubkey %s\n", pubhex);
  for (int i = 1; i < argc; i++) {
    static char auth[8192];
    nip98("GET", argv[i], NULL, auth, sizeof auth);
    printf("GET %s\t%s\n", argv[i], auth + strlen("Authorization: "));
  }
  return 0;
}
