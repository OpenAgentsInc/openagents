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
#ifdef __cplusplus
}
#endif
#endif
