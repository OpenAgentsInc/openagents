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
static const char *part_name[PARTS] = {"move", "abilities", "town", "spells", "people", "rules", "write", "send"};
// Room k's town for this tick (room-world.c world_town, GitHub #12713): the rules' Hit Points and the
// crimes since into it, a step against every seat, then the guards' blows, WANTED and where everyone is
// back to the room (the rules', and what the pages hear).
static void everglade_town(int k) {
  float hp[4], cr[3], seats[WORLD_SEATS * WORLD_SEAT_TOWN], took[2 * WORLD_SEATS], people[WORLD_FOLK * WORLD_PEOPLE], fv[WORLD_VIEW];
  coax_v_f32 hpv = {hp, 4}, crv = {cr, 3}, fvv = {fv, WORLD_VIEW};
  if (rm_eg_fresh(view(), k)) world_fresh(k);
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
  for (int s = 0; s < WORLD_SEATS; s++) rm_eg_seat_town(view(), k, s, (coax_v_f32){seats + s * WORLD_SEAT_TOWN, WORLD_SEAT_TOWN});
  /* the rowboats (#12861): a seat gone leaves its boat tied up; the floats a tick on; each boat for the pages */
  for (int s = 0; s < WORLD_SEATS; s++) if (seats[s * WORLD_SEAT_TOWN + 3] < 0.5f) world_gone(k, s);
  world_floats(k, 1.0f / WORLD_HZ);
  float bt[WORLD_BOAT];
  for (int n = 0; n < 3; n++) {
    if (!world_boat(k, n, bt)) break;
    rm_eg_boat(view(), k, n, (coax_v_f32){bt, WORLD_BOAT});
  }
  /* a place line (room-control.coax "place", #12861): the seat on the bank beside that rowboat */
  for (int s = 0; s < WORLD_SEATS; s++) { int b = (int)rm_eg_place_take(view(), k, s); if (b >= 0) world_place_boat(k, s, b); }
  /* a scripted Hold Person (room-control.coax "hold", #12886): the watch's hold on the seat, before the town steps */
  for (int s = 0; s < WORLD_SEATS; s++) if (rm_eg_hold_take(view(), k, s)) world_hold(k, s);
  int64_t t0 = now_us();
  world_town(k, 1.0f / WORLD_HZ, (double)wall_ms() / 1000.0, seats, took);
  int64_t t1 = now_us(); part_us[PART_TOWN] += t1 - t0;
  for (int s = 0; s < WORLD_SEATS; s++) {
    if (seats[s * WORLD_SEAT_TOWN + 3] < 0.5f) continue;
    rm_eg_hurt(view(), k, s, (int64_t)took[s], (int64_t)took[WORLD_SEATS + s]);
    int seed = (int)rm_eg_want_take(view(), k, s);   /* back WANTED at the level the room saved (#12794) */
    if (seed > 0) world_seed_wanted(k, s, seed);
    rm_eg_wanted(view(), k, s, world_wanted(k, s));
    rm_eg_cond(view(), k, s, world_cond(k, s));
    for (int c = 0; c < 3; c++) rm_eg_cond_left(view(), k, s, c, world_cond_left(k, s, c));
  }
  rm_eg_gates(view(), k, world_gates(k));   /* the gates shut at WANTED 4 (#12794): the doors row's FORM */
  /* the town's pieces: the rebuild, the strikes cast since, then what broke and caught for every page */
  if (rm_eg_rebuild(view(), k)) world_rebuild(k);
  float sk[9], ev[WORLD_EVENT]; coax_v_f32 skv = {sk, 9};
  int64_t sn = rm_eg_strikes(view(), k);
  for (int64_t q = 0; q < sn; q++) { rm_eg_strike(view(), k, q, skv); world_strike(k, (int)sk[0], (int)sk[1], sk + 2, sk + 5, (int)sk[8]); }
  rm_eg_strikes_heard(view(), k);
  int64_t t2 = now_us();
  int en = world_spells(k, 1.0f / WORLD_HZ);
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
  for (int64_t q = 0; q < n; q++) { rm_ra_cast(view(), k, q, cv); world_ability(k, c); }
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
      rm_ra_live(view(), k, s, ab, on, l[1], l[2], l[3], l[4], (int64_t)l[5]);
    }
  for (int i = 0; i < WORLD_FOLK; i++) rm_ra_air(view(), k, i, world_air(k, i));
  int hn = world_hurts(k);
  for (int q = 0; q < hn; q++) if (world_hurt(k, q, h)) rm_ra_hurt(view(), k, (int64_t)h[0], (int64_t)h[1], (int64_t)h[2]);
}
// Room k's tick (room-everglade.coax, 30 Hz): each seat at the gate when it sits, then its queued
// inputs through the movement step in its room's world (room-world.c), the hotbar abilities, the
// town, the casts' sight, the rolls, and every seat's D frame queued: false when room k is closed.
static bool everglade_room_tick(int k) {
  int64_t in[WORLD_IN]; float pose[WORLD_POSE];
  coax_v_s64 inv = {in, WORLD_IN};
  if (!rm_eg_begin(view(), k)) return false;
  world_dice(k, rm_eg_dice_fixed(view(), k));   /* a local room told "dice fixed": the world's too (room-control.coax) */
  everglade_ability_casts(k);
  int64_t tm = now_us();
  for (int s = 0; s < WORLD_SEATS; s++) {
    if (rm_eg_unplaced(view(), k, s) &&
        (rm_eg_back(view(), k, s, 0) > 0 ? world_place_at(k, s, rm_eg_back(view(), k, s, 1), rm_eg_back(view(), k, s, 2), rm_eg_back(view(), k, s, 3), rm_eg_back(view(), k, s, 4), pose)
                                          : world_place(k, s, pose)))
      rm_eg_moved(view(), k, s, 0, pose[0], pose[1], pose[2], pose[3], pose[4], pose[5], pose[6], (int64_t)pose[7], (int64_t)pose[8], (int64_t)pose[9]);
    while (rm_eg_input(view(), k, s, inv)) {
      if (!world_step(k, s, in, pose)) break;
      rm_eg_moved(view(), k, s, in[0], pose[0], pose[1], pose[2], pose[3], pose[4], pose[5], pose[6], (int64_t)pose[7], (int64_t)pose[8], (int64_t)pose[9]);
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
  int64_t tw = now_us(); part_us[PART_RULES] += tw - te;
  rm_eg_send(view(), k);
  part_us[PART_WRITE] += now_us() - tw;
  return true;
}
