// Everglade v2's C face for the browser (coax/web/host.js): scalar Wasm exports over everglade/client/client.coax,
// which says what is round you, where you may stand and what is drawn. It decides nothing.
// The controller steps your body as fivelab-face.c's Player mode does: everglade-step.coax
// (metaverse-engine/move/walk.coax, with player.coax's metaverse_player_*) on the keys and a phone's thumbs
// (metaverse-engine/core/sticks.coax). The scene is three stores: the records (the glade, its grass, your
// character's parts), the clips baked once (into the state, sized by everglade/scene/bodies.coax), and the
// kit pieces in view this frame.
// In a room (everglade/client/net.coax, #13054) the controller steps at the room's rate, 1/30 s, each step's
// input sent; the room's word on you starts you again where it has you and re-runs the inputs it
// hasn't stepped, the difference eased out of the draw (as everglade5e-face.c does for Everglade).
// Analytics (analytics.coax, everglade/client/track.coax): the face keeps the page's Track and what e2_track
// saw last frame, hands it the page's messages, keys and pointer, and sends its batches (analytics-face.h).
#include "client.h"     /* everglade/client/client.coax's header */
#include "Everglade-step.h"
#include "Sticks.h"
#include "face-address.h"
#include <stdbool.h>
#include <stddef.h>

#define EXPORT(name) __attribute__((export_name(name)))
#define IMPORT(name) __attribute__((import_module("env"), import_name(name)))
IMPORT("coaxial_socket_open") int32_t coaxial_socket_open(cx_face_addr url, uint32_t len);
IMPORT("coaxial_socket_send") void coaxial_socket_send(int32_t id, cx_face_addr ptr, uint32_t len);
IMPORT("coaxial_socket_close") void coaxial_socket_close(int32_t id);
IMPORT("coaxial_http") int32_t coaxial_http(cx_face_addr url, uint32_t url_len, cx_face_addr body, uint32_t body_len, int32_t flags);
IMPORT("coaxial_now_ms") int64_t coaxial_now_ms(void);

static uint8_t inbox[4096];
static uint8_t catalog[5242880];   /* the pack's catalog: 0.1 MB by cell (#13280), 1 MB in the tracked pack with the ground's record in its G lines (#13059), 4.3 MB in a pack built before the cells with the whole planting in its P lines (#13150) */
static uint8_t ground_record[1179648], catalog_rest[65536];   /* that record decoded (then the far record, #13509), and the catalog without it */
static coax_t_E2Tiles ground_tiles[2];   /* the ground's tile sets, published and staged (everglade/world/ground.coax) */
static coax_t_E2Cells world_cells;   /* the world's cell controller (everglade/client/cells.coax, #13280); the frame reads its levels from the state's view */
/* the world by cell (#13280): a cell's bytes as the host hands them over (everglade/world/cell-content.coax: the
   ground's record's blob and the cell's planting), and the cells the controller has asked for */
static uint8_t cell_bytes[2097152];
static int64_t cell_asks[2 * 16], cell_ask_n;
static coax_t_People rig;
static uint32_t baked;
static uint8_t rig_file[262144]; static uint32_t rig_bytes;   /* the pack's rig entry (VRG1, #13306): the people's set */
static uint32_t frames;
static int64_t draw_n;
static coax_t_Body player;
static coax_t_Sticks sticks;
static float width, height, dpr;
static double now;
/* the room (everglade/client/net.coax): the step's clock and the next step's speculation, the seat ticket's
   POST and the player key's mint (POST /api/playtest/player, kept by the host as action 5) */
static float step_acc, step_jump_held;
static coax_t_Body step_next;   /* the step this frame's input will make next: where you are drawn toward (#13167) */
static int64_t joined_ms = -1;
static int32_t ticket_id = -1, mint_id = -1;
static bool has_player, mint_tried, facts_seen;
static int key_wait;   /* frames until a minted key is the host's (action 5 runs after the frame that hands it over) */
static char ticket_url[256]; static uint32_t ticket_url_n;
static char ticket_body[64]; static uint32_t ticket_body_n;
static bool ticket_due;
static int32_t name_id = -1;
static char name_body[128]; static uint32_t name_body_n;
static bool name_due;
static coax_t_Track track_state;
static coax_t_E2Seen track_seen;   /* what e2_track saw last frame */
static bool entered;
static coax_v_t_Track tr(void) { return (coax_v_t_Track){&track_state, 1}; }
#define PLAYER_URL "/api/playtest/player"
#define NAME_URL "/api/playtest/player/name"

static cx_face_addr quality_state;   /* the state the pixel budget reads (coaxial_pixel_budget) */
static coax_string track_cstr(const char *c) { int64_t n = 0; while (c[n]) n++; return (coax_string){c, n}; }
/* The Track's batch to the host's HTTP import, after every call that may have written one (analytics-face.h). */
static void track_drain(void) {
  if (track_state.out_n <= 0) return;
  coax_string url = track_url();
  coaxial_http((cx_face_addr)(uintptr_t)url.data, (uint32_t)url.count, (cx_face_addr)(uintptr_t)track_state.out.e, (uint32_t)track_state.out_n, (int32_t)track_state.flags);
  track_sent(tr());
}
static coax_t_E2 *e2_at(cx_face_addr state) { return (coax_t_E2 *)(uintptr_t)state; }
static coax_v_t_E2 at(cx_face_addr state) { return (coax_v_t_E2){e2_at(state), 1}; }

EXPORT("coaxial_layout") uint32_t coaxial_layout(void) {
  return (uint32_t)(sizeof(coax_t_E2) * 1000003u + offsetof(coax_t_E2, inst) * 13u + 0x45324532u);
}
EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_E2); }
/* The Archery Range's pane (everglade/client/range.coax, apps/openagents-web/range-pane.coax; coax/web/host.js
   "A page's own pane"): its module's name, only on the range's page, and what it is told. */
