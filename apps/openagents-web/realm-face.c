// The Haunted House's C face for the browser: what HauntViewController.swift
// and HauntAssets.swift do on the phone, for coax/web/host.js. The
// engine's own Coax plays the house (haunt.coax), walks you through it (player.coax)
// and reads your thumbs (grid.coax), each built as its own library as the
// iOS app links them; realm.coax draws the HUD and decides the controls.
// This file only holds their memory and passes values between them, once a
// frame, in the order the phone's frame does. Every pointer is an offset
// into the host's WebAssembly.Memory.
//
// The page's assets (realms/haunted-house/index.html data-assets), in order:
// 0 the house's document (Haunt.pb, into memory here), 1 its vertices and
// 2 its indices (GPU buffers), 3 its texture sheets (a GPU texture array);
// the sounds come by name.
//
// The house is saved (save.coax) whenever it changed: verse_haunt_save's
// bytes in an AESV container with a CRC, kept in this browser as the current
// save and the last good one (the host's action 4 into IndexedDB, message 4
// on the next visit). A player who joined with an email (the facts' account)
// also saves on the server (/api/playtest/progress/haunted-house/main, with
// the host's player key); on the next visit, in any browser, the newer of
// the two opens. A save that doesn't open falls back to the last good one.
//
// Talking: a typed question (ASK) goes to the server (/api/playtest/npc/ask,
// with the player key), which answers as the character from who they are,
// what players taught them and the conversation so far. When it can't (an
// error, the day's answers used up, nothing back in NPC_WAIT seconds) the
// house's own lines answer (verse_haunt_ask). What you TEACH a character is
// added to what they know and kept with the house's save.
// A question VOICE heard (realm_heard) in a conversation goes the same way,
// and their answer, theirs or the house's lines, is spoken (realm_voice_say);
// voice.js is told whom you face (realm_voice_to), so the voice host leaves
// those turns to this page instead of answering as the house.
//
// The Sandbox's set pieces (rl-sandbox-pieces): in edit mode MAKE A SET
// PIECE (P) takes one sentence to the server's selector
// (/api/playtest/sandbox/piece), where Gemini picks a piece from the
// catalogue in verse/setpiece.coax; its answer is checked there again, laid
// out by its seeded generator in front of you, added as one undo step, and
// published to the shared Sandbox at once.
#include "realm.h"
#include "Player.h"
#include "Grid.h"
// As Bridge.h does: Haunt's 12-float packet alias is its own C type.
#define coax_a12_f32 coax_haunt_a12_f32
#include "Haunt.h"
#undef coax_a12_f32
#include "Haunt-buffer.h"
#include "Particles.h"
#include "Setpiece.h"
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
// The particles' emitters (particles.coax) and what they are chosen from.
static float emitters[24 * 16], lamp_floats[64 * 8], area_floats[64 * 6];
static int64_t emitter_n;
static uint8_t document[262144];   // Haunt.pb: the house's strings point into it, so it stays
static uint32_t document_n;

