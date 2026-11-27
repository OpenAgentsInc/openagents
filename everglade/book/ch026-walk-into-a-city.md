# 26. Walk into a city

> **Status: partly implemented.** Pure-function forest clearing and stream
> carving live in [`everglade/world/clearing.coax`](../world/clearing.coax),
> the 16-plot town grid and five-tier Housewright staged buildings live in
> [`everglade/world/town.coax`](../world/town.coax),
> [`everglade/world/housewright.coax`](../world/housewright.coax),
> [`everglade/world/town-stage.coax`](../world/town-stage.coax), and
> [`metaverse-engine/world/plots.coax`](../../metaverse-engine/world/plots.coax),
> the walled city blockout and 18 named gates/landmarks live in
> [`everglade/world/plan.coax`](../world/plan.coax) and
> [`everglade/world/town-points.coax`](../world/town-points.coax), and
> precomputed reachable navigation cells live in
> [`everglade/world/zone-cells.coax`](../world/zone-cells.coax), exercised by
> [`everglade/examples/city-town-plots.coax`](../examples/city-town-plots.coax).
> Live player-driven plot claiming and persistent buildings inside Everglade
> II's multiplayer city clearing remain to be wired into production rooms.

This chapter answers the player's question: "When the forest path opens into
the clearing, how does a real town—streets, plots, gates, and houses—grow out
of the same land without seams or invisible walls?"

Leaving the woods for a city is where a terrain demo becomes a world. The
clearing floor has to meet the surrounding forest hills smoothly, every house
footprint has to fit inside its plot with room for lanes between rows, and
every gate and landmark has to resolve from one table so navigation, rooms, and
captures never disagree about where a street begins.

## What this chapter teaches

- How pure clearing and stream functions (`clearing_open`, `clearing_floor`,
  `clearing_ring`) carve a flat, buildable city floor out of rolling terrain on
  both CPU and GPU.
- How a deterministic 4×4 plot grid (`TOWN_SIZE = 10 m`, `TOWN_PITCH = 12 m`)
  turns 16 plots into street-facing rows separated by 2 m lanes.
- How Housewright's five tiers (146 parts across 24 named stages) advance
  deterministically from the world clock (`pl_progress`, `pl_stage_done`).
- How named gates and landmarks (`eg_point`) and plot claims (`pl_claim`,
  `pl_site`) fail closed on unknown names, duplicate claims, or non-owners.

## What exists today

| Piece | Role | Where |
| --- | --- | --- |
| Clearing & stream carving | Levels a ragged circular clearing (`clearing_floor`), clears trees inside (`clearing_open`), frames a tree ring (`clearing_ring`), and cuts stream beds | [`everglade/world/clearing.coax`](../world/clearing.coax) |
| Plot grid & compare-and-set book | `PlotGrid`, `pl_at` (`-1` on lanes/outside), `pl_claim` (one plot per player), `pl_site`, `pl_progress` | [`metaverse-engine/world/plots.coax`](../../metaverse-engine/world/plots.coax) |
| Town plan & Housewright tiers | 16 plots, 5 construction tiers (lean-to through stone cottage, 146 parts), front doors facing onto central street | [`everglade/world/town.coax`](../world/town.coax), [`everglade/world/housewright.coax`](../world/housewright.coax), [`everglade/world/town-stage.coax`](../world/town-stage.coax) |
| Walled city blockout & named points | 4 palisade walls, 4 gates (`SOUTH/NORTH/EAST/WEST GATE`), building catalogue, and 18 named points (`eg_point`) | [`everglade/world/plan.coax`](../world/plan.coax), [`everglade/world/town-points.coax`](../world/town-points.coax), [`everglade/world/building-catalogue.coax`](../world/building-catalogue.coax) |
| Reachable navigation cells | 21 precomputed walkable cells (85,881 reachable ground samples from the glade spawn) | [`everglade/world/zone-cells.coax`](../world/zone-cells.coax) |

**What remains to ship:** The clearing floor, 16-plot town grid, five-tier
Housewright buildings, and time-lapse town stages (`city-empty`, `city-grows`,
`founder`) run today on deterministic Coax modules and in Everglade
Experimental's stand-in meadow; the walled town blockout lives in
`everglade/world/plan.coax`. Live player plot claiming in Everglade II's
multiplayer clearing, server-persisted player houses, and walkable interiors in
Everglade II's city remain to be wired into production rooms.

## The method

### Carve a flat city floor from the same ground function

In `everglade/world/clearing.coax`, a `Clearing` has a ragged radius
(`clearing_edge` perturbs `radius` by `rag` via value noise), an open meadow
mask (`clearing_open`), a smooth floor blend (`clearing_floor` holds exact
`level` inside the inner radius and transitions back to the zone's natural hill
height over `slope` metres), and a perimeter tree ring (`clearing_ring`). Both
CPU physics and GPU terrain shaders evaluate the exact same functions, so
houses never float above or sink into the clearing edge.

