// The Haunted House's C face for the browser: what HauntViewController.swift
// and HauntAssets.swift do on the phone, for coax/web/host.js. The
// engine's own Coax plays the house (haunt.coax), walks you through it (player.coax)
// and reads your thumbs (grid.coax), each built as its own library as the
// iOS app links them; realm.coax draws the HUD and decides the controls.
// This file only holds their memory and passes values between them, once a
// frame, in the order the phone's frame does. Every pointer is an offset
// into the host's WebAssembly.Memory.
//
// The page's assets (realm/haunted-house/index.html data-assets), in order:
// 0 the house's document (Haunt.pb, into memory here), 1 its vertices and
// 2 its indices (GPU buffers), 3 its texture sheets (a GPU texture array);
// the sounds come by name.
#include "realm.h"
#include "Player.h"
#include "Grid.h"
// As Bridge.h does: Haunt's 12-float packet alias is its own C type.
#define coax_a12_f32 coax_haunt_a12_f32
#include "Haunt.h"
#undef coax_a12_f32
#include "Haunt-buffer.h"
#include <math.h>
#include <stdbool.h>
#include <stdio.h>
#include <string.h>

#define EXPORT(name) __attribute__((export_name(name)))

// ---- memory: the phone's fixed capacities (HauntAssets.swift)
static coax_t_HauntMesh meshes[256];
static coax_t_HauntEntity entities[4096];
static coax_t_HauntArea areas[64];
static coax_t_HauntRect walks[256];
static coax_t_HauntLight lamps[63];
static coax_t_HauntPerson people[64];
static coax_t_HauntZone zones[32];
static coax_t_HauntAction actions[256];
static coax_t_HauntEvent pending[256];
static coax_t_HauntInstance instances[4096];
static coax_t_HauntDraw draws[256];
static coax_t_HauntGPUlight lights[64];
static float walls[4096 * 6];
static coax_t_HauntMapRect boxes[66];
static uint8_t document[262144];   // Haunt.pb: the house's strings point into it, so it stays
static uint32_t document_n;

static coax_t_Haunt house;
static coax_t_Player player;
static coax_t_Sticks sticks;
static coax_t_Realm realm;
static coax_t_RealmView view;
static double now;
static int64_t wall_count, draw_count, light_count, instance_end;
static float width, height, dpr;
static bool opened, failed;
static uint32_t ready;             // the assets that have arrived, a bit each
static uint32_t index_count;
static char sounds[2048];
static uint32_t sounds_n;

static coax_v_t_Realm realm_of(void) { return (coax_v_t_Realm){&realm, 1}; }
static coax_t_HauntPoint point(coax_t_Vec3 v) { return (coax_t_HauntPoint){v.x, v.y, v.z}; }

// The arena's state is unused: everything lives here, and a new build starts over.
EXPORT("coaxial_layout") uint32_t coaxial_layout(void) { return (uint32_t)(sizeof(coax_t_Haunt) * 31u + sizeof(coax_t_Realm)); }
EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return 16; }
EXPORT("coaxial_max_vertices") uint32_t coaxial_max_vertices(void) { return (uint32_t)coaxial_realm_max_vertices(); }
EXPORT("coaxial_vertex_floats") uint32_t coaxial_vertex_floats(void) { return (uint32_t)coaxial_realm_vertex_floats(); }
EXPORT("coaxial_uniform_floats") uint32_t coaxial_uniform_floats(void) { return (uint32_t)coaxial_realm_uniform_floats(); }

// ---- assets
EXPORT("coaxial_asset") uint32_t coaxial_asset(uint32_t state, int32_t i, uint32_t bytes) {
  (void)state;
  return i == 0 && bytes <= sizeof document ? (uint32_t)(uintptr_t)document : 0u;
}
EXPORT("coaxial_asset_ready") void coaxial_asset_ready(uint32_t state, int32_t i, uint32_t bytes) {
  (void)state;
  if (i < 0 || i > 31) return;
  if (bytes == 0) { failed = true; return; }
  ready |= 1u << i;
  if (i == 0) document_n = bytes;
  if (i == 2) index_count = bytes / 4;
}