static coax_t_Haunt house;
static coax_t_Player player;
static coax_t_Sticks sticks;
static coax_t_Realm realm;
static coax_t_RealmView view;
static double now;
// Golden mode (the release walk, coax/web/goldens.ts): the effects' clock
// (candle flicker, animated props, fireflies, embers, dust, the sky) stands
// at one moment, so two shots of the same build match. The house still
// runs on now; only what moves on its own is held.
#define GOLDEN_FX 2.0
static bool golden;
EXPORT("coaxial_golden") void coaxial_golden(uint32_t on) { golden = on != 0; }
static double fx_now(void) { return golden ? GOLDEN_FX : now; }
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
// The world the document is (its world_id): the Haunted House, or the
// Sandbox (rl-sandbox), everyone's open ground, which opens as its own
// shared world on the server, published when you leave edit mode.
static const char *save_world = "haunted-house";
#define SAVE_WORLD save_world
static bool sandbox;
#define SAVE_SCHEMA 1              // the house's save in this build; save.coax migrates older ones forward
#define PROGRESS_URL "/api/playtest/progress/haunted-house/main"
#define HTTP_ANSWER_PLAYER 12      // coaxial_http: an answer wanted (4), the player key sent (8)
// A save is under SNAP (32 KB, as undo's); the record holds two in base64.
static char kept[131072];          // the record for the host to keep (save.coax), keep_n of it
static uint32_t kept_n;            // the kept house's bytes waiting for it to open (0 none)
static uint32_t keep_n;            // the record's length when the host is to keep it now
static double kept_at = -10;
static uint8_t inbox[262144];      // the host's messages: typed text, the kept record, the server's save
static uint8_t slot_bytes[2 * SNAP + 1024], server_bytes[2 * SNAP + 1024], container[SNAP + 512];
static coax_t_SaveSlots slots;     // the browser's two saves, in slot_bytes
static double loaded_at;           // when the save that opened was made (ms), 0 none
static bool has_account;           // the player joined with an email: saves go to the server too
static bool has_player, facts_seen; // the facts named a player key (the host sends it), and came
static int32_t progress_ask = -1;  // the server's save, asked for
static char post[SNAP * 2];
static char feedback[512];         // the survey's answer, kept with the house
static char placed[1024][16];      // ids of what you placed: "placed-N"
static int placed_n;
#define NPC_URL "/api/playtest/npc/ask"
#define NPC_WAIT 12.0              // seconds before the house's own lines answer instead
#define HTTP_ASK 13                // coaxial_http: a POST (1), an answer wanted (4), the player key (8)
static int32_t npc_ask = -1;       // the question's request (-1 none waiting)
static double npc_asked_at;
static int64_t npc_who = -1;       // whom it was asked of
static char question[512];
static size_t question_n;
static char answer_text[1024];     // their answer: house.reply points here
static uint8_t npc_body[12288];
#define TAUGHT 16
static char taught[TAUGHT][4096];  // what characters were taught: their knowledge points here
static int64_t talking_was = -1;   // the conversation the realm remembers
static coax_string greet;          // what they said first
static bool npc_spoken;            // the question out was heard by VOICE: speak the answer
static char say_text[1024];        // the answer for voice.js, until the host's outbox takes it
static size_t say_n;
static bool say_waiting;

// Edit mode's hotbar: short names (the HUD's) and the house's mesh for each.
typedef struct { const char *label, *name, *mesh; } Item;
static const Item house_items[5] = {
  {"JACK", "Jack-o'-lantern", "Custom_JackOLantern"}, {"CANDLE", "Candlestick", "CandleStick"},
  {"GHOST", "Ghost", "Custom_Ghost"}, {"SPIDER", "Spider", "Custom_Spider"}, {"GRAVE", "Tombstone", "Custom_Tombstone_Round"},
};
static const Item sandbox_items[5] = {
  {"CRATE", "Crate", "Crate_Wooden"}, {"BARREL", "Barrel", "Barrel"}, {"LAMP", "Lamp", "CandleStick_Stand"},
  {"TREE", "Tree", "Custom_Tree_Round"}, {"ROCK", "Rock", "Custom_Rock"},
};
static const Item *items = house_items;
static int64_t item_mesh[5];

// What the house records (analytics.coax, realm_track): its Track and what it saw last frame.
static coax_t_Track track_state;
static coax_t_RealmSeen seen;
static coax_v_t_Track tr(void) { return (coax_v_t_Track){&track_state, 1}; }
static const char *link = "https://openagents.com/realms/haunted-house";

// The set piece asked for (-1 none waiting), its parts' ids, and what the
// publish says it was.
#define PIECE_URL "/api/playtest/sandbox/piece"
#define PIECE_WAIT 25.0
static int32_t piece_ask = -1;
static double piece_asked_at;
static uint8_t piece_body[1024];
static float places[64 * 12];
static char piece_ids[1024][16];
static int piece_id_n;
static char piece_said[96], send_said[128];

static bool world_holds(void);
static void world_open(coax_string h);
static coax_string world_name(void);
static bool world_send(void);
static void world_edited(void);
static void room_say(coax_string typed);

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