EXPORT("coaxial_pane") cx_face_addr coaxial_pane(cx_face_addr state) { return (cx_face_addr)(uintptr_t)e2rg_pane(at(state)).data; }
EXPORT("coaxial_pane_len") uint32_t coaxial_pane_len(cx_face_addr state) { return (uint32_t)e2rg_pane(at(state)).count; }
EXPORT("coaxial_pane_rev") int32_t coaxial_pane_rev(cx_face_addr state) { return (int32_t)e2rg_pane_rev(at(state)); }
EXPORT("coaxial_pane_model") cx_face_addr coaxial_pane_model(cx_face_addr state) { return (cx_face_addr)(uintptr_t)e2rg_pane_model(at(state)).data; }
EXPORT("coaxial_pane_model_len") uint32_t coaxial_pane_model_len(cx_face_addr state) { return (uint32_t)e2rg_pane_model(at(state)).count; }
EXPORT("coaxial_state_init") void coaxial_state_init(cx_face_addr state) { coax_default_t_E2((coax_t_E2 *)(uintptr_t)state); e2_ground_store(at(state), (coax_v_t_E2Tiles){ground_tiles, 2}); quality_state = state; }
EXPORT("coaxial_max_vertices") uint32_t coaxial_max_vertices(void) { return (uint32_t)coaxial_e2_max_vertices(); }
EXPORT("coaxial_vertex_floats") uint32_t coaxial_vertex_floats(void) { return (uint32_t)coaxial_e2_vertex_floats(); }
EXPORT("coaxial_uniform_floats") uint32_t coaxial_uniform_floats(void) { return (uint32_t)coaxial_e2_uniform_floats(); }
// The pixels the host draws at most (host.js drawn): the graphics governor's (everglade/client/client.coax e2_pixel_budget).
/* golden mode (host.js coaxial_golden): the governor holds the screen's rung (e2_quality_frame) */
static bool golden;
EXPORT("coaxial_golden") void coaxial_golden(uint32_t on) { golden = on != 0; }
/* the bloom passes the post pass reads this frame (host.js; e2_bloom_passes: the graphics quality's post knob) */
EXPORT("coaxial_bloom_passes_v1") uint32_t coaxial_bloom_passes_v1(cx_face_addr state) { return (uint32_t)e2_bloom_passes(at(state)); }
EXPORT("coaxial_pixel_budget") uint32_t coaxial_pixel_budget(void) {
  if (!quality_state) return 6400000u;
  int64_t px = e2_pixel_budget(at(quality_state), width, height, dpr);
  return px > 0 ? (uint32_t)px : 6400000u;
}

/* The host's instance cull (host.js "Instance culling", everglade/client/gpu/everglade2-cull.coax): the block
   everglade/client/cull.coax fills after each frame's scene. */
static float cull_u[100];
static uint32_t cull_buckets[2 * 36 * 3], cull_runs[4 * 384], cull_draws[384];
static int64_t cull_n;
static int64_t hud_n;   /* the HUD's vertices this frame, for the frame plan */
/* One step of the controller: the world round you, Everglade II's pace, its two halves over the trunks
   and the rocks' hulls, then kept where you may stand. The room's world2_step (coax/web/room/room-world.c) is the same. */
/* ride: the mount that carries you this step seats you (everglade/client/ride.coax: your mount is stepped by
   the same input, as the room's world2_step takes its `ride`); NULL or off, you walk. */
static coax_t_Body step_ride(cx_face_addr state, coax_t_Body p, coax_t_EgMoveIn input, float step, float frame, const coax_t_E2RBody *ride) {
  float fx = p.pos.x, fz = p.pos.z;
  coax_t_EgMoveWorld mw = e2_move_world(at(state), p.pos.x, p.pos.z);
  mw.pace = e2_pace(input.run);
  bool aboard = ride && ride->on;
  if (aboard) { mw.aboard = true; mw.seat_x = ride->x; mw.seat_z = ride->z; mw.seat_yaw = ride->yaw; }
  p = eg_step_begin(p, input, mw, step);
  coax_v_f32 near = e2_move_bodies(at(state)), hulls = e2_move_hulls(at(state));
  p = eg_step_advance_among(p, mw, (coax_v_f32){near.data, mw.bodies * 7}, (coax_v_f32){0, 0}, (coax_v_f32){0, 0}, (coax_v_f32){hulls.data, mw.hulls}, frame);
  if (aboard) return p;   /* a mount keeps itself to the land (everglade/shared/mount-land.coax) */
  coax_t_Point kept = e2_keep_from(at(state), p.pos.x, p.pos.z, fx, fz);
  if (kept.x != p.pos.x || kept.z != p.pos.z) p = metaverse_body_constrain(p, kept.x, kept.z);
  return p;
}
static coax_t_Body step_frame(cx_face_addr state, coax_t_Body p, coax_t_EgMoveIn input, float step, float frame) { return step_ride(state, p, input, step, frame, NULL); }
/* One input's step in a room: input `seq` steps your mount when one carries you (rerun: a re-run of an
   input the room has not stepped), then your body, 1/30 s as the room steps it (room-world.c world2_step). */
static coax_t_Body step_once(cx_face_addr state, coax_t_Body p, coax_t_EgMoveIn input, float dt, int64_t seq, bool rerun) {
  coax_t_E2RBody ride = e2_ride_step(at(state), input, seq, p.pos.x, p.pos.z, rerun);
  return step_ride(state, p, input, dt, dt, &ride);
}
/* The room's commands (everglade/client/net.coax's pipe) to the host, in order: open, send, close, and the
   seat ticket's ask (op 4: the path, a newline, the body). */
