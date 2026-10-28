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
//
// The house is kept in this browser (the host's action 4, message 4 on the
// next visit): verse_haunt_save's bytes as hex, whenever it changed. That is
// the quest's Save step on the web until the playtest server keeps saves.
#include "realm.h"
#include "Player.h"
#include "Grid.h"
// As Bridge.h does: Haunt's 12-float packet alias is its own C type.
#define coax_a12_f32 coax_haunt_a12_f32
#include "Haunt.h"
#undef coax_a12_f32
#include "Haunt-buffer.h"
#include "analytics-face.h"
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

// Kept in this browser: the save's bytes (strings of a restored house point
// into them, so they stay), the hex the host keeps, and its scratch.
static uint8_t saved[131072], scratch[131072], restored[131072];
// Undo: the house before each edit (HauntAssets.remember), and two live
// buffers a restored house's strings point into, used in turn so the one
// the house reads is never written.
#define HISTORY 16
#define SNAP 32768
static uint8_t history[HISTORY][SNAP];
static uint32_t history_len[HISTORY];
static int history_n, history_at;
static uint8_t live[2][SNAP];
static int live_k;
static int64_t edit_camera;   // the view before edit mode (it shows you in third person)
static char kept[262144];
static uint32_t kept_n;            // the kept house's bytes waiting for it to open (0 none)
static uint32_t keep_n;            // hex in `kept` for the host to keep now
static double kept_at = -10;
static uint8_t inbox[262144];      // the host's messages: typed text, the kept house (hex)
static char feedback[512];         // the survey's answer, kept with the house
static char placed[1024][16];      // ids of what you placed: "placed-N"
static int placed_n;

// Edit mode's hotbar: short names (the HUD's) and the house's mesh for each.
static const struct { const char *label, *name, *mesh; } items[5] = {
  {"JACK", "Jack-o'-lantern", "Custom_JackOLantern"}, {"CANDLE", "Candlestick", "CandleStick"},
  {"GHOST", "Ghost", "Custom_Ghost"}, {"SPIDER", "Spider", "Custom_Spider"}, {"GRAVE", "Tombstone", "Custom_Tombstone_Round"},
};
static int64_t item_mesh[5];

// What the house records (analytics.coax, realm_track): its Track and what it saw last frame.
static coax_t_Track track_state;
static coax_t_RealmSeen seen;
static coax_v_t_Track tr(void) { return (coax_v_t_Track){&track_state, 1}; }
#define LINK "https://openagents.com/realm/haunted-house"

static coax_v_t_Realm realm_of(void) { return (coax_v_t_Realm){&realm, 1}; }
static coax_t_HauntPoint point(coax_t_Vec3 v) { return (coax_t_HauntPoint){v.x, v.y, v.z}; }
static coax_string text(const char *t) { return (coax_string){t, (int64_t)strlen(t)}; }
static void toast(const char *t) { house.toast = text(t); house.toast_until = now + 3.5; }

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

// The house kept on an earlier visit (message 4), read out of the inbox as
// it arrives: later messages (the page facts, 600) reuse the inbox before the
// house opens. kept_n: its bytes in `restored`, 0 none.
static int hex(char c) { return c >= '0' && c <= '9' ? c - '0' : c >= 'a' && c <= 'f' ? c - 'a' + 10 : -1; }
static void take_kept(uint32_t length) {
  uint32_t n = length / 2;
  kept_n = 0;
  if (n == 0 || n > sizeof restored) return;
  for (uint32_t i = 0; i < n; i++) {
    int a = hex((char)inbox[2 * i]), b = hex((char)inbox[2 * i + 1]);
    if (a < 0 || b < 0) return;
    restored[i] = (uint8_t)(a * 16 + b);
  }
  kept_n = n;
}
// When the house opens: the kept house, if it is still valid for this one.
static void restore_kept(void) {
  coax_v_u8 bytes = {restored, kept_n};
  kept_n = 0;
  if (!verse_haunt_save_valid(house, bytes)) return;
  house = verse_haunt_restore(house, bytes);
  rebuild_walls();
  for (int64_t i = 0; i < house.entity_count; i++)   // placed ids go on from the highest kept
    if (entities[i].id.count > 7 && memcmp(entities[i].id.data, "placed-", 7) == 0) placed_n++;
}