// The record kept on an earlier visit (message 4), read out of the inbox as
// it arrives: later messages (the page facts, 600) reuse the inbox before the
// house opens. kept_n: 1 when there is one to open with the house.
static void take_kept(uint32_t length) {
  slots = save_read((coax_string){(const char *)inbox, length}, (coax_v_u8){slot_bytes, sizeof slot_bytes});
  kept_n = slots.cur_n || slots.prev_n ? 1 : 0;
}
static double wall_ms(void) { return track_state.epoch + track_state.t * 1000.0; }
static void count_placed(void) {
  placed_n = 0;
  for (int64_t i = 0; i < house.entity_count; i++)   // placed ids go on from the highest kept
    if (entities[i].id.count > 7 && memcmp(entities[i].id.data, "placed-", 7) == 0) placed_n++;
}
// Opens the house from saved bytes (the current save, else the last good
// one, whose house is valid for this one); true when one opened.
static bool open_save(uint8_t *bytes, size_t cap, coax_t_SaveSlots s, bool *fell_back, double *time) {
  coax_v_u8 all = {bytes, (int64_t)cap};
  for (int from = 1; from <= 2; from++) {
    coax_t_SavePick k = save_pick(all, s, text(SAVE_WORLD), SAVE_SCHEMA, from);
    if (k.which == 0 || k.count > (int64_t)sizeof restored) return false;
    memcpy(restored, bytes + k.at, (size_t)k.count);
    coax_v_u8 payload = {restored, k.count};
    if (!verse_haunt_save_valid(house, payload)) { if (k.which == 2) return false; continue; }
    house = verse_haunt_restore(house, payload);
    rebuild_walls();
    count_placed();
    *fell_back = k.which == 2;
    *time = k.time;
    if (k.schema < SAVE_SCHEMA) house.dirty = true;   // an older save: kept again as this build's
    return true;
  }
  return false;
}
// When the house opens: the kept house, if it is still valid for this one.
static void restore_kept(void) {
  kept_n = 0;
  bool fell_back = false; double time = 0;
  if (!open_save(slot_bytes, sizeof slot_bytes, slots, &fell_back, &time)) return;
  loaded_at = time;
  if (fell_back) { toast("Your last save didn't open; the one before it is back"); house.dirty = true; }
}
// The server's save (its answer to progress_ask): opens when it is newer
// than what this browser opened, and is kept here as the current save.
// It may come before the house opens: read now, opened with the house.
static coax_t_SaveSlots server_slots;
static bool server_waiting;
static void apply_server(void) {
  if (!server_waiting || !opened) return;
  server_waiting = false;
  if (world_holds()) return;
  coax_t_SavePick k = save_pick((coax_v_u8){server_bytes, sizeof server_bytes}, server_slots, text(SAVE_WORLD), SAVE_SCHEMA, 1);
  if (k.which == 0 || k.time <= loaded_at) return;
  bool fell_back = false; double time = 0;
  if (!open_save(server_bytes, sizeof server_bytes, server_slots, &fell_back, &time)) return;
  loaded_at = time;
  house.dirty = true;
  toast("Your saved house is back");
}
static void take_server(uint32_t length) {
  server_slots = save_from_server((coax_string){(const char *)inbox, length}, (coax_v_u8){server_bytes, sizeof server_bytes});
  server_waiting = true;
  apply_server();
}

// Saves the house when it changed (at most every 1.5 s, unless now): the
// record for the host's action 4, and the container to the server for a
// player with an email.
static void keep(bool at_once) {
  if (world_holds() || !house.dirty || realm.action || (!at_once && now - kept_at < 1.5)) return;
  int64_t n = verse_haunt_save(house, (coax_v_u8){saved, sizeof saved}, (coax_v_u8){scratch, sizeof scratch});
  if (n <= 0) return;
  int64_t cn = save_pack(text(SAVE_WORLD), SAVE_SCHEMA, (coax_v_u8){saved, n}, n, (coax_v_u8){container, sizeof container});
  if (cn <= 0) return;
  double at = wall_ms();
  int64_t rn = save_record((coax_v_u8){container, cn}, cn, at, (coax_v_u8){slot_bytes, sizeof slot_bytes}, slots,
                           text(SAVE_WORLD), SAVE_SCHEMA, (coax_v_u8){(uint8_t *)kept, sizeof kept});
  if (rn <= 0) return;
  slots = save_read((coax_string){kept, rn}, (coax_v_u8){slot_bytes, sizeof slot_bytes});
  keep_n = (uint32_t)rn;
  realm.action = 4;
  house.dirty = false;
  kept_at = now;
  loaded_at = at;
  if (has_account) {
    int64_t pn = save_post_body((coax_v_u8){container, cn}, cn, at, (coax_v_u8){(uint8_t *)post, sizeof post});
    if (pn > 0) coaxial_http((uint32_t)(uintptr_t)PROGRESS_URL, sizeof PROGRESS_URL - 1, (uint32_t)(uintptr_t)post, (uint32_t)pn, 1 | 8);
  }
}

