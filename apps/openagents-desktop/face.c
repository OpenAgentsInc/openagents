// The Mac app's C face over chat.coax: what chat-face.c does for the
// browser, with the HTTP asks handed to the Swift host's local backend.
#include "chat.h"
#include "face.h"
#include <stdlib.h>
#include <string.h>

static dk_http_fn http_fn;

static coax_v_t_ChatState at(void *state) { return (coax_v_t_ChatState){(coax_t_ChatState *)state, 1}; }

static void send_asks(void *state) {
  coax_t_ChatState *s = state;
  if (s->ask_n > 0) {
    int32_t id = http_fn(s->ask.e, (int32_t)s->ask_n, 0, 0, 0);
    if (id >= 0) coaxial_chat_asked(at(state), id);
  }
  if (s->post_n > 0) {
    int32_t id = http_fn(s->post.e, (int32_t)s->post_n, s->body.e, (int32_t)s->body_n, 1);
    if (id >= 0) coaxial_chat_posted(at(state), id);
  }
}

void *dk_new(dk_http_fn http) { http_fn = http; return calloc(1, sizeof(coax_t_ChatState)); }
int32_t dk_vertex_floats(void) { return (int32_t)coaxial_projects_vertex_floats(); }
int32_t dk_uniform_floats(void) { return (int32_t)coaxial_projects_uniform_floats(); }
int32_t dk_max_vertices(void) { return (int32_t)coaxial_chat_max_vertices(); }

int32_t dk_frame(void *state, float *vertices, float *uniforms, float width, float height, float dpr, float dt) {
  coax_v_f32 out = {vertices, coaxial_chat_max_vertices() * coaxial_projects_vertex_floats()};
  coax_v_f32 u = {uniforms, coaxial_projects_uniform_floats()};
  int64_t n = coaxial_chat_frame(at(state), out, u, width, height, dpr, dt, 0);
  send_asks(state);
  return n < 0 ? 0 : (int32_t)n;
}
int32_t dk_pointer(void *state, float x, float y, int32_t phase) {
  int32_t used = coaxial_chat_pointer(at(state), x, y, phase, 0) ? 1 : 0;
  send_asks(state);
  return used;
}
int32_t dk_key(void *state, int32_t key) {
  int32_t used = coaxial_chat_key(at(state), key) ? 1 : 0;
  send_asks(state);
  return used;
}
void dk_wheel(void *state, float dy) { coaxial_chat_wheel(at(state), dy); }
void dk_message(void *state, int32_t kind, const uint8_t *bytes, int32_t length) {
  coax_t_ChatState *s = state;
  int64_t cap = coaxial_chat_inbox_cap();
  if (length > cap) length = (int32_t)cap;
  memcpy(s->inbox.e, bytes, (size_t)length);
  coaxial_chat_message(at(state), kind, length);
  send_asks(state);
}
int32_t dk_wants_text(void *state) { return coaxial_chat_wants_text(at(state)) ? 1 : 0; }
int32_t dk_cursor(void *state) { return (int32_t)coaxial_chat_cursor(at(state)); }
int32_t dk_take_action(void *state, uint8_t *url, int32_t cap) {
  coax_t_ChatState *s = state;
  if (coaxial_chat_action(at(state)) == 0) return 0;
  int32_t n = (int32_t)s->out_n < cap ? (int32_t)s->out_n : cap;
  memcpy(url, s->outbox.e, (size_t)n);
  coaxial_chat_action_done(at(state));
  return n;
}