// Keeps the house in this browser when it changed (at most every 1.5 s,
// unless now), as hex for the host's action 4.
static void keep(bool at_once) {
  if (!house.dirty || realm.action || (!at_once && now - kept_at < 1.5)) return;
  int64_t n = verse_haunt_save(house, (coax_v_u8){saved, sizeof saved}, (coax_v_u8){scratch, sizeof scratch});
  if (n <= 0 || (uint64_t)n * 2 > sizeof kept) return;
  static const char digits[] = "0123456789abcdef";
  for (int64_t i = 0; i < n; i++) { kept[2 * i] = digits[saved[i] >> 4]; kept[2 * i + 1] = digits[saved[i] & 15]; }
  keep_n = (uint32_t)(n * 2);
  realm.action = 4;
  house.dirty = false;
  kept_at = now;
}

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
  if (kept_n) restore_kept();
  for (int i = 0; i < 5; i++) item_mesh[i] = verse_haunt_mesh(house, text(items[i].mesh));
  player = verse_player_place(7, 0, 2, 0);
  player.phi = 1.5707963f;
  sticks = verse_sticks_reset();
  opened = true;
}

static void do_goal(void);

// E: talk to whoever is near, else the quest's orb, else open the door
// (activateNearby, with the orb before the door so a door beside it
// never hides it).
static void use_nearby(void) {
  int64_t person = verse_haunt_near(house);
  if (person >= 0) { house = verse_haunt_talk(house, person, 0); return; }
  if (verse_haunt_at_goal(house)) { do_goal(); return; }
  int64_t door = verse_haunt_door(house);
  if (door >= 0) { house = verse_haunt_edit(house, door, 9, 0); rebuild_walls(); }
}

// Undo and redo (HauntAssets.remember, undo, redo), on saves of the house.
static bool snapshot(int k) {
  int64_t n = verse_haunt_save(house, (coax_v_u8){history[k], SNAP}, (coax_v_u8){scratch, sizeof scratch});
  history_len[k] = n > 0 ? (uint32_t)n : 0;
  return n > 0;
}
static void remember(void) {
  if (history_at == HISTORY) {   // full: the oldest goes
    memmove(history[0], history[1], sizeof history[0] * (HISTORY - 1));
    memmove(history_len, history_len + 1, sizeof history_len[0] * (HISTORY - 1));
    history_at--;
  }
  history_n = history_at;
  if (snapshot(history_n)) history_n = ++history_at;
}
static void back_to(int k) {
  int64_t selected = house.selected, camera = house.camera, quest = house.quest;
  bool editing = house.editing;
  live_k ^= 1;
  memcpy(live[live_k], history[k], history_len[k]);
  house = verse_haunt_restore(house, (coax_v_u8){live[live_k], history_len[k]});
  // An undo never takes back a step of the quest.
  house.editing = editing; house.camera = camera; house.dirty = true;
  if (house.quest < quest) house.quest = quest;
  house.selected = selected < house.entity_count ? selected : -1;
  rebuild_walls();
}
static void undo(void) {
  if (history_at == 0) return;
  if (history_at == history_n) { if (history_n == HISTORY || !snapshot(history_n)) return; history_n++; }
  back_to(--history_at);
}
static void redo(void) {
  if (history_at + 1 >= history_n) return;
  back_to(++history_at);
}

// Into or out of edit mode: it shows you in third person, and lets go of
// what's picked when it ends.
static void set_editing(bool on) {
  if (on == house.editing) return;
  house = verse_haunt_edit_mode(house, on);
  if (on) { edit_camera = house.camera; house.camera = 1; player.phi = 1.107f; }
  else { house.camera = edit_camera; player.phi = house.camera == 0 ? 1.5707963f : 1.107f; house.selected = -1; }
}