// The page facts' account (a line "account=..."): the player joined with an email.
static void read_account(uint32_t length) {
  const char *p = (const char *)inbox, *end = p + length;
  for (const char *line = p; line < end;) {
    const char *nl = memchr(line, '\n', (size_t)(end - line));
    if (!nl) nl = end;
    if (nl - line > 8 && !memcmp(line, "account=", 8)) has_account = true;
    if (nl - line > 7 && !memcmp(line, "player=", 7)) has_player = true;
    line = nl + 1;
  }
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
  if (house.world_id.count == 7 && !memcmp(house.world_id.data, "sandbox", 7)) {
    sandbox = true; save_world = "sandbox"; link = "https://openagents.com/realms/sandbox"; items = sandbox_items;
    house.quest = 8;   // no quest here: done, and the HUD shows none
    world_open(text("sandbox"));
  }
  if (kept_n && !world_holds()) restore_kept();
  for (int i = 0; i < 5; i++) item_mesh[i] = verse_haunt_mesh(house, text(items[i].mesh));
  player = sandbox ? verse_player_place(0, 0, -2, 0) : verse_player_place(7, 0, 2, 0);
  player.phi = 1.5707963f;
  sticks = verse_sticks_reset();
  opened = true;
  apply_server();   // the server's save, if it came first and is newer
}

static void do_goal(void);
static void follow_talk(void);

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
  else { house.camera = edit_camera; player.phi = house.camera == 0 ? 1.5707963f : 1.107f; house.selected = -1; world_edited(); }
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
  memcpy(realm.outbox.e, link, strlen(link));
  realm.out_n = (int64_t)strlen(link);
  coax_string w = world_name();
  if (world_holds() && realm.out_n + 1 + w.count <= (int64_t)sizeof realm.outbox.e) {
    realm.outbox.e[realm.out_n++] = '#';
    memcpy(realm.outbox.e + realm.out_n, w.data, (size_t)w.count);
    realm.out_n += w.count;
  }
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
  case 3:
    house = verse_haunt_quest(house, text("publish"));
    if (world_holds()) { house.dirty = false; world_send(); break; }
    keep(true); toast(has_account ? "Saved, here and on the server" : "Saved in this browser"); break;
  case 4: copy_link(); house = verse_haunt_quest(house, text("share")); toast("Link copied"); break;
  case 5: house = verse_haunt_fork(house, text("My Haunted House"), text("mine")); toast("This house is yours now"); break;
  case 6: copy_link(); house = verse_haunt_quest(house, text("invite")); toast("Invite link copied"); break;
  case 7: realm_ask_text(realm_of()); break;
  }
}

// The survey's answer to the server's feedback (realm_answer_body, pt-feedback).
#define ANSWER_URL "/api/playtest/survey/answer"
static void post_answer(size_t n) {
  static uint8_t body[4096];
  int64_t o = realm_answer_body(tr(), (coax_string){feedback, (int64_t)n}, (coax_v_u8){body, sizeof body});
  if (o > 0) coaxial_http((uint32_t)(uintptr_t)ANSWER_URL, sizeof ANSWER_URL - 1, (uint32_t)(uintptr_t)body, (uint32_t)o, 1);
}

// The survey's answer: kept with the house, and the quest is done.
static void send_feedback(void) {
  coax_string typed = realm_typed(realm_of());
  size_t n = typed.count < (int64_t)sizeof feedback ? (size_t)typed.count : sizeof feedback;
  memcpy(feedback, typed.data, n);
  house = verse_haunt_feedback(house, (coax_string){feedback, (int64_t)n});
  realm_track_answer(tr(), (coax_string){feedback, (int64_t)n});
  post_answer(n);
  realm_close_text(realm_of());
  toast("Thank you. The quest is complete");
}

// ---- talking (realm.coax realm_npc_body, realm_npc_text)

// A new conversation (or none): the realm forgets the last, a question
// still out is let go, and the typed box for whoever it was closes.
static void follow_talk(void) {
  if (house.talking == talking_was) return;
  talking_was = house.talking;
  realm_forget(realm_of());
  npc_ask = -1;
  npc_spoken = false;
  int64_t typing_for = realm_typing_for(realm_of());
  if (realm_wants_text(realm_of()) && (typing_for == 1 || typing_for == 2)) realm_close_text(realm_of());
  greet = house.talking >= 0 ? house.reply : (coax_string){"", 0};
  if (house.talking >= 0) realm_remember(realm_of(), true, greet);
}

// Their answer, spoken when VOICE asked it.
static void npc_say(coax_string words) {
  if (!npc_spoken) return;
  npc_spoken = false;
  say_n = words.count < (int64_t)sizeof say_text ? (size_t)words.count : sizeof say_text;
  memcpy(say_text, words.data, say_n);
  say_waiting = say_n > 0;
}

// The house's own lines answer.
static void npc_scripted(void) {
  house = verse_haunt_ask(house, (coax_string){question, (int64_t)question_n});
  if (house.talking >= 0) realm_remember(realm_of(), true, house.reply);
  npc_say(house.reply);
  follow_talk();   // "bye" ends it
}

