#include "ws.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

static uint32_t rol(uint32_t x, int k) { return (x << k) | (x >> (32 - k)); }

void ws_sha1(const uint8_t *data, size_t n, uint8_t out[20]) {
  uint32_t h[5] = {0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0};
  size_t total = ((n + 9 + 63) / 64) * 64;
  uint8_t block[64];
  for (size_t off = 0; off < total; off += 64) {
    for (size_t i = 0; i < 64; i++) {
      size_t p = off + i;
      uint8_t b = 0;
      if (p < n) b = data[p];
      else if (p == n) b = 0x80;
      else if (p >= total - 8) b = (uint8_t)(((uint64_t)n * 8) >> (8 * (total - 1 - p)));
      block[i] = b;
    }
    uint32_t w[80];
    for (int i = 0; i < 16; i++)
      w[i] = (uint32_t)block[4 * i] << 24 | (uint32_t)block[4 * i + 1] << 16 | (uint32_t)block[4 * i + 2] << 8 | block[4 * i + 3];
    for (int i = 16; i < 80; i++) w[i] = rol(w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16], 1);
    uint32_t a = h[0], b = h[1], c = h[2], d = h[3], e = h[4];
    for (int i = 0; i < 80; i++) {
      uint32_t f, k;
      if (i < 20) { f = (b & c) | (~b & d); k = 0x5A827999; }
      else if (i < 40) { f = b ^ c ^ d; k = 0x6ED9EBA1; }
      else if (i < 60) { f = (b & c) | (b & d) | (c & d); k = 0x8F1BBCDC; }
      else { f = b ^ c ^ d; k = 0xCA62C1D6; }
      uint32_t t = rol(a, 5) + f + e + k + w[i];
      e = d; d = c; c = rol(b, 30); b = a; a = t;
    }
    h[0] += a; h[1] += b; h[2] += c; h[3] += d; h[4] += e;
  }
  for (int i = 0; i < 5; i++) {
    out[4 * i] = (uint8_t)(h[i] >> 24); out[4 * i + 1] = (uint8_t)(h[i] >> 16);
    out[4 * i + 2] = (uint8_t)(h[i] >> 8); out[4 * i + 3] = (uint8_t)h[i];
  }
}

static const char B64[] = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

size_t ws_base64(const uint8_t *in, size_t n, char *out) {
  size_t o = 0;
  for (size_t i = 0; i < n; i += 3) {
    uint32_t v = (uint32_t)in[i] << 16 | (i + 1 < n ? (uint32_t)in[i + 1] << 8 : 0) | (i + 2 < n ? in[i + 2] : 0);
    out[o++] = B64[(v >> 18) & 63];
    out[o++] = B64[(v >> 12) & 63];
    out[o++] = i + 1 < n ? B64[(v >> 6) & 63] : '=';
    out[o++] = i + 2 < n ? B64[v & 63] : '=';
  }
  out[o] = 0;
  return o;
}

size_t ws_unbase64(const char *in, size_t n, uint8_t *out, size_t cap) {
  uint32_t acc = 0;
  int bits = 0;
  size_t o = 0;
  for (size_t i = 0; i < n; i++) {
    char c = in[i];
    int v;
    if (c >= 'A' && c <= 'Z') v = c - 'A';
    else if (c >= 'a' && c <= 'z') v = c - 'a' + 26;
    else if (c >= '0' && c <= '9') v = c - '0' + 52;
    else if (c == '+' || c == '-') v = 62;
    else if (c == '/' || c == '_') v = 63;
    else if (c == '=' ) break;
    else if (c == ' ' || c == '\n' || c == '\r' || c == '\t' || c == '\\') continue;
    else return 0;
    acc = (acc << 6) | (uint32_t)v;
    bits += 6;
    if (bits >= 8) {
      bits -= 8;
      if (o >= cap) return 0;
      out[o++] = (uint8_t)(acc >> bits);
    }
  }
  return o;
}

// Finds a header value (case-insensitive name) in an HTTP request.
static bool header(const char *req, size_t n, const char *name, char *val, size_t cap) {
  size_t ln = strlen(name);
  const char *p = req, *end = req + n;
  while (p < end) {
    const char *eol = memchr(p, '\n', (size_t)(end - p));
    if (!eol) eol = end;
    if ((size_t)(eol - p) > ln + 1 && strncasecmp(p, name, ln) == 0 && p[ln] == ':') {
      const char *v = p + ln + 1;
      while (v < eol && (*v == ' ' || *v == '\t')) v++;
      const char *ve = eol;
      while (ve > v && (ve[-1] == '\r' || ve[-1] == ' ')) ve--;
      size_t vl = (size_t)(ve - v);
      if (vl >= cap) return false;
      memcpy(val, v, vl);
      val[vl] = 0;
      return true;
    }
    p = eol + 1;
  }
  return false;
}

