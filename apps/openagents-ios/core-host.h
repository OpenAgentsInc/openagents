// Firm reason: 6 (opaque SDK lifetime boundary; no generated types cross Swift).
// The shared app core (apps/shared/app/core/host.coax) as plain C for Swift (AppCore.swift). Each
// call is the app_host_ procedure of the same name; texts are bytes and a count. A call answers
// APP_OK (1) or APP_GUEST (2, already routed here), or a negative refusal that changed nothing.
#ifndef OPENAGENTS_CORE_HOST_H
#define OPENAGENTS_CORE_HOST_H
#include <stdint.h>
// The core on the heap (about 19 MiB), initialised. NULL when there is no memory.
void *oa_core_open(void);
void oa_core_close(void *core);
int64_t oa_core_launch(void *core, int64_t mono_ms, int64_t wall_ms, const char *rejoin);
int64_t oa_core_clock(void *core, int64_t mono_ms, int64_t wall_ms);
int64_t oa_core_lifecycle(void *core, int64_t mono_ms, int64_t wall_ms, int64_t code);
int64_t oa_core_action(void *core, int64_t mono_ms, int64_t wall_ms, int64_t code, int flag, const char *text, const char *more);
int64_t oa_core_next(void *core);
// The request's six fields (kind, generation, method, limit, timeout, owner): 1, or 0 when not queued.
int oa_core_request(void *core, int64_t op, int64_t out[6]);
// A queued request's path, text or body: the byte count, or -1.
int64_t oa_core_path(void *core, int64_t op, uint8_t *out, int64_t cap);
int64_t oa_core_text(void *core, int64_t op, uint8_t *out, int64_t cap);
int64_t oa_core_body(void *core, int64_t op, uint8_t *out, int64_t cap);
int64_t oa_core_taken(void *core, int64_t mono_ms, int64_t wall_ms, int64_t op, int accepted);
int64_t oa_core_cancel_next(void *core);
int64_t oa_core_done(void *core, int64_t mono_ms, int64_t wall_ms, int64_t op, int64_t generation, int64_t outcome, int64_t status, int flag,
                     const uint8_t *text, int64_t text_n, const uint8_t *more, int64_t more_n, int64_t value);
int64_t oa_core_wake(void *core);
int64_t oa_core_busy(void *core);
int64_t oa_core_dirty_take(void *core, int64_t mask);
// The conversation to rejoin at the next launch: its byte count (0 for none).
int64_t oa_core_keep(void *core, uint8_t *out, int64_t cap);
// The installed HTTP policy (url-policy.coax up_http): method 1 GET, 2 POST.
// A dispatch that answers APP_GUEST hands the done event's text to this page hook (the environment
// picker's reads); with none set, the root lets the guest go.
void oa_core_on_guest(void (*hook)(void *context, const uint8_t *body, int64_t n), void *context);
int oa_policy_http(int64_t method, const char *path, int64_t body_bytes);
#endif