// Picks what's under (x, y) on the screen, in edit mode.
static void pick(float x, float y) {
  coax_t_Vec3 eye = verse_player_view_eye(player, house.camera), at = verse_player_view_at(player, house.camera);
  coax_t_HauntPoint ray = verse_haunt_ray(point(eye), point(at), x, y, width, height);
  house = verse_haunt_select(house, verse_haunt_pick(house, point(eye), ray));
}

// What a character does (their brain), as the row's word, and the action
// (verse_haunt_person_action) for what they do next: stay, follow, wander,
// guard, flee, and round again.
static int brain_is(coax_string b, const char *t) { return b.count == (int64_t)strlen(t) && memcmp(b.data, t, (size_t)b.count) == 0; }
static const char *mind_word(coax_string b) {
  return brain_is(b, "follower") ? "FOLLOWS" : brain_is(b, "townsfolk") ? "WANDERS" : brain_is(b, "guard") ? "GUARDS" : brain_is(b, "critter") ? "FLEES" : "STAYS";
}
static int64_t mind_next(coax_string b) {
  return brain_is(b, "follower") ? 1 : brain_is(b, "townsfolk") ? 2 : brain_is(b, "guard") ? 4 : brain_is(b, "critter") ? 3 : 0;
}

// An edit (realm.coax OP_*) to the picked thing, character or the nearest zone.
static void edit_op(int64_t op) {
  realm_track_edit(tr(), op);
  if (op == 12) { undo(); return; }
  if (op == 13) { redo(); return; }
  if (op == 14) { house.selected = -1; return; }
  // Moves go from where you look: left and right across the view, away and near along it.
  int64_t way = op >= 17 && op <= 20 ? op - 17 : op;
  float h = house.heading, fx = -sinf(h), fz = -cosf(h), step = 0.25f;
  float dx = way == 0 ? fz : way == 1 ? -fz : way == 2 ? fx : way == 3 ? -fx : 0;
  float dz = way == 0 ? -fx : way == 1 ? fx : way == 2 ? fz : way == 3 ? -fz : 0;
  if ((op >= 8 && op <= 11) || (op >= 17 && op <= 20)) {
    int64_t z = verse_haunt_zone_near(house);
    if (z < 0) return;
    remember();
    if (op >= 17) {
      if (dx != 0) house = verse_haunt_zone_edit(house, z, 3, dx * step);
      if (dz != 0) house = verse_haunt_zone_edit(house, z, 4, dz * step);
      return;
    }
    static const int64_t zone_op[4] = {1, 1, 0, 2};
    house = verse_haunt_zone_edit(house, z, zone_op[op - 8], op == 8 ? 0.25f : op == 9 ? -0.25f : 0);
    return;
  }
  int64_t i = house.selected;
  if (i < 0 || i >= house.entity_count) return;
  int64_t person = verse_haunt_person_of(house, i);
  remember();
  // A character (HauntViewController.characterMenu): what they do next, come
  // to where you look, or leave.
  if (person >= 0 && (op == 15 || op == 16 || op == 7)) {
    if (op == 15) house = verse_haunt_person_action(house, person, mind_next(people[person].brain));
    if (op == 16) house = verse_haunt_person_action(house, person, 5);
    if (op == 7) { house = verse_haunt_person_action(house, person, 6); house.selected = -1; }
    rebuild_walls();
    return;
  }
  if (op <= 3) {
    if (dx != 0) house = verse_haunt_edit(house, i, 0, dx * step);
    if (dz != 0) house = verse_haunt_edit(house, i, 2, dz * step);
  }
  if (op == 4) house = verse_haunt_edit(house, i, 3, 0.7853982f);
  if (op == 5) house = verse_haunt_edit(house, i, 4, 1.2f);
  if (op == 6) house = verse_haunt_edit(house, i, 4, 1 / 1.2f);
  if (op == 7) { house = verse_haunt_edit(house, i, 7, 0); house.selected = -1; }
  rebuild_walls();
}

// Copies the house's link (the host's action 2).
static void copy_link(void) {
  memcpy(realm.outbox.e, LINK, sizeof LINK - 1);
  realm.out_n = sizeof LINK - 1;
  realm.action = 2;
}

