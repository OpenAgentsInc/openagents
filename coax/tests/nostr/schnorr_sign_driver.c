// Native driver for schnorr_sign.py: one request per stdin line, hex fields.
//   S key aux message          -> signature hex, or REFUSED
//   E key aux created kind tags content -> event JSON hex, or REFUSED
// It only loops the Coax step entries; every decision is theirs.
#include "es.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static size_t unhex(const char *s, uint8_t *out, size_t cap) {
  size_t n = strlen(s) / 2;
  if (n > cap) exit(2);
  for (size_t i = 0; i < n; i++) { unsigned v; sscanf(s + 2 * i, "%2x", &v); out[i] = (uint8_t)v; }
  return n;
}
static void hex(const uint8_t *b, size_t n) { for (size_t i = 0; i < n; i++) printf("%02x", b[i]); printf("\n"); }
static char line[4 << 20], f[6][2 << 20];
static uint8_t key[32], aux[32], message[1 << 20], tags[1 << 19], content[1 << 19], scratch[262144], out[1 << 20];
static coax_t_SignWork sw; static coax_t_NostrSignWork ew;
int main(void) {
  while (fgets(line, sizeof line, stdin)) {
    char kind = line[0];
    int count = sscanf(line + 1, "%s %s %s %s %s %s", f[0], f[1], f[2], f[3], f[4], f[5]);
    if (kind == 'S' && count >= 2) {
      if (count == 2) f[2][0] = 0;
      size_t kn = unhex(f[0], key, 32), an = unhex(f[1], aux, 32), mn = unhex(strcmp(f[2], "-") ? f[2] : "", message, sizeof message);
      uint8_t sig[64] = {0}; int64_t status, calls = 0;
      memset(&sw, 0, sizeof sw);
      do { status = schnorr_sign_step((coax_v_u8){key, (int64_t)kn}, (coax_v_u8){aux, (int64_t)an}, (coax_v_u8){message, (int64_t)mn}, (coax_v_t_SignWork){&sw, 1}, (coax_v_u8){sig, 64}, 0); calls++; } while (status == 1 && calls < 16);
      if (status == 2 && sw.stage == 0) hex(sig, 64); else printf("REFUSED\n");
    } else if (kind == 'E' && count == 6) {
      size_t kn = unhex(f[0], key, 32), an = unhex(f[1], aux, 32);
      int64_t created = strtoll(f[2], 0, 10), k = strtoll(f[3], 0, 10);
      size_t tn = unhex(strcmp(f[4], "-") ? f[4] : "", tags, sizeof tags), cn = unhex(strcmp(f[5], "-") ? f[5] : "", content, sizeof content);
      int64_t status, calls = 0;
      memset(&ew, 0, sizeof ew);
      do { status = nostr_event_sign_step((coax_v_u8){key, (int64_t)kn}, (coax_v_u8){aux, (int64_t)an}, created, k, (coax_string){(const char *)tags, (int64_t)tn}, (coax_string){(const char *)content, (int64_t)cn}, (coax_v_t_NostrSignWork){&ew, 1}, (coax_v_u8){scratch, sizeof scratch}, (coax_v_u8){out, sizeof out}, 0); calls++; } while (status == 0 && calls < 16);
      if (status > 0 && ew.stage == 0) hex(out, (size_t)status); else printf("REFUSED\n");
    } else return 3;
    fflush(stdout);
  }
  return 0;
}
