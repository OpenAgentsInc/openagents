// The runner's credential sealing, as the runner does it (chat-runner.c seal_with, OpenSSL's
// AES-256-GCM, its associated data from runner-wire.coax), with a fixed TEST key and nonce:
// runner.c itself, its main renamed. Prints the sealed base64, then whether it opens for the same
// owner and kind (1) and for another kind (0). coax/web/api/test_zchat.py compares the first line
// with the goldens' sealed credential (proto/coder/agent-managed-v1alpha1.json), which
// runner-wire-goldens.coax also seals with coax/lib/crypto/aes_gcm.coax. Build (coax/web/api/test.sh;
// deploy.sh's runner-sign.c, runner-wire.c and their headers in WORK):
//   cc -std=c11 -D_DEFAULT_SOURCE -IWORK runner_seal_check.c WORK/runner-sign.c WORK/runner-wire.c WORK/seat-runner-wire.c -lsecp256k1 -lcrypto -lcurl -lm
//   ./runner_seal_check KEY_HEX NONCE_HEX OWNER KIND PLAIN
#define main runner_main
#include "runner.c"
#undef main

int main(int argc, char **argv) {
  if (argc != 6 || strlen(argv[1]) != 64 || strlen(argv[2]) != 24) { fprintf(stderr, "runner_seal_check KEY_HEX NONCE_HEX OWNER KIND PLAIN\n"); return 2; }
  unsigned char nonce[12];
  for (int i = 0; i < 32; i++) { unsigned v; if (sscanf(argv[1] + 2 * i, "%2x", &v) != 1) return 2; seal_key[i] = (unsigned char)v; }
  for (int i = 0; i < 12; i++) { unsigned v; if (sscanf(argv[2] + 2 * i, "%2x", &v) != 1) return 2; nonce[i] = (unsigned char)v; }
  seal_ready = true;
  char out[2000], back[600];
  if (!seal_with(argv[5], argv[3], argv[4], nonce, out, sizeof out)) { fprintf(stderr, "runner_seal_check: seal refused\n"); return 1; }
  printf("%s\n", out);
  printf("%d\n", unseal(out, argv[3], argv[4], back, sizeof back) && !strcmp(back, argv[5]));
  printf("%d\n", unseal(out, argv[3], strcmp(argv[4], "api") ? "api" : "oauth", back, sizeof back));
  return 0;
}