static void rebuild_walls(void) { wall_count = verse_haunt_walls(house, (coax_v_f32){walls, 4096 * 6}); }

// The house opens once its document and geometry are here (HauntAssets.init).
static void open_house(void) {
  coax_t_Haunt s = {0};
  s.meshes = (coax_v_t_HauntMesh){meshes, 256};
  s.entities = (coax_v_t_HauntEntity){entities, 4096};
  s.areas = (coax_v_t_HauntArea){areas, 64};
  s.walks = (coax_v_t_HauntRect){walks, 256};
  s.lights = (coax_v_t_HauntLight){lamps, 63};
  s.people = (coax_v_t_HauntPerson){people, 64};
  s.zones = (coax_v_t_HauntZone){zones, 32};
  s.actions = (coax_v_t_HauntAction){actions, 256};
  s.pending = (coax_v_t_HauntEvent){pending, 256};
  s.talking = -1; s.selected = -1; s.holding = -1; s.was_ground = true;
  house = verse_haunt_open((coax_v_u8){document, document_n}, s);
  if (!house.ok) { failed = true; return; }
  for (int64_t i = 0; i < house.mesh_count; i++)
    if (!verse_haunt_draw_range(meshes[i].first, meshes[i].count, index_count)) { failed = true; return; }
  rebuild_walls();
  player = verse_player_place(7, 0, 2, 0);
  player.phi = 1.5707963f;
  sticks = verse_sticks_reset();
  opened = true;
}

// E: talk to whoever is near, or open the door (activateNearby).
static void use_nearby(void) {
  int64_t person = verse_haunt_near(house);
  if (person >= 0) { house = verse_haunt_talk(house, person, 0); return; }
  int64_t door = verse_haunt_door(house);
  if (door >= 0) { house = verse_haunt_edit(house, door, 9, 0); rebuild_walls(); }
}

static void take_asks(void) {
  int64_t asks = realm_take(realm_of());
  if (asks & 1) use_nearby();
  if (asks & 2) { house = verse_haunt_view(house); player.phi = house.camera == 0 ? 1.5707963f : 1.107f; }
  if ((asks & 4) && house.talking >= 0) house = verse_haunt_talk(house, house.talking, 1);
  if (asks & 8) house = verse_haunt_talk(house, -1, 0);
  // Walked away: the conversation ends.
  if (house.talking >= 0 && !verse_haunt_reach(house, house.talking)) house = verse_haunt_talk(house, -1, 0);
}

// The sounds the house asked for this frame, a line each: name gain pan.
static void queue_sounds(void) {
  sounds_n = 0;
  for (int64_t i = 0; i < house.sound_count && i < 16; i++) {
    coax_t_HauntSound cue = house.sounds.e[i];
    if (cue.gain <= 0) continue;
    int n = snprintf(sounds + sounds_n, sizeof sounds - sounds_n, "%.*s %.3f %.3f\n", (int)cue.name.count, cue.name.data, cue.gain, cue.pan);
    if (n < 0 || sounds_n + (uint32_t)n >= sizeof sounds) break;
    sounds_n += (uint32_t)n;
  }
}

// The phone's frame (HauntViewController.tick), then the HUD over it.
static void step(float dt) {
  coax_v_t_Realm r = realm_of();
  coax_t_Vec3 was = player.pos;
  if (realm.zoom != 0) {
    coax_t_PlayerZoom z = verse_player_zoom_view(player, house.camera, realm.zoom);
    player = z.player; house.camera = z.mode; realm.zoom = 0;
  }
  bool run = sticks.run || (realm.held & 16), jump = sticks.jump || (realm.held & 32);
  player = verse_player_input(player, realm_key_x(r), realm_key_y(r), sticks.pad_x, sticks.pad_y, run, jump,
                                 sticks.dyaw + realm.dyaw, sticks.dpitch + realm.dpitch);
  realm.dyaw = 0; realm.dpitch = 0;
  sticks = verse_sticks_used(sticks);
  player = verse_player_advance_among(player, dt, (coax_v_f32){0, 0}, (coax_v_f32){walls, wall_count * 6});
  coax_t_HauntPoint kept = verse_haunt_constrain(house, point(was), point(player.pos));
  player = verse_player_constrain(player, kept.x, kept.z);
  coax_t_Vec3 feet = verse_player_feet(player);
  float distance = hypotf(feet.x - was.x, feet.z - was.z);
  house.facing = verse_player_face(player);
  coax_t_Vec3 eye = verse_player_view_eye(player, house.camera);
  house = verse_haunt_advance(house, now, point(feet), point(eye), player.theta, distance, player.on_ground);
  take_asks();
  queue_sounds();
}

