# World profiles, paid continuous walking and turn ordering

#13045, 2026-11-08. Pinned SRD 5.2.1 SHA-256
`8974902d109d6e63672d7c490bde9ccf052410503d9cfa768237154fbc5e3d87`.
Sources: p.13 (a round is about 6 seconds), p.14 (Difficult Terrain), p.106
(Duration), p.179 (Crawling), p.180 (Dash), p.187 (Simultaneous Effects),
p.188 (Speed).

## What changed

- `aethyr/rules/srd/world-profile.coax` (`#authority`): `RsWorldProfile` holds
  a world's timing, area-origin, press-budget and walking choices.
  `rs_set_world` applies all four or none before the clock is sealed;
  `rs_reset` restores `RS_STRICT_WORLD`; `rs_world_deviations` names each
  departure. `RsBattle` gains the applied `press` and `walk` fields.
- Every world now selects one profile: Experimental, the Everglade 5E room
  (and so Everglade II) `RS_EVERGLADE_WORLD`; Everglade 5E single play, 5E MMO
  and 5E Lab action mode `RS_ACTION_WORLD`. The three press paths
  (Experimental, room, Everglade II) read `b[0].press` instead of passing
  `RS_UNLIMITED_PRESS`. No world calls `rs_set_timing`/`rs_set_areas` directly.
- `aethyr/rules/srd/walk-budget.coax` (`#authority`): under `RS_WALK_PAID` a
  physical walker pays each foot walked from the turn's movement (two in
  Difficult Terrain or crawling, three for both; Dash adds Speed). `rs_walk_take`
  cuts a step where movement runs out. The part of a foot carries in
  `RsTurn.walk_ft`. Walking marks the turn moved and interrupts a rest under
  either walking mode. The Lab's Experimental controller rate now reads
  `rs_walk_scale`, which equals the old rate under Everglade's `RS_WALK_RATE`.
- `rs_minf` (f32 minimum) joins `rs_min` in `engine.coax`.

## Checks

- `coax/bin/coax test aethyr/rules/srd/world-timing-checks.coax --library`: 355 pass.
  Strict and Everglade worlds side by side (beat, grant, recovery, areas, press,
  walking); walking exactly Speed 7/25/30/45; Difficult Terrain/crawling costs;
  Dash and Speed loss; shared grid/continuous budget with a carried half foot;
  refusals; action-timing paid beats and Everglade unpaid walking.
- `coax/bin/coax test aethyr/rules/srd/walk-budget-walker-checks.coax --library`:
  668 pass. The actual walker (`walk.advance_scaled_in`, run speed 7.5 m/s for
  six seconds) stops at 9.144 m, or 18.288 m after a Dash.
- `coax/bin/coax test aethyr/rules/srd/turn-order-checks.coax --library`: 355
  pass, each in strict and action timing: every executable spell's timed
  duration and Concentration match the pinned source; a one-minute effect lasts
  ten rounds (40 action beats) and an hour 3,600 s; "until the end of your next
  turn" for caster and target; start-of-turn Dodge/Reaction/death save;
  end-of-turn repeated saves giving the same result in either order.
- Mutation, zero survivors, no exemptions:
  `coax mutate aethyr/rules/srd/world-timing-checks.coax --library --unit aethyr/rules/srd/world-profile.coax`: 60 killed.
  `... --unit aethyr/rules/srd/walk-budget.coax`: 80 killed.
  First runs had 4 and 16 survivors (bitwise-or equivalence, an untested
  profile comparison helper, boundary-equal minimums, the rest call); the code
  was restructured and checks added, not exempted. Comment-only source page
  corrections followed the gates.
- World regressions: `fivebar-test.coax` 386, `everglade2-rules.coax` 351,
  `everglade5e-room-rules.coax` 485, `timing-checks.coax`, `fivelab.coax --only experimental` 85 pass.

## Limits

No current world drives a paid physical walker: Everglade's continuous walking
is the declared `RS_WALK_RATE` deviation, and the grid action worlds walk on
grid steps. The SRD lets the turn's owner order simultaneous effects; the
engine uses a fixed order and no chooser ships. Everglade II's social targeting
stays a named rule in `everglade2-rules.coax`. Special speeds, jumping and
Haste remain outside this slice.
