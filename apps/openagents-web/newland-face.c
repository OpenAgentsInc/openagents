// The live homepage C face: scalar Wasm exports over the Coax procedures.
// The state the host keeps is the page's clock, its tables and its input.
#include "newland.h"
#include <stddef.h>
#include "analytics-face.h"
#include "newland-reader-bridge.h"

#define EXPORT(name) __attribute__((export_name(name)))

static coax_t_LandState *state_at(uint32_t state) { return (coax_t_LandState *)(uintptr_t)state; }
static coax_v_t_LandState view(uint32_t state) { return (coax_v_t_LandState){state_at(state), 1}; }
static unsigned char inbox[1048576];
static uint32_t golden_mode;
static coax_t_Track track_state;
static coax_v_t_Track tr(void) {return (coax_v_t_Track){&track_state,1};}
#define IMPORT(name) __attribute__((import_module("env"),import_name(name)))
IMPORT("coaxial_socket_open") int32_t coaxial_socket_open(uint32_t,uint32_t);
IMPORT("coaxial_socket_send") void coaxial_socket_send(int32_t,uint32_t,uint32_t);
IMPORT("coaxial_socket_close") void coaxial_socket_close(int32_t);
static void drain(uint32_t state) {
  coax_t_Pipe *p=&state_at(state)->net.e[0].pipe.e[0];int64_t n=p->n;
  for(int64_t k=0;k<n;k++) {
    uint32_t bytes=(uint32_t)(uintptr_t)&p->buf.e[p->from.e[k]],len=(uint32_t)p->len.e[k];
    if(p->op.e[k]==1)coaxial_land_network_opened(view(state),p->id.e[k],coaxial_socket_open(bytes,len));
    else if(p->op.e[k]==2)coaxial_socket_send((int32_t)p->id.e[k],bytes,len);
    else if(p->op.e[k]==3)coaxial_socket_close((int32_t)p->id.e[k]);
  }p->n=0;p->used=0;
}
EXPORT("coaxial_golden") void golden(uint32_t on) {golden_mode=on!=0;}
EXPORT("coaxial_inbox") uint32_t coaxial_inbox(uint32_t state) {(void)state;return (uint32_t)(uintptr_t)inbox;}
EXPORT("coaxial_inbox_cap") uint32_t coaxial_inbox_cap(void) {return sizeof inbox;}
EXPORT("coaxial_message") void coaxial_message(uint32_t state,int32_t kind,int32_t n) {
  if(n<0 || n>(int32_t)sizeof inbox)return;
  coax_string text={(const char *)inbox,n};
  coaxial_land_message(view(state),kind,text);
  if(!track_state.page_n)track_init(tr(),track_cstr("home"));
  _Bool mine=track_message(tr(),kind,text);track_drain(&track_state);
  if(!mine && !golden_mode)coaxial_land_network_message(view(state),kind,text);
  drain(state);
}

