/* The everglade rooms' tick, which both of the room server's hosts include (GitHub #12784; Aethyr
   book chapter 95): the native host (room-host.c, oa-room over TCP) and the browser's local host
   (room-worker.c, offline play in a Web Worker). A new server step goes here, not in one host, or the
   other host's game misses it. The host includes this after room.h and room-world.h and defines
   view(), now_us() and wall_ms(); it moves the frames rm_eg_send queued (relay_flush) itself. */
/* Where the everglade tick's time goes (#12717): microseconds summed over the report's minute for the
   movement steps, rm_eg_rules (the rules' step), rm_eg_send (every seat's D written) and the frames sent
   (relay_flush, pump_all), printed with the tick line; each tick's are the room view's workloads too
   (room-stats.h). abilities, town, spells and people were v1's parts: they stay in the report at 0
   until the report's columns change (#13267). */
/* the trunks the guard is handed each tick: everglade/server/guard.coax E2G_NEAR */
#define E2G_NEAR_C 24
enum { PART_MOVE, PART_ABILITIES, PART_TOWN, PART_SPELLS, PART_PEOPLE, PART_RULES, PART_WRITE, PART_SEND, PARTS };
static int64_t part_us[PARTS];
#ifdef COAX_ROOM_PROFILE
static int64_t profile_us[7];
#define ROOM_PROFILE_PHASE(q,at) do { int64_t stamp=now_us(); profile_us[q]+=stamp-at; at=stamp; } while(0)
#else
#define ROOM_PROFILE_PHASE(q,at) ((void)0)
#endif
static int64_t room_prepare_us[WORLD_ROOMS];
static int64_t world_publication_us;
/* The rules' dice key (#13258): 64 bits from the operating system's random source (getentropy: the
   kernel's on the native host, crypto.getRandomValues through WASI random_get in the browser's
   worker) into rm_dice_key (room.coax). Every room's dice draw from it and no frame carries it; the
   seed W sends is cosmetic. False when the source fails: the host then refuses to start. */
static bool dice_key_in(void) {
  uint8_t b[8];
  if (getentropy(b, sizeof b) != 0) return false;
  int64_t hi = (int64_t)b[0] << 24 | (int64_t)b[1] << 16 | (int64_t)b[2] << 8 | b[3];
  int64_t lo = (int64_t)b[4] << 24 | (int64_t)b[5] << 16 | (int64_t)b[6] << 8 | b[7];
  rm_dice_key(view(), hi, lo);
  return true;
}
/* The host's numbers are the room's (#13275): room-world.h's rooms, seats and tick, which size the
   host's own tables, against everglade/server/seat.coax's. False: the host refuses to start, so a
   seat past the host's tables is never stepped. */
static bool world_sizes_agree(void) {
  return rm_eg_rooms() == WORLD_ROOMS && rm_eg_seats() == WORLD_SEATS && rm_eg_hz() == WORLD_HZ;
}
static const char *part_name[PARTS] = {"move", "abilities", "town", "spells", "people", "rules", "write", "send"};
/* Only the opt-in native observation adapter consumes these SDK timestamps.
   The worker shares the same simulation and does not claim remote latency. */
#ifndef ROOM_LATENCY_STEP
#define ROOM_LATENCY_ACTIVE(k,s) false
#define ROOM_LATENCY_STEP(k,s,seq,began,ended) ((void)0)
#define ROOM_LATENCY_SERIALIZED(k,simulation,began,ended) ((void)0)
#endif
/* Everglade II's room k (#13053): each seat at the glade when it sits, its
   queued inputs through Everglade II's movement step (room-world.c world2_*), the rules' tick, and
   every seat's D frame queued. Its land has no town, river, watch or pieces to step. */
