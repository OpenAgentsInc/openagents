// /og's C face for the browser: scalar Wasm exports over og.coax, its one
// request (og/pages.txt) handed to the host's HTTP import (coax/web/host.js)
// with its answer wanted, and the pictures it names (coaxial_media). The host
// keeps one OgHost: the OgState and the buffers it sees, bound before every call.
// This page records no analytics: it is a tool for us.
#include "og.h"
#include <stddef.h>

#define EXPORT(name) __attribute__((export_name(name)))
#define IMPORT(name) __attribute__((import_module("env"), import_name(name)))
IMPORT("coaxial_http") int32_t coaxial_http(uint32_t url, uint32_t url_len, uint32_t body, uint32_t body_len, int32_t flags);

static coax_v_t_OgState at(uint32_t state) {
  coax_t_OgHost *h = (coax_t_OgHost *)(uintptr_t)state;
  coaxial_og_bind((coax_v_t_OgHost){h, 1});
  return (coax_v_t_OgState){h->s.e, 1};
}
static void send_ask(uint32_t state) {
  coax_t_OgState *s = at(state).data;
  if (s->ask_n <= 0) return;
  if (coaxial_http((uint32_t)(uintptr_t)s->ask.e, (uint32_t)s->ask_n, 0, 0, 4) >= 0) coaxial_og_asked(at(state));
}

EXPORT("coaxial_layout") uint32_t coaxial_layout(void) {
  return (uint32_t)(sizeof(coax_t_OgHost) * 7u + offsetof(coax_t_OgHost, inbox) * 3u + sizeof(coax_t_OgState) * 5u + 0x4f47u);
}
EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_OgHost); }
EXPORT("coaxial_max_vertices") uint32_t coaxial_max_vertices(void) { return (uint32_t)coaxial_og_max_vertices(); }
EXPORT("coaxial_vertex_floats") uint32_t coaxial_vertex_floats(void) { return (uint32_t)coaxial_projects_vertex_floats(); }
EXPORT("coaxial_uniform_floats") uint32_t coaxial_uniform_floats(void) { return (uint32_t)coaxial_projects_uniform_floats(); }

EXPORT("coaxial_frame")
uint32_t coaxial_frame(uint32_t state, uint32_t vertices, uint32_t uniforms, float width, float height, float dpr, float dt, uint32_t still) {
  coax_v_f32 out = {(float *)(uintptr_t)vertices, coaxial_og_max_vertices() * coaxial_projects_vertex_floats()};
  coax_v_f32 u = {(float *)(uintptr_t)uniforms, coaxial_projects_uniform_floats()};
  int64_t n = coaxial_og_frame(at(state), out, u, width, height, dpr, dt, still != 0);
  send_ask(state);
  return n < 0 ? 0u : (uint32_t)n;
}
EXPORT("coaxial_key") uint32_t coaxial_key(uint32_t state, int32_t key, int32_t mods) {
  (void)mods;
  return coaxial_og_key(at(state), key) ? 1u : 0u;
}
EXPORT("coaxial_pointer") uint32_t coaxial_pointer(uint32_t state, float x, float y, int32_t phase, int32_t kind, int32_t id) {
  (void)id;
  return coaxial_og_pointer(at(state), x, y, phase, kind) ? 1u : 0u;
}
EXPORT("coaxial_wheel") void coaxial_wheel(uint32_t state, float dy) { coaxial_og_wheel(at(state), dy); }
EXPORT("coaxial_inbox") uint32_t coaxial_inbox(uint32_t state) { return (uint32_t)(uintptr_t)at(state).data->inbox.data; }
EXPORT("coaxial_inbox_cap") uint32_t coaxial_inbox_cap(void) { return (uint32_t)coaxial_og_inbox_cap(); }
EXPORT("coaxial_message") void coaxial_message(uint32_t state, int32_t kind, int32_t length) { coaxial_og_message(at(state), kind, length); }
EXPORT("coaxial_hash") void coaxial_hash(uint32_t state, uint32_t length) { coaxial_og_hash(at(state), (int64_t)length); }
EXPORT("coaxial_cursor") int32_t coaxial_cursor(uint32_t state) { return (int32_t)coaxial_og_cursor(at(state)); }
EXPORT("coaxial_action") int32_t coaxial_action(uint32_t state) { return (int32_t)coaxial_og_action(at(state)); }
EXPORT("coaxial_outbox") uint32_t coaxial_outbox(uint32_t state) { return (uint32_t)(uintptr_t)at(state).data->outbox.e; }
EXPORT("coaxial_outbox_len") uint32_t coaxial_outbox_len(uint32_t state) { return (uint32_t)at(state).data->out_n; }
EXPORT("coaxial_action_done") void coaxial_action_done(uint32_t state) { coaxial_og_action_done(at(state)); }
EXPORT("coaxial_media") uint32_t coaxial_media(uint32_t state) { return (uint32_t)(uintptr_t)at(state).data->media.e; }
EXPORT("coaxial_media_len") uint32_t coaxial_media_len(uint32_t state) { return (uint32_t)at(state).data->media_n; }
