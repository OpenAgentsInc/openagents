// The Mac app's C face over chat.coax (apps/openagents-web/chat.coax): the
// same calls host.js makes of chat.wasm, as plain C for Swift.
#pragma once
#include <stdint.h>

typedef int32_t (*dk_http_fn)(const uint8_t *url, int32_t url_len, const uint8_t *body, int32_t body_len, int32_t post);

void *dk_new(dk_http_fn http);
int32_t dk_vertex_floats(void);
int32_t dk_uniform_floats(void);
int32_t dk_max_vertices(void);
int32_t dk_frame(void *state, float *vertices, float *uniforms, float width, float height, float dpr, float dt);
int32_t dk_pointer(void *state, float x, float y, int32_t phase);
int32_t dk_key(void *state, int32_t key);
void dk_wheel(void *state, float dy);
void dk_message(void *state, int32_t kind, const uint8_t *bytes, int32_t length);
int32_t dk_wants_text(void *state);
int32_t dk_cursor(void *state);
// A navigation chat.coax asked for (its outbox), copied into url; 0 when none.
int32_t dk_take_action(void *state, uint8_t *url, int32_t cap);
