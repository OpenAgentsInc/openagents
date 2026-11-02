# 73. Things that break

> **Status: unimplemented — design draft.** Today
> [`verse/demolish.coax`](../../verse/demolish.coax) breaks Everglade's town:
> kit pieces with hit points fracture, topple, fall on each other, sleep as
> rubble and grow back. It ports the previous Rust engine's demolition
> ([destructible-buildings.md](../../verse/everglade/previous/destructible-buildings.md))
> and runs only in Everglade. This chapter plans destruction as an Aethyr
> module over Part IV's rigid bodies and contact. Nothing in `aethyr/` runs
> yet.

Destruction tests whether the engine's systems agree. The
[game-engine systems inventory](../../docs/research/game-engine-systems.md)
uses a destructible wall as its integration test: when it breaks, geometry,
collision, navigation, visibility, sound, particles, AI knowledge,
replicated state, saved state and editor previews all have to change, and
they have to agree on identity, timing, authority and lifetime.

## What this chapter teaches

- How to fracture a mesh into chunks that show the material they broke
  from.
- How a support graph decides when a piece comes loose.
- How to keep rubble cheap by putting it to sleep.
- How to make destruction deterministic, budgeted and restorable.

## Outline

1. **Pieces and damage.** A realm registers each placed kit piece with its
   rest rows and mesh bounds, and gives it an armor class and hit points
   from its ruleset ([`verse/rules/srd5.coax`](../../verse/rules/srd5.coax)).
   Up to 4,000 pieces (`DEMO_PIECES`) and 96 meshes.
2. **Fracture.** A piece at 0 hit points breaks into 2 to 24 chunks
   (`DEMO_CUTS`): Voronoi cells round seeds scattered from the dice,
   thickest near the blow. Each chunk is up to 8 planes (`DEMO_PLANES`) in
   the mesh's own space, which the shader uses to cut the piece's own
   triangles, so a chunk shows plaster, timber, brick or tile and its broken
   interior.
3. **Blasts.** An explosion throws what it breaks out from its centre at 38
   degrees over the ground (`DEMO_BLAST_UP`), up to 26 m/s for 1.6 s
   (`DEMO_BLAST_SPEED`, `DEMO_THROWN`), above the ordinary 10 m/s cap.
4. **Structure.** A piece that doesn't stand on the ground is held by the
   pieces under it, found once on a grid of 8 m cells. It comes loose when
   fewer than half its supports stand (`DEMO_HOLD` 0.5; a roof at 0.75),
   tips outward from its building, and breaks when it leans past 62
   degrees, strikes something faster than 3 m/s, or has fallen for 6 s.
5. **Physics.** Bodies step at a fixed 60 Hz (`DEMO_HZ`, at most 6 steps a
   frame) with box contacts by impulses at the corners (restitution 0.12,
   friction 0.7). A chunk slow and held for 0.6 s sleeps as rubble until a
   blow, a blast or the loss of its support wakes it. In Aethyr these come
   from [chapters 18–22](ch018-rigid-bodies-and-time-steps.md) instead of a
   private solver.
6. **Budgets.** At most 260 chunks on a desktop and half on a phone
   (`DEMO_CHUNKS`), and 40 loose pieces. A new chunk replaces the oldest
   sleeping one, and a loose piece past the cap breaks where it stands.
7. **Everything else that must change.** Breaks post events (32 a frame) for
   dust and splinters ([chapter 72](ch072-effects-made-of-particles.md)),
   sound ([chapter 68](ch068-sound-in-the-world.md)) and the standing
   surfaces you walk on ([chapter 8](ch008-ground-and-collision.md)). Chunks
   in water float or sink by material (wood and cloth float at 600 kg/m³)
   ([chapter 70](ch070-rivers-ponds-and-the-sea.md)).
8. **Authority and saving.** The host owns building state. Reliable events
   (`PieceDamaged`, `PieceBroken { building, piece, pattern, seed }`,
   `BuildingSettled`) let clients rebuild chunks from pattern and seed;
   awake bodies replicate inside the 64 m radius, at most 64 per viewer
   ([chapter 50](ch050-shared-state-over-the-network.md)).
9. **Regrowth.** Every piece belongs to a group. Restoring a group stands
   its pieces back up and clears their chunks, dust and splinters.

## What exists today

- [verse/demolish.coax](../../verse/demolish.coax): fracture, throw,
  structure, physics, budgets, regrowth and water, with 18 `#test` checks.
- [verse/everglade/previous/destructible-buildings.md](../../verse/everglade/previous/destructible-buildings.md):
  the original design, its networking decision and its delivery plan
  (D0–D3).
- [aethyr/stand.coax](../../aethyr/stand.coax): the top of each standing
  piece as a floor you walk on.

## What it builds

- `aethyr/break/fracture.coax`: Voronoi cells, chunk planes and the cut
  shader inputs.
- `aethyr/break/structure.coax`: the support graph, loosening and toppling.
- `aethyr/break/events.coax`: break events, regrowth and their replication
  and save records.

## Acceptance checks and oracles

The chapter is done when:

- The same blows from the same seed give the same rubble, bit for bit, on
  one build profile.
- Every `#test` in today's `verse/demolish.coax` passes against the Aethyr
  modules.
- The destructible-wall test passes: one wall breaks, and collision, the
  standing surfaces, effects, sound, replicated state and the saved realm
  all agree on which pieces exist afterward.
- Chunk and loose-piece counts never exceed their budgets on either tier.
- Restoring a group returns every piece to its rest rows exactly.
- Everglade holds 60 FPS during a full building collapse.

## Dependencies

Chapters 6 (entities), 8 (ground), 16 (budgets), 18–22 (rigid bodies and
contact), 50 (shared state), 51 (the realm document, for groups) and 72
(effects). From Coax: R15 strict math for the deterministic profile.