static void net_drain(cx_face_addr state) {
  int64_t c = e2_net_pipe(at(state));
  for (int64_t k = 0; k < c; k++) {
    int64_t op = e2_net_pipe_op(at(state), k);
    cx_face_addr from = (cx_face_addr)(uintptr_t)&e2_at(state)->net.e[0].pipe.e[0].buf.e[e2_net_pipe_from(at(state), k)];
    uint32_t len = (uint32_t)e2_net_pipe_len(at(state), k);
    int32_t id = (int32_t)e2_net_pipe_id(at(state), k);
    if (op == 1) e2_net_opened(at(state), coaxial_socket_open(from, len));
    else if (op == 2) coaxial_socket_send(id, from, len);
    else if (op == 3) coaxial_socket_close(id);
    else if (op == 4) {
      const uint8_t *b = (const uint8_t *)(uintptr_t)from;
      uint32_t cut = 0; while (cut < len && b[cut] != '\n') cut++;
      uint32_t bn = cut < len ? len - cut - 1 : 0;
      if (cut < sizeof ticket_url && bn < sizeof ticket_body) {
        for (uint32_t i = 0; i < cut; i++) ticket_url[i] = (char)b[i];
        for (uint32_t i = 0; i < bn; i++) ticket_body[i] = (char)b[cut + 1 + i];
        ticket_url_n = cut; ticket_body_n = bn; ticket_due = true;
      }
    }
    else if (op == 9) {   /* your name's POST (everglade/client/net.coax e2n_rename): its path, a newline, the body */
      const uint8_t *b = (const uint8_t *)(uintptr_t)from;
      uint32_t cut = 0; while (cut < len && b[cut] != '\n') cut++;
      uint32_t bn = cut < len ? len - cut - 1 : 0;
      if (bn < sizeof name_body) { for (uint32_t i = 0; i < bn; i++) name_body[i] = (char)b[cut + 1 + i]; name_body_n = bn; name_due = true; }
      else e2_net_named(at(state), (coax_string){"413\n{}", 6});
    }
  }
  e2_net_pipe_done(at(state));
}
/* The seat ticket's and your name's POSTs, with the player key; a page with no key mints one first.
   The name goes first: the ticket waits for its answer (everglade/client/net.coax e2n_step), so it carries it. */
static void ticket_out(void) {
  if (!facts_seen || (!ticket_due && !name_due)) return;
  if (!has_player) {
    if (!mint_tried) {
      mint_tried = true;
      mint_id = coaxial_http((cx_face_addr)(uintptr_t)PLAYER_URL, sizeof PLAYER_URL - 1, (cx_face_addr)(uintptr_t)"{}", 2, 1 | 4);
      if (mint_id < 0) mint_id = -1;
    }
    return;
  }
  if (mint_id >= 0) return;
  if (name_due && name_id < 0) {
    name_due = false;
    name_id = coaxial_http((cx_face_addr)(uintptr_t)NAME_URL, sizeof NAME_URL - 1, (cx_face_addr)(uintptr_t)name_body, name_body_n, 1 | 4 | 8);
    if (name_id < 0) { name_id = -1; name_due = true; }
  }
  if (!ticket_due || ticket_id >= 0) return;
  ticket_due = false;
  if (!ticket_body_n) { ticket_body[0] = '{'; ticket_body[1] = '}'; ticket_body_n = 2; }
  ticket_id = coaxial_http((cx_face_addr)(uintptr_t)ticket_url, ticket_url_n, (cx_face_addr)(uintptr_t)ticket_body, ticket_body_n, 1 | 4 | 8);
  if (ticket_id < 0) { ticket_id = -1; ticket_due = true; }
}
static bool has_line(uint32_t length, const char *prefix) {
  uint32_t pn = 0; while (prefix[pn]) pn++;
  for (uint32_t i = 0; i + pn <= length; i++) {
    if (i > 0 && inbox[i - 1] != '\n') continue;
    uint32_t j = 0; while (j < pn && inbox[i + j] == (uint8_t)prefix[j]) j++;
    if (j == pn && i + pn < length && inbox[i + pn] != '\n') return true;
  }
  return false;
}

