// Firm reason: 6 (opaque C ABI boundary for coax/lib/nostr/host/tailnet.coax; all wire rules are Coax).
#include "tailnet-codec.h"
#include <string.h>

static coax_string oa_ht_cstr(const char *s) {
  return (coax_string){ s ? s : "", s ? (int64_t)strlen(s) : 0 };
}
static coax_string oa_ht_bytes(const uint8_t *p, int64_t n) {
  return (coax_string){ p ? (const char *)p : "", (p && n > 0) ? n : 0 };
}
static coax_v_u8 oa_ht_buf(uint8_t *p, int64_t n) {
  return (coax_v_u8){ p, (p && n > 0) ? n : 0 };
}

int64_t oa_tailnet_request_write(int chats, uint8_t *out, int64_t cap) {
  return oa_ht_request_write(chats != 0, oa_ht_buf(out, cap));
}
int64_t oa_tailnet_request_read(const uint8_t *raw, int64_t n) {
  return oa_ht_request_read(oa_ht_bytes(raw, n));
}
int64_t oa_tailnet_answer_write(const char *host, const char *label, const char *invitation, const char *refused, uint8_t *out, int64_t cap) {
  return oa_ht_answer_write(oa_ht_cstr(host), oa_ht_cstr(label), oa_ht_cstr(invitation), oa_ht_cstr(refused), oa_ht_buf(out, cap));
}
int64_t oa_tailnet_answer_read(const uint8_t *raw, int64_t n, const char *expected_host, uint8_t *out, int64_t cap, uint8_t *label_out, int64_t label_cap, int64_t *label_n) {
  int64_t ln = 0;
  int64_t r = oa_ht_answer_read(oa_ht_bytes(raw, n), oa_ht_cstr(expected_host), oa_ht_buf(out, cap), oa_ht_buf(label_out, label_cap), (coax_v_s64){ &ln, 1 });
  if (label_n) *label_n = ln;
  return r;
}
int64_t oa_tailnet_direct_hello_write(uint8_t *out, int64_t cap) {
  return oa_ht_direct_hello_write(oa_ht_buf(out, cap));
}
int oa_tailnet_direct_hello_read(const uint8_t *raw, int64_t n) {
  return oa_ht_direct_hello_read(oa_ht_bytes(raw, n)) ? 1 : 0;
}
int64_t oa_tailnet_direct_ack_write(const char *refused, uint8_t *out, int64_t cap) {
  return oa_ht_direct_ack_write(oa_ht_cstr(refused), oa_ht_buf(out, cap));
}
int64_t oa_tailnet_direct_ack_read(const uint8_t *raw, int64_t n, uint8_t *out, int64_t cap) {
  return oa_ht_direct_ack_read(oa_ht_bytes(raw, n), oa_ht_buf(out, cap));
}