### Plots, streets, and staged construction

`everglade/world/town.coax` places a 4×4 `PlotGrid` with `TOWN_SIZE = 10.0 m`
and `TOWN_PITCH = 12.0 m`, automatically leaving 2.0 m lanes between every pair
of adjacent plots (`pl_at` returns `-1` in any lane). Rows 0–1 face south
(`turn = 0`) and rows 2–3 face north (`turn = 2`) onto the central street.
Every tier's maximum horizontal reach (`HW_REACH`) fits inside a 10 m plot with
at least 0.8 m of clearance on every side (`town.coax` enforces this in a
compile-time `#test`).

Ownership and site placement follow compare-and-set rules in `plots.coax`:
`pl_claim` refuses an already-taken plot, a player who already holds another
plot, or an out-of-range plot ID; `pl_site` refuses any caller who does not own
that plot. Once placed, every client and room computes identical construction
progress `pl_progress(site, now)` from world time alone, so small lean-tos
finish first and stone cottages finish last by day 14.

### Named gates and landmarks fail closed

Scripts, tests, and room spawns never hard-code coordinates for gates or plazas.
`everglade/world/town-points.coax` registers 18 canonical `MapPoint` entries
(`spawn`, `gate-south` at `z = -56`, `gate-north`, `gate-east`, `gate-west`,
`commons`, `market`, `guilds`, `bridge`, and more). Looking up a stale or
misspelled name like `"the-old-spawn"` returns `found = false`.

## Worked trace: clearing floor, 16-plot town, and claim refusals

```text
Clearing (34, -114), radius=36, slope=20, natural hill=7.5 m, floor=-2.0 m:
  at centre (34, -114):     clearing_open = 1.0, clearing_floor = -2.0 m (-20 dm)
  at 60 m east (94, -114):  clearing_open = 0.0, clearing_floor =  7.5 m (75 dm)
  4 m outside ragged edge:  clearing_ring > 0.9

Town grid (4x4, size=10 m, pitch=12 m, workday=1200 s):
  centre (34, -114) falls in the 2 m cross-lane -> pl_at = -1
  plot 5 (founder Finn Barrow) centre           -> pl_at = 5
  at day 3.4 (4,080 s): 5 finished, 9 mid-build, 2 waiting; all 16 done by day 14

Named points & refusals:
  eg_point("gate-south")    -> found=true, z=-56
  eg_point("the-old-spawn") -> found=false
  pl_claim(plot 5, p101)    -> ok
  pl_claim(plot 5, p102)    -> refused (plot taken, refused=1)
  pl_claim(plot 6, p101)    -> refused (player already holds plot 5, refused=2)
  pl_site(plot 5, p102)     -> refused (not plot owner, refused=3)
```

## Runnable example

[`everglade/examples/city-town-plots.coax`](../examples/city-town-plots.coax)
runs Everglade's clearing floor carving, 16-plot town book progression, named
gate lookups, and plot/site refusal paths directly against
[`everglade/world/clearing.coax`](../world/clearing.coax),
[`everglade/world/town.coax`](../world/town.coax),
[`metaverse-engine/world/plots.coax`](../../metaverse-engine/world/plots.coax),
and [`everglade/world/town-points.coax`](../world/town-points.coax) (committed
transcript
[`everglade/examples/city-town-plots.out`](../examples/city-town-plots.out)):