EXPORT("coaxial_frame")
uint32_t coaxial_frame(cx_face_addr state, cx_face_addr vertices, cx_face_addr uniforms, float w, float h, float scale, float dt, uint32_t still) {
  width = w; height = h; dpr = scale; now += dt;
  quality_state = state; e2_quality_frame(at(state), dt, w, h, scale, golden);
  if (e2_start(at(state))) player = metaverse_body_place(e2_at(state)->you_x, 0, e2_at(state)->you_z, e2_at(state)->you_face);
  /* Character Create's run starts from you as the studio drew you (everglade/client/create.coax e2c_walk) */
  if (e2_reseat(at(state))) player = metaverse_body_place(player.pos.x, 0, player.pos.z, e2_at(state)->you_face);
  float frame = dt;
  float step = metaverse_body_counts(player, frame);
  e2_look(at(state), sticks.dyaw, sticks.dpitch, sticks.zoom);
  coax_t_EgMoveIn input = e2_move_in(at(state));
  input.pad_x = sticks.pad_x; input.pad_y = sticks.pad_y;
  input.run = metaverse_runs(input.run, sticks.run, sticks.pad_x, sticks.pad_y);
  input.jump = input.jump || sticks.jump;
  int64_t mono = coaxial_now_ms();
  if (e2_net_live(at(state))) {
    /* the room's rate: each 1/30 s step's input as the wire carries it, sent; the room's word re-runs the rest */
    const float STEP = 1.0f / 30;   /* everglade-wire.coax WIRE_HZ */
    input = e2_net_as_sent(at(state), input);
    if (input.jump) step_jump_held = 1;
    if (e2_net_fix_due(at(state))) {
      coax_t_Vec3 was = eg_step_feet(player);
      coax_t_NetFix f = e2_net_fix(at(state));
      player = eg_step_set(player, f.x, f.y, f.z, f.yaw, f.vx, f.vy, f.vz, f.ground, f.carry, f.jump);
      e2_ride_fix(at(state));   /* your mount is the room's row again before the re-run steps it */
      int64_t again = e2_net_replay_n(at(state));
      for (int64_t k = 0; k < again; k++) player = step_once(state, player, e2_net_replay_input(at(state), k), STEP, e2_net_replay_seq(at(state), k), true);
      coax_t_Vec3 now_at = eg_step_feet(player);
      e2_net_fixed(at(state), now_at.x - was.x, now_at.y - was.y, now_at.z - was.z);
    }
    step_acc += frame > 0.25f ? 0.25f : frame;
    while (step_acc >= STEP) {
      coax_t_EgMoveIn one = input; one.jump = step_jump_held != 0; step_jump_held = 0;
      int64_t seq = e2_net_input(at(state), one);
      player = step_once(state, player, one, STEP, seq, false);
      step_acc -= STEP;
    }
    /* the next step as it will run on this frame's input, not yet sent: drawn toward by the time gone
       of it, so a key shows on the next frame and not up to a step later (#13167). Only drawn: the
       sent steps and the room's re-runs are the only ones that move you. */
    coax_t_EgMoveIn next = input; next.jump = step_jump_held != 0;
    coax_t_E2RBody ride_next = e2_ride_peek(at(state), next, player.pos.x, player.pos.z);   /* a copy's step: your mount is not moved */
    step_next = step_ride(state, player, next, STEP, STEP, &ride_next);
  } else {
    /* the run holds its feet while the room seats you (E2N_HOLD_MS at most); alone, the frame's own step */
    if (joined_ms < 0 && e2_at(state)->net.e[0].joined) joined_ms = mono;
    if (joined_ms >= 0 && e2_net_holding(at(state), mono - joined_ms)) { input.key_x = 0; input.key_y = 0; input.pad_x = 0; input.pad_y = 0; input.jump = false; }
    step_acc = 0; step_jump_held = 0;
    player = step_frame(state, player, input, step, frame);
  }
  coax_t_Vec3 feet = metaverse_body_feet(player);
  if (e2_net_live(at(state))) {
    /* drawn from where the last step left you toward where the next one will, by its time gone, less
       what a re-run moved you, eased out */
    coax_t_Point off = e2_net_offset(at(state), frame);
    feet = eg_step_drawn(eg_step_feet(player), step_next, step_acc * 30);
    feet.x += off.x; feet.y += off.y; feet.z += off.z;
  }
  float face = metaverse_body_face(player);
  if (e2_net_live(at(state))) {
    /* carried by a mount: drawn from where you stood to its saddle and back (everglade/client/ride-view.coax) */
    coax_t_E2RvFeet saddle = e2_ride_feet(at(state));
    if (saddle.on) { feet.x = saddle.x; feet.y = saddle.y; feet.z = saddle.z; face = saddle.face; }
  }
  e2_you(at(state), feet.x, feet.y, feet.z, face);
  sticks = metaverse_sticks_used(sticks);
  coax_v_f32 out = {(float *)(uintptr_t)vertices, coaxial_e2_max_vertices() * coaxial_e2_vertex_floats()};
  coax_v_t_E2U u = {(coax_t_E2U *)(uintptr_t)uniforms, 1};   /* the frame's uniforms, a typed block (everglade/client/gpu/everglade2-uniforms.coax, #13451) */
  int64_t n = e2_frame(at(state), out, u, w, h, scale, dt, still != 0);
  draw_n = 0;
  if (e2_at(state)->bodies.e[0].ready) {
    if (!baked && e2_bake(at(state), (coax_v_t_People){&rig, 1}, (coax_v_u8){rig_file, rig_bytes})) baked = 1;   /* once the pack's rig is in (#13508) */
    /* the world's cells round you and the eye. A pack that came whole has each cell's content when it
       is asked for. One paged by cell (the catalog's Z lines) answers through coaxial_cell_loaded_v1 as
       the host fetches each cell; a cell that left gives its planting back, and the scene waits for
       the cells round you the first time */
    if (e2_cells_due(at(state))) {
      coax_v_t_E2Cells cells = {&world_cells, 1};
      bool paged = e2_paged(at(state));
      float x = e2_at(state)->you_x, z = e2_at(state)->you_z, ex = e2_at(state)->eye.e[0], ez = e2_at(state)->eye.e[2];
      e2cl_page_frame(cells, e2_cell_view(at(state)), x, z, ex, ez, !paged);
      if (paged) e2_cells_settle(at(state), e2cl_near_in(cells, x, z, ex, ez));
    }
    draw_n = e2_world_ready(at(state)) ? e2_scene(at(state)) : 0;
  }
  cull_n = draw_n > 0 ? e2_cull_fill(at(state), u, (coax_v_f32){cull_u, 100}, (coax_v_u32){cull_buckets, sizeof cull_buckets / 4},
                                     (coax_v_u32){cull_runs, sizeof cull_runs / 4}, (coax_v_u32){cull_draws, sizeof cull_draws / 4}) : 0;
  e2_net_step(at(state), mono);
  net_drain(state);
  if (key_wait > 0 && e2_at(state)->player_key_n == 0 && --key_wait == 0) has_player = true;
  ticket_out();
  e2_track(at(state), (coax_v_t_E2Seen){&track_seen, 1}, tr(), draw_n > 0);
  track_frame(tr(), dt, w, h, scale);
  if (n > 0 && !entered && track_state.ready) { entered = true; track_world(tr(), track_cstr("everglade2")); track_loaded(tr(), track_cstr("load-engine")); }
  track_drain();
  frames += 1;
  hud_n = n < 0 ? 0 : n;
  return n < 0 ? 0u : (uint32_t)n;
}

/* The frame's order (metaverse-engine/render/graph/passes.coax, #13303): the host walks these records
   (metaverse-engine/render/graph/frame-plan.coax) and decides no order of its own. */
EXPORT("coaxial_frame_plan_v1") uint32_t coaxial_frame_plan_v1(cx_face_addr state, uint32_t features, cx_face_addr out, uint32_t cap) {
  return (uint32_t)e2_frame_plan(at(state), features, (int64_t)width, (int64_t)height, hud_n, cull_n, (coax_v_u32){(uint32_t *)(uintptr_t)out, cap});
}
// ---- the pack's catalog; the draws and the separate shadow list over the three stores
/* Asset 4 is the dragon's frames (realms.json's data asset after the pack's four parts, #13519): they
   don't fit the pack's reader beside the world, so they come after it, optional (the world never waits
   for them), straight into the baked-clips store past the bakes (everglade/client/ride-view.coax
   e2_dragon_frames), and the store goes to the GPU again. */
