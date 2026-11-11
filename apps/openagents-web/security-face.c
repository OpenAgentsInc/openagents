/* /settings/security's scalar ABI. The screens, requests, typing and drawing are Coax
   (security.coax); this hands its one request to the host's HTTP import (coax/web/host.js) and
   records no analytics: the page shows a recovery code and takes one typed. */
#include "security.h"
#include <stdint.h>
#define EXPORT(name) __attribute__((export_name(name)))
#define IMPORT(name) __attribute__((import_module("env"), import_name(name)))
IMPORT("coaxial_http") int32_t coaxial_http(uint32_t url, uint32_t url_len, uint32_t body, uint32_t body_len, int32_t flags);
static unsigned char inbox[16384];
static uint32_t golden_mode;
static coax_t_Sc *at(uint32_t p) { return (coax_t_Sc *)(uintptr_t)p; }
static coax_v_t_Sc view(uint32_t p) { return (coax_v_t_Sc){at(p), 1}; }
/* The request security.coax asks for, once the host has a free slot (flag 4: answer wanted). */
static void send_ask(uint32_t p) {
  coax_t_Sc *s = at(p);
  if (!s->want) return;
  int32_t id = coaxial_http((uint32_t)(uintptr_t)s->url.e, (uint32_t)s->url_n, (uint32_t)(uintptr_t)s->body.e, (uint32_t)s->body_n, (int32_t)s->flags);
  if (id >= 0) sc_sent(view(p), id);
}
EXPORT("coaxial_golden") void golden(uint32_t on) { golden_mode = on != 0; }
EXPORT("coaxial_layout") uint32_t layout(void) { return sizeof(coax_t_Sc) * 1000003u + 11u; }
EXPORT("coaxial_state_size") uint32_t state_size(void) { return sizeof(coax_t_Sc); }
EXPORT("coaxial_state_init") void coaxial_state_init(uint32_t state) { coax_default_t_Sc((coax_t_Sc *)(uintptr_t)state); }
EXPORT("coaxial_page") void page(uint32_t p, int32_t n) { (void)n; sc_page(view(p)); send_ask(p); }
EXPORT("coaxial_max_vertices") uint32_t max_vertices(void) { return (uint32_t)sc_max_vertices(); }
EXPORT("coaxial_vertex_floats") uint32_t vertex_floats(void) { return (uint32_t)coaxial_land_vertex_floats(); }
EXPORT("coaxial_uniform_floats") uint32_t uniform_floats(void) { return (uint32_t)coaxial_land_uniform_floats(); }
EXPORT("coaxial_frame") uint32_t frame(uint32_t p, uint32_t vertices, uint32_t uniforms, float w, float h, float d, float dt, uint32_t still) {
  int64_t n = sc_frame(view(p), (coax_v_t_LandState){0, 0}, (coax_v_f32){(float *)(uintptr_t)vertices, sc_max_vertices() * coaxial_land_vertex_floats()},
                       (coax_v_f32){(float *)(uintptr_t)uniforms, coaxial_land_uniform_floats()}, w, h, d, dt, still != 0 || golden_mode != 0);
  send_ask(p);
  return n < 0 ? 0u : (uint32_t)n;
}
EXPORT("coaxial_pointer") uint32_t pointer(uint32_t p, float x, float y, int32_t phase, int32_t kind, int32_t id) {
  (void)kind; (void)id; uint32_t used = sc_pointer(view(p), x, y, phase); send_ask(p); return used;
}
EXPORT("coaxial_wheel") void wheel(uint32_t p, float dy) { sc_wheel(view(p), dy); }
EXPORT("coaxial_key") uint32_t key(uint32_t p, int32_t k, int32_t mods) { uint32_t used = sc_key_in(view(p), k, mods); send_ask(p); return used; }
EXPORT("coaxial_cursor") int32_t cursor(uint32_t p) { return (int32_t)sc_cursor(view(p)); }
EXPORT("coaxial_wants_text") int32_t wants_text(uint32_t p) { return sc_wants_text(view(p)) ? 1 : 0; }
EXPORT("coaxial_field") uint32_t field(uint32_t p) { return (uint32_t)(uintptr_t)at(p)->field.e; }
EXPORT("coaxial_action") int32_t action(uint32_t p) { return (int32_t)at(p)->action; }
EXPORT("coaxial_outbox") uint32_t outbox(uint32_t p) { return (uint32_t)(uintptr_t)at(p)->outbox.e; }
EXPORT("coaxial_outbox_len") uint32_t outbox_len(uint32_t p) { return (uint32_t)at(p)->out_n; }
EXPORT("coaxial_action_done") void action_done(uint32_t p) { sc_done(view(p)); }
EXPORT("coaxial_inbox") uint32_t incoming(uint32_t p) { (void)p; return (uint32_t)(uintptr_t)inbox; }
EXPORT("coaxial_inbox_cap") uint32_t incoming_cap(void) { return sizeof inbox; }
EXPORT("coaxial_message") void message(uint32_t p, int32_t kind, int32_t n) {
  if (n < 0 || n > (int32_t)sizeof inbox) return;
  sc_message(view(p), kind, (coax_string){(const char *)inbox, n});
  send_ask(p);
}
EXPORT("coaxial_hash") void hash(uint32_t p, uint32_t n) {
  if (n > sizeof inbox) return;
  sc_hash(view(p), (coax_string){(const char *)inbox, (int64_t)n});
  send_ask(p);
}
/* Read-only probes for browser checks: coaxial.probe() is screen + 10 * level + 100 * recovery + 1000 * keys
   (screen 0 loading, 1 main, 2 confirm, 3 code shown, 4 logged out). */
EXPORT("coaxial_probe") int32_t probe(uint32_t p) { coax_t_Sc *s = at(p); return (int32_t)(s->screen + 10 * s->level + 100 * s->rec + 1000 * s->nk); }
EXPORT("coaxial_security_screen") int32_t screen(uint32_t p) { return (int32_t)at(p)->screen; }
EXPORT("coaxial_security_level") int32_t level(uint32_t p) { return (int32_t)at(p)->level; }
EXPORT("coaxial_security_recovery") int32_t recovery(uint32_t p) { return (int32_t)at(p)->rec; }
