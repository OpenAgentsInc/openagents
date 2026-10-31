# 6. Entities are born and die

> **Status: unimplemented — design draft.** Dead Gears has a small entity
> system (`verse/deadgears/ecs.coax`: generation-checked handles, deferred
> destroy, component bits); every other realm keeps fixed arrays of its own
> things addressed by index. This chapter plans one identity and lifetime
> scheme for every realm. Engine-core task: `ec-entity` (#12609); the
> engine-core agent fills this chapter in as the module lands. Nothing here
> runs yet.

A realm is full of things that appear and disappear: a thrown cone, a
minion, a spark, a visitor's placed lamp. Systems refer to them across steps
(a guard chasing a player, a camera following a car), saves store them,
networking names them, editing changes them, analytics counts them. Two
failures recur. A reference outlives its target and silently reaches the
next thing put in the same slot. A thing destroyed while a loop walks the
list shifts the list and skips or repeats an element. Aethyr answers both
with handles that know their generation and with structural changes deferred
to the end of the step.

## What this chapter teaches

- Slots, generations and handles; why an index alone is not an identity.
- Stable identities that survive save, load and replay, separate from slots.
- Deferred spawn and despawn within a step.
- Ordered iteration as the only iteration.

## The method

Two identities, for two jobs:

```jai
Handle   = (generation << 20) | slot      // runtime: fast, checked, reused slots
alive(h) = slot < high && live[slot] && gen[slot] == generation(h)
EntityId = u64 from state.next_id++       // durable: saves, network, edits
```

A handle is the fast path inside a step. Destroying bumps the slot's
generation, so a stale handle fails `alive` instead of reaching the slot's
next owner. Generations wrap after 4,096 reuses of one slot; the check
reports a wrap so the slot can be retired rather than risk a false match.

An `EntityId` is assigned from a counter in the realm's state, so the same
session assigns the same IDs on replay (chapter 5) and a save records them
directly. A map from `EntityId` to `Handle` is rebuilt on load; nothing
durable stores a handle.

Structural changes are commands. `spawn` reserves an ID and queues the new
entity's initial components; `despawn` marks the entity. Both apply at the
step's flush, in a defined order (despawns in slot order, then spawns in
queue order), so a system may despawn what it is walking and every system in
a step sees the same set of entities. A spawned entity takes part from the
next step.

## What exists today

- [verse/deadgears/ecs.coax](../../verse/deadgears/ecs.coax): `Entities`,
  `ecs_handle` with `ECS_INDEX_BITS :: 20` and `ECS_GEN_MASK :: 4095`,
  `ecs_create` reusing free slots, `ecs_destroy` deferred until `ecs_flush`,
  `ecs_remove_later`, component bits checked by `ecs_match`, and slot-order
  walking described as "the ordered view (saves, replays, determinism)".
  Spawn is immediate rather than deferred, and there is no durable ID.
- [verse/edit/edit.coax](../../verse/edit/edit.coax) and the realm board:
  rows identified by the server (`coax/web/api/migrations/0050_realm_edit.sql`),
  a durable identity outside the simulation.
- Orbit League's fixed body indices (`BALL`, `YOU`, `BOT`, `MATE`, `RIVAL`)
  and the room server's seats ([coax/web/room/room.coax](../../coax/web/room/room.coax)).

## What it builds

- `aethyr/entity.coax`: slot store, `Handle`, `EntityId`, the ID map,
  `spawn`, `despawn`, `flush`, ordered iteration, and component columns
  declared by the realm beside its entities (the Dead Gears pattern,
  generalized).
- Snapshot support: the slot table, generations, free list, `next_id` and
  pending commands are part of the state digest and snapshot (chapter 5).

## Acceptance checks and oracles

- IDs survive save, load and replay: a session that spawns and despawns
  hundreds of entities is saved mid-way and reloaded; every `EntityId` maps
  to the same entity, and replay assigns the same IDs.
- Despawning during iteration is safe and checked: property checks despawn
  random subsets while walking and require every live entity visited exactly
  once and none visited after its flush.
- A stale handle never reaches a slot's next owner; a generation wrap is
  reported.
- Oracle: a reference model using a plain ordered dictionary of `EntityId`
  to component values, compared after every flush.
- Mutation testing of `aethyr/entity.coax`: 0 survivors.

## Dependencies

[Chapter 3](ch003-state-starts-from-its-defaults.md) (entities live in the
realm's constructed state) and [chapter 5](ch005-replay-a-session-exactly.md)
(digests and snapshots). From Coax: data layout
([Coax chapter 13](../../coax/book/ch13-data-layout.md)) and R16 snapshots of
entity identities and generations ([Coax roadmap](../../coax/ROADMAP.md)).
Background: [game-engine systems inventory, section 7](../../docs/research/game-engine-systems.md).