#define DRAGON_ASSET 4
static uint32_t dragon_room(void) { return (uint32_t)e2_dragon_room() * 4u; }
EXPORT("coaxial_asset_optional_v1") uint32_t coaxial_asset_optional_v1(int32_t i) { return i == DRAGON_ASSET; }
EXPORT("coaxial_asset") cx_face_addr coaxial_asset(cx_face_addr state, int32_t i, uint32_t bytes) {
  if (i == DRAGON_ASSET) return bytes <= dragon_room() ? (cx_face_addr)(uintptr_t)&e2_at(state)->bake.e[e2_dragon_at()] : 0u;
  return i == 0 && bytes <= sizeof catalog ? (cx_face_addr)(uintptr_t)catalog : 0u;
}
/* the pack's rig entry (kind 7), handed over before the catalog: the bake reads it */
EXPORT("coaxial_pack_rig") cx_face_addr coaxial_pack_rig(cx_face_addr state, uint32_t bytes) { (void)state; return bytes <= sizeof rig_file ? (cx_face_addr)(uintptr_t)rig_file : 0u; }
EXPORT("coaxial_pack_rig_capacity") uint32_t coaxial_pack_rig_capacity(cx_face_addr state) { (void)state; return (uint32_t)sizeof rig_file; }
EXPORT("coaxial_pack_rig_ready") void coaxial_pack_rig_ready(cx_face_addr state, uint32_t bytes) { (void)state; rig_bytes = bytes <= sizeof rig_file ? bytes : 0u; }
/* the same for a native host (metaverse-engine/host/README.md: coaxial_rig_v1, #13508) */
EXPORT("coaxial_rig_v1") cx_face_addr coaxial_rig_v1(cx_face_addr state, uint32_t bytes) { return coaxial_pack_rig(state, bytes); }
EXPORT("coaxial_rig_ready_v1") void coaxial_rig_ready_v1(cx_face_addr state, uint32_t bytes) { coaxial_pack_rig_ready(state, bytes); }
EXPORT("coaxial_asset_capacity") uint32_t coaxial_asset_capacity(cx_face_addr state, int32_t i) { (void)state; return i == 0 ? (uint32_t)sizeof catalog : i == DRAGON_ASSET ? dragon_room() : 0u; }
EXPORT("coaxial_asset_ready") void coaxial_asset_ready(cx_face_addr state, int32_t i, uint32_t bytes) {
  if (i == DRAGON_ASSET) { if (bytes > 0 && bytes <= dragon_room()) (void)e2_dragon_frames(at(state), (int64_t)bytes); return; }
  if (i == 0 && bytes > 0 && bytes <= sizeof catalog) e2_catalog(at(state), (coax_string){(const char *)catalog, bytes}, (coax_v_u8){ground_record, sizeof ground_record}, (coax_v_u8){catalog_rest, sizeof catalog_rest});
  else if (i == 0) e2_plant_due(at(state));   /* no catalog: the woods are planted here */
}
/* The world by cell (coax/web/host.js pumpCells, #13280). The host asks which cells wait for their content
   (asks: how many; ask: field 0 the cell, 1 its request, 2 its key for the catalog's line), fetches each
   one's ranges, copies them joined to the buffer and says so. The answer is admitted only for the cell's
   current request at the manifest's digest (everglade/client/cells.coax e2cl_loaded), and only then is the
   content taken; bytes the page can't take are a failed load, which the controller asks for again. */
EXPORT("coaxial_cell_asks_v1") uint32_t coaxial_cell_asks_v1(cx_face_addr state) {
  cell_ask_n = e2_paged(at(state)) ? e2cl_asked((coax_v_t_E2Cells){&world_cells, 1}, (coax_v_s64){cell_asks, 32}) : 0;
  return (uint32_t)cell_ask_n;
}
EXPORT("coaxial_cell_ask_v1") uint32_t coaxial_cell_ask_v1(cx_face_addr state, int32_t i, int32_t field) {
  (void)state;
  if (i < 0 || i >= cell_ask_n) return 0u;
  if (field == 2) return (uint32_t)e2cl_key((coax_v_t_E2Cells){&world_cells, 1}, cell_asks[i * 2]);
  return (uint32_t)cell_asks[i * 2 + (field == 1)];
}
EXPORT("coaxial_cell_buffer_v1") cx_face_addr coaxial_cell_buffer_v1(cx_face_addr state, uint32_t bytes) { (void)state; return bytes <= sizeof cell_bytes ? (cx_face_addr)(uintptr_t)cell_bytes : 0u; }
EXPORT("coaxial_cell_loaded_v1") int32_t coaxial_cell_loaded_v1(cx_face_addr state, int32_t cell, int32_t request, uint32_t digest, uint32_t bytes, uint32_t failed) {
  coax_v_u8 content = {cell_bytes, bytes <= sizeof cell_bytes ? bytes : 0};
  bool good = !failed && bytes > 0 && bytes <= sizeof cell_bytes && e2_cell_checks(at(state), cell, content);
  int64_t said = e2cl_loaded((coax_v_t_E2Cells){&world_cells, 1}, cell, request, good ? (int64_t)digest : 0, !good);
  if (said == 0 && good) e2_cell_take(at(state), cell, content);   /* CELL_OK */
  return (int32_t)said;
}
EXPORT("coaxial_draws") cx_face_addr coaxial_draws(cx_face_addr state) { return (cx_face_addr)(uintptr_t)e2_at(state)->draws.e; }
EXPORT("coaxial_draw_count") uint32_t coaxial_draw_count(cx_face_addr state) { (void)state; return draw_n > 0 ? (uint32_t)draw_n : 0u; }
EXPORT("coaxial_shadow_draws_v1") cx_face_addr coaxial_shadow_draws_v1(cx_face_addr state) { return (cx_face_addr)(uintptr_t)e2_at(state)->shadow.e; }
EXPORT("coaxial_shadow_draw_count_v1") uint32_t coaxial_shadow_draw_count_v1(cx_face_addr state) { return draw_n > 0 ? (uint32_t)e2_at(state)->shadow_n : 0u; }
/* The key light's shadow tiles (metaverse-engine/render/graph/cascades.coax, everglade/client/client.coax e2_shadows; coax/web/host.js shadowLayout). */
EXPORT("coaxial_shadow_atlas_v1") uint32_t coaxial_shadow_atlas_v1(cx_face_addr state, int32_t coarse, int32_t which) { return (uint32_t)e2_shadow_atlas(at(state), coarse, which); }
EXPORT("coaxial_shadow_v1") int32_t coaxial_shadow_v1(cx_face_addr state, int32_t field) { return (int32_t)e2_shadow_field(at(state), field); }
EXPORT("coaxial_shadow_event_v1") void coaxial_shadow_event_v1(cx_face_addr state, int32_t kind, int32_t arg) { e2_shadow_event(at(state), kind, arg); }
EXPORT("coaxial_layers_side_v1") uint32_t coaxial_layers_side_v1(cx_face_addr state) { return (uint32_t)e2_layers_side(at(state)); }
/* store 4: the ground's material table and baked AO (everglade/world/ground.coax e2g_gpu, #13052) */
EXPORT("coaxial_stores") uint32_t coaxial_stores(void) { return 6; }
/* the pack's data layers (normal and ORM maps) bound after the colour layers' sampler (host.js) */
EXPORT("coaxial_data_layers_v1") uint32_t coaxial_data_layers_v1(void) { return 1; }
EXPORT("coaxial_frame_store_cap") uint32_t coaxial_frame_store_cap(void) { return (uint32_t)sizeof(((coax_t_E2 *)0)->frame.e); }
EXPORT("coaxial_frame_store") cx_face_addr coaxial_frame_store(cx_face_addr state) { return (cx_face_addr)(uintptr_t)e2_at(state)->frame.e; }
EXPORT("coaxial_frame_store_bytes") uint32_t coaxial_frame_store_bytes(cx_face_addr state) { (void)state; return (uint32_t)sizeof(((coax_t_E2 *)0)->frame.e); }
/* The retained scene (#13303). Store 2 is the pieces drawn, a word each (their row of store 5 and
   fade code), in three regions on the GPU: the page's picked pieces, then the cull's two phases'
   survivors (everglade/client/gpu/everglade2-cull.coax E2Q_OUT1, E2Q_OUT2). Store 5 is every planted
   piece's row, copied once (its version moves only when the woods are planted). */