static void describe(void) {
  coax_t_RealmView v = view;
  v.ready = opened; v.failed = failed;
  v.loaded = (float)(((ready >> 0) & 1) + ((ready >> 1) & 1) + ((ready >> 2) & 1) + ((ready >> 3) & 1)) / 4;
  if (opened) {
    coax_t_Vec3 feet = verse_player_feet(player);
    int64_t area = verse_haunt_area(house, point(feet));
    v.title = area >= 0 ? areas[area].name : (coax_string){"Haunted House", 13};
    v.haunts = house.zone_count;
    uint32_t mask = house.zone_count >= 32 ? 0xffffffffu : (1u << house.zone_count) - 1;
    v.found = __builtin_popcount(house.fired & mask);
    v.notice = now < house.toast_until ? house.toast : (coax_string){"", 0};
    int64_t person = verse_haunt_near(house), door = verse_haunt_door(house);
    v.target = person >= 0 ? people[person].name : door >= 0 ? (coax_string){"Door", 4} : (coax_string){"", 0};
    v.talking = house.talking >= 0 ? people[house.talking].name : (coax_string){"", 0};
    v.reply = house.reply;
    v.move_on = sticks.move_on; v.ox = sticks.ox; v.oy = sticks.oy; v.pad_x = sticks.pad_x; v.pad_y = sticks.pad_y;
    v.map_n = 0;
    if (realm.map) {
      coax_t_Box b = realm_map_box(realm_of());
      int64_t n = verse_haunt_map(house, b.x1 - b.x0, b.y1 - b.y0, (coax_v_t_HauntMapRect){boxes, 66});
      for (int64_t i = 0; i < n && i < 66; i++) {
        float *m = v.map.e + i * 5;
        m[0] = boxes[i].x; m[1] = boxes[i].y; m[2] = boxes[i].width; m[3] = boxes[i].height; m[4] = (float)boxes[i].kind;
      }
      v.map_n = n;
    }
  }
  view = v;
}

EXPORT("coaxial_frame")
uint32_t coaxial_frame(uint32_t state, uint32_t vertices, uint32_t uniforms, float w, float h, float scale, float dt, uint32_t still) {
  (void)state; (void)still;
  if (!realm.dpr) realm_reset(realm_of());
  width = w; height = h; dpr = scale;
  dt = dt < 0 ? 0 : dt > 0.1f ? 0.1f : dt;
  now += dt;
  if (!opened && !failed && (ready & 15) == 15) open_house();
  draw_count = 0;
  if (opened) step(dt);
  describe();
  float *u = (float *)(uintptr_t)uniforms;
  coax_v_f32 out = {(float *)(uintptr_t)vertices, coaxial_realm_max_vertices() * coaxial_realm_vertex_floats()};
  int64_t n = realm_draw(realm_of(), view, out, (coax_v_f32){u, coaxial_realm_uniform_floats()}, w, h, scale, dt);
  if (opened) {
    coax_t_Vec3 eye = verse_player_view_eye(player, house.camera), at = verse_player_view_at(player, house.camera);
    verse_grid_view(eye.x, eye.y, eye.z, at.x, at.y, at.z, w, h, (coax_v_f32){u + 8, 16});
    draw_count = verse_haunt_instances(house, now, (coax_v_t_HauntInstance){instances, 4096}, (coax_v_t_HauntDraw){draws, 256});
    light_count = verse_haunt_lights(house, now, (coax_v_t_HauntGPUlight){lights, 64});
    instance_end = 0;
    for (int64_t i = 0; i < draw_count; i++) if (draws[i].base + draws[i].instances > instance_end) instance_end = draws[i].base + draws[i].instances;
    u[24] = house.eye.x; u[25] = house.eye.y; u[26] = house.eye.z; u[27] = (float)light_count;
    u[28] = now < house.flash_until ? 1.0f : 0.0f;
  }
  return n < 0 ? 0u : (uint32_t)n;
}