<!-- source: everglade/examples/city-town-plots.coax city-town-plots -->
```jai
city_run :: () -> bool {
  // 1. Level a buildable city clearing out of rolling forest hills (ground 7.5 m -> floor -2.0 m).
  c := Clearing.{cx = 34.0, cz = -114.0, radius = 36.0, edge = 8.0, rag = 0.25, slope = 20.0, ring = 12.0};
  open_mid := clearing_open(c, 34.0, -114.0);
  open_far := clearing_open(c, 34.0 + 50.0, -114.0);
  floor_mid := clearing_floor(c, 34.0, -114.0, 7.5, -2.0);
  floor_far := clearing_floor(c, 34.0 + 60.0, -114.0, 7.5, -2.0);
  ring_at := clearing_ring(c, c.cx + clearing_edge(c, c.cx + 40.0, c.cz) + 4.0, c.cz);
  print("clearing_mid_open=% far_open=% floor_mid_dm=% floor_far_dm=% ring_gt90=%\n",
    open_mid == 1.0, open_far == 0.0, cast(int) (floor_mid * 10.0), cast(int) (floor_far * 10.0), ring_at > 0.9);

  // 2. Claim Everglade's 4x4 town grid (16 plots of 10 m on 12 m pitch -> 2 m street lanes).
  g := town.town_grid(34.0, -114.0);
  b: [1] plots.PlotBook;
  b[0] = town.town_book(g, 0.0);
  founder_name := plots.pl_owner_name(b[..], town.TOWN_FOUNDER);
  in_lane := plots.pl_at(g, 34.0, -114.0);
  p5 := plots.pl_centre(g, town.TOWN_FOUNDER);
  on_plot5 := plots.pl_at(g, p5[0], p5[1]);
  // At workday 3.4: houses rise at their own tier pace (some finished, some building, some waiting).
  fin := 0; mid := 0; wait := 0;
  t_mid := 3.4 * hw.HW_WORKDAY_S;
  for i: 0..<town.TOWN_PLOTS {
    prog := plots.pl_progress(b[0].site[i], t_mid);
    if prog == f32(b[0].site[i].stages) { fin += 1; }
    else if prog > 0.0 { mid += 1; }
    else { wait += 1; }
  }
  done_days := cast(int) (plots.pl_all_done(b[..]) / hw.HW_WORKDAY_S + 0.5);
  print("plots=% founder=% lane_at=% plot5_at=% day34_fin=% mid=% wait=% all_done_days=%\n",
    plots.pl_count(g), founder_name, in_lane, on_plot5, fin, mid, wait, done_days);

  // 3. Named gates and city landmarks resolve deterministically; unknown names fail closed.
  gate := eg_point("gate-south");
  commons := eg_point("commons");
  unknown := eg_point("the-old-spawn");
  print("gate_south=% z=% commons=% unknown_refused=%\n",
    gate.found, cast(int) gate.z, commons.found, !unknown.found);

  // 4. Refusals: claiming a taken plot, holding a second plot, or building on another's plot.
  rb: [1] plots.PlotBook;
  rb[0] = plots.pl_book(g);
  ok1 := plots.pl_claim(rb[..], 5, 101, "Finn Barrow");
  dup_plot := plots.pl_claim(rb[..], 5, 102, "Greta Lark");
  dup_owner := plots.pl_claim(rb[..], 6, 101, "Finn again");
  wrong_builder := plots.pl_site(rb[..], 5, 102, town.town_site(g, 5, 0.0));
  print("claim_ok=% refuse_taken=% refuse_second=% refuse_non_owner=% refused_count=%\n",
    ok1, !dup_plot, !dup_owner, !wrong_builder, rb[0].refused);

  return open_mid == 1.0 && open_far == 0.0 && floor_mid == -2.0 && floor_far == 7.5 &&
         ring_at > 0.9 && plots.pl_count(g) == 16 && in_lane == -1 && on_plot5 == 5 &&
         fin >= 3 && mid >= 5 && wait >= 1 && done_days == 14 &&
         gate.found && commons.found && !unknown.found &&
         ok1 && !dup_plot && !dup_owner && !wrong_builder && rb[0].refused == 3;
}
```

Output (`coax/bin/coax run everglade/examples/city-town-plots.coax`, identical in `--release`):

```text
clearing_mid_open=true far_open=true floor_mid_dm=-20 floor_far_dm=75 ring_gt90=true
plots=16 founder=Finn Barrow lane_at=-1 plot5_at=5 day34_fin=5 mid=9 wait=2 all_done_days=14
gate_south=true z=-56 commons=true unknown_refused=true
claim_ok=true refuse_taken=true refuse_second=true refuse_non_owner=true refused_count=3
ok=true
```

## The checks

```shell
coax/bin/coax test --library everglade/world/clearing.coax
coax/bin/coax test --library metaverse-engine/world/plots.coax
coax/bin/coax test --library everglade/world/town.coax
coax/bin/coax test --library everglade/world/town-points.coax
coax/bin/coax run everglade/examples/city-town-plots.coax
```

## Mistakes

- **Drifting spawn and gate coordinates across scripts (`#13078`).** Early
  capture tools hard-coded gate and plaza coordinates independently of
  `plan.coax`; moving a wall left scripts staring at palisade timber. Moving
  every point into `everglade/world/town-points.coax` (`eg_point`) and
  checking it against `plan.coax` keeps every tool and test on one coordinate
  table.
- **Houses spilling into street lanes.** Before adding `HW_REACH` boundary
  checks in `everglade/world/town.coax`, wider stone-cottage eaves could
  encroach on the 2 m lane between plots. A compile-time `#test` now verifies
  every corner of all 16 houses clears its plot boundary by at least 0.8 m.

## What comes next

Walls and streets are only scenery until people stand at the gates, run the
inn, and point travellers down the right road. [Chapter 27](ch027-townspeople.md)
fills the city with townspeople, speakers, and bounded guard-guide AI.