EXPORT("coaxial_store_cap") uint32_t coaxial_store_cap(int32_t k) {
  if (k == 4) return (uint32_t)sizeof(((coax_t_E2 *)0)->eg.e[0].gpu.e);
  if (k == 5) return (uint32_t)sizeof(((coax_t_E2 *)0)->items.e);
  return k == 0 ? (uint32_t)sizeof(((coax_t_E2 *)0)->records.e) : k == 1 ? (uint32_t)sizeof(((coax_t_E2 *)0)->bake.e) : k == 3 ? (uint32_t)sizeof(((coax_t_E2 *)0)->mats.e) : 3u * (uint32_t)sizeof(((coax_t_E2 *)0)->inst.e);
}
/* The instance cull's block (host.js "Instance culling"): field 0 the runs this frame, 1 the
   uniforms (100 floats), 2 the runs' words (4 each), 3 each kit piece's first index and index
   count, 4 the kit pieces, 5 the store the pieces' words are in, 6 their base instance, 7 each
   run's index in the draw list, 8 the store their rows are in. */
EXPORT("coaxial_cull_v1") int32_t coaxial_cull_v1(cx_face_addr state, int32_t field) {
  (void)state;
#if defined(COAXIAL_NATIVE_ADDRESS)
  /* this block hands its addresses back as 32-bit words, which a native address doesn't fit: no
     runs, so a native host draws every piece the page picked (as the browser does with ?cull=0) */
  if (field == 0) return 0;
  if (field == 1 || field == 2 || field == 3 || field == 7) return -1;
#endif
  switch (field) {
    case 0: return (int32_t)cull_n;
    case 1: return (int32_t)(uintptr_t)cull_u;
    case 2: return (int32_t)(uintptr_t)cull_runs;
    case 3: return (int32_t)(uintptr_t)cull_buckets;
    case 4: return (int32_t)cull_u[97];   /* E2Q_BUCKETS */
    case 5: return 2;
    case 6: return 1048576;   /* everglade/client/client.coax E2_KIT_BASE */
    case 7: return (int32_t)(uintptr_t)cull_draws;
    case 8: return 5;
  }
  return -1;
}
EXPORT("coaxial_store") cx_face_addr coaxial_store(cx_face_addr state, int32_t k) {
  if (k == 4) return (cx_face_addr)(uintptr_t)e2_at(state)->eg.e[0].gpu.e;
  if (k == 5) return (cx_face_addr)(uintptr_t)e2_at(state)->items.e;
  return k == 0 ? (cx_face_addr)(uintptr_t)e2_at(state)->records.e : k == 1 ? (cx_face_addr)(uintptr_t)e2_at(state)->bake.e : k == 3 ? (cx_face_addr)(uintptr_t)e2_at(state)->mats.e : (cx_face_addr)(uintptr_t)e2_at(state)->inst.e;
}
EXPORT("coaxial_store_bytes") uint32_t coaxial_store_bytes(cx_face_addr state, int32_t k) {
  if (k == 0) { uint32_t r = (uint32_t)e2_record_bytes(at(state)); return r ? r : 96u; }
  if (k == 5) return (uint32_t)e2_items_bytes(at(state));
  if (k == 1) return (uint32_t)sizeof(((coax_t_E2 *)0)->bake.e);
  if (k == 3) return (uint32_t)sizeof(((coax_t_E2 *)0)->mats.e);
  if (k == 4) return (uint32_t)sizeof(((coax_t_E2 *)0)->eg.e[0].gpu.e);
  uint32_t n = (uint32_t)e2_inst_bytes(at(state)); return n ? n : 4u;
}
/* Of store 0, the records, only those that changed since the host last took them (host.js
   coaxial_store_span_v1, metaverse-engine/render/graph/retained-rows.coax, #13540): field 0 takes the span and
   answers its first byte, field 1 its bytes. Every other store has no span: it goes whole. */