static void ask_npc(coax_string typed, bool spoken) {
  npc_spoken = spoken;
  if (house.talking < 0 || house.talking >= house.people_count) return;
  question_n = typed.count < (int64_t)sizeof question ? (size_t)typed.count : sizeof question;
  memcpy(question, typed.data, question_n);
  coax_string q = {question, (int64_t)question_n};
  coax_t_HauntPerson p = people[house.talking];
  int64_t n = realm_npc_body(realm_of(), p.name, p.role, p.knowledge, greet, q, (coax_v_u8){npc_body, sizeof npc_body});
  realm_remember(realm_of(), false, q);
  npc_ask = n > 0 ? coaxial_http((uint32_t)(uintptr_t)NPC_URL, sizeof NPC_URL - 1, (uint32_t)(uintptr_t)npc_body, (uint32_t)n, HTTP_ASK) : -1;
  if (npc_ask < 0) { realm_track_ask(tr(), p.name, q, false); npc_scripted(); return; }
  npc_who = house.talking;
  npc_asked_at = now;
  house.reply = text("...");   // they think
}

// The server's answer ("STATUS\n" then the body): their words, else their own lines.
static void take_answer(uint32_t length) {
  npc_ask = -1;
  if (npc_who < 0 || npc_who >= house.people_count) return;
  const char *p = (const char *)inbox, *nl = memchr(p, '\n', length);
  int status = 0;
  for (const char *c = p; nl && c < nl && *c >= '0' && *c <= '9'; c++) status = status * 10 + (*c - '0');
  bool ok = false;
  if (status == 200 && house.talking == npc_who) {
    int64_t n = realm_npc_text((coax_string){nl + 1, (int64_t)(p + length - nl - 1)}, (coax_v_u8){(uint8_t *)answer_text, sizeof answer_text});
    if (n > 0) { house.reply = (coax_string){answer_text, n}; realm_remember(realm_of(), true, house.reply); npc_say(house.reply); ok = true; }
  }
  realm_track_ask(tr(), people[npc_who].name, (coax_string){question, (int64_t)question_n}, ok);
  if (!ok && house.talking == npc_who) npc_scripted();
}

// Nothing back in time: their own lines, and a late answer is let go.
static void npc_waited(void) {
  if (npc_ask < 0 || now - npc_asked_at < NPC_WAIT) return;
  npc_ask = -1;
  if (npc_who >= 0 && npc_who < house.people_count) realm_track_ask(tr(), people[npc_who].name, (coax_string){question, (int64_t)question_n}, false);
  if (house.talking == npc_who) npc_scripted();
}

// TEACH: the lesson is added to what they know (a sentence of its own),
// into the buffer their knowledge is in, else one nobody's is in; undo takes it back.
static void teach(int64_t who, coax_string lesson) {
  int64_t a = 0, b = lesson.count;
  while (a < b && lesson.data[a] == ' ') a++;
  while (b > a && lesson.data[b - 1] == ' ') b--;
  if (a == b || who < 0 || who >= house.people_count) return;
  static char next[4096];
  coax_string had = people[who].knowledge;
  size_t m = had.count > 0 ? (size_t)had.count : 0;
  if (m > sizeof next - 2) m = sizeof next - 2;
  memcpy(next, had.data, m);
  if (m) next[m++] = ' ';
  size_t k = (size_t)(b - a) < sizeof next - 1 - m ? (size_t)(b - a) : sizeof next - 1 - m;
  memcpy(next + m, lesson.data + a, k);
  m += k;
  char last = next[m - 1];
  if (last != '.' && last != '!' && last != '?') next[m++] = '.';
  int slot = -1;
  for (int s = 0; s < TAUGHT && slot < 0; s++) {
    int64_t users = 0; bool theirs = false;
    for (int64_t i = 0; i < house.people_count; i++)
      if (people[i].knowledge.data >= taught[s] && people[i].knowledge.data < taught[s] + sizeof taught[s]) { users++; theirs |= i == who; }
    if (theirs && users == 1) slot = s;
  }
  for (int s = 0; s < TAUGHT && slot < 0; s++) {
    bool used = false;
    for (int64_t i = 0; i < house.people_count; i++)
      if (people[i].knowledge.data >= taught[s] && people[i].knowledge.data < taught[s] + sizeof taught[s]) used = true;
    if (!used) slot = s;
  }
  if (slot < 0) { toast("They can't take in any more today"); return; }
  memcpy(taught[slot], next, m);
  remember();
  house = verse_haunt_teach(house, who, (coax_string){taught[slot], (int64_t)m});
  realm_track_teach(tr(), people[who].name, (coax_string){lesson.data + a, b - a});
  static char said[160];
  snprintf(said, sizeof said, "%.*s will remember that", (int)people[who].name.count, people[who].name.data);
  toast(said);
}

