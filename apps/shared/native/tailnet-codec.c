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
int64_t oa_tailnet_grant_request_write(const char *device, const char *label, uint8_t *out, int64_t cap) {
  return oa_ht_grant_request_write(oa_ht_cstr(device), oa_ht_cstr(label), oa_ht_buf(out, cap));
}
int64_t oa_tailnet_grant_request_read(const uint8_t *raw, int64_t n, uint8_t *device_out, int64_t device_cap, uint8_t *label_out, int64_t label_cap, int64_t *label_n) {
  int64_t ln = 0;
  int64_t r = oa_ht_grant_request_read(oa_ht_bytes(raw, n), oa_ht_buf(device_out, device_cap), oa_ht_buf(label_out, label_cap), (coax_v_s64){ &ln, 1 });
  if (label_n) *label_n = ln;
  return r;
}
int64_t oa_tailnet_grant_answer_write(const char *host, const char *label, const char *grant, int64_t epoch, int64_t expires, const char *refused, uint8_t *out, int64_t cap) {
  return oa_ht_grant_answer_write(oa_ht_cstr(host), oa_ht_cstr(label), oa_ht_cstr(grant), epoch, expires, oa_ht_cstr(refused), oa_ht_buf(out, cap));
}
int64_t oa_tailnet_grant_answer_read(const uint8_t *raw, int64_t n, uint8_t *host_out, int64_t host_cap, uint8_t *label_out, int64_t label_cap, uint8_t *grant_out, int64_t grant_cap, int64_t meta[3]) {
  return oa_ht_grant_answer_read(oa_ht_bytes(raw, n), oa_ht_buf(host_out, host_cap), oa_ht_buf(label_out, label_cap), oa_ht_buf(grant_out, grant_cap), (coax_v_s64){ meta, meta ? 3 : 0 });
}
int64_t oa_tailnet_subscribe_write(const char *session, uint8_t *out, int64_t cap) {
  return oa_ht_subscribe_write(oa_ht_cstr(session), oa_ht_buf(out, cap));
}
int64_t oa_tailnet_subscribe_read(const uint8_t *raw, int64_t n, uint8_t *session_out, int64_t session_cap) {
  return oa_ht_subscribe_read(oa_ht_bytes(raw, n), oa_ht_buf(session_out, session_cap));
}
int64_t oa_tailnet_push_write(int64_t seq, const char *kind, const char *session, int64_t revision, int busy, int has_interaction, uint8_t *out, int64_t cap) {
  return oa_ht_push_write(seq, oa_ht_cstr(kind), oa_ht_cstr(session), revision, busy != 0, has_interaction != 0, oa_ht_buf(out, cap));
}
int64_t oa_tailnet_push_read(const uint8_t *raw, int64_t n, uint8_t *kind_out, int64_t kind_cap, uint8_t *session_out, int64_t session_cap, int64_t meta[5]) {
  return oa_ht_push_read(oa_ht_bytes(raw, n), oa_ht_buf(kind_out, kind_cap), oa_ht_buf(session_out, session_cap), (coax_v_s64){ meta, meta ? 5 : 0 });
}
