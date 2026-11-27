// Firm reason: 6 (thin C wrapper forwarding Swift Cloud Chat v2, durable turn, Tier 1 tools, placement, Full Auto, and Voice Duplex calls into #authority Coax compiled in Generated/gallery.c).
#include "cloud-chat-bridge.h"
#include "gallery.h"
#include <string.h>

static coax_string cc_s(const char *s) {
  return (coax_string){s ? s : "", s ? (int64_t)strlen(s) : 0};
}

bool oa_cc_compute_valid(const char *compute, int64_t cost_msat) {
  return cc_compute_valid(cc_s(compute), cost_msat);
}

int64_t oa_cc_event_write(
    const char *conversation, const char *turn, int64_t sequence,
    int64_t kind, int64_t state, const char *text,
    const char *compute, const char *model, int64_t cost_msat,
    const char *source_device, const char *workspace,
    const char *stop_reason, int64_t created_at_ms,
    uint8_t *out, int64_t out_cap) {
  if (!out || out_cap < 1) return -1;
  coax_t_CtUsage u = {-1, -1, -1, -1, -1};
  coax_t_CcEvent ev = {0};
  ev.conversation = cc_s(conversation);
  ev.turn = cc_s(turn);
  ev.sequence = sequence;
  ev.kind = kind;
  ev.state = state;
  ev.text = cc_s(text);
  ev.tool_id = cc_s("");
  ev.tool_name = cc_s("");
  ev.tool_args = cc_s("");
  ev.diff = cc_s("");
  ev.failed = false;
  ev.approval_status = cc_s("");
  ev.attachments = cc_s("");
  ev.compute = cc_s(compute);
  ev.model = cc_s(model);
  ev.cost_msat = cost_msat;
  ev.source_device = cc_s(source_device);
  ev.workspace = cc_s(workspace);
  ev.has_usage = false;
  ev.usage = u;
  ev.stop_reason = cc_s(stop_reason);
  ev.created_at_ms = created_at_ms;
  return cc_event_write((coax_v_t_CcEvent){&ev, 1}, (coax_v_u8){out, out_cap});
}

int64_t oa_cc_tool_event_write(
    const char *conversation, const char *turn, int64_t sequence,
    int64_t kind, int64_t state,
    const char *tool_id, const char *tool_name, const char *tool_args,
    const char *text, bool failed,
    const char *compute, const char *model, int64_t cost_msat,
    const char *source_device, const char *workspace,
    int64_t created_at_ms,
    uint8_t *out, int64_t out_cap) {
  if (!out || out_cap < 1) return -1;
  coax_t_CtUsage u = {-1, -1, -1, -1, -1};
  coax_t_CcEvent ev = {0};
  ev.conversation = cc_s(conversation);
  ev.turn = cc_s(turn);
  ev.sequence = sequence;
  ev.kind = kind;
  ev.state = state;
  ev.text = cc_s(text);
  ev.tool_id = cc_s(tool_id);
  ev.tool_name = cc_s(tool_name);
  ev.tool_args = cc_s(tool_args);
  ev.diff = cc_s("");
  ev.failed = failed;
  ev.approval_status = cc_s("");
  ev.attachments = cc_s("");
  ev.compute = cc_s(compute);
  ev.model = cc_s(model);
  ev.cost_msat = cost_msat;
  ev.source_device = cc_s(source_device);
  ev.workspace = cc_s(workspace);
  ev.has_usage = false;
  ev.usage = u;
  ev.stop_reason = cc_s("");
  ev.created_at_ms = created_at_ms;
  return cc_event_write((coax_v_t_CcEvent){&ev, 1}, (coax_v_u8){out, out_cap});
}