// The sentence to the selector.
static void ask_piece(coax_string typed) {
  if (!sandbox || piece_ask >= 0) { if (piece_ask >= 0) toast("Still making the last one"); return; }
  int64_t n = verse_piece_body(typed, (coax_v_u8){piece_body, sizeof piece_body});
  piece_ask = n > 0 ? coaxial_http((uint32_t)(uintptr_t)PIECE_URL, sizeof PIECE_URL - 1, (uint32_t)(uintptr_t)piece_body, (uint32_t)n, HTTP_ASK) : -1;
  if (piece_ask < 0) { toast("That didn't reach the server; try again"); return; }
  piece_asked_at = now;
  toast("Making it...");
}

// Lays the piece out in front of you: one undo step, then published.
static void make_piece(coax_t_SetPiece p) {
  coax_t_Vec3 feet = verse_player_feet(player);
  uint32_t seed = (uint32_t)(int64_t)wall_ms() * 2654435761u ^ (uint32_t)piece_id_n;
  int64_t n = verse_piece_layout(p, (int64_t)seed, feet.x, feet.z, house.heading, (coax_v_f32){places, sizeof places / sizeof places[0]});
  if (n <= 0) { toast("That isn't one of the Sandbox's set pieces yet"); return; }
  int h0 = history_at;
  remember();
  int64_t added = 0;
  for (int64_t i = 0; i < n && piece_id_n < 1024; i++) {
    const float *o = places + i * 12;
    int64_t m = (int64_t)o[0], before = house.entity_count;
    int64_t mesh = verse_haunt_mesh(house, verse_piece_mesh(m));
    snprintf(piece_ids[piece_id_n], sizeof piece_ids[0], "s%06x-%02d", (unsigned)(seed & 0xffffff), (int)i);
    house = verse_haunt_put(house, mesh, text(piece_ids[piece_id_n]), verse_piece_label(m), (coax_t_HauntPoint){o[1], o[2], o[3]},
                            o[4], o[5], (coax_t_HauntPoint){o[6], o[7], o[8]}, o[9], (int64_t)o[10], o[11] > 0.5f);
    if (house.entity_count > before) { piece_id_n++; added++; }
  }
  if (!added) { history_at = history_n = h0; toast("There's no room there"); return; }   // nothing changed: no undo step
  coax_string said = verse_piece_says(p, (coax_v_u8){(uint8_t *)piece_said, sizeof piece_said - 1});
  piece_said[said.count] = 0;
  rebuild_walls();
  realm_track_place(tr(), said, text("piece"));
  house.selected = -1;
  toast(piece_said);
  snprintf(send_said, sizeof send_said, "Made %c%s in the Sandbox", piece_said[0] >= 'A' && piece_said[0] <= 'Z' ? piece_said[0] - 'A' + 'a' : piece_said[0], piece_said + 1);
  if (world_holds() && world_send()) house.dirty = false;   // one publish, not again at DONE
  send_said[0] = 0;
}

// The selector's answer ("STATUS\n" then {piece, count, kind, colour, size}).
static void take_piece(uint32_t length) {
  piece_ask = -1;
  const char *b = (const char *)inbox, *nl = memchr(b, '\n', length);
  int status = 0;
  for (const char *c = b; nl && c < nl && *c >= '0' && *c <= '9'; c++) status = status * 10 + (*c - '0');
  if (status == 200 && nl) {
    coax_t_SetPiece p = verse_piece_read((coax_string){nl + 1, (int64_t)(b + length - nl - 1)});
    if (p.piece >= 0) { make_piece(p); return; }
  }
  toast(status == 422 ? "The Sandbox can't make that yet: try birds, lanterns, trees or a pond"
      : status == 429 ? "A little slower: one set piece at a time" : "That didn't reach the server; try again");
}
static void piece_waited(void) {
  if (piece_ask < 0 || now - piece_asked_at < PIECE_WAIT) return;
  piece_ask = -1;
  toast("That didn't reach the server; try again");
}

// What's typed, sent: the survey's answer, a question or a lesson.
static void send_typed(void) {
  int64_t kind = realm_typing_for(realm_of());
  if (kind == 0) { send_feedback(); return; }
  coax_string typed = realm_typed(realm_of());
  realm_close_text(realm_of());
  if (kind == 3) { room_say(typed); return; }
  if (kind == 4) { ask_piece(typed); return; }
  if (house.talking < 0) return;
  if (kind == 2) teach(house.talking, typed);
  else ask_npc(typed, false);
}