EXPORT("coaxial_layout") uint32_t coaxial_layout(void) {
  return (uint32_t)(sizeof(coax_t_LandState) * 1000003u + _Alignof(coax_t_LandState) * 31u
                    + offsetof(coax_t_LandState, plot) * 7u + offsetof(coax_t_LandState, login) * 13u + 0x4c414eu);
}
EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_LandState); }
EXPORT("coaxial_land_captured") int64_t captured(void) {return coaxial_land_captured();}
EXPORT("coaxial_land_pylons") uint32_t pylons(uint32_t state) {return (uint32_t)state_at(state)->pylons;}
EXPORT("coaxial_land_pylon_status") int32_t pylon_status(uint32_t state,uint32_t i) {return i<(uint32_t)state_at(state)->pylons?(int32_t)state_at(state)->pyl.e[i].status:-1;}
EXPORT("coaxial_probe") uint32_t probe(uint32_t state) {return (uint32_t)coaxial_land_network_probe(view(state));}
EXPORT("coaxial_land_ready_projects") uint32_t ready_projects(uint32_t state) {return (uint32_t)state_at(state)->net.e[0].ready;}
EXPORT("coaxial_land_beacons_received") uint32_t beacons_received(uint32_t state) {return (uint32_t)state_at(state)->beacons_received;}
EXPORT("coaxial_land_refreshes") uint32_t refreshes(uint32_t state) {return (uint32_t)state_at(state)->net.e[0].refreshes;}
EXPORT("coaxial_land_refused") uint32_t refused(uint32_t state) {return (uint32_t)state_at(state)->net.e[0].refused;}
EXPORT("coaxial_land_project_tasks") uint32_t project_tasks(uint32_t state,uint32_t k) {return k<6?(uint32_t)state_at(state)->plot.e[k].tasks:0;}
EXPORT("coaxial_land_project_cursor") uint32_t project_cursor(uint32_t state) {return (uint32_t)state_at(state)->net.e[0].project;}
EXPORT("coaxial_land_query_stage") uint32_t query_stage(uint32_t state) {return (uint32_t)state_at(state)->net.e[0].stage;}
EXPORT("coaxial_land_pylon_answers") uint32_t pylon_answers(uint32_t state,uint32_t k) {return k<(uint32_t)state_at(state)->pylons?(uint32_t)state_at(state)->pyl.e[k].jobs:0;}
EXPORT("coaxial_land_pylon_updates") uint32_t pylon_updates(uint32_t state,uint32_t k) {return k<(uint32_t)state_at(state)->pylons?(uint32_t)state_at(state)->pyl.e[k].updates:0;}
EXPORT("coaxial_land_pylon_free") uint32_t pylon_free(uint32_t state,uint32_t k) {return k<(uint32_t)state_at(state)->pylons?(uint32_t)state_at(state)->pyl.e[k].free:0;}
EXPORT("coaxial_max_vertices") uint32_t coaxial_max_vertices(void) { return (uint32_t)coaxial_land_max_vertices(); }
EXPORT("coaxial_vertex_floats") uint32_t coaxial_vertex_floats(void) { return (uint32_t)coaxial_land_vertex_floats(); }
EXPORT("coaxial_uniform_floats") uint32_t coaxial_uniform_floats(void) { return (uint32_t)coaxial_land_uniform_floats(); }

EXPORT("coaxial_frame")
uint32_t coaxial_frame(uint32_t state, uint32_t vertices, uint32_t uniforms,
                       float width, float height, float dpr, float dt, uint32_t still) {
  coax_v_f32 out = {(float *)(uintptr_t)vertices, coaxial_land_max_vertices() * coaxial_land_vertex_floats()};
  coax_v_f32 u = {(float *)(uintptr_t)uniforms, coaxial_land_uniform_floats()};
  state_at(state)->golden=golden_mode!=0;
  if(!golden_mode)coaxial_land_network_tick(view(state));
  int64_t n = coaxial_land_frame(view(state), out, u, width, height, dpr, dt, still != 0 || golden_mode != 0);
  drain(state);
  if(!track_state.page_n)track_init(tr(),track_cstr("home"));
  track_frame(tr(),dt,width,height,dpr);track_drain(&track_state);
  return n < 0 ? 0u : (uint32_t)n;
}

// Input: the pointer pans and pinches the view, opens a card, and LOG IN, MAKE YOUR
// OWN and a card's OPEN PROJECT navigate (action 1 and its URL).
EXPORT("coaxial_pointer") uint32_t coaxial_pointer(uint32_t state, float x, float y, int32_t phase, int32_t kind, int32_t id) {
  return coaxial_land_pointer(view(state), x, y, phase, kind, id) ? 1u : 0u;
}
// The wheel and a trackpad's pinch zoom; Esc closes the card.
EXPORT("coaxial_wheel") void coaxial_wheel(uint32_t state, float dy) { coaxial_land_wheel(view(state), dy); }
EXPORT("coaxial_key") uint32_t coaxial_key(uint32_t state, int32_t key, int32_t mods) { return coaxial_land_key(view(state), key, mods) ? 1u : 0u; }
EXPORT("coaxial_cursor") int32_t coaxial_cursor(uint32_t state) { return (int32_t)coaxial_land_cursor(view(state)); }
EXPORT("coaxial_action") int32_t coaxial_action(uint32_t state) { return (int32_t)coaxial_land_action(view(state)); }
EXPORT("coaxial_outbox") uint32_t coaxial_outbox(uint32_t state) { return (uint32_t)(uintptr_t)&state_at(state)->outbox; }
EXPORT("coaxial_outbox_len") uint32_t coaxial_outbox_len(uint32_t state) { return (uint32_t)state_at(state)->out_n; }
EXPORT("coaxial_action_done") void coaxial_action_done(uint32_t state) { coaxial_land_action_done(view(state)); }
