// Firm reason: 6 (opaque C bridge for #authority coax/web/api/account-chat-cloud-wire.coax on iOS).
#ifndef OPENAGENTS_IOS_CLOUD_CHAT_BRIDGE_H
#define OPENAGENTS_IOS_CLOUD_CHAT_BRIDGE_H

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

bool oa_cc_compute_valid(const char *compute, int64_t cost_msat);

int64_t oa_cc_event_write(
    const char *conversation, const char *turn, int64_t sequence,
    int64_t kind, int64_t state, const char *text,
    const char *compute, const char *model, int64_t cost_msat,
    const char *source_device, const char *workspace,
    const char *stop_reason, int64_t created_at_ms,
    uint8_t *out, int64_t out_cap);

int64_t oa_cc_sync_write(
    const char *operation, const char *conversation, const char *turn,
    const char *title, bool has_pinned, bool pinned, bool has_archived, bool archived,
    const char *compute, const char *model, int64_t cost_msat,
    const char *source_device, const char *workspace,
    const char *user_text, int64_t state, const char *output,
    const char *stop_reason, const char *error_text,
    const char *single_event_json,
    uint8_t *out, int64_t out_cap);

int64_t oa_cc_meta_write(
    const char *operation,
    bool has_title, const char *title,
    bool has_pinned, bool pinned,
    bool has_archived, bool archived,
    uint8_t *out, int64_t out_cap);

#ifdef __cplusplus
}
#endif

#endif