// ---- the scene the host draws: indexed draws over the vertices and indices,
// with this frame's instances and lights
EXPORT("coaxial_draws") uint32_t coaxial_draws(uint32_t state) { (void)state; return (uint32_t)(uintptr_t)draws; }
EXPORT("coaxial_draw_count") uint32_t coaxial_draw_count(uint32_t state) { (void)state; return opened ? (uint32_t)draw_count : 0u; }
EXPORT("coaxial_stores") uint32_t coaxial_stores(void) { return 2; }
EXPORT("coaxial_store_cap") uint32_t coaxial_store_cap(int32_t k) { return k == 0 ? (uint32_t)sizeof instances : (uint32_t)sizeof lights; }
EXPORT("coaxial_store") uint32_t coaxial_store(uint32_t state, int32_t k) { (void)state; return (uint32_t)(uintptr_t)(k == 0 ? (void *)instances : (void *)lights); }
EXPORT("coaxial_store_bytes") uint32_t coaxial_store_bytes(uint32_t state, int32_t k) {
  (void)state;
  return k == 0 ? (uint32_t)(instance_end * sizeof(coax_t_HauntInstance)) : (uint32_t)(light_count * sizeof(coax_t_HauntGPUlight));
}
EXPORT("coaxial_sounds") uint32_t coaxial_sounds(uint32_t state) { (void)state; return (uint32_t)(uintptr_t)sounds; }
EXPORT("coaxial_sounds_len") uint32_t coaxial_sounds_len(uint32_t state) { (void)state; uint32_t n = sounds_n; sounds_n = 0; return n; }

// ---- input: keys held and let go, the pointer (a finger off the HUD is the
// thumbs', as on the phone), the wheel zooms
EXPORT("coaxial_key") uint32_t coaxial_key(uint32_t state, int32_t key, int32_t mods) {
  (void)state; return realm_key(realm_of(), view, key, mods, true) ? 1u : 0u;
}
EXPORT("coaxial_key_up") uint32_t coaxial_key_up(uint32_t state, int32_t key, int32_t mods) {
  (void)state; return realm_key(realm_of(), view, key, mods, false) ? 1u : 0u;
}
EXPORT("coaxial_blur") void coaxial_blur(uint32_t state) { (void)state; realm_blur(realm_of()); sticks = verse_sticks_reset(); }
EXPORT("coaxial_pointer") uint32_t coaxial_pointer(uint32_t state, float x, float y, int32_t phase, int32_t kind, int32_t id) {
  (void)state;
  if (realm_pointer(realm_of(), view, x, y, phase, kind, id)) return 1u;
  if (kind != 0 && opened) sticks = verse_sticks_world_touch(sticks, id, phase, x, y, width, height, dpr, now);
  return kind != 0 ? 1u : 0u;
}
EXPORT("coaxial_wheel") void coaxial_wheel(uint32_t state, float dy) { (void)state; realm_wheel(realm_of(), dy); }
EXPORT("coaxial_cursor") int32_t coaxial_cursor(uint32_t state) { (void)state; return (int32_t)realm_cursor(realm_of()); }

// Output: EXIT's navigation.
EXPORT("coaxial_action") int32_t coaxial_action(uint32_t state) { (void)state; return (int32_t)realm.action; }
EXPORT("coaxial_outbox") uint32_t coaxial_outbox(uint32_t state) { (void)state; return (uint32_t)(uintptr_t)realm.outbox.e; }
EXPORT("coaxial_outbox_len") uint32_t coaxial_outbox_len(uint32_t state) { (void)state; return (uint32_t)realm.out_n; }
EXPORT("coaxial_action_done") void coaxial_action_done(uint32_t state) { (void)state; realm.action = 0; realm.out_n = 0; }