EXPORT("coaxial_store_span_v1") uint32_t coaxial_store_span_v1(cx_face_addr state, int32_t k, int32_t field) {
  if (k != 0) return field == 1 ? 0xFFFFFFFFu : 0u;
  return (uint32_t)e2_record_span(at(state), field);
}
/* store 1 goes again when the people's clips are baked and when the dragon's frames come */
EXPORT("coaxial_store_version") uint32_t coaxial_store_version(cx_face_addr state, int32_t k) { return k == 1 ? baked + (uint32_t)e2_at(state)->rv.e[0].streamed : k == 3 ? (uint32_t)e2_at(state)->mats_v : k == 4 ? (uint32_t)e2_at(state)->eg.e[0].gpu_v : k == 5 ? (uint32_t)e2_items_version(at(state)) : frames; }

/* The host's keyboard field over Character Create's NAME while it takes keys (host.js placeField). */
static float field_box[4];
EXPORT("coaxial_wants_text") uint32_t coaxial_wants_text(cx_face_addr state) { return e2_wants_text(at(state)) ? 1u : 0u; }
EXPORT("coaxial_field") cx_face_addr coaxial_field(cx_face_addr state) { e2_text_field(at(state), (coax_v_f32){field_box, 4}); return (cx_face_addr)(uintptr_t)field_box; }
EXPORT("coaxial_key") uint32_t coaxial_key(cx_face_addr state, int32_t key, int32_t mods) {
  e2_track_key(at(state), tr(), key);   /* in Character Create; in the world only walking counts */
  track_drain();
  return e2_key(at(state), key, mods) ? 1u : 0u;
}
EXPORT("coaxial_key_up") uint32_t coaxial_key_up(cx_face_addr state, int32_t key, int32_t mods) { track_key_up(tr(), key); return e2_key_up(at(state), key, mods) ? 1u : 0u; }
EXPORT("coaxial_blur") void coaxial_blur(cx_face_addr state) { e2_blur(at(state)); sticks = metaverse_sticks_reset(); track_blur(tr()); }
/* Metaverse Engine host contract v1 (metaverse-engine/host/README.md). The face has what v1 asks of a face: addresses
   of the host's width (face-address.h), focus as a call of its own, and the whole allocations its
   action, media and sound-bank bytes are in. A native build says so. The browser's module does not
   yet: there v1 also means the shared transport and lifecycle adapter (coax/web/render-face.c,
   which a page gets with its render carrier), and host.js refuses a module that says 1 without it. */
#if defined(COAXIAL_NATIVE_ADDRESS)
EXPORT("coaxial_host_contract_version") uint32_t coaxial_host_contract_version(void) { return 1; }
#endif
/* Nothing is held across a blur, so focus has nothing to restore. */
EXPORT("coaxial_focus") void coaxial_focus(cx_face_addr state) { (void)state; }
EXPORT("coaxial_pointer") uint32_t coaxial_pointer(cx_face_addr state, float x, float y, int32_t phase, int32_t kind, int32_t id) {
  e2_track_pointer(at(state), tr(), x, y, phase, kind); track_drain();
  if (e2_pointer(at(state), x, y, phase, kind, id)) return 1u;
  // a finger is a thumb's: the stick, the look, a tap's jump
  if (kind != 0) { sticks = metaverse_sticks_world_touch(sticks, id, phase, x, y, width, height, dpr, now); return 1u; }
  return 0u;
}
EXPORT("coaxial_pointer_buttons_v1") uint32_t coaxial_pointer_buttons_v1(cx_face_addr state, float x, float y, int32_t phase, int32_t kind, int32_t id, int32_t button) {
  e2_track_pointer(at(state), tr(), x, y, phase, kind); track_drain();
  if (e2_pointer_buttons(at(state), x, y, phase, kind, id, button)) return 1u;
  if (kind != 0) { sticks = metaverse_sticks_world_touch(sticks, id, phase, x, y, width, height, dpr, now); return 1u; }
  return 0u;
}
EXPORT("coaxial_wheel") uint32_t coaxial_wheel(cx_face_addr state, float dy) { return e2_wheel(at(state), dy) ? 1u : 0u; }
/* Mouselook (metaverse-engine/cam/mouselook.coax): host.js holds the pointer when this says 2 and forwards the mouse's travel. */
EXPORT("coaxial_mouselook") int32_t coaxial_mouselook(cx_face_addr state) { return (int32_t)e2_mouselook(at(state)); }
EXPORT("coaxial_look_delta") void coaxial_look_delta(cx_face_addr state, float dx, float dy) { e2_look_delta(at(state), dx, dy); }
EXPORT("coaxial_pointer_lock") void coaxial_pointer_lock(cx_face_addr state, int32_t status) { e2_pointer_lock(at(state), status); }
EXPORT("coaxial_cursor") int32_t coaxial_cursor(cx_face_addr state) { return (int32_t)e2_cursor(at(state)); }

