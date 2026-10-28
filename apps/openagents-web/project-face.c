// A project page's C face for the browser (as face.c is the splash's):
// scalar Wasm exports over the Coax procedures in project.coax. The state
// the host keeps is the page's clock, its tree, its work and its line to the
// relay (project-live.coax). The face carries the line's commands out to the
// host's socket imports (coax/web/host.js, "sockets") after every call, and
// its events come back through the inbox.
#include "project.h"
#include <stddef.h>
#include "analytics-face.h"

#define EXPORT(name) __attribute__((export_name(name)))
#define IMPORT(name) __attribute__((import_module("env"), import_name(name)))

IMPORT("coaxial_socket_open") int32_t coaxial_socket_open(uint32_t url, uint32_t len);
IMPORT("coaxial_socket_send") void coaxial_socket_send(int32_t id, uint32_t ptr, uint32_t len);
IMPORT("coaxial_socket_close") void coaxial_socket_close(int32_t id);

typedef struct { coax_t_ProjectState page; coax_t_Tree tree; coax_t_Work work; coax_t_Live live; } State;
static State *state_at(uint32_t state) { return (State *)(uintptr_t)state; }
static coax_v_t_Live live_of(State *st) { coax_v_t_Live l = {&st->live, 1}; return l; }
// What people do here (analytics.coax), as page "project".
static coax_t_Track track_state;
static coax_v_t_Track tr(void) { return (coax_v_t_Track){&track_state, 1}; }

// The pipe's commands, in order, into the host; then the pipe is empty.
static void drain(State *st) {
  coax_t_Pipe *p = &st->live.pipe.e[0];
  int64_t n = p->n;
  for (int64_t k = 0; k < n; k++) {
    uint32_t at = (uint32_t)(uintptr_t)&p->buf.e[p->from.e[k]], len = (uint32_t)p->len.e[k];
    int32_t id = (int32_t)p->id.e[k];
    if (p->op.e[k] == 1) coaxial_project_opened(live_of(st), coaxial_socket_open(at, len));
    else if (p->op.e[k] == 2) coaxial_socket_send(id, at, len);
    else if (p->op.e[k] == 3) coaxial_socket_close(id);
  }
  p->n = 0; p->used = 0;
}

EXPORT("coaxial_inbox") uint32_t coaxial_inbox(uint32_t state) { return (uint32_t)(uintptr_t)state_at(state)->live.inbox.e; }
EXPORT("coaxial_inbox_cap") uint32_t coaxial_inbox_cap(void) { return (uint32_t)coaxial_project_inbox_cap(); }
EXPORT("coaxial_message") void coaxial_message(uint32_t state, int32_t kind, int32_t length) {
  State *st = state_at(state);
  int64_t cap = coaxial_project_inbox_cap();
  _Bool mine = track_message(tr(), kind, (coax_string){(const char *)st->live.inbox.e, length < cap ? length : cap});
  track_drain(&track_state);
  if (mine) return;
  coaxial_project_message(live_of(st), kind, length);
  drain(st);
}

EXPORT("coaxial_layout") uint32_t coaxial_layout(void) {
  return (uint32_t)(sizeof(State) * 1000003u + _Alignof(State) * 31u + offsetof(State, tree) * 7u + offsetof(State, work) * 13u + offsetof(State, live) * 17u + 0x50524fu);
}
EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(State); }
EXPORT("coaxial_max_vertices") uint32_t coaxial_max_vertices(void) { return (uint32_t)coaxial_project_max_vertices(); }
EXPORT("coaxial_vertex_floats") uint32_t coaxial_vertex_floats(void) { return (uint32_t)coaxial_project_vertex_floats(); }
EXPORT("coaxial_uniform_floats") uint32_t coaxial_uniform_floats(void) { return (uint32_t)coaxial_project_uniform_floats(); }
EXPORT("coaxial_page") void coaxial_page(uint32_t state, int32_t page) { state_at(state)->page.page = page; }

