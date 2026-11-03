# 58. Everglade

> **Status: unimplemented — design draft.** Everglade runs today on its own
> private code at `/realms/everglade`:
> [everglade.coax](../../apps/openagents-web/everglade.coax) and its many
> `everglade-*.coax` files over [verse/everglade/](../../verse/everglade/plan.coax).
> This chapter plans rebuilding it only from Aethyr's earlier chapters.
> Nothing here runs yet.

Everglade is the Verse's hub: a forest city behind a palisade with four
gates, a commons with a fountain and a quest board, a market, an inn, guild
halls, a temple, an observatory, a river crossing and trails out to the deep
wood. Its people remember you, its watch keeps order, its spells burn and
smoke, its day turns, and it is the largest realm, so it is where budgets,
streaming and frame time are decided.

## What this chapter teaches

- A large hand-built realm composed from Aethyr's chapters, with a fixed plan
  quests and characters can rely on.
- Level of detail and streaming a forest so a phone draws a few hundred
  kit trees.
- Characters with routines, memory and bounded decisions.
- Effects (spells, fire, smoke, water) as engine systems, not page code.

## The method

- **Plan** ([chapters 2](ch002-spaces-and-units.md) and
  [6](ch006-entities-are-born-and-die.md)): `plan.coax`'s boxes, path graph
  and character spots in world metres (fountain at the origin, +z north,
  +x west), every building and stall an entity with a stable ID.
- **Forest and bounds** ([chapters 7](ch007-ask-the-world-where-things-are.md),
  [8](ch008-ground-and-collision.md), [38](ch038-stream-a-large-world.md)):
  trunks and rocks near you as round colliders; kit meshes in view, cones
  beyond; a thicket at the world's edge, never an invisible wall.
- **People and the watch** ([chapters 1](ch001-a-frame-has-a-fixed-step.md),
  [9](ch009-a-character-moves.md), [14](ch014-figures-and-animation.md)):
  routines on the town's shared clock, beats walked along the path graph,
  figures with clips. A guard's choice among authored lines may come from a
  decision model; its absence never breaks play.
- **Quests** ([chapter 3](ch003-state-starts-from-its-defaults.md)): data an
  agent can write (GO, TALK, RETURN steps) and progress in the save slot.
- **Look** ([chapters 11](ch011-the-wgsl-renderer.md)–[13](ch013-light-and-shadow.md)):
  the painterly style at the realm's time of day, lamps and shadows.
- **Effects** ([chapters 23](ch023-particles-and-their-neighbors.md),
  [24](ch024-liquids-from-particles.md), [28](ch028-smoke-on-a-grid.md)):
  spells' fire and smoke as particles, the river on the water system; the
  game layer may keep visual-only effects where the simulation is not needed.
- **Others, edits and forks** ([chapters 49](ch049-rooms-and-presence.md),
  [52](ch052-edit-a-realm-in-place.md), [55](ch055-fork-a-realm.md)):
  other players, admin edit mode with UNDO and voice edits, and forks with
  `night`, `waves`, `nodestroy` and `lowgrav` as declared rules.
- **Budgets** ([chapter 16](ch016-budgets-that-refuse.md)): kit trees,
  vertices and particle counts per tier.

## What exists today

- [docs/verse/everglade.md](../../docs/verse/everglade.md): the plan, the map
  ([everglade-map.svg](../../docs/verse/everglade-map.svg)), its people and
  edit mode; the Rust build's specifications in
  [verse/everglade/previous/](../../verse/everglade/previous/README.md).
- [verse/everglade/](../../verse/everglade/plan.coax): `plan.coax`,
  `routines.coax`, `watch.coax` (SRD 5.2.1 guards, escalation),
  `quests.coax`, `people.coax`, `rumours.coax`, `river.coax`, the guard
  decision path (`guard-*.coax`,
  [pylon-guard-decisions.md](../../docs/verse/pylon-guard-decisions.md)).
- The page: `everglade.coax` (4,643 lines) and its parts: forest, spells
  (`everglade-spells.coax`, drawn by `everglade-fx.wgsl` through
  [aethyr/particles.coax](../../aethyr/particles.coax)), storm, tide, frost,
  interiors, the doors and rubble, the fork pane and the edit mode.
- Walking on [aethyr/player.coax](../../aethyr/player.coax), with
  `forest_bodies` and `everglade_hold` at the edge; the asset pack from
  [verse/packs/everglade.json](../../verse/packs/everglade.json).
- [coax/web/perf.ts](../../coax/web/perf.ts): the 60 FPS check at four
  places and times of day, at DPR 1 and 2, casting a meteor every 6.7 s.

## What it builds

- Everglade's content (plan, people, quests, watch, rules) over `aethyr/`;
  its private loop, ground, camera and effect plumbing deleted.

## Acceptance checks and oracles

- `perf.ts` passes at every case: 95th percentile frame at most 17.5 ms and
  none over 33 ms, before and after the move.
- No open ground is behind an invisible wall: a walk along every trail
  reaches its clearing, and the edge stops you only in the thicket.
- Every quest can be completed by a scripted walk replayed from its inputs.
- A fork with each rule on changes only what the rule declares
  (chapter 55's isolation check); the original city is unchanged.
- `everglade.png`'s golden changes only where a fix is visible.
- Everglade's `#test`s keep passing.

## Dependencies

Chapters 1–16, 23, 24, 28, 38, 49, 52 and 55.