#include "realm-flight.c"
#include "realm-world.c"
#include "realm-room.c"

static void take_asks(void) {
  if (!opened) return;
  int64_t asks = realm_take(realm_of());
  if (asks & 32) do_goal();
  if (asks & 64) send_typed();
  if (asks & 128) set_editing(!house.editing);
  if ((asks & 256) && house.editing) { remember(); place(realm.slot, "hotbar"); }
  if ((asks & 512) && house.editing) pick(realm.pick_x, realm.pick_y);
  if ((asks & 1024) && house.editing) edit_op(realm.op);
  if (asks & 2048) world_decide(true);
  if (asks & 4096) world_decide(false);
  if (asks & 1) use_nearby();
  if (asks & 2) { house = verse_haunt_view(house); player.phi = house.camera == 0 ? 1.5707963f : 1.107f; }
  if ((asks & 4) && house.talking >= 0) {
    house = verse_haunt_talk(house, house.talking, 1);
    realm_remember(realm_of(), true, house.reply);
  }
  if (asks & 8) house = verse_haunt_talk(house, -1, 0);
  // VOICE heard a question: in a conversation, theirs to answer (the voice host answers the rest as the house).
  if ((asks & 8192) && house.talking >= 0) ask_npc(realm_heard(realm_of()), true);
  // Walked away: the conversation ends.
  if (house.talking >= 0 && !verse_haunt_reach(house, house.talking)) house = verse_haunt_talk(house, -1, 0);
  follow_talk();
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
  npc_waited();
  realm_voice_to(r, house.talking >= 0 && house.talking < house.people_count ? people[house.talking].name : (coax_string){"", 0});
  if (say_waiting && realm_voice_say(r, (coax_string){say_text, (int64_t)say_n})) say_waiting = false;
  piece_waited();
  // A step of the quest is kept at once; other changes at most every 1.5 s.
  static int64_t quest_kept = -1;
  world_frame();
  room_frame(dt);
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
    v.title = area >= 0 ? areas[area].name : sandbox ? text("Sandbox") : (coax_string){"Haunted House", 13};
    bool goal = verse_haunt_at_goal(house);
    v.quest = sandbox ? -1 : house.quest;
    v.no_voice = sandbox;
    v.pieces = sandbox;
    v.quest_line = sandbox ? (coax_string){"", 0} : verse_haunt_step_line(house);
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
    world_describe(&v);
    room_describe(&v);
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

// The house's lights (where this frame put them) and areas, for particles.coax to choose from.
static void choose_particles(float ex, float ey, float ez) {
  int64_t ln = house.light_count < house.lights.count ? house.light_count : house.lights.count;
  int64_t an = house.area_count < house.areas.count ? house.area_count : house.areas.count;
  if (ln > 64) ln = 64;
  if (an > 64) an = 64;
  for (int64_t i = 0; i < ln; i++) {
    coax_t_HauntLight l = house.lights.data[i];
    float *o = lamp_floats + i * 8, *g = lights[i].color.e;
    coax_t_HauntPoint p = verse_haunt_light_flame(house, i);
    bool lit = g[0] + g[1] + g[2] > 0;
    o[0] = p.x; o[1] = p.y; o[2] = p.z; o[3] = l.radius;
    o[4] = l.color.x; o[5] = l.color.y; o[6] = l.color.z; o[7] = lit ? l.flicker : 0;
  }
  for (int64_t i = 0; i < an; i++) {
    coax_t_HauntArea a = house.areas.data[i];
    float *o = area_floats + i * 6;
    o[0] = a.rect.x0; o[1] = a.rect.z0; o[2] = a.rect.x1; o[3] = a.rect.z1; o[4] = a.room ? 0 : 1; o[5] = a.dark ? 1 : 0;
  }
  emitter_n = verse_particles_haunt((coax_v_f32){lamp_floats, ln * 8}, ln, (coax_v_f32){area_floats, an * 6}, an,
    ex, ey, ez, (coax_v_f32){emitters, 24 * 16});
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
  if (golden) u[2] = (float)GOLDEN_FX;   // the shaders' time: the sky, the particles
  if (!track_state.page_n) track_init(tr(), track_cstr("haunt"));
  track_frame(tr(), dt, w, h, scale);
  realm_track(realm_of(), (coax_v_t_RealmSeen){&seen, 1}, tr(), view, house.camera);
  track_drain(&track_state);
  if (opened) {
    coax_t_Vec3 eye = verse_player_view_eye(player, house.camera), at = verse_player_view_at(player, house.camera);
    bool flown = flight_view(&eye, &at, still != 0);
    verse_grid_view(eye.x, eye.y, eye.z, at.x, at.y, at.z, w, h, (coax_v_f32){u + 8, 16});
    draw_count = verse_haunt_instances(house, fx_now(), (coax_v_t_HauntInstance){instances, 4096}, (coax_v_t_HauntDraw){draws, 256});
    light_count = verse_haunt_lights(house, fx_now(), (coax_v_t_HauntGPUlight){lights, 64});
    instance_end = 0;
    for (int64_t i = 0; i < draw_count; i++) if (draws[i].base + draws[i].instances > instance_end) instance_end = draws[i].base + draws[i].instances;
    u[24] = house.eye.x; u[25] = house.eye.y; u[26] = house.eye.z; u[27] = (float)light_count;
    if (flown) { u[24] = eye.x; u[25] = eye.y; u[26] = eye.z; }
    choose_particles(u[24], u[25], u[26]);
    u[28] = now < house.flash_until ? 1.0f : 0.0f;
  }
  return n < 0 ? 0u : (uint32_t)n;
}

// ---- the particles the host draws after the house (particles.wgsl)
EXPORT("coaxial_particle_cap") uint32_t coaxial_particle_cap(void) { return (uint32_t)sizeof emitters; }
EXPORT("coaxial_particles") uint32_t coaxial_particles(uint32_t state) { (void)state; return (uint32_t)(uintptr_t)emitters; }
EXPORT("coaxial_particle_bytes") uint32_t coaxial_particle_bytes(uint32_t state) { (void)state; return (uint32_t)(emitter_n * 16 * sizeof(float)); }
EXPORT("coaxial_particle_instances") uint32_t coaxial_particle_instances(uint32_t state) {
  (void)state;
  return opened ? (uint32_t)(emitter_n * verse_particles_per()) : 0u;
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
  if (opened && flight_skip()) return 1u;
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
  if (phase == 0 && opened && flight_skip()) return 1u;
  if (realm_pointer(realm_of(), view, x, y, phase, kind, id)) { take_asks(); return 1u; }
  if (kind != 0 && opened) sticks = verse_sticks_world_touch(sticks, id, phase, x, y, width, height, dpr, now);
  return kind != 0 ? 1u : 0u;
}
EXPORT("coaxial_wheel") void coaxial_wheel(uint32_t state, float dy) { (void)state; realm_wheel(realm_of(), dy); }
EXPORT("coaxial_cursor") int32_t coaxial_cursor(uint32_t state) { (void)state; return (int32_t)realm_cursor(realm_of()); }

// Output: EXIT's navigation, a copied link, the house to keep.
// The graphics the host draws with (host.js), so GFX names it: 0 HIGH, 1 MEDIUM, 2 LOW.
EXPORT("coaxial_quality") void coaxial_quality(uint32_t state, int32_t q) { (void)state; realm_set_quality(realm_of(), q); }
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
  if (kind == 600) {   // the facts: a player with an email has a save on the server too
    read_account(length);
    facts_seen = true;
    if (has_account && progress_ask < 0)
      progress_ask = coaxial_http((uint32_t)(uintptr_t)PROGRESS_URL, sizeof PROGRESS_URL - 1, 0, 0, HTTP_ANSWER_PLAYER);
  }
  if (progress_ask >= 0 && kind == 512 + progress_ask) { progress_ask = -2; take_server(length); return; }
  if (npc_ask >= 0 && kind == 512 + npc_ask) { take_answer(length); return; }
  if (piece_ask >= 0 && kind == 512 + piece_ask) { take_piece(length); return; }
  if (world_message(kind, length)) return;
  if (room_message(kind, length)) return;
  if (kind >= 512 && kind < 520) return;   // an answer let go

  if (track_message(tr(), kind, (coax_string){(const char *)inbox, length})) { track_drain(&track_state); return; }
  if (kind == 1) realm_type(realm_of(), (coax_string){(const char *)inbox, length});
  if (kind == 50) realm_voice(realm_of(), (coax_string){(const char *)inbox, length});   // coax/web/voice.js
  if (kind == 4 && !opened) take_kept(length);   // restore_kept opens it with the house
}
EXPORT("coaxial_wants_text") int32_t coaxial_wants_text(uint32_t state) { (void)state; return realm_wants_text(realm_of()) ? 1 : 0; }
