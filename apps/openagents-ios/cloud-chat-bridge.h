// Firm reason: 6 (opaque C bridge for #authority Cloud Chat v2, durable turns, Tier 1 Cloud Tools, Auto placement, Full Auto, and Voice Duplex policies on iOS).
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

int64_t oa_cc_tool_event_write(
    const char *conversation, const char *turn, int64_t sequence,
    int64_t kind, int64_t state,
    const char *tool_id, const char *tool_name, const char *tool_args,
    const char *text, bool failed,
    const char *compute, const char *model, int64_t cost_msat,
    const char *source_device, const char *workspace,
    int64_t created_at_ms,
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

int64_t oa_cc_submit_write(
    const char *operation,
    const char *conversation,
    int64_t revision,
    const char *project,
    int64_t project_revision,
    const char *prompt,
    uint8_t *out, int64_t out_cap);

int64_t oa_cc_stop_write(
    const char *operation,
    const char *turn,
    uint8_t *out, int64_t out_cap);

bool oa_cc_tool_kind_valid(const char *tool);
int64_t oa_cc_web_tools_json(uint8_t *out, int64_t out_cap);
int64_t oa_cc_classify_prompt(const char *prompt);

bool oa_cc_fa_should_gate(bool full_auto, bool approval_mode);
int64_t oa_cc_fa_decide_prompt(bool full_auto, bool auto_answer_questions, int64_t kind);

int64_t oa_cc_vd_vad_step(int64_t state_io[7], int64_t rms_bp, int64_t dt_ms);
int64_t oa_cc_vd_sanitize_speech(const char *src, uint8_t *out, int64_t out_cap);
int64_t oa_cc_vd_sentence_end(const char *src);

#ifdef __cplusplus
}
#endif

#endif