EXPORT("coaxial_inbox") cx_face_addr coaxial_inbox(cx_face_addr state) { (void)state; return (cx_face_addr)(uintptr_t)inbox; }
EXPORT("coaxial_inbox_cap") uint32_t coaxial_inbox_cap(void) { return sizeof inbox; }
// Everglade's HUD is the FANTASY style (metaverse-engine/ui/hud-style.coax): its strokes blend straight alpha, each
// sealed with its coverage (charactercreator.coax hud_seal, everglade/scene/gpu/fivelab.coax fivelab_hud_fs), so dark
// iron can sit over the world.
EXPORT("coaxial_fx_alpha_v1") uint32_t coaxial_fx_alpha_v1(void) { return 1; }
EXPORT("coaxial_message") void coaxial_message(cx_face_addr state, int32_t kind, uint32_t length) {
  if (length > sizeof inbox) return;
  if (!track_state.page_n) track_init(tr(), track_cstr("everglade2"));
  if (track_message(tr(), kind, (coax_string){(const char *)inbox, length})) track_drain();
  if (kind == 600) { facts_seen = true; if (has_line(length, "player=")) has_player = true; }
  if (mint_id >= 0 && kind == 512 + mint_id) {
    mint_id = -1;
    if (length > 4 && inbox[0] == '2' && inbox[1] == '0' && inbox[2] == '0' && inbox[3] == '\n') {
      e2_keep_player(at(state), (coax_string){(const char *)inbox + 4, length - 4});
      key_wait = 2;   /* the POSTs that carry the key wait until the host keeps it (X-Verse-Player) */
    } else if (name_due) { name_due = false; e2_net_named(at(state), (coax_string){"503\n{}", 6}); }   /* no player: the name can't go */
    return;
  }
  if (ticket_id >= 0 && kind == 512 + ticket_id) { ticket_id = -1; e2_net_ticket(at(state), (coax_string){(const char *)inbox, length}); return; }
  if (name_id >= 0 && kind == 512 + name_id) { name_id = -1; e2_net_named(at(state), (coax_string){(const char *)inbox, length}); return; }
  e2_message(at(state), kind, (coax_string){(const char *)inbox, length});
  if (kind >= 256 && kind < 512) net_drain(state);
}
// Character Create's actions (everglade/client/create.coax): 16 keeps the record, 19 says the address.
/* the page's hash (host.js sendHash): #fly=PATH hands the camera to the flyover benchmark (e2_hash) */
EXPORT("coaxial_hash") void coaxial_hash(cx_face_addr state, uint32_t length) {
  if (length <= sizeof inbox) e2_hash(at(state), (coax_string){(const char *)inbox, length});
}
EXPORT("coaxial_action") int32_t coaxial_action(cx_face_addr state) { return (int32_t)e2_action(at(state)); }
EXPORT("coaxial_outbox") cx_face_addr coaxial_outbox(cx_face_addr state) { return (cx_face_addr)(uintptr_t)e2_at(state)->outbox.e; }
EXPORT("coaxial_outbox_len") uint32_t coaxial_outbox_len(cx_face_addr state) { return (uint32_t)e2_at(state)->out_n; }
/* the allocations those bytes and the media packet's are in, whole (host contract v1) */
EXPORT("coaxial_outbox_capacity") uint32_t coaxial_outbox_capacity(cx_face_addr state) { return (uint32_t)sizeof e2_at(state)->outbox.e; }
// The class pane's picture for the host to draw (coax/web/host.js coaxial_media): everglade/client/create.coax e2c_media.
EXPORT("coaxial_media") cx_face_addr coaxial_media(cx_face_addr state) { return (cx_face_addr)(uintptr_t)e2_at(state)->media.e; }
EXPORT("coaxial_media_len") uint32_t coaxial_media_len(cx_face_addr state) { return (uint32_t)e2_at(state)->media_n; }
EXPORT("coaxial_media_capacity") uint32_t coaxial_media_capacity(cx_face_addr state) { return (uint32_t)sizeof e2_at(state)->media.e; }
EXPORT("coaxial_action_done") void coaxial_action_done(cx_face_addr state) { e2_action_done(at(state)); }

// Your saved character arrives as message 16 (the browser's own copy, from Character Create).
EXPORT("coaxial_player_appearance_v1") uint32_t coaxial_player_appearance_v1(void) { return 1u; }
/* The agent driver (metaverse-engine/agent/drive.coax, coax/web/host.js coaxial.drive): the answer to the last command (message 700). */
EXPORT("coaxial_drive_out") cx_face_addr coaxial_drive_out(cx_face_addr state) { return (cx_face_addr)(uintptr_t)e2_drive_out(at(state)).data; }
EXPORT("coaxial_drive_out_len") uint32_t coaxial_drive_out_len(cx_face_addr state) { return (uint32_t)e2_drive_out(at(state)).count; }
EXPORT("coaxial_field_probe") int32_t coaxial_field_probe(cx_face_addr state, int32_t field) { return (int32_t)e2_probe(at(state), field); }

// ---- the casts (#13058, everglade/client/cast.coax): their effects, drawn after the scene and depth-tested
// against it (everglade2-fx.wgsl, host.js particles: puffs of 12 floats), and their sounds, the
// host's to play once each (sounds:/realms/everglade/sfx/ and the bank Experimental renders).
EXPORT("coaxial_particle_cap") uint32_t coaxial_particle_cap(void) { return (uint32_t)sizeof(((coax_t_E2 *)0)->casts.e[0].puffs.e[0].out); }
EXPORT("coaxial_particles") cx_face_addr coaxial_particles(cx_face_addr state) { return (cx_face_addr)(uintptr_t)e2_at(state)->casts.e[0].puffs.e[0].out.e; }
EXPORT("coaxial_particle_bytes") uint32_t coaxial_particle_bytes(cx_face_addr state) { return (uint32_t)(e2_at(state)->casts.e[0].puffs.e[0].out_n * 12 * sizeof(float)); }
EXPORT("coaxial_particle_instances") uint32_t coaxial_particle_instances(cx_face_addr state) { return (uint32_t)e2_at(state)->casts.e[0].puffs.e[0].out_n; }
EXPORT("coaxial_sounds") cx_face_addr coaxial_sounds(cx_face_addr state) { return (cx_face_addr)(uintptr_t)e2_at(state)->casts.e[0].audio.e[0].lines.e; }
EXPORT("coaxial_sounds_capacity") uint32_t coaxial_sounds_capacity(cx_face_addr state) { return sizeof e2_at(state)->casts.e[0].audio.e[0].lines.e; }
EXPORT("coaxial_sounds_len") uint32_t coaxial_sounds_len(cx_face_addr state) { coax_t_ExperimentalSound *audio = &e2_at(state)->casts.e[0].audio.e[0]; uint32_t n = (uint32_t)audio->n; audio->n = 0; return n; }
#include "experimental-sound-bank.h"
