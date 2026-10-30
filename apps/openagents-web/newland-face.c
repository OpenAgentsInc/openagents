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
// The Grid's walkers (newland-walkers.coax): who walks the Grid now, asked about once a second; and
// WALK AROUND counted once when pressed.
static int32_t walkers_id=-1;
static void walkers_out(uint32_t state) {
  coax_t_LandState *s=state_at(state);
  if(s->walk_click){s->walk_click=0;if(track_begin(tr(),track_cstr("walk-around")))track_end(tr());track_drain(&track_state);}
  if(golden_mode || walkers_id>=0 || !coaxial_land_walkers_ask(view(state)))return;
  coax_string url=coaxial_land_walkers_url();
  walkers_id=coaxial_http((uint32_t)(uintptr_t)url.data,(uint32_t)url.count,0,0,4|8);
  if(walkers_id<0){walkers_id=-1;coaxial_land_walkers_answer(view(state),(coax_string){"0\n",2});}
}
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
  if(walkers_id>=0 && kind==512+walkers_id){walkers_id=-1;coaxial_land_walkers_answer(view(state),text);return;}
  coaxial_land_message(view(state),kind,text);
  if(!track_state.page_n)track_init(tr(),track_cstr("home"));
  _Bool mine=track_message(tr(),kind,text);track_drain(&track_state);
  if(!mine && !golden_mode)coaxial_land_network_message(view(state),kind,text);
  drain(state);
}

EXPORT("coaxial_layout") uint32_t coaxial_layout(void) {
  return (uint32_t)(sizeof(coax_t_LandState) * 1000003u + _Alignof(coax_t_LandState) * 31u
                    + offsetof(coax_t_LandState, plot) * 7u + offsetof(coax_t_LandState, playtest) * 13u + 0x4c414eu);
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
  walkers_out(state);
  if(!track_state.page_n)track_init(tr(),track_cstr("home"));
  track_frame(tr(),dt,width,height,dpr);
  // what people do here (analytics.coax): the page drawn, each card opened (CARD_*)
  static int64_t card_seen;
  if(n>0)track_loaded(tr(),track_cstr("load-engine"));
  if(state_at(state)->card!=card_seen){card_seen=state_at(state)->card;if(card_seen>0)track_act_n(tr(),track_cstr("card"),track_cstr(""),card_seen);}
  track_drain(&track_state);
  return n < 0 ? 0u : (uint32_t)n;
}

// Input: the pointer pans and pinches the view, opens a card, and PLAYTEST
// and a card's OPEN PROJECT navigate (action 1 and its URL).
EXPORT("coaxial_pointer") uint32_t coaxial_pointer(uint32_t state, float x, float y, int32_t phase, int32_t kind, int32_t id) {
  track_pointer(tr(),x,y,phase,kind,track_cstr(""));track_drain(&track_state);
  return coaxial_land_pointer(view(state), x, y, phase, kind, id) ? 1u : 0u;
}
// The wheel and a trackpad's pinch zoom; Esc closes the card.
EXPORT("coaxial_wheel") void coaxial_wheel(uint32_t state, float dy) { coaxial_land_wheel(view(state), dy); }
EXPORT("coaxial_key") uint32_t coaxial_key(uint32_t state, int32_t key, int32_t mods) { track_key(tr(),key,0);track_key_up(tr(),key);track_drain(&track_state); return coaxial_land_key(view(state), key, mods) ? 1u : 0u; }
EXPORT("coaxial_cursor") int32_t coaxial_cursor(uint32_t state) { return (int32_t)coaxial_land_cursor(view(state)); }
EXPORT("coaxial_action") int32_t coaxial_action(uint32_t state) { return (int32_t)coaxial_land_action(view(state)); }
EXPORT("coaxial_outbox") uint32_t coaxial_outbox(uint32_t state) { return (uint32_t)(uintptr_t)&state_at(state)->outbox; }
EXPORT("coaxial_outbox_len") uint32_t coaxial_outbox_len(uint32_t state) { return (uint32_t)state_at(state)->out_n; }
EXPORT("coaxial_action_done") void coaxial_action_done(uint32_t state) {
  coax_t_LandState *s=state_at(state);   // where PLAYTEST or OPEN PROJECT went
  if(s->out_n>0)track_act(tr(),track_cstr("open"),(coax_string){(const char *)&s->outbox,s->out_n<80?s->out_n:80});
  track_drain(&track_state);
  coaxial_land_action_done(view(state));
}

/* Read-only UI probes exercise the same Coax layout and presentation state. */
EXPORT("coaxial_land_network_open") uint32_t network_open(uint32_t state) {return state_at(state)->card==5;}
EXPORT("coaxial_land_network_online") uint32_t network_online(uint32_t state) {return (uint32_t)network_counts(view(state)).online;}
EXPORT("coaxial_land_network_free") uint32_t network_free(uint32_t state) {return (uint32_t)network_counts(view(state)).free;}
EXPORT("coaxial_land_network_scroll") float network_scroll(uint32_t state) {return state_at(state)->network_scroll;}
EXPORT("coaxial_land_pylon_control") float pylon_control(uint32_t state,uint32_t which,uint32_t axis) {
  coax_t_LandState *s=state_at(state);coax_t_Box b={0};
  if(which==0)b=s->network_box;else if(which==1)b=s->network_join;else if(which==2)b=s->network_guide;else if(which==3)b=s->close_box;
  if(axis==0)return b.x0;if(axis==1)return b.y0;if(axis==2)return b.x1;if(axis==3)return b.y1;return 0;
}
