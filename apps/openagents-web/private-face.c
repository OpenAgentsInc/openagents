// A private project page's C face for the browser: scalar Wasm exports over
// private.coax, and the requests it asks for (a read of the state, or one
// post) handed to the host's HTTP import (coax/web/host.js) with their
// answers wanted. The page records no analytics: what it shows is private.
#include "private.h"
#include <stddef.h>

#define EXPORT(name) __attribute__((export_name(name)))
#define IMPORT(name) __attribute__((import_module("env"), import_name(name)))
IMPORT("coaxial_http") int32_t coaxial_http(uint32_t url, uint32_t url_len, uint32_t body, uint32_t body_len, int32_t flags);

static coax_v_t_PrivateState at(uint32_t state) { return (coax_v_t_PrivateState){(coax_t_PrivateState *)(uintptr_t)state, 1}; }

// What private.coax asks for, once there's a free slot (flag 4: answer wanted; 1: POST).
static void send_asks(uint32_t state) {
  coax_t_PrivateState *s = at(state).data;
  if (s->ask_n > 0) {
    int32_t id = coaxial_http((uint32_t)(uintptr_t)s->ask.e, (uint32_t)s->ask_n, 0, 0, 4);
    if (id >= 0) coaxial_private_asked(at(state), id);
  }
  if (s->post_n > 0) {
    int32_t id = coaxial_http((uint32_t)(uintptr_t)s->post.e, (uint32_t)s->post_n, (uint32_t)(uintptr_t)s->body.e, (uint32_t)s->body_n, 5);
    if (id >= 0) coaxial_private_posted(at(state), id);
  }
}

EXPORT("coaxial_layout") uint32_t coaxial_layout(void) {
  return (uint32_t)(sizeof(coax_t_PrivateState) * 7u + offsetof(coax_t_PrivateState, inbox) * 3u + 0x505256u);
}
EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_PrivateState); }
EXPORT("coaxial_max_vertices") uint32_t coaxial_max_vertices(void) { return (uint32_t)coaxial_private_max_vertices(); }
EXPORT("coaxial_vertex_floats") uint32_t coaxial_vertex_floats(void) { return (uint32_t)coaxial_projects_vertex_floats(); }
EXPORT("coaxial_uniform_floats") uint32_t coaxial_uniform_floats(void) { return (uint32_t)coaxial_projects_uniform_floats(); }
EXPORT("coaxial_page") void coaxial_page(uint32_t state, int32_t page) { coaxial_private_page(at(state), page); }

EXPORT("coaxial_frame")
uint32_t coaxial_frame(uint32_t state, uint32_t vertices, uint32_t uniforms, float width, float height, float dpr, float dt, uint32_t still) {
  coax_v_f32 out = {(float *)(uintptr_t)vertices, coaxial_private_max_vertices() * coaxial_projects_vertex_floats()};
  coax_v_f32 u = {(float *)(uintptr_t)uniforms, coaxial_projects_uniform_floats()};
  int64_t n = coaxial_private_frame(at(state), out, u, width, height, dpr, dt, still != 0);
  send_asks(state);
  return n < 0 ? 0u : (uint32_t)n;
}
EXPORT("coaxial_key") uint32_t coaxial_key(uint32_t state, int32_t key, int32_t mods) {
  (void)mods;
  uint32_t used = coaxial_private_key(at(state), key) ? 1u : 0u;
  send_asks(state);
  return used;
}
EXPORT("coaxial_pointer") uint32_t coaxial_pointer(uint32_t state, float x, float y, int32_t phase, int32_t kind, int32_t id) {
  (void)id;
  uint32_t used = coaxial_private_pointer(at(state), x, y, phase, kind) ? 1u : 0u;
  send_asks(state);
  return used;
}
EXPORT("coaxial_wheel") void coaxial_wheel(uint32_t state, float dy) { coaxial_private_wheel(at(state), dy); }
EXPORT("coaxial_inbox") uint32_t coaxial_inbox(uint32_t state) { return (uint32_t)(uintptr_t)at(state).data->inbox.e; }
EXPORT("coaxial_inbox_cap") uint32_t coaxial_inbox_cap(void) { return (uint32_t)coaxial_private_inbox_cap(); }
EXPORT("coaxial_message") void coaxial_message(uint32_t state, int32_t kind, int32_t length) { coaxial_private_message(at(state), kind, length); send_asks(state); }
EXPORT("coaxial_hash") void coaxial_hash(uint32_t state, uint32_t length) { coaxial_private_hash(at(state), (int64_t)length); }
EXPORT("coaxial_wants_text") int32_t coaxial_wants_text(uint32_t state) { return coaxial_private_wants_text(at(state)) ? 1 : 0; }
EXPORT("coaxial_field") uint32_t coaxial_field(uint32_t state) { return (uint32_t)(uintptr_t)at(state).data->field.e; }
EXPORT("coaxial_cursor") int32_t coaxial_cursor(uint32_t state) { return (int32_t)coaxial_private_cursor(at(state)); }
EXPORT("coaxial_action") int32_t coaxial_action(uint32_t state) { return (int32_t)coaxial_private_action(at(state)); }
EXPORT("coaxial_outbox") uint32_t coaxial_outbox(uint32_t state) { return (uint32_t)(uintptr_t)at(state).data->outbox.e; }
EXPORT("coaxial_outbox_len") uint32_t coaxial_outbox_len(uint32_t state) { return (uint32_t)at(state).data->out_n; }
EXPORT("coaxial_action_done") void coaxial_action_done(uint32_t state) { coaxial_private_action_done(at(state)); }