// Places edit mode's item `slot` in front of you.
static void place(int64_t slot, const char *via) {
  if (slot < 0 || slot >= 5 || item_mesh[slot] < 0 || placed_n >= 1024) return;
  coax_t_HauntPoint at = verse_haunt_place_point(house);
  int64_t before = house.entity_count;
  snprintf(placed[placed_n], sizeof placed[0], "placed-%d", placed_n);
  house = verse_haunt_add(house, item_mesh[slot], text(placed[placed_n]), text(items[slot].name), at.x, at.z);
  if (house.entity_count == before) { toast("There's no room there"); return; }
  placed_n++;
  rebuild_walls();
  realm_track_place(tr(), text(items[slot].name), text(via));
}

// E at the quest's orb: the step it asks for.
static void do_goal(void) {
  if (!verse_haunt_at_goal(house)) return;
  switch (house.quest) {
  case 1: set_editing(true); break;
  case 2: remember(); place(0, "orb"); break;
  case 3: house = verse_haunt_quest(house, text("publish")); keep(true); toast("Saved in this browser"); break;
  case 4: copy_link(); house = verse_haunt_quest(house, text("share")); toast("Link copied"); break;
  case 5: house = verse_haunt_fork(house, text("My Haunted House"), text("mine")); toast("This house is yours now"); break;
  case 6: copy_link(); house = verse_haunt_quest(house, text("invite")); toast("Invite link copied"); break;
  case 7: realm_ask_text(realm_of()); break;
  }
}

// The survey's answer: kept with the house, and the quest is done.
static void send_feedback(void) {
  coax_string typed = realm_typed(realm_of());
  size_t n = typed.count < (int64_t)sizeof feedback ? (size_t)typed.count : sizeof feedback;
  memcpy(feedback, typed.data, n);
  house = verse_haunt_feedback(house, (coax_string){feedback, (int64_t)n});
  realm_track_answer(tr(), (coax_string){feedback, (int64_t)n});
  realm_close_text(realm_of());
  toast("Thank you. The quest is complete");
}

static void take_asks(void) {
  if (!opened) return;
  int64_t asks = realm_take(realm_of());
  if (asks & 32) do_goal();
  if (asks & 64) send_feedback();
  if (asks & 128) set_editing(!house.editing);
  if ((asks & 256) && house.editing) { remember(); place(realm.slot, "hotbar"); }
  if ((asks & 512) && house.editing) pick(realm.pick_x, realm.pick_y);
  if ((asks & 1024) && house.editing) edit_op(realm.op);
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
  // A step of the quest is kept at once; other changes at most every 1.5 s.
  static int64_t quest_kept = -1;
  keep(house.quest != quest_kept);
  if (!house.dirty) quest_kept = house.quest;
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
    bool goal = verse_haunt_at_goal(house);
    v.quest = house.quest;
    v.quest_line = verse_haunt_step_line(house);
    v.goal = goal ? verse_haunt_goal_prompt(house) : (coax_string){"", 0};
    v.bearing = verse_haunt_goal_bearing(house);
    v.editing = house.editing;
    v.item_n = 0;
    for (int i = 0; i < 5; i++) if (item_mesh[i] >= 0) v.items.e[v.item_n++] = text(items[i].label);
    bool picked = house.editing && house.selected >= 0 && house.selected < house.entity_count;
    v.selected = picked ? entities[house.selected].name : (coax_string){"", 0};
    if (picked && v.selected.count == 0) v.selected = text("Thing");
    int64_t who = picked ? verse_haunt_person_of(house, house.selected) : -1;
    v.person = who >= 0;
    if (who >= 0) { v.selected = people[who].name; v.mind = text(mind_word(people[who].brain)); }
    int64_t zone = house.editing && !picked ? verse_haunt_zone_near(house) : -1;
    v.zone = zone >= 0 ? zones[zone].name : (coax_string){"", 0};
    v.zone_once = zone >= 0 && zones[zone].once;
    v.can_undo = history_at > 0;
    v.can_redo = history_at + 1 < history_n;
    v.notice = now < house.toast_until ? house.toast : (coax_string){"", 0};
    int64_t person = verse_haunt_near(house), door = verse_haunt_door(house);
    v.target = person >= 0 ? people[person].name : !goal && door >= 0 ? (coax_string){"Door", 4} : (coax_string){"", 0};
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
  if (!track_state.page_n) track_init(tr(), track_cstr("haunt"));
  track_frame(tr(), dt, w, h, scale);
  realm_track(realm_of(), (coax_v_t_RealmSeen){&seen, 1}, tr(), view, house.camera);
  track_drain(&track_state);
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
  (void)state;
  bool typing = realm_wants_text(realm_of());
  bool used = realm_key(realm_of(), view, key, mods, true);
  take_asks();   // now, inside the key event: a copy reaches the clipboard
  track_key(tr(), key, typing);   // never what's typed into the survey box: that's its answer
  track_drain(&track_state);
  return used ? 1u : 0u;
}
EXPORT("coaxial_key_up") uint32_t coaxial_key_up(uint32_t state, int32_t key, int32_t mods) {
  (void)state; track_key_up(tr(), key); return realm_key(realm_of(), view, key, mods, false) ? 1u : 0u;
}
EXPORT("coaxial_blur") void coaxial_blur(uint32_t state) { (void)state; realm_blur(realm_of()); sticks = verse_sticks_reset(); track_blur(tr()); }
EXPORT("coaxial_pointer") uint32_t coaxial_pointer(uint32_t state, float x, float y, int32_t phase, int32_t kind, int32_t id) {
  (void)state;
  track_pointer(tr(), x, y, phase, kind, realm_button_label(realm_of(), x, y));
  track_drain(&track_state);
  if (realm_pointer(realm_of(), view, x, y, phase, kind, id)) { take_asks(); return 1u; }
  if (kind != 0 && opened) sticks = verse_sticks_world_touch(sticks, id, phase, x, y, width, height, dpr, now);
  return kind != 0 ? 1u : 0u;
}
EXPORT("coaxial_wheel") void coaxial_wheel(uint32_t state, float dy) { (void)state; realm_wheel(realm_of(), dy); }
EXPORT("coaxial_cursor") int32_t coaxial_cursor(uint32_t state) { (void)state; return (int32_t)realm_cursor(realm_of()); }

