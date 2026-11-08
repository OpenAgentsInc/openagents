/* The everglade rooms' tick, which both of the room server's hosts include (GitHub #12784; Aethyr
   book chapter 95): the native host (room-host.c, oa-room over TCP) and the browser's local host
   (room-worker.c, offline play in a Web Worker). A new server step goes here, not in one host, or the
   other host's game misses it. The host includes this after room.h and room-world.h and defines
   view(), now_us() and wall_ms(); it moves the frames rm_eg_send queued (relay_flush) itself. */
/* Where the everglade tick's time goes (#12717): microseconds summed over the report's minute for the
   movement steps, the hotbar abilities, the town (watch and folk), the spells' pieces, the people for the rules and the pages,
   rm_eg_rules (the rules' step, the raid's), rm_eg_send (every seat's D written) and the frames sent (relay_flush,
   pump_all), printed with the tick line; each tick's are the room view's workloads too (room-stats.h). */
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
static const char *part_name[PARTS] = {"move", "abilities", "town", "spells", "people", "rules", "write", "send"};
/* Only the opt-in native observation adapter consumes these SDK timestamps.
   The worker shares the same simulation and does not claim remote latency. */
#ifndef ROOM_LATENCY_STEP
#define ROOM_LATENCY_ACTIVE(k,s) false
#define ROOM_LATENCY_STEP(k,s,seq,began,ended) ((void)0)
#define ROOM_LATENCY_SERIALIZED(k,simulation,began,ended) ((void)0)
#endif
// Room k's town for this tick (room-world.c world_town, GitHub #12713): the rules' Hit Points and the
// crimes since into it, a step against every seat, then the guards' blows, WANTED and where everyone is
// back to the room (the rules', and what the pages hear).
static void everglade_town(int k) {
#ifdef COAX_ROOM_PROFILE
  int64_t phase_start=now_us();
#endif
  float hp[4], cr[3], seats[WORLD_SEATS * WORLD_SEAT_TOWN], took[2 * WORLD_SEATS], people[WORLD_FOLK * WORLD_PEOPLE], fv[WORLD_VIEW];
  coax_v_f32 hpv = {hp, 4}, crv = {cr, 3}, fvv = {fv, WORLD_VIEW};
  for (int j = 0; j < WORLD_FOLK; j++)
    if (rm_eg_folk_hp(view(), k, j, hpv)) world_folk_hp(k, j, (int)hp[0], (int)hp[1], (int)hp[2], (int)hp[3]);
  int64_t cn = rm_eg_crimes(view(), k);
  for (int64_t q = 0; q < cn; q++) { rm_eg_crime(view(), k, q, crv); world_crime(k, (int)cr[0], cr[1], cr[2]); }
  rm_eg_crimes_heard(view(), k);
  /* the doors the seats used (before this tick's steps, so the doorway is open for them) and the talks, then which doors stand open */
  int64_t dn = rm_eg_use_asks(view(), k);
  for (int64_t q = 0; q < dn; q++) {
    int64_t a = rm_eg_use_ask(view(), k, q);
    if (a < 0) continue;
    int kind = (int)(a / 1000000), seat = (int)(a / 1000 % 1000), id = (int)(a % 1000) - 1;
    (void)world_use(k, kind, seat, id);
  }
  rm_eg_use_asks_heard(view(), k);
  for (int w = 0; w < 2; w++) rm_eg_doors(view(), k, w, world_doors(k, w));
  for (int s = 0; s < WORLD_SEATS; s++) rm_eg_seat_town(view(), k, (coax_t_SeatId){ s}, (coax_v_f32){seats + s * WORLD_SEAT_TOWN, WORLD_SEAT_TOWN});
  /* the rowboats (#12861): a seat gone leaves its boat tied up; the floats a tick on; each boat for the pages */
  for (int s = 0; s < WORLD_SEATS; s++) if (seats[s * WORLD_SEAT_TOWN + 3] < 0.5f) world_gone(k, s);
  world_floats(k, 1.0f / WORLD_HZ);
  float bt[WORLD_BOAT];
  for (int n = 0; n < 3; n++) {
    if (!world_boat(k, n, bt)) break;
    rm_eg_boat(view(), k, n, (coax_v_f32){bt, WORLD_BOAT});
  }
  /* a place line (room-control.coax "place", #12861): the seat on the bank beside that rowboat */
  for (int s = 0; s < WORLD_SEATS; s++) { int b = (int)rm_eg_place_take(view(), k, (coax_t_SeatId){ s}); if (b >= 0) world_place_boat(k, s, b); }
  /* a scripted Hold Person (room-control.coax "hold", #12886): the watch's hold on the seat, before the town steps */
  for (int s = 0; s < WORLD_SEATS; s++) if (rm_eg_hold_take(view(), k, (coax_t_SeatId){ s})) world_hold(k, s);
  int64_t t0 = now_us();
  ROOM_PROFILE_PHASE(4,phase_start);
  world_town(k, 1.0f / WORLD_HZ, (double)wall_ms() / 1000.0, seats, took);
  int64_t t1 = now_us(); part_us[PART_TOWN] += t1 - t0;
#ifdef COAX_ROOM_PROFILE
  phase_start=t1;
#endif
  for (int s = 0; s < WORLD_SEATS; s++) {
    if (seats[s * WORLD_SEAT_TOWN + 3] < 0.5f) continue;
    rm_eg_hurt(view(), k, (coax_t_SeatId){ s}, (int64_t)took[s], (int64_t)took[WORLD_SEATS + s]);
    int seed = (int)rm_eg_want_take(view(), k, (coax_t_SeatId){ s});   /* back WANTED at the level the room saved (#12794) */
    if (seed > 0) world_seed_wanted(k, s, seed);
    rm_eg_wanted(view(), k, (coax_t_SeatId){ s}, world_wanted(k, s));
    rm_eg_cond(view(), k, (coax_t_SeatId){ s}, world_cond(k, s));
    for (int c = 0; c < 3; c++) rm_eg_cond_left(view(), k, (coax_t_SeatId){ s}, c, world_cond_left(k, s, c));
  }
  rm_eg_gates(view(), k, world_gates(k));   /* the gates shut at WANTED 4 (#12794): the doors row's FORM */
  /* the town's pieces: the rebuild, the strikes cast since, then what broke and caught for every page */
  if (rm_eg_rebuild(view(), k)) world_rebuild(k);
  float sk[10], ev[WORLD_EVENT]; coax_v_f32 skv = {sk, 10};
  int64_t sn = rm_eg_strikes(view(), k);
  int64_t taken=0;
  for (;taken<sn;taken++) { if(!rm_eg_strike_current(view(),k,taken))continue;rm_eg_strike(view(),k,taken,skv);if(!world_strike(k,(int)sk[9],(int)sk[0],(int)sk[1],sk+2,sk+5,(int)sk[8]))break; }
  rm_eg_strikes_taken(view(),k,taken);
  int64_t t2 = now_us();
  ROOM_PROFILE_PHASE(5,phase_start);
  int en = world_spells(k, 1.0f / WORLD_HZ);
  float tree[WORLD_TREE];
  for (int q = 1; q < WORLD_TREES; q++) {
    int n = world_tree(k, q, tree) ? WORLD_TREE : 0;
    rm_eg_tree(view(), k, q, (coax_v_f32){tree, n});
  }
  for(int source=0;source<rm_effect_sources();source++)rm_effect_free(view(),k,source,world_effect_free(k,source));
  for (int q = 0; q < en; q++)
    if (world_event(k, q, ev)) {
      /* a Repair (KIND 3): the pieces it stood up are whole for the pages that join later too */
      if ((int)ev[0] == 3) { int pn2 = world_pieces(k); for (int i = 0; i < pn2; i++) if (world_piece_up(k, i)) rm_eg_piece_up(view(), k, i); }
      rm_eg_piece(view(), k, (int64_t)ev[0], (int64_t)ev[1], ev[2], ev[3], ev[4], ev[5], ev[6]);
    }
  int64_t t3 = now_us(); part_us[PART_SPELLS] += t3 - t2;
  int pn = world_people(k, people);
  rm_eg_people(view(), k, (coax_v_f32){people, WORLD_FOLK * WORLD_PEOPLE}, pn);
  for (int i = 0; i < WORLD_FOLK; i++) if (world_view(k, i, fv)) rm_eg_folk(view(), k, i, fvv);
  /* the watch's spells that left their casters' hands: every page near draws them fly (e wm) */
  float ms[WORLD_MISSILE];
  for (int q = 0; q < 16 && world_missile(k, ms); q++) rm_eg_missile(view(), k, (coax_v_f32){ms, WORLD_MISSILE});
  part_us[PART_PEOPLE] += now_us() - t3;
}
// The hotbar abilities' casts and ends since into room k's world. Before a tick's steps: a cast started,
// or an end taken, at the last tick's end (rm_eg_rules) has its place among its seat's inputs (the inputs
// after it waited for it: room-everglade.coax eg_waits), so the steps after it feel what it holds up, as
// its page's prediction did (#12877).
static void everglade_ability_casts(int k) {
  float c[WORLD_CAST]; coax_v_f32 cv = {c, WORLD_CAST};
  int64_t n = rm_ra_casts(view(), k);
  for (int64_t q = 0; q < n; q++) { if(!rm_ra_cast_current(view(),k,q))continue;rm_ra_cast(view(), k, q, cv); world_ability(k, c); }
  rm_ra_heard(view(), k);
}
// Room k's hotbar abilities the world runs (room-abilities.coax, everglade-room-abilities.coax; #12718):
// the rules' saves and the casts and ends since into it, its step, then what it holds up and the falls' damage back.
static void everglade_abilities(int k) {
  float l[WORLD_LIFE] = {0}, h[3];
  /* the saves the rules rolled against the abilities cast: their copies take them, not a second roll (#12874) */
  for (int j = 0; j < WORLD_FOLK; j++)
    if (rm_eg_folk_save(view(), k, j, (coax_v_f32){h, 3})) world_folk_save(k, j, (int)h[0], (int)h[1], h[2] > 0.5f);
  everglade_ability_casts(k);   /* (and those since the tick's steps: a cast's start and its end with them) */
  world_abilities(k, 1.0f / WORLD_HZ);
  /* every seat's abilities up, each its own entity */
  for (int s = 0; s < WORLD_SEATS; s++)
    for (int ab = 0; ab < WORLD_ABS; ab++) {
      bool on = world_life(k, s, ab, l);
      rm_ra_live(view(), k, (coax_t_SeatId){ s}, ab, on, l[1], l[2], l[3], l[4], (int64_t)l[5]);
    }
  for (int i = 0; i < WORLD_FOLK; i++) rm_ra_air(view(), k, i, world_air(k, i));
  int hn = world_hurts(k);
  for (int q = 0; q < hn; q++) if (world_hurt(k, q, h)) rm_ra_hurt(view(), k, (int64_t)h[0], (int64_t)h[1], (int64_t)h[2]);
}
// Room k's tick (room-everglade.coax, 30 Hz): each seat at the gate when it sits, then its queued
// inputs through the movement step in its room's world (room-world.c), the hotbar abilities, the
// town, the casts' sight, the rolls, and every seat's D frame queued: false when room k is closed.
static void everglade_place(int k, int s, float *pose) {
  if (rm_eg_unplaced(view(), k, (coax_t_SeatId){ s}) &&
      (rm_eg_glade(view(), k, (coax_t_SeatId){ s}) ? world_place_glade(k, s, pose) :   /* Character Create's page: the glade's clearing */
       rm_eg_back(view(), k, (coax_t_SeatId){ s}, 0) > 0 ? world_place_at(k, s, rm_eg_back(view(), k, (coax_t_SeatId){ s}, 1), rm_eg_back(view(), k, (coax_t_SeatId){ s}, 2), rm_eg_back(view(), k, (coax_t_SeatId){ s}, 3), rm_eg_back(view(), k, (coax_t_SeatId){ s}, 4), pose)
                                        : world_place(k, s, pose)))
    rm_eg_moved(view(), k, (coax_t_SeatId){ s}, 0, pose[0], pose[1], pose[2], pose[3], pose[4], pose[5], pose[6], (int64_t)pose[7], (int64_t)pose[8], (int64_t)pose[9]);
}
static bool everglade_prepare(int k) {
  if (world_prepared(k)) return true;
  int64_t preparing = now_us();bool prepared = world_prepare(k);
  room_prepare_us[k] = now_us() - preparing;
  return prepared;
}
/* Everglade II's room k (#13053, room-everglade.coax rm_eg_e2): each seat at the glade when it sits, its
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
  int64_t tm = now_us();
  for (int s = 0; s < WORLD_SEATS; s++) {
    everglade2_place(k, s, pose);
    while (rm_eg_input(view(), k, (coax_t_SeatId){ s}, inv)) {
      int64_t input_began = ROOM_LATENCY_ACTIVE(k,s) ? now_us() : 0;
      if (!world2_step(k, s, in, pose)) break;
      rm_eg_moved(view(), k, (coax_t_SeatId){ s}, in[0], pose[0], pose[1], pose[2], pose[3], pose[4], pose[5], pose[6], (int64_t)pose[7], (int64_t)pose[8], (int64_t)pose[9]);
      if(input_began)ROOM_LATENCY_STEP(k,s,in[0],input_began,now_us());
    }
  }
  part_us[PART_MOVE] += now_us() - tm;
  int64_t te = now_us();
  rm_eg_rules(view(), k);
  for (int s = 0; s < WORLD_SEATS; s++) if (world_cut_take(k, s)) rm_eg_cut(view(), k, (coax_t_SeatId){ s});
  int64_t tw = now_us(); part_us[PART_RULES] += tw - te;
  rm_eg_send(view(), k);
  ROOM_LATENCY_SERIALIZED(k,tw,tw,now_us());
  part_us[PART_WRITE] += now_us() - tw;
  rm_work_done(view(), k, now_us() - room_work_start);
  return true;
}
static bool everglade_room_tick(int k) {
  int64_t room_work_start = now_us();
#ifdef COAX_ROOM_PROFILE
  int64_t phase_start=room_work_start;
#endif
  int64_t in[WORLD_IN]; float pose[WORLD_POSE];
  coax_v_s64 inv = {in, WORLD_IN};
  if (!rm_eg_begin(view(), k)) {
    ROOM_PROFILE_PHASE(0,phase_start);
    /* Placement resolves the capture's standing height, without advancing a
       paused encounter. The initial durable capture must precede gameplay. */
    if (rm_checkpoint_paused(view(), k)) {
      if (!world_prepared(k)) {
        bool prepared=everglade_prepare(k);
        rm_work_done(view(),k,now_us()-room_work_start);
        if(!prepared)return false;
      }
      for (int s = 0; s < WORLD_SEATS; s++) { if (rm_eg_e2(view(), k)) everglade2_place(k, s, pose); else everglade_place(k, s, pose); }
    }
    return false;
  }
  ROOM_PROFILE_PHASE(0,phase_start);
  if (rm_eg_e2(view(), k)) return everglade2_room_tick(k, room_work_start);
  if (!everglade_prepare(k)) { rm_work_done(view(), k, now_us() - room_work_start); return false; }
  ROOM_PROFILE_PHASE(1,phase_start);
  if (rm_eg_fresh(view(), k)) world_fresh(k);
  for (int s=0;s<WORLD_SEATS;s++) world_seat_owner(k,s,rm_eg_seat_owner(view(),k,(coax_t_SeatId){s}));
  ROOM_PROFILE_PHASE(2,phase_start);
  world_dice(k, rm_eg_dice_fixed(view(), k));   /* a local room told "dice fixed": the world's too (room-control.coax) */
  everglade_ability_casts(k);
  ROOM_PROFILE_PHASE(3,phase_start);
  int64_t tm = now_us();
  for (int s = 0; s < WORLD_SEATS; s++) {
    everglade_place(k, s, pose);
    while (rm_eg_input(view(), k, (coax_t_SeatId){ s}, inv)) {
      int64_t input_began = ROOM_LATENCY_ACTIVE(k,s) ? now_us() : 0;
      if (!world_step(k, s, in, pose)) break;
      rm_eg_moved(view(), k, (coax_t_SeatId){ s}, in[0], pose[0], pose[1], pose[2], pose[3], pose[4], pose[5], pose[6], (int64_t)pose[7], (int64_t)pose[8], (int64_t)pose[9]);
      if(input_began)ROOM_LATENCY_STEP(k,s,in[0],input_began,now_us());
    }
  }
  /* the summoned creatures stand on the ground (a new one is heard once it does) */
  float xz[2]; coax_v_f32 xzv = {xz, 2};
  for (int q = 0; q < rm_eg_beasts(); q++)
    if (rm_eg_beast_at(view(), k, q, xzv)) rm_eg_beast_ground(view(), k, q, world_ground(k, xz[0], xz[1]));
  part_us[PART_MOVE] += now_us() - tm;
  int64_t ta = now_us();
  everglade_abilities(k);
  part_us[PART_ABILITIES] += now_us() - ta;
  everglade_town(k);
  if (rm_forge_active(view())) {
    world_forge_prepare(k);
    float ask[6], moved[5];
    for (int q = 0; q < rm_eg_beasts(); q++)
      if (rm_forge_motion(view(), k, q, (coax_v_f32){ask, 6})) {
        world_forge_step(k, ask, moved);
        rm_forge_moved(view(), k, q, (coax_v_f32){moved, 5});
      }
  }
  int64_t te = now_us();
  /* the casts starting or released this tick: does each caster see where it lands (room-casts.coax, #12790) */
  float sg[6]; coax_v_f32 sgv = {sg, 6};
  int64_t sn = rm_eg_sights(view(), k);
  for (int64_t q = 0; q < sn; q++) if (rm_eg_sight(view(), k, q, sgv)) rm_eg_sight_is(view(), k, q, world_sight(k, sg, sg + 3));
  rm_eg_rules(view(), k);
  /* the seats the world moved in one jump: their next row is a WARP (#12835) */
  for (int s = 0; s < WORLD_SEATS; s++) if (world_cut_take(k, s)) rm_eg_cut(view(), k, (coax_t_SeatId){ s});
  int64_t tw = now_us(); part_us[PART_RULES] += tw - te;
  rm_eg_send(view(), k);
  ROOM_LATENCY_SERIALIZED(k,tw,tw,now_us());
  part_us[PART_WRITE] += now_us() - tw;
  rm_work_done(view(), k, now_us() - room_work_start);
  return true;
}
