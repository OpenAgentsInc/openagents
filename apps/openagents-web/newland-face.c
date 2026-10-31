// The live homepage C face: scalar Wasm exports over the Coax procedures.
// The state the host keeps is the page's clock, its tables and its input.
// The same module is the Grid (/realms/the-grid, data-page 1, newland-walk.coax): the same
// scene walked. There verse/player.coax walks you (its own library, as every realm links it) and
// verse/grid.coax reads a phone's thumbs; once a frame this face hands values between them in
// order: the page's controls, the player's step, then the page's frame with where you are. Where
// you are goes to the playtest room the-grid about five times a second (a browser with no player
// key gets one first), and the room's answer is everyone else, for the page. It decides nothing.
#include "newland.h"
#include "Player.h"
#include "Grid.h"
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
// The Grid's half (newland-walk.coax): you, the thumbs, and the room.
static coax_t_Player player;
static coax_t_Sticks sticks;
static _Bool placed, entered, facts_seen, has_player, mint_tried, keeping;
static double now, room_at = -10, room_asked_at;
static float view_w, view_h, view_dpr;
static coax_t_Vec3 last_feet;
#define PLAYER_URL "/api/playtest/player"
#define ROOM_WAIT_S 5.0
static int32_t room_id = -1, mint_id = -1;
static char room_url_buf[120];
static uint8_t room_body[256], player_key[256];
static uint32_t player_key_n;
static _Bool walking(uint32_t state) {return state_at(state)->walking;}
static void walk_start(uint32_t state) {
  if (placed) return;
  player = verse_player_place(coaxial_walk_spawn_x(), 0, coaxial_walk_spawn_z(), coaxial_walk_spawn_face());
  player.phi = coaxial_walk_phi();
  player.max_dist = player.dist = player.prev_dist = coaxial_walk_dist();
  sticks = verse_sticks_reset();
  last_feet = verse_player_feet(player);
  placed = 1;
}
// Where you are, to the room about five times a second, once the browser has a player key (minting
// one when it has none); the answer is everyone else, who walk the page (newland-walkers.coax).
static void room_out(void) {
  if (golden_mode || !facts_seen) return;
  if (!has_player && !mint_tried) {
    mint_tried = 1;
    mint_id = coaxial_http((uint32_t)(uintptr_t)PLAYER_URL, sizeof PLAYER_URL - 1, (uint32_t)(uintptr_t)"{}", 2, 1 | 4);
    if (mint_id < 0) mint_id = -1;
    return;
  }
  if (mint_id >= 0 || !has_player) return;
  if (room_id >= 0 && now - room_asked_at < ROOM_WAIT_S) return;
  if (now - room_at < coaxial_walk_post_every()) return;
  room_at = now;
  int64_t un = room_url(coaxial_walk_room(), 0, (coax_v_u8){(uint8_t *)room_url_buf, sizeof room_url_buf});
  coax_t_Vec3 feet = verse_player_feet(player);
  int64_t bn = room_presence_body(feet.x, feet.y > 0 ? feet.y : 0, feet.z, verse_player_face(player), 0, (coax_v_u8){room_body, sizeof room_body});
  if (un <= 0 || bn <= 0) return;
  room_id = coaxial_http((uint32_t)(uintptr_t)room_url_buf, (uint32_t)un, (uint32_t)(uintptr_t)room_body, (uint32_t)bn, 1 | 4 | 8);
  if (room_id < 0) room_id = -1;
  room_asked_at = now;
}
static uint32_t walk_frame(uint32_t state, coax_v_f32 out, coax_v_f32 u, float w, float h, float dpr, float dt, uint32_t still) {
  walk_start(state);
  coax_t_LandState *s = state_at(state);
  view_w = w; view_h = h; view_dpr = dpr;
  dt = dt < 0 ? 0 : dt > 0.1f ? 0.1f : dt;
  now += dt;
  coaxial_walk_controls(view(state), dt);
  coax_t_Walk *k = &s->wk.e[0];
  if (sticks.zoom != 0) coaxial_walk_wheel(view(state), sticks.zoom);   /* a pinch zooms as the wheel does (verse/grid.coax) */
  player = verse_player_input(player, k->key_x, k->key_y, sticks.pad_x, sticks.pad_y, verse_runs(k->run, sticks.run, sticks.pad_x, sticks.pad_y),
                              k->jump || sticks.jump, k->look_yaw + sticks.dyaw, k->look_pitch + sticks.dpitch);
  if (k->zoom != 0) player = verse_player_zoom(player, k->zoom);
  coaxial_walk_used(view(state));
  sticks = verse_sticks_used(sticks);
  if (!golden_mode) player = verse_player_advance_among(player, dt, (coax_v_f32){0, 0}, (coax_v_f32){0, 0});
  coax_t_Vec3 feet = verse_player_feet(player), eye = verse_player_view_eye(player, 1), at = verse_player_view_at(player, 1);
  int64_t n = coaxial_walk_frame(view(state), out, u, w, h, dpr, dt, still != 0 || golden_mode != 0, feet.x, feet.y, feet.z, verse_player_face(player),
                                 verse_player_act(player), eye.x, eye.y, eye.z, at.x, at.y, at.z,
                                 sticks.move_on, sticks.ox, sticks.oy, sticks.pad_x, sticks.pad_y);
  room_out();
  if (!track_state.page_n) track_init(tr(), track_cstr("the-grid"));
  float dx = feet.x - last_feet.x, dz = feet.z - last_feet.z;
  track_walk(tr(), (double)__builtin_sqrtf(dx * dx + dz * dz));
  last_feet = feet;
  track_frame(tr(), dt, w, h, dpr);
  // world-enter and load-engine once it draws, like every realm (the analytics contract, analytics-contract.json)
  if (n > 0 && !entered) { entered = 1; track_world(tr(), track_cstr("the-grid")); track_loaded(tr(), track_cstr("load-engine")); }
  track_drain(&track_state);
  return n < 0 ? 0u : (uint32_t)n;
}