// Output: EXIT's navigation, a copied link, the house to keep.
EXPORT("coaxial_action") int32_t coaxial_action(uint32_t state) { (void)state; return (int32_t)realm.action; }
EXPORT("coaxial_outbox") uint32_t coaxial_outbox(uint32_t state) {
  (void)state; return (uint32_t)(uintptr_t)(realm.action == 4 ? (void *)kept : (void *)realm.outbox.e);
}
EXPORT("coaxial_outbox_len") uint32_t coaxial_outbox_len(uint32_t state) { (void)state; return realm.action == 4 ? keep_n : (uint32_t)realm.out_n; }
EXPORT("coaxial_action_done") void coaxial_action_done(uint32_t state) { (void)state; realm.action = 0; realm.out_n = 0; keep_n = 0; }

// In: typed or pasted text for the survey (1), the house kept last visit (4),
// and the host's page facts and visibility for analytics.coax (600 up).
EXPORT("coaxial_inbox") uint32_t coaxial_inbox(uint32_t state) { (void)state; return (uint32_t)(uintptr_t)inbox; }
EXPORT("coaxial_inbox_cap") uint32_t coaxial_inbox_cap(void) { return sizeof inbox; }
EXPORT("coaxial_message") void coaxial_message(uint32_t state, int32_t kind, uint32_t length) {
  (void)state;
  if (length > sizeof inbox) return;
  if (!track_state.page_n) track_init(tr(), track_cstr("haunt"));   // the facts come before the first frame
  if (track_message(tr(), kind, (coax_string){(const char *)inbox, length})) { track_drain(&track_state); return; }
  if (kind == 1) realm_type(realm_of(), (coax_string){(const char *)inbox, length});
  if (kind == 4 && !opened) take_kept(length);   // restore_kept opens it with the house
}
EXPORT("coaxial_wants_text") int32_t coaxial_wants_text(uint32_t state) { (void)state; return realm_wants_text(realm_of()) ? 1 : 0; }