int64_t oa_cc_sync_write(
    const char *operation, const char *conversation, const char *turn,
    const char *title, bool has_pinned, bool pinned, bool has_archived, bool archived,
    const char *compute, const char *model, int64_t cost_msat,
    const char *source_device, const char *workspace,
    const char *user_text, int64_t state, const char *output,
    const char *stop_reason, const char *error_text,
    const char *single_event_json,
    uint8_t *out, int64_t out_cap) {
  if (!out || out_cap < 1) return -1;
  coax_t_CtUsage u = {-1, -1, -1, -1, -1};
  coax_t_CcSyncReq req = {0};
  req.operation = cc_s(operation);
  req.conversation = cc_s(conversation);
  req.turn = cc_s(turn);
  req.title = cc_s(title);
  req.has_pinned = has_pinned;
  req.pinned = pinned;
  req.has_archived = has_archived;
  req.archived = archived;
  req.compute = cc_s(compute);
  req.model = cc_s(model);
  req.cost_msat = cost_msat;
  req.source_device = cc_s(source_device);
  req.workspace = cc_s(workspace);
  req.user_text = cc_s(user_text);
  req.attachments = cc_s("");
  req.state = state;
  req.output = cc_s(output);
  req.stop_reason = cc_s(stop_reason);
  req.error = cc_s(error_text);
  req.has_usage = false;
  req.usage = u;
  coax_string evs[1];
  int64_t ev_count = 0;
  if (single_event_json && single_event_json[0] != '\0') {
    evs[0] = cc_s(single_event_json);
    ev_count = 1;
  }
  return cc_sync_write((coax_v_t_CcSyncReq){&req, 1}, (coax_v_string){evs, ev_count}, (coax_v_u8){out, out_cap});
}

int64_t oa_cc_meta_write(
    const char *operation,
    bool has_title, const char *title,
    bool has_pinned, bool pinned,
    bool has_archived, bool archived,
    uint8_t *out, int64_t out_cap) {
  if (!out || out_cap < 1) return -1;
  coax_t_CcMetaReq req = {0};
  req.operation = cc_s(operation);
  req.has_title = has_title;
  req.title = cc_s(title);
  req.has_pinned = has_pinned;
  req.pinned = pinned;
  req.has_archived = has_archived;
  req.archived = archived;
  return cc_meta_write((coax_v_t_CcMetaReq){&req, 1}, (coax_v_u8){out, out_cap});
}

int64_t oa_cc_submit_write(
    const char *operation,
    const char *conversation,
    int64_t revision,
    const char *project,
    int64_t project_revision,
    const char *prompt,
    uint8_t *out, int64_t out_cap) {
  if (!out || out_cap < 1) return -1;
  return ios_cc_submit_write(
      cc_s(operation),
      cc_s(conversation),
      revision,
      cc_s(project),
      project_revision,
      cc_s(prompt),
      (coax_v_u8){out, out_cap});
}

int64_t oa_cc_stop_write(
    const char *operation,
    const char *turn,
    uint8_t *out, int64_t out_cap) {
  if (!out || out_cap < 1) return -1;
  return ios_cc_stop_write(cc_s(operation), cc_s(turn), (coax_v_u8){out, out_cap});
}

bool oa_cc_tool_kind_valid(const char *tool) {
  return ios_cc_tool_kind_valid(cc_s(tool));
}

int64_t oa_cc_web_tools_json(uint8_t *out, int64_t out_cap) {
  if (!out || out_cap < 1) return -1;
  return ios_cc_web_tools_json((coax_v_u8){out, out_cap});
}

int64_t oa_cc_classify_prompt(const char *prompt) {
  return ios_cc_classify_prompt(cc_s(prompt));
}

bool oa_cc_fa_should_gate(bool full_auto, bool approval_mode) {
  return ios_cc_fa_should_gate(full_auto, approval_mode);
}

int64_t oa_cc_fa_decide_prompt(bool full_auto, bool auto_answer_questions, int64_t kind) {
  return ios_cc_fa_decide_prompt(full_auto, auto_answer_questions, kind);
}

int64_t oa_cc_vd_vad_step(int64_t state_io[7], int64_t rms_bp, int64_t dt_ms) {
  if (!state_io) return 0;
  return ios_cc_vd_vad_step((coax_v_s64){state_io, 7}, rms_bp, dt_ms);
}

int64_t oa_cc_vd_sanitize_speech(const char *src, uint8_t *out, int64_t out_cap) {
  if (!out || out_cap < 1) return 0;
  return ios_cc_vd_sanitize_speech(cc_s(src), (coax_v_u8){out, out_cap});
}

int64_t oa_cc_vd_sentence_end(const char *src) {
  return ios_cc_vd_sentence_end(cc_s(src));
}
