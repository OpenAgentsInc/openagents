// Hecate's Cemetery's C face for the browser (coax/web/host.js): villa.coax
// is the page, verse/player.coax walks you, verse/grid.coax reads a phone's
// thumbs and villafig.coax poses the figures (each its own library). Once
// a frame it passes values between them in order: the page's controls, the
// player's step among the garden's walls, the page's step (the flight, the
// camera, the dog), the figures posed where the page put them, then the
// page's drawing. It decides nothing. Every pointer is an offset into the
// host's WebAssembly.Memory; the page's state is the host's.
#include "villa.h"
#include "Player.h"
#include "Grid.h"
// villafig.coax's library, declared here by hand: its header and villa.h each
// define the same small array types. The face holds its Figs as bytes.
typedef struct { void *data; int64_t count; } figs_view;
void vf_start(figs_view f);
void vf_set(figs_view f, int64_t i, _Bool dog, coax_v_f32 numbers, int64_t pattern, float cr, float cg, float cb, float mr, float mg, float mb);
void vf_tick(figs_view f, float dt);
void vf_step(figs_view f, int64_t i, float speed, float dt);
int64_t vf_place(figs_view f, int64_t i, float x, float y, float z, float yaw, coax_v_f32 out, coax_v_f32 bound);
_Bool vf_still(figs_view f, int64_t i);
int64_t vf_bone_floats(void);
int64_t vf_max_bones(void);
int64_t vf_size(void);
int64_t vf_pet_emote(figs_view f, int64_t n);
_Bool vf_emote(figs_view f, int64_t i, int64_t motion);
#include "analytics-face.h"
#include <stdbool.h>
#include <stddef.h>
#include <math.h>
#include <string.h>

#define EXPORT(name) __attribute__((export_name(name)))
#define IMPORT(name) __attribute__((import_module("env"), import_name(name)))
IMPORT("coaxial_http") int32_t coaxial_http(uint32_t url, uint32_t url_len, uint32_t body, uint32_t body_len, int32_t flags);
#define PLAYER_URL "/api/playtest/player"

static coax_t_Player player;
static coax_t_Sticks sticks;
static _Alignas(16) uint8_t figs[512 * 1024];
static bool placed;
static double now;
static float width, height, dpr;
static uint8_t inbox[16384];
static coax_t_Track track_state;
static bool golden;
static bool reduced;
static bool flight_told;
// Your pet's requests (villapet.coax), one at a time; a player key minted first if the browser has none.
static int32_t pet_ask = -1; static int64_t pet_kind;
static double pet_asked_at;
static char pet_url_buf[160];
static uint8_t pet_body_buf[12288];
static bool facts_seen, has_player, minted;
static int32_t mint_ask = -1;
static char mint_record[200]; static uint32_t mint_n;
static uint8_t action_out[256]; static uint32_t action_n; static int32_t action_kind;
EXPORT("coaxial_golden") void coaxial_golden(uint32_t on) { golden = on != 0; }
static coax_v_t_Track tr(void) { return (coax_v_t_Track){&track_state, 1}; }
static figs_view fv(void) { return (figs_view){figs, 1}; }

static coax_t_Villa *villa(uint32_t state) { return (coax_t_Villa *)(uintptr_t)state; }
static coax_v_t_Villa at(uint32_t state) { return (coax_v_t_Villa){villa(state), 1}; }

// You: a slight shadow of a person. The dog: a dog of the usual build, its
// coat pale so it glows white (villa.coax draws it so).
static void cast(void) {
  if (vf_size() > (int64_t)sizeof figs) __builtin_trap();
  vf_start(fv());
  float you[10] = {1.0f, 1.0f, 1.0f, 0.88f, 1.0f, 1.0f, 0.0f, 1.0f, 1.0f, 1.0f};
  vf_set(fv(), 0, false, (coax_v_f32){you, 10}, 0, 0.02f, 0.02f, 0.03f, 0, 0, 0);
  float dog[10] = {1.0f, 1.0f, 1.0f, 1.0f, 1.0f, 1.0f, 0.8f, 1.1f, 1.0f, 1.2f};
  vf_set(fv(), 1, true, (coax_v_f32){dog, 10}, 0, 0.92f, 0.94f, 1.0f, 0.86f, 0.9f, 1.0f);
}

static void start(uint32_t state) {
  if (!track_state.page_n) track_init(tr(), track_cstr("hecates-cemetery"));
  if (placed) return;
  coax_t_Point spawn = villa_spawn(at(state));
  player = verse_player_place(spawn.x, spawn.y, spawn.z, 0);
  sticks = verse_sticks_reset();
  cast();
  placed = true;
}

