# 6. Entities are born and die

> Engine-core task: `ec-entity` (#12609). The module is
> [aethyr/entity.coax](../entity.coax), with its reference model in
> [aethyr/tests/entity-oracle.coax](../tests/entity-oracle.coax). It
> generalizes the entity system in
> [verse/deadgears/ecs.coax](../../verse/deadgears/ecs.coax).

A realm is full of things that appear and disappear: a thrown cone, a
minion, a spark, or a lamp that a visitor places. Systems refer to them
across steps. A guard chases a player, and a camera follows a car. Saves
store them, networking names them, editing changes them, and analytics
counts them.

Two failures recur:

- A reference outlives its target and reaches the next thing put in the
  same slot.
- A system destroys a thing while a loop walks the list. The list shifts,
  and the loop skips or repeats an element.

Aethyr prevents both. Handles know their generation, and spawns and despawns
wait until the end of the step.

## What this chapter teaches

- Why a slot index alone isn't an identity, and how generations fix that.
- The difference between a handle, for use inside a step, and an
  `EntityId`, for saves, the network, and edits.
- How spawns and despawns wait for the step's flush.
- How to check a store against a plain list of IDs.

## The method

An entity has two identities:

- A **handle** packs a slot and the slot's generation into one `int`. Use it
  inside a step: it's fast, and `entity_alive` checks it. A despawn raises
  the slot's generation, so an old handle fails `entity_alive` instead of
  reaching the slot's next owner.
- An **`EntityId`** comes from a counter in the store. Use it for anything
  that outlives a step: saves, network messages, and edits. The same session
  assigns the same IDs on replay ([chapter 5](ch005-replay-a-session-exactly.md)),
  and `entity_find` gives the handle for an ID. Nothing durable stores a
  handle.

Spawns and despawns are commands:

1. `entity_spawn` reserves a slot and an `EntityId`. The realm can fill the
   slot's component columns at once, but the entity isn't alive yet.
2. `entity_despawn` marks a live entity. It stays alive and visible.
3. `entity_flush`, at the end of the step, applies the despawns in slot
   order and then the spawns in the order they were asked for.

So every system in a step sees the same entities. A system can despawn what
it's walking, and a spawned entity takes part from the next step.

## The store

The realm owns the storage. An `EntityStore` holds views of the realm's
arrays, one element per slot, so passing it copies only the views. The
realm keeps its component values in its own columns, beside the store, as
Dead Gears does.

<!-- source: aethyr/entity.coax store -->
```jai
ENTITY_SLOT_BITS :: 20;
ENTITY_SLOT_MASK :: 1048575;   // (1 << 20) - 1
ENTITY_GEN_MASK :: 4095;       // 12 bits: 4,096 owners a slot, then it is retired
ENTITY_NONE :: -1;             // no handle
ENTITY_NO_ID :: 0;             // no EntityId: IDs start at 1

// The store's counters, at these places in `n` (ENTITY_COUNTERS of them).
ENTITY_HIGH :: 0;      // slots ever used
ENTITY_FREE :: 1;      // free slots waiting in `free`
ENTITY_LIVE :: 2;      // live entities
ENTITY_DOOMED :: 3;    // live entities that despawn at the next flush
ENTITY_SPAWNS :: 4;    // spawns waiting in `spawns`
ENTITY_NEXT_ID :: 5;   // the next EntityId, less one
ENTITY_RETIRED :: 6;   // slots retired because their generation would wrap
ENTITY_REFUSED :: 7;   // spawns refused because every slot was in use
ENTITY_COUNTERS :: 8;

EntityStore :: struct {
  gen: [] int;       // each slot's generation
  live: [] bool;     // the slot holds a live entity
  born: [] bool;     // the slot holds an entity that is live from the next flush
  doomed: [] bool;   // the slot's entity despawns at the next flush
  ids: [] int;       // each slot's EntityId, or ENTITY_NO_ID
  free: [] int;      // free slots, a stack
  spawns: [] int;    // slots spawned since the last flush, in order
  n: [] int;         // the counters
}
```

A generation has 12 bits. A slot whose generation would wrap after 4,096
owners is retired instead of reused, and `ENTITY_RETIRED` counts it. A
spawn when every slot is in use is refused, and `ENTITY_REFUSED` counts it.

## Handles and IDs

<!-- source: aethyr/entity.coax handles -->
```jai
// Whether h names a live entity: its slot is in use, holds a live entity, and has h's generation.
entity_alive :: (w: EntityStore, h: int) -> bool {
  if h < 0 { return false; }
  i := entity_slot(h);
  return i < w.n[ENTITY_HIGH] && w.live[i] && w.gen[i] == entity_generation(h);
}
// The live entity in slot i, or ENTITY_NONE: the ordered walk is slots 0..<entity_high.
entity_at :: (w: EntityStore, i: int) -> int {
  if i < 0 || i >= w.n[ENTITY_HIGH] || !w.live[i] { return ENTITY_NONE; }
  return entity_handle(i, w.gen[i]);
}
// A live entity's EntityId, or ENTITY_NO_ID.
entity_id :: (w: EntityStore, h: int) -> int {
  if !entity_alive(w, h) { return ENTITY_NO_ID; }
  return w.ids[entity_slot(h)];
}
// The live entity with EntityId id, or ENTITY_NONE: how a save, a message or an edit finds it.
entity_find :: (w: EntityStore, id: int) -> int {
  if id == ENTITY_NO_ID { return ENTITY_NONE; }
  for i: 0..<w.n[ENTITY_HIGH] { if w.live[i] && w.ids[i] == id { return entity_handle(i, w.gen[i]); } }
  return ENTITY_NONE;
}
```

Walking slots `0..<entity_high` in order, with `entity_at`, is the only way
to visit entities. The order is the same on every run, which saves and
replays need.

## Spawns, despawns, and the flush

<!-- source: aethyr/entity.coax commands -->
```jai
// A spawn: the new entity's handle and EntityId, or ENTITY_NONE and no ID when every slot is in
// use (counted in ENTITY_REFUSED). Its slot is the realm's to fill now; it is alive from the
// next flush.
Spawned :: struct { handle: int = ENTITY_NONE; id: int; }
entity_spawn :: (w: EntityStore) -> Spawned {
  i: int;
  if w.n[ENTITY_FREE] > 0 {
    w.n[ENTITY_FREE] -= 1;
    i = w.free[w.n[ENTITY_FREE]];
  } else if w.n[ENTITY_HIGH] < w.gen.count {
    i = w.n[ENTITY_HIGH];
    w.n[ENTITY_HIGH] += 1;
  } else {
    w.n[ENTITY_REFUSED] += 1;
    return Spawned.{};
  }
  w.n[ENTITY_NEXT_ID] += 1;
  w.born[i] = true;
  w.ids[i] = w.n[ENTITY_NEXT_ID];
  w.spawns[w.n[ENTITY_SPAWNS]] = i;
  w.n[ENTITY_SPAWNS] += 1;
  return Spawned.{handle = entity_handle(i, w.gen[i]), id = w.ids[i]};
}
// A despawn: the entity stays alive and visible until the next flush. Asking twice is asking once.
entity_despawn :: (w: EntityStore, h: int) {
  if !entity_alive(w, h) { return; }
  i := entity_slot(h);
  if !w.doomed[i] { w.doomed[i] = true; w.n[ENTITY_DOOMED] += 1; }
}
entity_doomed :: (w: EntityStore, h: int) -> bool { return entity_alive(w, h) && w.doomed[entity_slot(h)]; }

// The step's end: the despawns in slot order, then the spawns in the order they were asked for.
// A despawned slot's generation goes up and the slot is free again, unless its generation would
// wrap: then it is retired and counted.
entity_flush :: (w: EntityStore) {
  for i: 0..<w.n[ENTITY_HIGH] {
    if !(w.live[i] && w.doomed[i]) { continue; }
    w.live[i] = false; w.doomed[i] = false; w.ids[i] = ENTITY_NO_ID;
    w.n[ENTITY_LIVE] -= 1;
    if w.gen[i] == ENTITY_GEN_MASK { w.n[ENTITY_RETIRED] += 1; continue; }
    w.gen[i] += 1;
    w.free[w.n[ENTITY_FREE]] = i;
    w.n[ENTITY_FREE] += 1;
  }
  w.n[ENTITY_DOOMED] = 0;
  for k: 0..<w.n[ENTITY_SPAWNS] {
    i := w.spawns[k];
    w.born[i] = false; w.live[i] = true;
    w.n[ENTITY_LIVE] += 1;
  }
  w.n[ENTITY_SPAWNS] = 0;
}
```

The free list is a stack. The flush pushes freed slots in slot order, so the
slot that a later spawn takes depends only on the session.

## Saves and digests

`entity_digest` turns a store into one number, using FNV-1a over the
counters, every used slot, the free list, and the waiting spawns. Two equal
stores have equal digests, so a replay can compare them each step.
`entity_copy` saves a store into another one, or loads it back.

<!-- source: aethyr/entity.coax digest -->
```jai
// The store as one number (FNV-1a over the counters and every used slot): equal stores give equal
// digests, so a replay (chapter 5) can compare them a step at a time.
entity_mix :: (h: int, v: int) -> int { return ((h ^ (v & 4294967295)) * 16777619) & 4294967295; }
entity_digest :: (w: EntityStore) -> int {
  h := 2166136261;
  for c: 0..<ENTITY_COUNTERS { h = entity_mix(h, w.n[c]); }
  for i: 0..<w.n[ENTITY_HIGH] {
    flags := (ifx w.live[i] then 1 else 0) + (ifx w.born[i] then 2 else 0) + (ifx w.doomed[i] then 4 else 0);
    h = entity_mix(entity_mix(entity_mix(h, w.gen[i]), w.ids[i]), flags);
  }
  for k: 0..<w.n[ENTITY_FREE] { h = entity_mix(h, w.free[k]); }
  for k: 0..<w.n[ENTITY_SPAWNS] { h = entity_mix(h, w.spawns[k]); }
  return h;
}
// Copies a store into another of the same capacity: a save, or a load. False if they differ.
entity_copy :: (from: EntityStore, to: EntityStore) -> bool {
  if from.gen.count != to.gen.count || from.n.count < ENTITY_COUNTERS || to.n.count < ENTITY_COUNTERS { return false; }
  for i: 0..<from.gen.count {
    to.gen[i] = from.gen[i]; to.live[i] = from.live[i]; to.born[i] = from.born[i];
    to.doomed[i] = from.doomed[i]; to.ids[i] = from.ids[i]; to.free[i] = from.free[i]; to.spawns[i] = from.spawns[i];
  }
  for c: 0..<ENTITY_COUNTERS { to.n[c] = from.n[c]; }
  return true;
}
```

## An example

The example [aethyr/examples/born-and-gone.coax](../examples/born-and-gone.coax)
spawns three cones, despawns one while it walks them, and spawns another:

<!-- source: aethyr/examples/born-and-gone.coax example -->
```jai
#load "../entity.coax";
#import "Basic";

// Three cones are thrown in one step and take part from the next. A system despawns the first
// while it walks them; it stays alive until the flush. A new cone then takes the freed slot
// with a new generation, so the old handle no longer reaches it, and it gets a new EntityId.
main :: () {
  // The realm owns the storage: eight slots.
  gen: [8] int; live: [8] bool; born: [8] bool; doomed: [8] bool;
  ids: [8] int; free: [8] int; spawns: [8] int; n: [ENTITY_COUNTERS] int;
  w := EntityStore.{gen = gen[..], live = live[..], born = born[..], doomed = doomed[..], ids = ids[..], free = free[..], spawns = spawns[..], n = n[..]};
  first := entity_spawn(w);
  for k: 0..<2 { made := entity_spawn(w); print("spawned id % in slot %\n", made.id, entity_slot(made.handle)); }
  print("before the flush: alive %, count %\n", entity_alive(w, first.handle), entity_count(w));
  entity_flush(w);
  print("after the flush: alive %, count %\n", entity_alive(w, first.handle), entity_count(w));
  for i: 0..<entity_high(w) {
    h := entity_at(w, i);
    if i == 0 { entity_despawn(w, h); }
    print("walk: slot %, id %, alive %, despawning %\n", i, entity_id(w, h), entity_alive(w, h), entity_doomed(w, h));
  }
  entity_flush(w);
  next := entity_spawn(w);
  entity_flush(w);
  print("new cone: id % in slot %, generation %\n", next.id, entity_slot(next.handle), entity_generation(next.handle));
  print("old handle alive %, id 1 found %, id 4 found %\n", entity_alive(w, first.handle), entity_find(w, 1) != ENTITY_NONE, entity_find(w, 4) == next.handle);
}
```

```shell
coax/bin/coax run aethyr/examples/born-and-gone.coax
```

```text
spawned id 2 in slot 1
spawned id 3 in slot 2
before the flush: alive false, count 0
after the flush: alive true, count 3
walk: slot 0, id 1, alive true, despawning true
walk: slot 1, id 2, alive true, despawning false
walk: slot 2, id 3, alive true, despawning false
new cone: id 4 in slot 0, generation 1
old handle alive false, id 1 found false, id 4 found true
```

The cones aren't alive until the flush. The despawned cone is still alive
during the walk. The new cone takes slot 0 with generation 1, so the first
cone's handle doesn't reach it, and it gets `EntityId` 4, not 1.

## Checks

The reference model doesn't use slots, generations, or a free list. It keeps
the live `(EntityId, value)` pairs in a list ordered by ID. A seeded session
spawns zero to three entities each step, writes a value into a component
column for each, walks the live entities, and despawns about one in five.
After every flush, the store and the model must hold the same IDs with the
same values, and nothing else.

<!-- source: aethyr/entity.coax model -->
```jai
#forall "a store matches a plain list of IDs and values after every flush" seed: 0..<24 {
  b: [1] OracleBacking; w := oracle_store(b[..]);
  m: [1] OracleModel;
  for s: 0..<60 {
    oracle_session(w, b[0].value[..], m[..], seed, s, s + 1);
    #assert oracle_agrees(w, b[0].value[..], m[..]);
  }
  #assert m[0].next_id == w.n[ENTITY_NEXT_ID] && m[0].next_id > 30;
}
```

Despawning while walking is checked over 32 random walks. Each walk
despawns the entity it visits, or a later one, or none. Every live entity
must be visited once, every despawned one must stay alive until the flush,
and after the flush exactly the despawned ones must be gone.

<!-- source: aethyr/entity.coax walk -->
```jai
#forall "despawning while walking: every live entity is visited once, and none after its flush" seed: 0..<32 {
  b: [1] OracleBacking; w := oracle_store(b[..]);
  for k: 0..<40 { #assert entity_spawn(w).id == k + 1; }
  entity_flush(w);
  r := seed * 7919 + 1;
  visits: [ORACLE_SLOTS] int;
  doomed: [ORACLE_SLOTS] bool;
  before := entity_count(w);
  gone := 0;
  for i: 0..<entity_high(w) {
    h := entity_at(w, i);
    if h == ENTITY_NONE { continue; }
    visits[i] += 1;
    // despawn this one, or a later one, or none
    r = oracle_rng(r);
    pick := (r / 65536) % 3;
    target := ifx pick == 0 then h else entity_at(w, (i + 1 + (r / 8) % 7) % entity_high(w));
    if pick < 2 && target != ENTITY_NONE && !doomed[entity_slot(target)] {
      entity_despawn(w, target); doomed[entity_slot(target)] = true; gone += 1;
    }
    #assert entity_alive(w, h);    // despawned or not, it is alive until the flush
  }
  for i: 0..<entity_high(w) { #assert visits[i] == 1; }
  #assert entity_count(w) == before && w.n[ENTITY_DOOMED] == gone && gone > 0;
  entity_flush(w);
  #assert entity_count(w) == before - gone && w.n[ENTITY_DOOMED] == 0;
  for i: 0..<entity_high(w) { #assert (entity_at(w, i) == ENTITY_NONE) == doomed[i]; }
}
```

IDs must survive a save, a load, and a replay. A session of 80 steps is
saved after step 40. The save is loaded into a fresh store, which runs steps
40 to 80 again. A third store replays all 80 steps from nothing. All three
must end with the same digest and the same `EntityId` in every slot.

<!-- source: aethyr/entity.coax save -->
```jai
#forall "IDs survive save, load and replay" seed: 0..<16 {
  // one session, 80 steps; saved after step 40
  b: [1] OracleBacking; w := oracle_store(b[..]);
  model: [1] OracleModel;
  none := model[0..<0];
  oracle_session(w, b[0].value[..], none, seed, 0, 40);
  saved: [1] OracleBacking; s := oracle_store(saved[..]);
  #assert entity_copy(w, s);
  for i: 0..<ORACLE_SLOTS { saved[0].value[i] = b[0].value[i]; }
  oracle_session(w, b[0].value[..], none, seed, 40, 80);
  // load the save into a fresh store and run the rest again
  loaded: [1] OracleBacking; l := oracle_store(loaded[..]);
  #assert entity_copy(s, l) && entity_digest(l) == entity_digest(s);
  for i: 0..<ORACLE_SLOTS { loaded[0].value[i] = saved[0].value[i]; }
  oracle_session(l, loaded[0].value[..], none, seed, 40, 80);
  // replay the whole session from nothing
  again: [1] OracleBacking; a := oracle_store(again[..]);
  oracle_session(a, again[0].value[..], none, seed, 0, 80);
  #assert entity_digest(l) == entity_digest(w) && entity_digest(a) == entity_digest(w);
  for i: 0..<ORACLE_SLOTS {
    #assert loaded[0].ids[i] == b[0].ids[i] && again[0].ids[i] == b[0].ids[i] && loaded[0].value[i] == b[0].value[i];
  }
  // every ID live at the save that is still live maps to the same value
  for i: 0..<ORACLE_SLOTS {
    if !saved[0].live[i] { continue; }
    h := entity_find(w, saved[0].ids[i]);
    if h != ENTITY_NONE { #assert b[0].value[entity_slot(h)] == saved[0].value[i] && entity_slot(h) == i; }
  }
  #assert entity_count(w) > 0 && w.n[ENTITY_NEXT_ID] > 40 && w.n[ENTITY_RETIRED] == 0;
}
```

```shell
coax/bin/coax test aethyr/entity.coax --library
coax/bin/coax mutate aethyr/entity.coax --library --root-only
```

`aethyr/entity.coax` has seven checks. They cover:

- stale handles, and IDs that are never reused;
- retiring a slot whose generation would wrap, and refusing a spawn when the
  store is full;
- despawning while walking;
- the reference model;
- save, load, and replay;
- the digest and the capacity check in `entity_copy`;
- a pinned digest value, and `entity_clear` and `entity_copy` carrying every
  field.

`coax mutate` makes 197 mutants of `aethyr/entity.coax`, and the checks kill all 197.

## Still to do

The realm migration ([chapter 64](ch064-every-realm-runs-on-aethyr.md))
moves the realms onto this module:

- Dead Gears' `Entities` (`verse/deadgears/ecs.coax`), with its component
  bits, becomes a store from this module. Its spawns then wait for the flush
  too.
- Realms that keep fixed arrays addressed by index, such as Orbit League's
  bodies, give their things handles and IDs.
- The realm board's rows (`verse/edit/edit.coax`) refer to entities by
  `EntityId`.

A map from `EntityId` to handle can replace the scan in `entity_find` when a
realm has many entities; it must give the same answers.

## Dependencies

Entities live in the realm's state ([chapter 3](ch003-state-starts-from-its-defaults.md)).
Replay ([chapter 5](ch005-replay-a-session-exactly.md)) compares
`entity_digest` each step. From Coax, data layout
([Coax chapter 13](../../coax/book/ch13-data-layout.md)) and R16 snapshots
of entity identities and generations
([Coax roadmap](../../coax/ROADMAP.md)) come later. For background, see the
[game-engine systems inventory, section 7](../../docs/research/game-engine-systems.md).
