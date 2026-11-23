// Firm reason: 6 (opaque C ABI boundary for coax/lib/nostr/host/tailnet.coax; all wire rules are Coax).
#ifndef OA_TAILNET_CODEC_H
#define OA_TAILNET_CODEC_H
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
int64_t oa_tailnet_request_write(int chats, uint8_t *out, int64_t cap);
int64_t oa_tailnet_request_read(const uint8_t *raw, int64_t n);
int64_t oa_tailnet_answer_write(const char *host, const char *label, const char *invitation, const char *refused, uint8_t *out, int64_t cap);
int64_t oa_tailnet_answer_read(const uint8_t *raw, int64_t n, const char *expected_host, uint8_t *out, int64_t cap, uint8_t *label_out, int64_t label_cap, int64_t *label_n);
int64_t oa_tailnet_direct_hello_write(uint8_t *out, int64_t cap);
int oa_tailnet_direct_hello_read(const uint8_t *raw, int64_t n);
int64_t oa_tailnet_direct_ack_write(const char *refused, uint8_t *out, int64_t cap);
int64_t oa_tailnet_direct_ack_read(const uint8_t *raw, int64_t n, uint8_t *out, int64_t cap);
int64_t oa_tailnet_grant_request_write(const char *device, const char *label, uint8_t *out, int64_t cap);
int64_t oa_tailnet_grant_request_read(const uint8_t *raw, int64_t n, uint8_t *device_out, int64_t device_cap, uint8_t *label_out, int64_t label_cap, int64_t *label_n);
int64_t oa_tailnet_grant_answer_write(const char *host, const char *label, const char *grant, int64_t epoch, int64_t expires, const char *refused, uint8_t *out, int64_t cap);
int64_t oa_tailnet_grant_answer_read(const uint8_t *raw, int64_t n, uint8_t *host_out, int64_t host_cap, uint8_t *label_out, int64_t label_cap, uint8_t *grant_out, int64_t grant_cap, int64_t meta[3]);
int64_t oa_tailnet_subscribe_write(const char *session, uint8_t *out, int64_t cap);
int64_t oa_tailnet_subscribe_read(const uint8_t *raw, int64_t n, uint8_t *session_out, int64_t session_cap);
int64_t oa_tailnet_push_write(int64_t seq, const char *kind, const char *session, int64_t revision, int busy, int has_interaction, uint8_t *out, int64_t cap);
int64_t oa_tailnet_push_read(const uint8_t *raw, int64_t n, uint8_t *kind_out, int64_t kind_cap, uint8_t *session_out, int64_t session_cap, int64_t meta[5]);
#ifdef __cplusplus
}
#endif
#endif
