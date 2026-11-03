// One-Shot's C face for the browser (coax/web/host.js): scalar Wasm exports over oneshot.coax,
// which holds the Table (oneshot-table.coax, on the SRD engine in aethyr/rules/srd/) and its
// drawing. It decides nothing. It carries the page's socket commands (socket.coax's pipe) to the
// host after each call, as stair-face.c does. No scene: gpu/oneshot.coax reuses 5E Lab's bg_fs
// for the 3D glade and the strokes os_frame writes.
#include "oneshot.h"
#include <stdbool.h>
#include <stddef.h>

#define EXPORT(name) __attribute__((export_name(name)))
#define IMPORT(name) __attribute__((import_module("env"), import_name(name)))
IMPORT("coaxial_socket_open") int32_t coaxial_socket_open(uint32_t url, uint32_t len);
IMPORT("coaxial_socket_send") void coaxial_socket_send(int32_t id, uint32_t ptr, uint32_t len);
IMPORT("coaxial_socket_close") void coaxial_socket_close(int32_t id);

static uint8_t inbox[24576];   // a kept list of campaigns runs to about 17 KB
static bool golden;

static coax_t_Os *os_at(uint32_t state) { return (coax_t_Os *)(uintptr_t)state; }
static coax_v_t_Os at(uint32_t state) { return (coax_v_t_Os){os_at(state), 1}; }

// The pipe's socket commands in order; then the pipe is empty.
static void drain(uint32_t state) {
  coax_t_Pipe *p = &os_at(state)->pipe.e[0];
  int64_t n = p->n;
  for (int64_t k = 0; k < n; k++) {
    uint32_t from = (uint32_t)(uintptr_t)&p->buf.e[p->from.e[k]], len = (uint32_t)p->len.e[k];
    int32_t id = (int32_t)p->id.e[k];
    if (p->op.e[k] == 1) os_socket_opened(at(state), coaxial_socket_open(from, len));
    else if (p->op.e[k] == 2) coaxial_socket_send(id, from, len);
    else if (p->op.e[k] == 3) coaxial_socket_close(id);
  }
  p->n = 0; p->used = 0;
}

EXPORT("coaxial_layout") uint32_t coaxial_layout(void) {
  return (uint32_t)(sizeof(coax_t_Os) * 1000003u + offsetof(coax_t_Os, outbox) * 13u + 0x4f4e4553u);
}
EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_Os); }
EXPORT("coaxial_state_init") void coaxial_state_init(uint32_t state) { coax_default_t_Os((coax_t_Os *)(uintptr_t)state); }
EXPORT("coaxial_max_vertices") uint32_t coaxial_max_vertices(void) { return (uint32_t)coaxial_os_max_vertices(); }
EXPORT("coaxial_vertex_floats") uint32_t coaxial_vertex_floats(void) { return (uint32_t)coaxial_os_vertex_floats(); }
EXPORT("coaxial_uniform_floats") uint32_t coaxial_uniform_floats(void) { return (uint32_t)coaxial_os_uniform_floats(); }
// The 3D glade traces every figure per pixel, as 5E Lab's: at most 1.6 million pixels drawn.
EXPORT("coaxial_pixel_budget") uint32_t coaxial_pixel_budget(void) { return 1600000u; }
EXPORT("coaxial_golden") void coaxial_golden(uint32_t on) { golden = on != 0; }

EXPORT("coaxial_frame")
uint32_t coaxial_frame(uint32_t state, uint32_t vertices, uint32_t uniforms, float w, float h, float scale, float dt, uint32_t still) {
  os_golden(at(state), golden);
  coax_v_f32 out = {(float *)(uintptr_t)vertices, coaxial_os_max_vertices() * coaxial_os_vertex_floats()};
  coax_v_f32 u = {(float *)(uintptr_t)uniforms, coaxial_os_uniform_floats()};
  int64_t n = os_frame(at(state), out, u, w, h, scale, dt, still != 0);
  drain(state);
  return n < 0 ? 0u : (uint32_t)n;
}

EXPORT("coaxial_key") uint32_t coaxial_key(uint32_t state, int32_t key, int32_t mods) { uint32_t r = os_key(at(state), key, mods) ? 1u : 0u; drain(state); return r; }
EXPORT("coaxial_key_up") uint32_t coaxial_key_up(uint32_t state, int32_t key, int32_t mods) { return os_key_up(at(state), key, mods) ? 1u : 0u; }
EXPORT("coaxial_blur") void coaxial_blur(uint32_t state) { os_blur(at(state)); }
EXPORT("coaxial_pointer") uint32_t coaxial_pointer(uint32_t state, float x, float y, int32_t phase, int32_t kind, int32_t id) {
  uint32_t r = os_pointer(at(state), x, y, phase, kind, id) ? 1u : 0u;
  drain(state);
  return r;
}
EXPORT("coaxial_wheel") uint32_t coaxial_wheel(uint32_t state, float dy) { return os_wheel(at(state), dy) ? 1u : 0u; }
EXPORT("coaxial_cursor") int32_t coaxial_cursor(uint32_t state) { return (int32_t)os_cursor(at(state)); }
EXPORT("coaxial_wants_text") int32_t coaxial_wants_text(uint32_t state) { return os_wants_text(at(state)) ? 1 : 0; }
EXPORT("coaxial_field") uint32_t coaxial_field(uint32_t state) { return (uint32_t)(uintptr_t)os_at(state)->field_box.e; }

EXPORT("coaxial_inbox") uint32_t coaxial_inbox(uint32_t state) { (void)state; return (uint32_t)(uintptr_t)inbox; }
EXPORT("coaxial_inbox_cap") uint32_t coaxial_inbox_cap(void) { return sizeof inbox; }
EXPORT("coaxial_message") void coaxial_message(uint32_t state, int32_t kind, uint32_t length) {
  if (length > sizeof inbox) return;
  os_message(at(state), kind, (coax_string){(const char *)inbox, length});
  drain(state);
}

EXPORT("coaxial_action") int32_t coaxial_action(uint32_t state) { return (int32_t)os_action(at(state)); }
EXPORT("coaxial_outbox") uint32_t coaxial_outbox(uint32_t state) { return (uint32_t)(uintptr_t)os_at(state)->outbox.e; }
EXPORT("coaxial_outbox_len") uint32_t coaxial_outbox_len(uint32_t state) { return (uint32_t)os_at(state)->out_n; }
EXPORT("coaxial_action_done") void coaxial_action_done(uint32_t state) { os_action_done(at(state)); }