static void everglade2_place(int k, int s, float *pose) {
  float at[3];
  if (rm_eg_unplaced(view(), k, (coax_t_SeatId){ s}) &&
      world2_place(k, s, rm_e2_place(view(), k, (coax_t_SeatId){ s}, (coax_v_f32){at, 3}) ? at : NULL, pose))
    rm_eg_moved(view(), k, (coax_t_SeatId){ s}, 0, pose[0], pose[1], pose[2], pose[3], pose[4], pose[5], pose[6], (int64_t)pose[7], (int64_t)pose[8], (int64_t)pose[9]);
}
static bool everglade2_room_tick(int k, int64_t room_work_start) {
  int64_t in[WORLD_IN]; float pose[WORLD_POSE];
  coax_v_s64 inv = {in, WORLD_IN};
  if (!world2_prepare()) { rm_work_done(view(), k, now_us() - room_work_start); return false; }
  /* the arrows fly among the trunks and rocks the seats' steps walk among (#13166) */
  rm_e2_land(view(), (coax_v_t_E2MoveGround){(coax_t_E2MoveGround *)world2_ground(), 1});
  int64_t tm = now_us();
  float guard[2] = {0, 0};
  bool guard_on = rm_e2_guard_body(view(), k, (coax_v_f32){guard, 2});
  world2_guard(k, guard_on, guard[0], guard[1]);
  /* the trunks round him, which his steps go round (everglade/server/guard.coax) */
  float near[E2G_NEAR_C * 3];
  rm_e2_guard_near(view(), k, (coax_v_f32){near, guard_on ? world2_near(guard[0], guard[1], near, E2G_NEAR_C) * 3 : 0});
  for (int s = 0; s < WORLD_SEATS; s++) {
    everglade2_place(k, s, pose);
    /* admin mode's teleport (everglade/server/admin.coax): asked of a local room only */
    float warp[3];
    if (rm_e2_admin_warp(view(), k, (coax_t_SeatId){ s}, (coax_v_f32){warp, 3}) && world2_place(k, s, warp, pose))
      rm_eg_moved(view(), k, (coax_t_SeatId){ s}, 0, pose[0], pose[1], pose[2], pose[3], pose[4], pose[5], pose[6], (int64_t)pose[7], (int64_t)pose[8], (int64_t)pose[9]);
    while (rm_eg_input(view(), k, (coax_t_SeatId){ s}, inv)) {
      int64_t input_began = ROOM_LATENCY_ACTIVE(k,s) ? now_us() : 0;
      if (!world2_step(k, s, in, pose)) break;
      rm_eg_moved(view(), k, (coax_t_SeatId){ s}, in[0], pose[0], pose[1], pose[2], pose[3], pose[4], pose[5], pose[6], (int64_t)pose[7], (int64_t)pose[8], (int64_t)pose[9]);
      if(input_began)ROOM_LATENCY_STEP(k,s,in[0],input_began,now_us());
    }
  }
  part_us[PART_MOVE] += now_us() - tm;
  int64_t te = now_us();
  /* The kit asks about contact in the same collision world that stepped these bodies. */
  float ray[6];
  coax_v_f32 query = {ray, 6};
  int64_t count = rm_eg_sights(view(), k);
  for (int64_t q = 0; q < count; q++)
    if (rm_eg_sight(view(), k, q, query))
      rm_eg_sight_is(view(), k, q, world2_contact(ray, ray + 3));
  rm_eg_rules(view(), k);
  for (int s = 0; s < WORLD_SEATS; s++) if (world_cut_take(k, s)) rm_eg_cut(view(), k, (coax_t_SeatId){ s});
  int64_t tw = now_us(); part_us[PART_RULES] += tw - te;
  rm_eg_send(view(), k);
  ROOM_LATENCY_SERIALIZED(k,tw,tw,now_us());
  part_us[PART_WRITE] += now_us() - tw;
  rm_work_done(view(), k, now_us() - room_work_start);
  return true;
}
/* Room k's tick (everglade/server/tick.coax, 30 Hz): false when room k is closed. Every room is
   Everglade II's (docs/decisions/0005-production-room-runs-everglade-ii.md, #13267); v1's tick is the
   frozen build's (everglade/server/legacy/v1.sh). */
static bool everglade_room_tick(int k) {
  int64_t room_work_start = now_us();
#ifdef COAX_ROOM_PROFILE
  int64_t phase_start=room_work_start;
#endif
  float pose[WORLD_POSE];
  if (!rm_eg_begin(view(), k)) {
    ROOM_PROFILE_PHASE(0,phase_start);
    /* Placement resolves the capture's standing height, without advancing a
       paused encounter. The initial durable capture must precede gameplay. */
    if (rm_checkpoint_paused(view(), k)) for (int s = 0; s < WORLD_SEATS; s++) everglade2_place(k, s, pose);
    return false;
  }
  ROOM_PROFILE_PHASE(0,phase_start);
  return everglade2_room_tick(k, room_work_start);
}