EXPORT("coaxial_layout") uint32_t coaxial_layout(void) {
  return (uint32_t)(sizeof(coax_t_Villa) * 1000003u + offsetof(coax_t_Villa, objects) * 7u + offsetof(coax_t_Villa, draws) * 13u + 0x56494c4cu);
}
EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_Villa); }
EXPORT("coaxial_max_vertices") uint32_t coaxial_max_vertices(void) { return (uint32_t)coaxial_villa_max_vertices(); }
EXPORT("coaxial_vertex_floats") uint32_t coaxial_vertex_floats(void) { return (uint32_t)coaxial_villa_vertex_floats(); }
EXPORT("coaxial_uniform_floats") uint32_t coaxial_uniform_floats(void) { return (uint32_t)coaxial_villa_uniform_floats(); }

// The dog's look from your pet's avatar (villapet.coax), when it changes.
static void pet_look(uint32_t state) {
  coax_t_PetUi *p = &villa(state)->pet.e[0];
  if (!p->look_new) return;
  p->look_new = false;
  vf_set(fv(), 1, true, (coax_v_f32){p->avatar.e, 10}, p->pattern, p->coat.e[0], p->coat.e[1], p->coat.e[2], p->marks.e[0], p->marks.e[1], p->marks.e[2]);
}

// An action the panel asked for, handed to the host (in the press, for the file picker).
static void pet_action(uint32_t state) {
  coax_t_PetUi *p = &villa(state)->pet.e[0];
  if (!p->action || action_kind) return;
  memcpy(action_out, p->outbox.e, (size_t)p->out_n); action_n = (uint32_t)p->out_n; action_kind = (int32_t)p->action;
  p->action = 0; p->out_n = 0;
}

// The pet's requests: the next one wanted goes when none is waiting.
static void pet_requests(uint32_t state) {
  coax_t_PetUi *p = &villa(state)->pet.e[0];
  if (p->emote >= 0) { vf_emote(fv(), 1, vf_pet_emote(fv(), p->emote)); p->emote = -1; }
  pet_action(state);
  if (mint_n && !action_kind) { memcpy(action_out, mint_record, mint_n); action_n = mint_n; action_kind = 5; mint_n = 0; has_player = true; }
  if (!facts_seen) return;
  if (!has_player) {
    if (!minted) { minted = true; mint_ask = coaxial_http((uint32_t)(uintptr_t)PLAYER_URL, sizeof PLAYER_URL - 1, (uint32_t)(uintptr_t)"{}", 2, 1 | 4); }
    return;
  }
  if (pet_ask >= 0 && now - pet_asked_at < 200) return;
  int64_t kind = villa_pet_next(at(state));
  if (!kind) return;
  int64_t un = villa_pet_url(at(state), kind, (coax_v_u8){(uint8_t *)pet_url_buf, sizeof pet_url_buf});
  int64_t bn = villa_pet_post(kind) ? villa_pet_body(at(state), kind, (coax_v_u8){pet_body_buf, sizeof pet_body_buf}) : 0;
  if (un <= 0 || bn < 0) { villa_pet_answer(at(state), kind, 0, (coax_string){"", 0}); return; }
  int32_t id = coaxial_http((uint32_t)(uintptr_t)pet_url_buf, (uint32_t)un, (uint32_t)(uintptr_t)pet_body_buf, (uint32_t)bn,
                            (villa_pet_post(kind) ? 1 : 0) | 4 | 8);
  if (id < 0) return;
  pet_ask = id; pet_kind = kind; pet_asked_at = now;
}

// The figures where the page left them: you (0) on your feet, the dog (1) where it walks.
static void pose(uint32_t state, float dt) {
  coax_t_Villa *v = villa(state);
  vf_tick(fv(), dt);
  int64_t per = vf_max_bones() * vf_bone_floats();
  coax_t_Point feet = v->feet;
  vf_step(fv(), 0, villa_you_speed(at(state)), dt);
  int64_t n = vf_place(fv(), 0, feet.x, feet.y, feet.z, v->face, (coax_v_f32){v->bones.e, per}, (coax_v_f32){v->bound.e, 4});
  villa_set_bones(at(state), 0, n);
  coax_t_Follower dog = villa_dog(at(state));
  float speed = sqrtf(dog.vel.x * dog.vel.x + dog.vel.z * dog.vel.z);
  vf_step(fv(), 1, speed, dt);
  n = vf_place(fv(), 1, dog.pos.x, dog.pos.y, dog.pos.z, dog.face, (coax_v_f32){v->bones.e + per, per}, (coax_v_f32){v->bound.e + 4, 4});
  villa_set_bones(at(state), 1, n);
  villa_set_pet_still(at(state), vf_still(fv(), 1));
}

