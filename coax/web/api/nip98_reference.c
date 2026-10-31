// The API's previous C NIP-98 check, kept only as the reference that
// nip98_diff.c compares nip98.coax against; the service no longer links it.
// NIP-98 HTTP auth for the API: "Authorization: Nostr <base64 of a kind 27235
// event>". The event's id is checked against its NIP-01 serialization
// (SHA-256, OpenSSL) and its signature with BIP-340 Schnorr from
// libsecp256k1 (Bitcoin Core's library, vetted; the server's for now until
// Coax's own signing lands, the Coax milestone "Schnorr signatures"). Then
// the NIP-98 rules: kind 27235, created_at within 60 s of now, the "u" tag
// the exact URL, the "method" tag the request's method, and a "payload" tag,
// when there is one, the body's SHA-256.
#include "nostr.h"

#include <openssl/sha.h>
#include <secp256k1.h>
#include <secp256k1_extrakeys.h>
#include <secp256k1_schnorrsig.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

// ---- base64 (standard and URL alphabets, padding optional)
static int b64v(unsigned char c) {
  if (c >= 'A' && c <= 'Z') return c - 'A';
  if (c >= 'a' && c <= 'z') return c - 'a' + 26;
  if (c >= '0' && c <= '9') return c - '0' + 52;
  if (c == '+' || c == '-') return 62;
  if (c == '/' || c == '_') return 63;
  return -1;
}
static size_t b64_decode(const char *in, unsigned char *out, size_t cap) {
  size_t o = 0; unsigned acc = 0; int bits = 0;
  for (; *in && *in != '='; in++) {
    int v = b64v((unsigned char)*in);
    if (v < 0) return 0;
    acc = (acc << 6) | (unsigned)v; bits += 6;
    if (bits >= 8) { bits -= 8; if (o >= cap) return 0; out[o++] = (unsigned char)(acc >> bits); }
  }
  return o;
}

// ---- a strict reader for one Nostr event's JSON
typedef struct { const char *p, *end; } cursor;
static void ws(cursor *c) { while (c->p < c->end && (*c->p == ' ' || *c->p == '\n' || *c->p == '\r' || *c->p == '\t')) c->p++; }
static bool lit(cursor *c, char ch) { ws(c); if (c->p < c->end && *c->p == ch) { c->p++; return true; } return false; }
static int hexv(char ch) { return ch >= '0' && ch <= '9' ? ch - '0' : ch >= 'a' && ch <= 'f' ? ch - 'a' + 10 : ch >= 'A' && ch <= 'F' ? ch - 'A' + 10 : -1; }
static bool utf8_put(unsigned cp, char *out, size_t *o, size_t cap) {
  char b[4]; int n;
  if (cp < 0x80) { b[0] = (char)cp; n = 1; }
  else if (cp < 0x800) { b[0] = (char)(0xc0 | cp >> 6); b[1] = (char)(0x80 | (cp & 63)); n = 2; }
  else if (cp < 0x10000) { b[0] = (char)(0xe0 | cp >> 12); b[1] = (char)(0x80 | ((cp >> 6) & 63)); b[2] = (char)(0x80 | (cp & 63)); n = 3; }
  else { b[0] = (char)(0xf0 | cp >> 18); b[1] = (char)(0x80 | ((cp >> 12) & 63)); b[2] = (char)(0x80 | ((cp >> 6) & 63)); b[3] = (char)(0x80 | (cp & 63)); n = 4; }
  if (*o + (size_t)n >= cap) return false;
  memcpy(out + *o, b, (size_t)n); *o += (size_t)n;
  return true;
}
static unsigned hex4(const char *p) { unsigned v = 0; for (int i = 0; i < 4; i++) { int h = hexv(p[i]); if (h < 0) return 0x110000; v = v * 16 + (unsigned)h; } return v; }
// A JSON string, decoded into out (UTF-8, NUL-terminated).
static bool str(cursor *c, char *out, size_t cap) {
  if (!lit(c, '"')) return false;
  size_t o = 0;
  while (c->p < c->end && *c->p != '"') {
    unsigned char ch = (unsigned char)*c->p++;
    if (ch < 0x20) return false;
    if (ch != '\\') { if (o + 1 >= cap) return false; out[o++] = (char)ch; continue; }
    if (c->p >= c->end) return false;
    char e = *c->p++;
    const char *simple = strchr("\"\\/bfnrt", e);
    if (simple && e) { if (o + 1 >= cap) return false; out[o++] = "\"\\/\b\f\n\r\t"[simple - "\"\\/bfnrt"]; continue; }
    if (e != 'u' || c->end - c->p < 4) return false;
    unsigned cp = hex4(c->p); c->p += 4;
    if (cp > 0xffff) return false;
    if (cp >= 0xd800 && cp < 0xdc00) {
      if (c->end - c->p < 6 || c->p[0] != '\\' || c->p[1] != 'u') return false;
      unsigned lo = hex4(c->p + 2); c->p += 6;
      if (lo < 0xdc00 || lo > 0xdfff) return false;
      cp = 0x10000 + ((cp - 0xd800) << 10) + (lo - 0xdc00);
    } else if (cp >= 0xdc00 && cp < 0xe000) return false;
    if (!utf8_put(cp, out, &o, cap)) return false;
  }
  if (!lit(c, '"')) return false;
  out[o] = 0;
  return true;
}
static bool integer(cursor *c, long long *v) {
  ws(c);
  char *e; long long x = strtoll(c->p, &e, 10);
  if (e == c->p || e > c->end || (*e == '.' || *e == 'e' || *e == 'E')) return false;
  c->p = e; *v = x;
  return true;
}

