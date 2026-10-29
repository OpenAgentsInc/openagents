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
#include "analytics-face.h"
#include <stdbool.h>
#include <stddef.h>
#include <math.h>

#define EXPORT(name) __attribute__((export_name(name)))

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
  pose(state, dt);
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
  if (villa_pointer(at(state), x, y, phase, kind, id)) return 1u;
  if (kind != 0) sticks = verse_sticks_world_touch(sticks, id, phase, x, y, width, height, dpr, now);
  return kind != 0 ? 1u : 0u;
}
EXPORT("coaxial_wheel") void coaxial_wheel(uint32_t state, float dy) { villa_wheel(at(state), dy); }

EXPORT("coaxial_inbox") uint32_t coaxial_inbox(uint32_t state) { (void)state; return (uint32_t)(uintptr_t)inbox; }
EXPORT("coaxial_inbox_cap") uint32_t coaxial_inbox_cap(void) { return sizeof inbox; }
EXPORT("coaxial_message") void coaxial_message(uint32_t state, int32_t kind, uint32_t length) {
  if (length > sizeof inbox) return;
  start(state);
  if (track_message(tr(), kind, (coax_string){(const char *)inbox, length})) track_drain(&track_state);
}