EXPORT("coaxial_frame")
uint32_t coaxial_frame(uint32_t state, uint32_t vertices, uint32_t uniforms, float w, float h, float scale, float dt, uint32_t still) {
  start(state);
  coax_t_Villa *v = villa(state);
  width = w; height = h; dpr = scale;
  dt = dt < 0 ? 0 : dt > 0.1f ? 0.1f : dt;
  now += dt;
  v->golden = golden;
  if (!v->started || v->time == 0) v->still = still != 0;
  v->wall_ms = track_state.epoch + track_state.t * 1000.0;
  villa_controls(at(state), dt, sticks.dyaw, sticks.dpitch);
  bool busy = villa_busy(at(state));
  if (busy) sticks = verse_sticks_reset();
  player = verse_player_input(player, v->key_x, v->key_y, sticks.pad_x, sticks.pad_y, v->run || sticks.run, v->jump || sticks.jump, 0, 0);
  player.theta = villa_player_theta(at(state));
  sticks = verse_sticks_used(sticks);
  int64_t walls = villa_walls(at(state));
  player = verse_player_advance_among(player, dt, (coax_v_f32){0, 0}, (coax_v_f32){v->walls.e, walls * 6});
  coax_t_Vec3 feet = verse_player_feet(player);
  villa_step(at(state), dt, feet.x, feet.y, feet.z, verse_player_face(player), verse_player_act(player), player.vel.x, player.vel.z);
  pet_look(state);
  pose(state, dt);
  pet_requests(state);
  coax_v_f32 out = {(float *)(uintptr_t)vertices, coaxial_villa_max_vertices() * coaxial_villa_vertex_floats()};
  coax_v_f32 u = {(float *)(uintptr_t)uniforms, coaxial_villa_uniform_floats()};
  int64_t n = villa_draw(at(state), out, u, w, h, scale, sticks.move_on, sticks.ox, sticks.oy, sticks.pad_x, sticks.pad_y);
  track_frame(tr(), dt, w, h, scale);
  if (n > 0) track_loaded(tr(), track_cstr("load-hecates-cemetery"));
  // The flight, once: that it played to its end or was skipped.
  if (!flight_told && v->time > 0.2 && !villa_busy(at(state))) {
    flight_told = true;
    if (villa_fly_wanted(at(state)) && track_begin(tr(), track_cstr("villa-flight"))) {
      track_bool(tr(), track_cstr("skipped"), v->fl_t < 8.5f);
      track_end(tr());
    }
  }
  track_drain(&track_state);
  return n < 0 ? 0u : (uint32_t)n;
}

EXPORT("coaxial_draws") uint32_t coaxial_draws(uint32_t state) { return (uint32_t)(uintptr_t)villa(state)->draws.e; }
EXPORT("coaxial_draw_count") uint32_t coaxial_draw_count(uint32_t state) { return villa(state)->started ? (uint32_t)villa_draws(at(state)) : 0u; }
EXPORT("coaxial_stores") uint32_t coaxial_stores(void) { return 2; }
EXPORT("coaxial_store_cap") uint32_t coaxial_store_cap(int32_t k) { return (uint32_t)villa_store_cap(k); }
EXPORT("coaxial_store") uint32_t coaxial_store(uint32_t state, int32_t k) {
  return (uint32_t)(uintptr_t)(k == 0 ? (void *)villa(state)->heights.e : (void *)villa(state)->objects.e);
}
EXPORT("coaxial_store_bytes") uint32_t coaxial_store_bytes(uint32_t state, int32_t k) { (void)state; return (uint32_t)villa_store_cap(k); }

// ---- the particles the host draws after the scene (particles.wgsl)
EXPORT("coaxial_particle_cap") uint32_t coaxial_particle_cap(void) { return (uint32_t)sizeof(((coax_t_Villa *)0)->emitters); }
EXPORT("coaxial_particles") uint32_t coaxial_particles(uint32_t state) { return (uint32_t)(uintptr_t)villa(state)->emitters.e; }
EXPORT("coaxial_particle_bytes") uint32_t coaxial_particle_bytes(uint32_t state) {
  return (uint32_t)(villa_emitter_count(at(state)) * 16 * sizeof(float));
}
EXPORT("coaxial_particle_instances") uint32_t coaxial_particle_instances(uint32_t state) {
  return (uint32_t)(villa_emitter_count(at(state)) * villa_particles_per());
}