// NIP-01's escaping for the id's serialization.
static bool esc(const char *s, char *out, size_t *o, size_t cap) {
  if (*o + 1 >= cap) return false;
  out[(*o)++] = '"';
  for (; *s; s++) {
    const char *rep = NULL;
    switch (*s) { case '"': rep = "\\\""; break; case '\\': rep = "\\\\"; break; case '\n': rep = "\\n"; break;
                  case '\r': rep = "\\r"; break; case '\t': rep = "\\t"; break; case '\b': rep = "\\b"; break; case '\f': rep = "\\f"; break; }
    size_t n = rep ? 2 : 1;
    if (*o + n + 1 >= cap) return false;
    if (rep) { memcpy(out + *o, rep, 2); } else out[*o] = *s;
    *o += n;
  }
  out[(*o)++] = '"';
  return true;
}

static bool is_hex(const char *s, size_t n) { if (strlen(s) != n) return false; for (size_t i = 0; i < n; i++) if (hexv(s[i]) < 0 || (s[i] >= 'A' && s[i] <= 'F')) return false; return true; }
static void from_hex(const char *s, unsigned char *out, size_t n) { for (size_t i = 0; i < n; i++) out[i] = (unsigned char)(hexv(s[2 * i]) * 16 + hexv(s[2 * i + 1])); }
static void to_hex(const unsigned char *in, size_t n, char *out) { for (size_t i = 0; i < n; i++) snprintf(out + 2 * i, 3, "%02x", in[i]); }

#define TAGS_MAX 16
#define TAG_FIELDS 4
#define FIELD_CAP 1024

static bool fail(char *err, size_t cap, const char *why) { snprintf(err, cap, "%s", why); return false; }

