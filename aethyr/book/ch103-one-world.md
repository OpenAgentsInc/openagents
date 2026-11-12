# 103. One world: columns, systems and a spatial index

> **Status: partly implemented.** The module is
> [aethyr/core/world.coax](../core/world.coax) (#13274, 530 mutants, all
> killed), on the entity store of [chapter 6](ch006-entities-are-born-and-die.md)
> and the spatial grid in [aethyr/core/spatial.coax](../core/spatial.coax)
> (214 mutants, all killed), which a room's interest grid
> ([chapter 97](ch097-interest-without-leaks.md)) already ran on as
> `aethyr/net/grid.coax`. The example steps 2,000 entities through three
> systems and replays the session. Not built yet: Everglade's room, client
> and offline worker holding their players, creatures and projectiles in one
> `World` (#13275, #13276), and the creature runtime on it (#13293).

Chapter 6 gave a realm entities and left their values to the realm: "the
realm keeps its component values in its own columns". Every realm then
wrote its own columns, its own order of work and its own way to ask who is
nearby. The room server kept seats in one array, monsters in another and
arrows in a third, each with its own loop.

A `World` is the one place a realm's entities live. The client, the room
and the offline worker run the same one.

## What this chapter teaches

- Component columns keyed by slot, and why fixed columns are enough.
- An ordered system list, where each system declares what it reads and
  writes and the World refuses anything else.
- How spawns, despawns and the spatial index settle at the step's end.
- A digest of the whole World, so a replay compares it a step at a time.

## The method

A World has four parts:

- **The entity store** from chapter 6: handles, `EntityId`s, and spawns and
  despawns that apply at the flush.
- **Component columns.** There are five components: a transform, a body,
  health, a kind and a replication row. Each is one or more columns, and
  each column has one element a slot. A slot's `has` bits say which
  components it carries. A realm that needs more keeps its own columns
  beside these, one element a slot.
- **The system list.** A realm adds its systems once, in order. Each has an
  id, the components it reads and the components it writes. A step runs
  every system in that order.
- **A spatial index**: a uniform grid over the slots that carry a
  transform, which answers "which slots are within `r` metres of this
  point" without a scan.

The World is bounded. Its caps are the lengths of the storage the realm
gives it, and `world_init` takes that storage whole or refuses it. A step
allocates nothing. Archetypes, in the style of Flecs or Bevy, are more than
these realms need: an Everglade zone has a few thousand entities of a few
kinds, and fixed columns keep the layout one that a C host, a save and a
replay can all read.

## The columns

<!-- source: aethyr/core/world.coax columns -->
```jai
// The components, a bit each.
WORLD_TRANSFORM :: 1;   // where it stands and faces: WF_X WF_Y WF_Z WF_YAW
WORLD_BODY :: 2;        // how it moves and how big it is: WF_VX WF_VY WF_VZ WF_RADIUS WF_HEIGHT
WORLD_HEALTH :: 4;      // WI_HP WI_HP_MAX
WORLD_KIND :: 8;        // what it is, the realm's number: WI_KIND
WORLD_ROW :: 16;        // its replication row in the realm's table: WI_ROW
WORLD_COMPONENTS :: 5;
WORLD_ALL :: 31;

// The f32 columns and the int columns: column c's element for slot i is f[c * cap + i].
WF_X :: 0; WF_Y :: 1; WF_Z :: 2; WF_YAW :: 3;
WF_VX :: 4; WF_VY :: 5; WF_VZ :: 6; WF_RADIUS :: 7; WF_HEIGHT :: 8;
WORLD_F :: 9;
WI_HP :: 0; WI_HP_MAX :: 1; WI_KIND :: 2; WI_ROW :: 3;
WORLD_I :: 4;

// The component an f32 column belongs to, and an int column's.
world_f_component :: (col: int) -> int #effects(all) { return ifx col <= WF_YAW then WORLD_TRANSFORM else WORLD_BODY; }
world_i_component :: (col: int) -> int #effects(all) {
  if col <= WI_HP_MAX { return WORLD_HEALTH; }
  return ifx col == WI_KIND then WORLD_KIND else WORLD_ROW;
}
```

Column `c`'s element for slot `i` is `f[c * cap + i]`, so each column is
contiguous and a system that walks one column touches one run of memory.

## The World

<!-- source: aethyr/core/world.coax world -->
```jai
// The World's counters, at these places in `n` (WORLD_COUNTERS of them).
WORLD_READY :: 0;        // 1 once world_init took the storage
WORLD_TICK :: 1;         // steps run
WORLD_SYSTEMS :: 2;      // systems in the list
WORLD_STEPPING :: 3;     // 1 inside a step
WORLD_NEXT :: 4;         // the place in the list of the system due next this step
WORLD_RUNNING :: 5;      // the running system's place in the list, plus one (0 none)
WORLD_VIOLATIONS :: 6;   // accesses refused: a column the running system did not declare, or no system running in a step
WORLD_FIRST :: 7;        // the first violation's system id (0: no system was running)
WORLD_FIRST_WHAT :: 8;   // and what it asked for: the component, plus WORLD_WROTE for a write
WORLD_SKIPPED :: 9;      // systems begun out of order, and steps ended before every system ran
WORLD_AUDIT :: 10;       // 1: each system's undeclared columns are compared before and after it (world_audit)
WORLD_AUDITED :: 11;     // undeclared changes the audit found
WORLD_SEEN_HIGH :: 12;   // the slots ever used when the running system began: what its audit covers
WORLD_COUNTERS :: 13;
WORLD_WROTE :: 256;

World :: struct {
  ents: EntityStore;                 // who is here (aethyr/core/entity.coax)
  has: [] int;                       // each slot's components, a bit each
  f: [] f32;                         // the f32 columns, WORLD_F of them, one after another
  i: [] int;                         // the int columns, WORLD_I of them
  grid: SpatialGrid;                 // where each slot with a transform stood at the last step's end
  sys_id: [] int;                    // the system list, in order: each system's id (1 or more),
  sys_reads: [] int; sys_writes: [] int;   // the components it reads, and those it writes
  seen: [] int;                      // the audit's digest of each component before the running system
  n: [] int;                         // the counters
}
world_cap :: (w: World) -> int #effects(all) { return w.ents.gen.count; }

// Whether the views make a World: one element a slot in every slot view, whole columns, a grid over
// the same slots, room for a system and for the counters.
world_shaped :: (w: World) -> bool #effects(all) {
  cap := world_cap(w);   // 1 or more: a grid has a slot
  e := w.ents;
  if e.live.count != cap || e.born.count != cap || e.doomed.count != cap || e.ids.count != cap || e.free.count != cap || e.spawns.count != cap || e.n.count < ENTITY_COUNTERS { return false; }
  if w.has.count != cap || w.f.count != WORLD_F * cap || w.i.count != WORLD_I * cap { return false; }
  if !spatial_shaped(w.grid) || w.grid.at.count != cap { return false; }
  if w.sys_id.count < 1 || w.sys_reads.count != w.sys_id.count || w.sys_writes.count != w.sys_id.count { return false; }
  return w.seen.count >= WORLD_COMPONENTS && w.n.count >= WORLD_COUNTERS;
}
// An empty World over the realm's storage, its index in cells `cell` metres a side: no entities,
// no systems, tick 0. False, and nothing changed, for storage that is not a World's.
world_init :: (w: World, cell: f32) -> bool #effects(all) {
  if !world_shaped(w) { return false; }
  entity_clear(w.ents);
  for k: 0..<w.has.count { w.has[k] = 0; }
  for k: 0..<w.f.count { w.f[k] = 0; }
  for k: 0..<w.i.count { w.i[k] = 0; }
  _ := spatial_init(w.grid, cell);
  for k: 0..<w.sys_id.count { w.sys_id[k] = 0; w.sys_reads[k] = 0; w.sys_writes[k] = 0; }
  for k: 0..<w.seen.count { w.seen[k] = 0; }
  for k: 0..<w.n.count { w.n[k] = ifx k == WORLD_READY then 1 else 0; }
  return true;
}
world_ready :: (w: World) -> bool #effects(all) { return w.n.count >= WORLD_COUNTERS && w.n[WORLD_READY] == 1; }
world_tick :: (w: World) -> int #effects(all) { return w.n[WORLD_TICK]; }
world_violations :: (w: World) -> int #effects(all) { return w.n[WORLD_VIOLATIONS]; }
```

Like an `EntityStore`, a `World` is views: passing one copies only the
views, and every write reaches the realm's storage. `WorldStore` is that
storage at a fixed size, for a realm that wants one field, and
`world_of(store, cap)` is the World over its first `cap` slots.

## Systems declare what they touch

Coax has no procedure values, so the list holds each system's id and its
declarations, and the realm calls its own procedure between
`world_system_begin` and `world_system_end`. The World checks the order:
`world_system_begin` is true only for the system the list has next, and
`world_step_end` is true only when every system ran.

<!-- source: aethyr/core/world.coax systems -->
```jai
// Adds a system to the end of the list: its id (1 or more, the realm's), the components it reads
// and those it writes. A system may read what it writes. False, and nothing changed, once the
// World has stepped (the list is the session's), for an id below 1 or already in the list,
// components that are not components, or a full list.
world_system :: (w: World, id: int, reads: int, writes: int) -> bool #effects(all) {
  if !world_ready(w) || w.n[WORLD_TICK] != 0 || w.n[WORLD_STEPPING] != 0 { return false; }
  k := w.n[WORLD_SYSTEMS];
  if id < 1 || k >= w.sys_id.count || (reads | writes) < 0 || (reads | writes) > WORLD_ALL { return false; }
  for j: 0..<k { if w.sys_id[j] == id { return false; } }
  w.sys_id[k] = id; w.sys_reads[k] = reads; w.sys_writes[k] = writes;
  w.n[WORLD_SYSTEMS] = k + 1;
  return true;
}
// A step opens. False if the World is not ready or a step is open already.
world_step_begin :: (w: World) -> bool #effects(all) {
  if !world_ready(w) || w.n[WORLD_STEPPING] != 0 { return false; }
  w.n[WORLD_STEPPING] = 1; w.n[WORLD_NEXT] = 0; w.n[WORLD_RUNNING] = 0;
  return true;
}
// One component's columns as one number, over slots 0..<high (the audit's before and after: the
// slots in use when the system began, so a spawn's new slot is not a change).
world_component_digest :: (w: World, component: int, high: int) -> int #effects(all) {
  cap := world_cap(w); h := 2166136261;
  for c: 0..<WORLD_F {
    if world_f_component(c) != component { continue; }
    for s: 0..<high { h = entity_mix(h, cast(int) f32_bits(w.f[c * cap + s])); }
  }
  for c: 0..<WORLD_I {
    if world_i_component(c) != component { continue; }
    for s: 0..<high { h = entity_mix(entity_mix(h, w.i[c * cap + s]), w.i[c * cap + s] >> 32); }
  }
  return h;
}
// The system with this id runs now: true only for the system the list has next, inside a step, with
// none running. Anything else is counted in WORLD_SKIPPED and nothing runs.
world_system_begin :: (w: World, id: int) -> bool #effects(all) {
  k := w.n[WORLD_NEXT];
  if w.n[WORLD_STEPPING] != 1 || w.n[WORLD_RUNNING] != 0 || k >= w.n[WORLD_SYSTEMS] || w.sys_id[k] != id {
    w.n[WORLD_SKIPPED] = add_sat(w.n[WORLD_SKIPPED], 1);
    return false;
  }
  w.n[WORLD_RUNNING] = k + 1;
  if w.n[WORLD_AUDIT] == 1 {
    w.n[WORLD_SEEN_HIGH] = entity_high(w.ents);
    for c: 0..<WORLD_COMPONENTS { w.seen[c] = world_component_digest(w, 1 << c, w.n[WORLD_SEEN_HIGH]); }
  }
  return true;
}
// A refused access, counted; the first one is kept (WORLD_FIRST, WORLD_FIRST_WHAT).
world_violation :: (w: World, component: int, write: bool) #effects(all) {
  if w.n[WORLD_VIOLATIONS] == 0 {
    r := w.n[WORLD_RUNNING];
    w.n[WORLD_FIRST] = ifx r > 0 then w.sys_id[r - 1] else 0;
    w.n[WORLD_FIRST_WHAT] = component + (ifx write then WORLD_WROTE else 0);
  }
  w.n[WORLD_VIOLATIONS] = add_sat(w.n[WORLD_VIOLATIONS], 1);
}
// The running system is done: with the audit on, a component it did not declare a write to and
// changed all the same (past the accessors) is a violation. The list moves to its next system.
world_system_end :: (w: World) #effects(all) {
  r := w.n[WORLD_RUNNING];
  if r == 0 { return; }
  if w.n[WORLD_AUDIT] == 1 {
    for c: 0..<WORLD_COMPONENTS {
      if (w.sys_writes[r - 1] & (1 << c)) != 0 || w.seen[c] == world_component_digest(w, 1 << c, w.n[WORLD_SEEN_HIGH]) { continue; }
      w.n[WORLD_AUDITED] = add_sat(w.n[WORLD_AUDITED], 1);
      world_violation(w, 1 << c, true);
    }
  }
  w.n[WORLD_RUNNING] = 0;
  w.n[WORLD_NEXT] = r;
}
// Turns the audit on or off, between steps (false inside one).
world_audit :: (w: World, on: bool) -> bool #effects(all) {
  if !world_ready(w) || w.n[WORLD_STEPPING] != 0 { return false; }
  w.n[WORLD_AUDIT] = ifx on then 1 else 0;
  return true;
}
// Whether the caller may read (or write) a component now. Outside a step, yes: the columns are the
// realm's. Inside one, only the running system, and only what it declared (a write is a read too).
// A refusal is counted.
world_may :: (w: World, component: int, write: bool) -> bool #effects(all) {
  if w.n[WORLD_STEPPING] == 0 { return true; }
  r := w.n[WORLD_RUNNING];
  if r > 0 {
    mask := ifx write then w.sys_writes[r - 1] else w.sys_reads[r - 1] | w.sys_writes[r - 1];
    if (mask & component) != 0 { return true; }
  }
  world_violation(w, component, write);
  return false;
}
```

Two checks hold a system to its declarations:

- **The accessors.** `world_f`, `world_set_f`, `world_i` and `world_set_i`
  ask `world_may`. Inside a step, a read of a component the running system
  did not declare returns 0, a write does nothing, and both are counted in
  `WORLD_VIOLATIONS`. The first violation's system and component are kept.
- **The audit.** With `world_audit` on, the World takes a digest of every
  component before a system runs and compares it after. A change to a
  component the system did not declare, made past the accessors, is a
  violation too.

Outside a step the columns are the realm's: it fills a spawn's slot, reads
rows to replicate and loads a save without a system.

<!-- source: aethyr/core/world.coax access -->
```jai
// One element of an f32 column: 0 for a column or slot that is none, or an access refused.
world_f :: (w: World, col: int, slot: int) -> f32 #effects(all) {
  cap := world_cap(w);
  if col < 0 || col >= WORLD_F || slot < 0 || slot >= cap || !world_may(w, world_f_component(col), false) { return 0; }
  return w.f[col * cap + slot];
}
// Sets it: false, and nothing changed, for a column or slot that is none, or an access refused.
world_set_f :: (w: World, col: int, slot: int, v: f32) -> bool #effects(all) {
  cap := world_cap(w);
  if col < 0 || col >= WORLD_F || slot < 0 || slot >= cap || !world_may(w, world_f_component(col), true) { return false; }
  w.f[col * cap + slot] = v;
  return true;
}
world_i :: (w: World, col: int, slot: int) -> int #effects(all) {
  cap := world_cap(w);
  if col < 0 || col >= WORLD_I || slot < 0 || slot >= cap || !world_may(w, world_i_component(col), false) { return 0; }
  return w.i[col * cap + slot];
}
world_set_i :: (w: World, col: int, slot: int, v: int) -> bool #effects(all) {
  cap := world_cap(w);
  if col < 0 || col >= WORLD_I || slot < 0 || slot >= cap || !world_may(w, world_i_component(col), true) { return false; }
  w.i[col * cap + slot] = v;
  return true;
}
// Whether slot carries every one of these components.
world_has :: (w: World, slot: int, components: int) -> bool #effects(all) {
  return slot >= 0 && slot < world_cap(w) && (w.has[slot] & components) == components;
}
```

The declarations are also what a later scheduler needs. Two systems whose
writes don't meet the other's reads or writes can run at once, and the list
already says so.

## Spawns, despawns and the index settle at the step's end

<!-- source: aethyr/core/world.coax life -->
```jai
// A spawn carrying these components. A free slot's columns are zero (init and the settle that
// frees a slot see to it), so it starts at zero, for the realm (or the spawning system, for the
// components it writes) to fill now. It is alive, and in the index, from the step's end.
// ENTITY_NONE when every slot is in use or the components are not components.
world_spawn :: (w: World, components: int) -> Spawned #effects(all) {
  if !world_ready(w) || components < 0 || components > WORLD_ALL { return Spawned.{}; }
  made := entity_spawn(w.ents);
  if made.handle != ENTITY_NONE { w.has[entity_slot(made.handle)] = components; }
  return made;
}
// A despawn: the entity stays alive, with its columns, until the step's end.
world_despawn :: (w: World, h: Handle(Entity)) #effects(all) { entity_despawn(w.ents, h); }
world_alive :: (w: World, h: Handle(Entity)) -> bool #effects(all) { return entity_alive(w.ents, h); }
// The live entity in slot i, or ENTITY_NONE: a system's ordered walk is slots 0..<world_high.
world_at :: (w: World, i: int) -> Handle(Entity) #effects(all) { return entity_at(w.ents, i); }
world_high :: (w: World) -> int #effects(all) { return entity_high(w.ents); }
world_count :: (w: World) -> int #effects(all) { return entity_count(w.ents); }

// Applies the despawns and spawns (the entity store's flush) and brings the index up to date: a
// slot given up loses its components and its columns go back to zero, every live slot with a
// transform stands in the index where its columns say, and no other slot does.
world_settle :: (w: World) #effects(all) {
  cap := world_cap(w);
  for s: 0..<entity_high(w.ents) {
    if !w.ents.doomed[s] { continue; }
    w.has[s] = 0;
    for c: 0..<WORLD_F { w.f[c * cap + s] = 0; }
    for c: 0..<WORLD_I { w.i[c * cap + s] = 0; }
  }
  entity_flush(w.ents);
  for s: 0..<entity_high(w.ents) {
    if w.ents.live[s] && (w.has[s] & WORLD_TRANSFORM) != 0 { _ := spatial_put(w.grid, s, w.f[WF_X * cap + s], w.f[WF_Z * cap + s]); }
    else { spatial_drop(w.grid, s); }
  }
}
// The realm's setup between steps is in: what it spawned is alive and indexed. False inside a step.
world_commit :: (w: World) -> bool #effects(all) {
  if !world_ready(w) || w.n[WORLD_STEPPING] != 0 { return false; }
  world_settle(w);
  return true;
}
// The step closes: true only when every system in the list ran, in order, and none is running.
// Then spawns and despawns apply, the index is brought up to date and the tick moves on. A step
// ended early is counted in WORLD_SKIPPED and stays open.
world_step_end :: (w: World) -> bool #effects(all) {
  if w.n[WORLD_STEPPING] != 1 { return false; }
  if w.n[WORLD_RUNNING] != 0 || w.n[WORLD_NEXT] != w.n[WORLD_SYSTEMS] {
    w.n[WORLD_SKIPPED] = add_sat(w.n[WORLD_SKIPPED], 1);
    return false;
  }
  world_settle(w);
  w.n[WORLD_STEPPING] = 0;
  w.n[WORLD_TICK] = add_sat(w.n[WORLD_TICK], 1);
  return true;
}
// The slots within r metres of (x, z) as the index has them (where each stood at the last step's
// end), into out (as many as fit); how many there are. It reads transforms: 0 when refused.
world_near :: (w: World, x: f32, z: f32, r: f32, out: [] int) -> int #effects(all) {
  if !world_may(w, WORLD_TRANSFORM, false) { return 0; }
  return spatial_query(w.grid, x, z, r, out);
}
```

A free slot's columns are always zero: `world_init` zeroes them and the
settle zeroes a slot when its entity leaves. So a spawn starts at zero
without the spawning system writing components it did not declare.

The index holds where each slot stood at the last step's end. A system that
moves an entity does not move it in the index mid-step, so every system in a
step gets the same answer from `world_near`, whatever its place in the list.

## The digest

<!-- source: aethyr/core/world.coax digest -->
```jai
// The World as one number: the entity store, the tick, and each live or waiting slot's components
// and the columns it carries. Equal worlds give equal digests on every host.
world_digest :: (w: World) -> int #effects(all) {
  cap := world_cap(w);
  h := entity_mix(entity_digest(w.ents), w.n[WORLD_TICK]);
  for s: 0..<entity_high(w.ents) {
    if !w.ents.live[s] && !w.ents.born[s] { continue; }
    h = entity_mix(h, w.has[s]);
    for c: 0..<WORLD_F { if (w.has[s] & world_f_component(c)) != 0 { h = entity_mix(h, cast(int) f32_bits(w.f[c * cap + s])); } }
    for c: 0..<WORLD_I { if (w.has[s] & world_i_component(c)) != 0 { h = entity_mix(entity_mix(h, w.i[c * cap + s]), w.i[c * cap + s] >> 32); } }
  }
  return h;
}
```

## The example

[aethyr/examples/one-world.coax](../examples/one-world.coax) puts 2,000
walkers in one World and steps them 120 times, twice.

<!-- source: aethyr/examples/one-world.coax example -->
```jai
#load "../core/world.coax";
#load "../core/replay.coax";
#import "Basic";

// Two thousand walkers in one World, stepped by three systems in one order:
//   steer  reads the transform and the kind, writes the body: drifters circle the middle, seekers
//          head for it, and a walker in a crowd (the index's answer) hurries
//   move   reads the body, writes the transform
//   wear   reads the transform, writes health and the kind: far from the middle a walker tires; a
//          spent one despawns and a new one is spawned at the middle
// The session runs twice. The second run is a replay: each step's digest must be the recording's.
FIELD :: 2000;
STEPS :: 120;
SYS_STEER :: 1; SYS_MOVE :: 2; SYS_WEAR :: 3;
Session :: struct { marks: [STEPS] int; events: [1] ReplayEvent; channels: [REPLAY_SOURCES] ReplayChannel; n: [REPLAY_COUNTERS] int; }

build :: (b: [] WorldStore) -> World {
  w := world_of(b, FIELD + 400);
  _ := world_init(w, 4);
  _ := world_system(w, SYS_STEER, WORLD_TRANSFORM | WORLD_KIND, WORLD_BODY);
  _ := world_system(w, SYS_MOVE, WORLD_BODY, WORLD_TRANSFORM);
  _ := world_system(w, SYS_WEAR, WORLD_TRANSFORM, WORLD_HEALTH | WORLD_KIND);
  _ := world_audit(w, true);
  s := 13274;
  for k: 0..<FIELD {
    made := world_spawn(w, WORLD_TRANSFORM | WORLD_BODY | WORLD_HEALTH | WORLD_KIND); i := entity_slot(made.handle);
    s = (s * 1103515245 + 12345) & 0x7fffffff; _ := world_set_f(w, WF_X, i, f32((s >> 8) % 20000) / 100.0 - 100.0);
    s = (s * 1103515245 + 12345) & 0x7fffffff; _ := world_set_f(w, WF_Z, i, f32((s >> 8) % 20000) / 100.0 - 100.0);
    _ := world_set_i(w, WI_HP, i, 20); _ := world_set_i(w, WI_KIND, i, k % 2);
  }
  _ := world_commit(w);
  return w;
}
steer :: (w: World) {
  near: [8] int;
  for i: 0..<world_high(w) {
    if world_at(w, i) == ENTITY_NONE { continue; }
    x := world_f(w, WF_X, i); z := world_f(w, WF_Z, i);
    hurry: f32 = ifx world_near(w, x, z, 3, near[..]) > 6 then 2.0f32 else 1.0f32;
    if world_i(w, WI_KIND, i) == 0 { _ := world_set_f(w, WF_VX, i, -z * 0.02 * hurry); _ := world_set_f(w, WF_VZ, i, x * 0.02 * hurry); }
    else { _ := world_set_f(w, WF_VX, i, -x * 0.01 * hurry); _ := world_set_f(w, WF_VZ, i, -z * 0.01 * hurry); }
  }
}
move :: (w: World) {
  for i: 0..<world_high(w) {
    if world_at(w, i) == ENTITY_NONE { continue; }
    _ := world_set_f(w, WF_X, i, world_f(w, WF_X, i) + world_f(w, WF_VX, i));
    _ := world_set_f(w, WF_Z, i, world_f(w, WF_Z, i) + world_f(w, WF_VZ, i));
  }
}
wear :: (w: World) {
  for i: 0..<world_high(w) {
    h := world_at(w, i);
    if h == ENTITY_NONE { continue; }
    x := world_f(w, WF_X, i); z := world_f(w, WF_Z, i);
    if x * x + z * z <= 90.0 * 90.0 { continue; }
    hp := world_i(w, WI_HP, i) - 1;
    _ := world_set_i(w, WI_HP, i, hp);
    if hp > 0 { continue; }
    world_despawn(w, h);
    made := world_spawn(w, WORLD_TRANSFORM | WORLD_BODY | WORLD_HEALTH | WORLD_KIND);
    if made.handle != ENTITY_NONE { _ := world_set_i(w, WI_HP, entity_slot(made.handle), 20); _ := world_set_i(w, WI_KIND, entity_slot(made.handle), 1); }
  }
}
step :: (w: World) -> bool {
  if !world_step_begin(w) { return false; }
  if world_system_begin(w, SYS_STEER) { steer(w); world_system_end(w); }
  if world_system_begin(w, SYS_MOVE) { move(w); world_system_end(w); }
  if world_system_begin(w, SYS_WEAR) { wear(w); world_system_end(w); }
  return world_step_end(w);
}

main :: () {
  a: [1] WorldStore; b: [1] WorldStore; k: [1] Session;
  s := ReplaySession.{events = k[0].events[..], marks = k[0].marks[..], channels = k[0].channels[..], n = k[0].n[..]};
  w := build(a[..]);
  print("built: % entities, cap %, digest %\n", world_count(w), world_cap(w), world_digest(w));
  replay_record(s, world_digest(w), 13274);
  for t: 0..<STEPS { if step(w) { replay_stepped(s, world_digest(w)); } }
  print("recorded: tick %, % entities, % born in all, digest %\n", world_tick(w), world_count(w), w.ents.n[ENTITY_NEXT_ID], world_digest(w));
  print("violations %, audited %, skipped %\n", world_violations(w), w.n[WORLD_AUDITED], w.n[WORLD_SKIPPED]);
  v := build(b[..]);
  print("replay starts: %\n", replay_play(s, world_digest(v)));
  for t: 0..<STEPS { if step(v) { replay_stepped(s, world_digest(v)); } }
  print("replay matched: %, first step that differs %\n", replay_matched(s), replay_diverged(s));
  // A system that reaches past its declaration: move writes health. The write is refused and counted.
  _ := world_step_begin(v);
  if world_system_begin(v, SYS_STEER) { world_system_end(v); }
  if world_system_begin(v, SYS_MOVE) {
    print("move writes health: %, health still %\n", world_set_i(v, WI_HP, 0, 0), v.i[WI_HP * world_cap(v)]);
    world_system_end(v);
  }
  print("violations %, by system %, asked for % (health %, a write %)\n", world_violations(v), v.n[WORLD_FIRST], v.n[WORLD_FIRST_WHAT], WORLD_HEALTH, WORLD_WROTE);
  // and a step that skips a system does not close
  print("a step without wear closes: %\n", world_step_end(v));
}
```

It prints:

```text
built: 2000 entities, cap 2400, digest 1220032810
recorded: tick 120, 1898 entities, 2417 born in all, digest 2329652596
violations 0, audited 0, skipped 0
replay starts: true
replay matched: true, first step that differs -1
move writes health: false, health still 20
violations 1, by system 2, asked for 260 (health 4, a write 256)
a step without wear closes: false
```

The second run rebuilds the World and steps it again, and the replay
session of [chapter 5](ch005-replay-a-session-exactly.md) compares each
step's digest with the recording's. The last lines show the two refusals:
`move` declared a write to the transform only, so its write to health is
refused and counted, and a step that skips `wear` does not close.

## What the checks prove

- Storage is taken whole or refused, one wrong view at a time, and
  `world_init` empties whatever the storage held.
- The list takes each id once, with real components, until the first step.
- A step closes only after every system ran in the list's order.
- Inside a step, each column is read and written only by a system that
  declared it; the first violation names its system and component.
- The audit finds an undeclared write made past the accessors, and does not
  mistake a spawn or a despawn for one.
- Through random steps of moves, spawns and despawns, `world_near` returns
  exactly what a scan of the columns returns, and two Worlds given the same
  session have the same digest.

Run them with `coax/bin/coax test aethyr/core/world.coax` and
`coax/bin/coax test aethyr/core/spatial.coax`.

## What's next

Everglade's room holds its seats, monsters, guard, familiars and arrows in
one `World` (#13275), and each replicated entity's row comes from its
columns (#13276). The creature runtime (#13293) and navigation (#13281)
are systems in the same list.