EXPORT("coaxial_key") uint32_t coaxial_key(uint32_t state, int32_t key, int32_t mods) {
  start(state);
  track_key(tr(), key, false);
  bool used = villa_key(at(state), key, mods);
  track_drain(&track_state);
  return used ? 1u : 0u;
}
EXPORT("coaxial_key_up") uint32_t coaxial_key_up(uint32_t state, int32_t key, int32_t mods) {
  track_key_up(tr(), key);
  return villa_key_up(at(state), key, mods) ? 1u : 0u;
}
EXPORT("coaxial_blur") void coaxial_blur(uint32_t state) { villa_blur(at(state)); sticks = verse_sticks_reset(); track_blur(tr()); }
EXPORT("coaxial_pointer") uint32_t coaxial_pointer(uint32_t state, float x, float y, int32_t phase, int32_t kind, int32_t id) {
  start(state);
  track_pointer(tr(), x, y, phase, kind, (coax_string){"", 0});
  track_drain(&track_state);
  if (villa_pointer(at(state), x, y, phase, kind, id)) { pet_action(state); return 1u; }
  if (kind != 0) sticks = verse_sticks_world_touch(sticks, id, phase, x, y, width, height, dpr, now);
  return kind != 0 ? 1u : 0u;
}
EXPORT("coaxial_wheel") void coaxial_wheel(uint32_t state, float dy) { villa_wheel(at(state), dy); }

EXPORT("coaxial_inbox") uint32_t coaxial_inbox(uint32_t state) { (void)state; return (uint32_t)(uintptr_t)inbox; }
EXPORT("coaxial_inbox_cap") uint32_t coaxial_inbox_cap(void) { return sizeof inbox; }
static int http_status(uint32_t length, uint32_t *body) {
  int status = 0; uint32_t i = 0;
  while (i < length && inbox[i] >= '0' && inbox[i] <= '9') { status = status * 10 + (inbox[i] - '0'); i++; }
  while (i < length && inbox[i] != '\n') i++;
  *body = i < length ? i + 1 : length;
  return status;
}
EXPORT("coaxial_message") void coaxial_message(uint32_t state, int32_t kind, uint32_t length) {
  if (length > sizeof inbox) return;
  start(state);
  if (kind == 600) {   // the page facts: whether the browser has a player key
    facts_seen = true;
    for (uint32_t i = 0; i + 7 < length; i++)
      if ((i == 0 || inbox[i - 1] == '\n') && !memcmp(inbox + i, "player=", 7) && i + 7 < length && inbox[i + 7] != '\n') has_player = true;
  }
  if (kind == 1) { villa_typed(at(state), (coax_string){(const char *)inbox, length}); return; }
  if (kind == 52) { villa_pet_files(at(state), (coax_string){(const char *)inbox, length}); return; }
  if (kind == 53) { villa_pet_scanned(at(state), (coax_string){(const char *)inbox, length}); return; }
  if (mint_ask >= 0 && kind == 512 + mint_ask) {
    mint_ask = -1;
    uint32_t b; int st = http_status(length, &b);
    if (st == 200 && length - b > 0 && length - b <= sizeof mint_record) { memcpy(mint_record, inbox + b, length - b); mint_n = length - b; }
    return;
  }
  if (pet_ask >= 0 && kind == 512 + pet_ask) {
    pet_ask = -1;
    uint32_t b; int st = http_status(length, &b);
    villa_pet_answer(at(state), pet_kind, st, (coax_string){(const char *)inbox + b, length - b});
    return;
  }
  if (kind >= 512 && kind < 520) return;
  if (track_message(tr(), kind, (coax_string){(const char *)inbox, length})) track_drain(&track_state);
}

// The page's actions: copy a link (2), keep the player key (5), pick files to upload (30, uploads.js).
EXPORT("coaxial_action") int32_t coaxial_action(uint32_t state) { (void)state; return action_kind; }
EXPORT("coaxial_outbox") uint32_t coaxial_outbox(uint32_t state) { (void)state; return (uint32_t)(uintptr_t)action_out; }
EXPORT("coaxial_outbox_len") uint32_t coaxial_outbox_len(uint32_t state) { (void)state; return action_n; }
EXPORT("coaxial_action_done") void coaxial_action_done(uint32_t state) { (void)state; action_kind = 0; action_n = 0; }
// Typing in the pet's panel or a memorial's note: the keyboard field over the box drawn.
EXPORT("coaxial_wants_text") int32_t coaxial_wants_text(uint32_t state) { return villa_wants_text(at(state)) ? 1 : 0; }
EXPORT("coaxial_field") uint32_t coaxial_field(uint32_t state) { return (uint32_t)(uintptr_t)&villa(state)->pet.e[0].field_box; }
// A memorial's link: #m/<slug>.
EXPORT("coaxial_hash") void coaxial_hash(uint32_t state, uint32_t length) {
  if (length > sizeof inbox) return;
  start(state);
  villa_hash(at(state), (coax_string){(const char *)inbox, length});
}