static bool nostr_verify_event(const char *json, size_t len, nostr_event *ev, char *err, size_t errcap) {
  static char tags[TAGS_MAX][TAG_FIELDS][FIELD_CAP];
  int ntags = 0, nfields[TAGS_MAX] = {0};
  static char content[8192], key[32], tmp[FIELD_CAP];
  bool have_id = false, have_pub = false, have_sig = false, have_at = false, have_kind = false, have_tags = false, have_content = false;
  char sig[129] = {0};
  memset(ev, 0, sizeof *ev);
  cursor c = {json, json + len};
  if (!lit(&c, '{')) return fail(err, errcap, "not an object");
  if (!lit(&c, '}')) for (;;) {
    if (!str(&c, key, sizeof key) || !lit(&c, ':')) return fail(err, errcap, "bad key");
    if (!strcmp(key, "id")) { if (!str(&c, tmp, sizeof tmp) || !is_hex(tmp, 64)) return fail(err, errcap, "bad id"); memcpy(ev->id, tmp, 65); have_id = true; }
    else if (!strcmp(key, "pubkey")) { if (!str(&c, tmp, sizeof tmp) || !is_hex(tmp, 64)) return fail(err, errcap, "bad pubkey"); memcpy(ev->pubkey, tmp, 65); have_pub = true; }
    else if (!strcmp(key, "sig")) { if (!str(&c, tmp, sizeof tmp) || !is_hex(tmp, 128)) return fail(err, errcap, "bad sig"); memcpy(sig, tmp, 129); have_sig = true; }
    else if (!strcmp(key, "created_at")) { if (!integer(&c, &ev->created_at)) return fail(err, errcap, "bad created_at"); have_at = true; }
    else if (!strcmp(key, "kind")) { long long k; if (!integer(&c, &k) || k < 0 || k > 65535) return fail(err, errcap, "bad kind"); ev->kind = (int)k; have_kind = true; }
    else if (!strcmp(key, "content")) { if (!str(&c, content, sizeof content)) return fail(err, errcap, "bad content"); have_content = true; }
    else if (!strcmp(key, "tags")) {
      if (!lit(&c, '[')) return fail(err, errcap, "bad tags");
      if (!lit(&c, ']')) for (;;) {
        if (ntags >= TAGS_MAX || !lit(&c, '[')) return fail(err, errcap, "bad tag");
        if (!lit(&c, ']')) for (;;) {
          if (nfields[ntags] >= TAG_FIELDS || !str(&c, tags[ntags][nfields[ntags]], FIELD_CAP)) return fail(err, errcap, "bad tag field");
          nfields[ntags]++;
          if (lit(&c, ']')) break;
          if (!lit(&c, ',')) return fail(err, errcap, "bad tag");
        }
        ntags++;
        if (lit(&c, ']')) break;
        if (!lit(&c, ',')) return fail(err, errcap, "bad tags");
      }
      have_tags = true;
    } else return fail(err, errcap, "unknown field");
    if (lit(&c, '}')) break;
    if (!lit(&c, ',')) return fail(err, errcap, "bad object");
  }
  ws(&c);
  if (c.p != c.end) return fail(err, errcap, "trailing bytes");
  if (!(have_id && have_pub && have_sig && have_at && have_kind && have_tags && have_content)) return fail(err, errcap, "missing a field");

  // The id: SHA-256 of [0,pubkey,created_at,kind,tags,content].
  static char ser[16384]; size_t o = 0;
  o += (size_t)snprintf(ser, sizeof ser, "[0,\"%s\",%lld,%d,[", ev->pubkey, ev->created_at, ev->kind);
  for (int t = 0; t < ntags; t++) {
    if (t) ser[o++] = ',';
    ser[o++] = '[';
    for (int f = 0; f < nfields[t]; f++) { if (f) ser[o++] = ','; if (!esc(tags[t][f], ser, &o, sizeof ser - 8)) return fail(err, errcap, "too long"); }
    ser[o++] = ']';
  }
  ser[o++] = ']'; ser[o++] = ',';
  if (!esc(content, ser, &o, sizeof ser - 4)) return fail(err, errcap, "too long");
  ser[o++] = ']';
  unsigned char hash[32]; SHA256((const unsigned char *)ser, o, hash);
  char hex[65]; to_hex(hash, 32, hex);
  if (strcmp(hex, ev->id)) return fail(err, errcap, "id does not match the event");

  static secp256k1_context *ctx;
  if (!ctx) ctx = secp256k1_context_create(SECP256K1_CONTEXT_NONE);
  unsigned char pub[32], sg[64];
  from_hex(ev->pubkey, pub, 32); from_hex(sig, sg, 64);
  secp256k1_xonly_pubkey xonly;
  if (!secp256k1_xonly_pubkey_parse(ctx, &xonly, pub)) return fail(err, errcap, "pubkey is not a point");
  if (!secp256k1_schnorrsig_verify(ctx, sg, hash, 32, &xonly)) return fail(err, errcap, "signature does not verify");

  for (int t = 0; t < ntags; t++) {
    if (nfields[t] < 2) continue;
    if (!strcmp(tags[t][0], "u")) snprintf(ev->u, sizeof ev->u, "%s", tags[t][1]);
    else if (!strcmp(tags[t][0], "method")) snprintf(ev->method, sizeof ev->method, "%s", tags[t][1]);
    else if (!strcmp(tags[t][0], "payload")) snprintf(ev->payload, sizeof ev->payload, "%s", tags[t][1]);
  }
  return true;
}

bool nip98_reference_check(const char *authorization, const char *method, const char *url, const void *body, size_t body_len,
                 long long now, nostr_event *ev, char *err, size_t errcap) {
  if (!authorization || strncmp(authorization, "Nostr ", 6)) return fail(err, errcap, "no Nostr authorization");
  static unsigned char json[12288];
  size_t n = b64_decode(authorization + 6, json, sizeof json - 1);
  if (!n) return fail(err, errcap, "authorization is not base64");
  if (!nostr_verify_event((const char *)json, n, ev, err, errcap)) return false;
  if (ev->kind != 27235) return fail(err, errcap, "not a kind 27235 event");
  if (ev->created_at < now - 60 || ev->created_at > now + 60) return fail(err, errcap, "created_at is not within 60 s");
  if (strcmp(ev->u, url)) return fail(err, errcap, "the u tag is not this URL");
  if (strcasecmp(ev->method, method)) return fail(err, errcap, "the method tag is not this method");
  if (ev->payload[0]) {
    unsigned char h[32]; char hex[65];
    SHA256(body, body_len, h); to_hex(h, 32, hex);
    if (strcmp(hex, ev->payload)) return fail(err, errcap, "the payload tag is not this body");
  }
  return true;
}