EXPORT("coaxial_frame")
uint32_t coaxial_frame(uint32_t state, uint32_t vertices, uint32_t uniforms,
                       float width, float height, float dpr, float dt, uint32_t still) {
  State *st = state_at(state);
  coax_v_t_ProjectState s = {&st->page, 1};
  coax_v_t_Tree t = {&st->tree, 1};
  coax_v_t_Work w = {&st->work, 1};
  coax_v_t_Live l = live_of(st);
  coax_v_f32 out = {(float *)(uintptr_t)vertices, coaxial_project_max_vertices() * coaxial_project_vertex_floats()};
  coax_v_f32 u = {(float *)(uintptr_t)uniforms, coaxial_project_uniform_floats()};
  int64_t n = coaxial_project_frame(s, t, w, l, out, u, width, height, dpr, dt, still != 0);
  drain(st);
  if (!track_state.page_n) track_init(tr(), track_cstr("project"));
  track_frame(tr(), dt, width, height, dpr);
  if (n > 0) track_loaded(tr(), track_cstr("load-landing"));
  track_drain(&track_state);
  return n < 0 ? 0u : (uint32_t)n;
}

// Input (host.js): the pointer, keys and the wheel open, scroll and close a
// task's detail; the location's hash comes in through the inbox (shared with
// the relay's frames: the host fills it for one call at a time) and goes out
// as action 3 with the open task's alias in the outbox.
static coax_v_t_ProjectState page_of(uint32_t state) { coax_v_t_ProjectState s = {&state_at(state)->page, 1}; return s; }
static coax_v_t_Work work_of(uint32_t state) { coax_v_t_Work w = {&state_at(state)->work, 1}; return w; }
EXPORT("coaxial_pointer") uint32_t coaxial_pointer(uint32_t state, float x, float y, int32_t phase, int32_t kind, int32_t id) {
  (void)id;
  track_pointer(tr(), x, y, phase, kind, track_cstr(coaxial_project_cursor(page_of(state), work_of(state)) == 1 ? "link" : ""));
  track_drain(&track_state);
  return coaxial_project_pointer(page_of(state), work_of(state), x, y, phase, kind) ? 1u : 0u;
}
EXPORT("coaxial_key") uint32_t coaxial_key(uint32_t state, int32_t key, int32_t mods) {
  track_key(tr(), key, 0);
  track_drain(&track_state);
  return coaxial_project_key(page_of(state), work_of(state), key, mods) ? 1u : 0u;
}
EXPORT("coaxial_key_up") uint32_t coaxial_key_up(uint32_t state, int32_t key, int32_t mods) { (void)state; (void)mods; track_key_up(tr(), key); return 0u; }
EXPORT("coaxial_blur") void coaxial_blur(uint32_t state) { (void)state; track_blur(tr()); }
EXPORT("coaxial_wheel") void coaxial_wheel(uint32_t state, float dy) { coaxial_project_wheel(page_of(state), dy); }
EXPORT("coaxial_cursor") int32_t coaxial_cursor(uint32_t state) { return (int32_t)coaxial_project_cursor(page_of(state), work_of(state)); }
EXPORT("coaxial_hash") void coaxial_hash(uint32_t state, uint32_t length) { coaxial_project_hash(page_of(state), live_of(state_at(state)), (int64_t)length); }
EXPORT("coaxial_action") int32_t coaxial_action(uint32_t state) { return (int32_t)coaxial_project_action(page_of(state)); }
EXPORT("coaxial_outbox") uint32_t coaxial_outbox(uint32_t state) { return (uint32_t)(uintptr_t)&state_at(state)->page.outbox; }
EXPORT("coaxial_outbox_len") uint32_t coaxial_outbox_len(uint32_t state) { return (uint32_t)state_at(state)->page.out_n; }
EXPORT("coaxial_action_done") void coaxial_action_done(uint32_t state) { coaxial_project_action_done(page_of(state)); }