static void accept_key(const char *key, char out[29]) {
  char buf[128];
  snprintf(buf, sizeof buf, "%s258EAFA5-E914-47DA-95CA-C5AB0DC85B11", key);
  uint8_t d[20];
  ws_sha1((const uint8_t *)buf, strlen(buf), d);
  ws_base64(d, 20, out);
}

size_t ws_server_handshake(const char *req, size_t n, char *out, size_t cap) {
  if (n < 4 || strncmp(req, "GET ", 4) != 0) return 0;
  char key[64], up[64], ver[16];
  if (!header(req, n, "Sec-WebSocket-Key", key, sizeof key)) return 0;
  if (!header(req, n, "Upgrade", up, sizeof up) || strcasecmp(up, "websocket") != 0) return 0;
  if (!header(req, n, "Sec-WebSocket-Version", ver, sizeof ver) || strcmp(ver, "13") != 0) return 0;
  if (strlen(key) != 24) return 0;
  char acc[29];
  accept_key(key, acc);
  int w = snprintf(out, cap,
                   "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n"
                   "Sec-WebSocket-Accept: %s\r\n\r\n", acc);
  return w > 0 && (size_t)w < cap ? (size_t)w : 0;
}

static const char CLIENT_KEY[] = "b2Etdm9pY2UtZGV2LWtleQ==";  // fixed 16-byte nonce; dev client only

size_t ws_client_request(const char *host, int port, const char *path, char *out, size_t cap) {
  int w = snprintf(out, cap,
                   "GET %s HTTP/1.1\r\nHost: %s:%d\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n"
                   "Sec-WebSocket-Key: %s\r\nSec-WebSocket-Version: 13\r\n\r\n", path, host, port, CLIENT_KEY);
  return w > 0 && (size_t)w < cap ? (size_t)w : 0;
}

bool ws_client_accepts(const char *resp, size_t n) {
  if (n < 12 || strncmp(resp, "HTTP/1.1 101", 12) != 0) return false;
  char acc[64], want[29];
  if (!header(resp, n, "Sec-WebSocket-Accept", acc, sizeof acc)) return false;
  accept_key(CLIENT_KEY, want);
  return strcmp(acc, want) == 0;
}

int ws_parse(uint8_t *buf, size_t n, size_t max_payload, ws_frame *f) {
  if (n < 2) return 0;
  bool fin = buf[0] & 0x80;
  if (buf[0] & 0x70) return -1;  // no extensions negotiated
  int op = buf[0] & 0x0f;
  bool masked = buf[1] & 0x80;
  uint64_t len = buf[1] & 0x7f;
  size_t h = 2;
  if (len == 126) {
    if (n < 4) return 0;
    len = (uint64_t)buf[2] << 8 | buf[3];
    h = 4;
  } else if (len == 127) {
    if (n < 10) return 0;
    len = 0;
    for (int i = 0; i < 8; i++) len = len << 8 | buf[2 + i];
    h = 10;
  }
  if (op >= 8 && (len > 125 || !fin)) return -1;
  if (len > max_payload) return -1;
  uint8_t mask[4] = {0};
  if (masked) {
    if (n < h + 4) return 0;
    memcpy(mask, buf + h, 4);
    h += 4;
  }
  if (n < h + len) return 0;
  uint8_t *p = buf + h;
  if (masked)
    for (size_t i = 0; i < len; i++) p[i] ^= mask[i & 3];
  f->fin = fin;
  f->opcode = op;
  f->payload = p;
  f->length = (size_t)len;
  f->consumed = h + (size_t)len;
  return 1;
}

size_t ws_build(uint8_t *out, size_t cap, int opcode, const uint8_t *payload, size_t n, bool mask) {
  size_t h = 2;
  if (n >= 126) h = n <= 0xffff ? 4 : 10;
  if (mask) h += 4;
  if (cap < h + n) return 0;
  out[0] = (uint8_t)(0x80 | (opcode & 0x0f));
  uint8_t mb = mask ? 0x80 : 0;
  size_t p = 2;
  if (n < 126) out[1] = (uint8_t)(mb | n);
  else if (n <= 0xffff) { out[1] = mb | 126; out[2] = (uint8_t)(n >> 8); out[3] = (uint8_t)n; p = 4; }
  else {
    out[1] = mb | 127;
    for (int i = 0; i < 8; i++) out[2 + i] = (uint8_t)((uint64_t)n >> (56 - 8 * i));
    p = 10;
  }
  uint8_t key[4] = {0};
  if (mask) {
    uint32_t r = arc4random();
    memcpy(key, &r, 4);
    memcpy(out + p, key, 4);
    p += 4;
  }
  for (size_t i = 0; i < n; i++) out[p + i] = mask ? payload[i] ^ key[i & 3] : payload[i];
  return p + n;
}
