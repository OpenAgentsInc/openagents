// Differential check: the API's NIP-98 check in Coax (nostr.c over
// nip98.coax) against the former C check (nip98_reference.c), on the signed
// corpus (nip98_corpus.tsv, from nip98_fixtures.ts) and on seeded random
// mutations of every case's event, method, URL, time and body. Every
// decision, refusal word and returned field must agree.
//   nip98_diff CORPUS [MUTATIONS_PER_CASE]   (test.sh builds and runs it)
#include "nostr.h"

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

bool nip98_reference_check(const char *authorization, const char *method, const char *url, const void *body, size_t body_len,
                           long long now, nostr_event *ev, char *err, size_t errcap);

static uint64_t seed = 0x9e3779b97f4a7c15ULL;
static uint64_t rnd(void) { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; return seed; }

static const char B64[] = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
static size_t b64_encode(const unsigned char *in, size_t n, char *out) {
  size_t o = 0;
  for (size_t i = 0; i < n; i += 3) {
    unsigned v = (unsigned)in[i] << 16 | (i + 1 < n ? (unsigned)in[i + 1] << 8 : 0) | (i + 2 < n ? in[i + 2] : 0);
    out[o++] = B64[v >> 18 & 63]; out[o++] = B64[v >> 12 & 63];
    out[o++] = i + 1 < n ? B64[v >> 6 & 63] : '='; out[o++] = i + 2 < n ? B64[v & 63] : '=';
  }
  out[o] = 0; return o;
}
static size_t b64_decode(const char *in, unsigned char *out) {
  size_t o = 0; unsigned acc = 0; int bits = 0;
  for (; *in && *in != '='; in++) {
    const char *p = strchr(B64, *in); int v = p && *in ? (int)(p - B64) : *in == '-' ? 62 : *in == '_' ? 63 : -1;
    if (v < 0) return 0;
    acc = (acc << 6) | (unsigned)v; bits += 6;
    if (bits >= 8) { bits -= 8; out[o++] = (unsigned char)(acc >> bits); }
  }
  return o;
}
static int unhex(char c) { return c >= '0' && c <= '9' ? c - '0' : c - 'a' + 10; }

static long cases, accepted, disagreements, past_end;
static void compare(const char *name, const char *auth, const char *method, const char *url, const unsigned char *body, size_t body_len, long long now) {
  nostr_event a, b; char ea[256] = "", eb[256] = "";
  bool ra = nip98_reference_check(auth, method, url, body, body_len, now, &a, ea, sizeof ea);
  bool rb = nip98_check(auth, method, url, body, body_len, now, &b, eb, sizeof eb);
  cases++;
  bool same = ra == rb && (ra ? !strcmp(a.id, b.id) && !strcmp(a.pubkey, b.pubkey) && a.created_at == b.created_at && a.kind == b.kind &&
                                    !strcmp(a.u, b.u) && !strcmp(a.method, b.method) && !strcmp(a.payload, b.payload)
                                  : !strcmp(ea, eb));
  if (ra && rb) accepted++;
  if (!same) {
    // The one documented difference: a number running to the very end of the event (C read past it).
    if (!ra && !rb && !strcmp(eb, "bad object") && (!strcmp(ea, "bad created_at") || !strcmp(ea, "bad kind"))) { past_end++; return; }
    if (disagreements++ < 20) fprintf(stderr, "nip98 diff %s: C %s '%s', Coax %s '%s' (%s)\n", name, ra ? "accepts" : "refuses", ea, rb ? "accepts" : "refuses", eb, auth);
  }
}

static const char ALPHABET[] = "{}[]\",:\\ 0123456789abcdefu+-.eE\n\t";
int main(int argc, char **argv) {
  if (argc < 2) { fprintf(stderr, "usage: nip98_diff CORPUS [MUTATIONS]\n"); return 2; }
  long mutations = argc > 2 ? atol(argv[2]) : 400;
  FILE *f = fopen(argv[1], "r"); if (!f) { perror(argv[1]); return 2; }
  static char line[40000], json[16384], mutated[16384], auth[24000];
  static unsigned char decoded[16384], body[256];
  while (fgets(line, sizeof line, f)) {
    line[strcspn(line, "\n")] = 0;
    char *field[6]; int n = 0;
    for (char *s = line; n < 6; n++) { field[n] = s; char *t = strchr(s, '\t'); if (!t) { n++; break; } *t = 0; s = t + 1; }
    if (n != 6) { fprintf(stderr, "nip98 diff: bad corpus line\n"); return 2; }
    const char *name = field[0], *method = field[1], *url = field[2], *bodyhex = field[4], *a = field[5];
    long long now = atoll(field[3]);
    size_t body_len = strlen(bodyhex) / 2;
    for (size_t i = 0; i < body_len; i++) body[i] = (unsigned char)(unhex(bodyhex[2 * i]) * 16 + unhex(bodyhex[2 * i + 1]));
    compare(name, a, method, url, body, body_len, now);
    // The event under random edits, and the request around it.
    size_t jn = strncmp(a, "Nostr ", 6) ? 0 : b64_decode(a + 6, decoded);
    if (!jn || jn >= sizeof json) continue;
    memcpy(json, decoded, jn);
    for (long m = 0; m < mutations; m++) {
      size_t k = jn; memcpy(mutated, json, jn);
      int edits = 1 + (int)(rnd() % 3);
      for (int e = 0; e < edits && k > 0 && k < sizeof mutated - 2; e++) {
        size_t at = rnd() % k; char ch = ALPHABET[rnd() % (sizeof ALPHABET - 1)];
        switch (rnd() % 4) {
          case 0: mutated[at] = ch; break;
          case 1: memmove(mutated + at + 1, mutated + at, k - at); mutated[at] = ch; k++; break;
          case 2: memmove(mutated + at, mutated + at + 1, k - at - 1); k--; break;
          default: if (rnd() % 8 == 0) k = at; break; // truncation
        }
      }
      strcpy(auth, "Nostr "); b64_encode((unsigned char *)mutated, k, auth + 6);
      if (rnd() % 16 == 0) { size_t l = strlen(auth); if (l > 7) auth[l - 1 - rnd() % 2] = (char)("*=-_A"[rnd() % 5]); }
      const char *mth = rnd() % 8 == 0 ? (rnd() % 2 ? "post" : "GET") : method;
      const char *u = rnd() % 16 == 0 ? "https://openagents.com/api/other" : url;
      long long t = now + (rnd() % 8 == 0 ? (long long)(rnd() % 200) - 100 : 0);
      compare(name, auth, mth, u, body, rnd() % 16 == 0 && body_len ? body_len - 1 : body_len, t);
    }
  }
  fclose(f);
  printf("nip98 diff: %ld cases (%ld accepted by both, %ld numbers past the end), %ld disagreements\n", cases, accepted, past_end, disagreements);
  return disagreements ? 1 : 0;
}