EXPORT("coaxial_page") void page(uint32_t state, uint32_t n) {coaxial_land_page(view(state), (int64_t)n);}
EXPORT("coaxial_golden") void golden(uint32_t on) {golden_mode=on!=0;}
EXPORT("coaxial_inbox") uint32_t coaxial_inbox(uint32_t state) {(void)state;return (uint32_t)(uintptr_t)inbox;}
EXPORT("coaxial_inbox_cap") uint32_t coaxial_inbox_cap(void) {return sizeof inbox;}
EXPORT("coaxial_message") void coaxial_message(uint32_t state,int32_t kind,int32_t n) {
  if(n<0 || n>(int32_t)sizeof inbox)return;
  coax_string text={(const char *)inbox,n};
  if(walkers_id>=0 && kind==512+walkers_id){walkers_id=-1;coaxial_land_walkers_answer(view(state),text);return;}
  if(walking(state)){
    if(kind==600){
      facts_seen=1;coaxial_walk_facts(view(state),text);
      for(int32_t i=0;i+7<=n;i++)
        if((i==0||inbox[i-1]=='\n')&&!memcmp(inbox+i,"player=",7)&&i+7<n&&inbox[i+7]!='\n')has_player=1;
    }
    if(mint_id>=0 && kind==512+mint_id){   // a new player key: host.js keeps it (action 5)
      mint_id=-1;
      if(n>4&&n-4<=(int32_t)sizeof player_key&&!memcmp(inbox,"200\n",4)){memcpy(player_key,inbox+4,(size_t)(n-4));player_key_n=(uint32_t)(n-4);has_player=1;}
      return;
    }
    if(room_id>=0 && kind==512+room_id){
      room_id=-1;
      if(n>=4&&!memcmp(inbox,"403\n",4)&&has_player){has_player=0;mint_tried=0;return;}   // the key was refused: a new one
      coaxial_land_walkers_answer(view(state),text);
      return;
    }
  }
  coaxial_land_message(view(state),kind,text);
  if(!track_state.page_n)track_init(tr(),track_cstr(walking(state)?"the-grid":"home"));
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
  if(walking(state)){uint32_t m=walk_frame(state,out,u,width,height,dpr,dt,still);drain(state);return m;}
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
  if (walking(state)) {
    walk_start(state);
    if (coaxial_walk_pointer(view(state), x, y, phase, kind, id)) return 1u;
    if (kind != 0) sticks = verse_sticks_world_touch(sticks, id, phase, x, y, view_w, view_h, view_dpr, now);
    return kind != 0 ? 1u : 0u;
  }
  return coaxial_land_pointer(view(state), x, y, phase, kind, id) ? 1u : 0u;
}
// The wheel and a trackpad's pinch zoom; Esc closes the card.
EXPORT("coaxial_wheel") void coaxial_wheel(uint32_t state, float dy) { if (walking(state)) coaxial_walk_wheel(view(state), dy); else coaxial_land_wheel(view(state), dy); }
EXPORT("coaxial_key") uint32_t coaxial_key(uint32_t state, int32_t key, int32_t mods) {
  if (walking(state)) { track_key(tr(),key,0);track_drain(&track_state); return coaxial_walk_key(view(state), key, mods) ? 1u : 0u; }
  track_key(tr(),key,0);track_key_up(tr(),key);track_drain(&track_state); return coaxial_land_key(view(state), key, mods) ? 1u : 0u;
}
// The Grid holds keys (WASD); the homepage only presses them.
EXPORT("coaxial_key_up") uint32_t coaxial_key_up(uint32_t state, int32_t key, int32_t mods) {
  if (!walking(state)) return 0u;
  track_key_up(tr(),key);
  return coaxial_walk_key_up(view(state), key, mods) ? 1u : 0u;
}
EXPORT("coaxial_blur") void coaxial_blur(uint32_t state) { if (walking(state)) { coaxial_walk_blur(view(state)); sticks = verse_sticks_reset(); track_blur(tr()); } }
// The OpenAgents app's gyro turns the Grid's camera (coaxial.tilt).
EXPORT("coaxial_tilt") void coaxial_tilt(uint32_t state, float dyaw, float dpitch) {
  if (!walking(state)) return;
  coax_t_Walk *k = &state_at(state)->wk.e[0];
  if (dyaw == dyaw && dyaw > -0.5f && dyaw < 0.5f) k->look_yaw += dyaw;
  if (dpitch == dpitch && dpitch > -0.5f && dpitch < 0.5f) k->look_pitch += dpitch;
}
EXPORT("coaxial_cursor") int32_t coaxial_cursor(uint32_t state) { return walking(state) ? (int32_t)coaxial_walk_cursor(view(state)) : (int32_t)coaxial_land_cursor(view(state)); }
// A new player key goes first (action 5: host.js keeps the outbox as the browser's player).
EXPORT("coaxial_action") int32_t coaxial_action(uint32_t state) {
  if (player_key_n && state_at(state)->action == 0) { keeping = 1; return 5; }
  return (int32_t)coaxial_land_action(view(state));
}
EXPORT("coaxial_outbox") uint32_t coaxial_outbox(uint32_t state) { return keeping ? (uint32_t)(uintptr_t)player_key : (uint32_t)(uintptr_t)&state_at(state)->outbox; }
EXPORT("coaxial_outbox_len") uint32_t coaxial_outbox_len(uint32_t state) { return keeping ? player_key_n : (uint32_t)state_at(state)->out_n; }
EXPORT("coaxial_action_done") void coaxial_action_done(uint32_t state) {
  if (keeping) { keeping = 0; player_key_n = 0; return; }
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
