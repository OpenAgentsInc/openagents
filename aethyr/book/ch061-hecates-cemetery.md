# 61. Hecate's Cemetery

> **Status: unimplemented — design draft.** Hecate's Cemetery runs today on
> its own private code at `/realms/hecates-cemetery`:
> [villa.coax](../../apps/openagents-web/villa.coax) with `villa.wgsl`,
> `villa-face.c` and the pet memorials served by `coax/web/api/villa.c`.
> This chapter plans rebuilding it only from Aethyr's earlier chapters.
> Nothing here runs yet.

Hecate's Cemetery is the first expansion of Planetary Villa, Kiki's world on
the Verse: a moonlit memorial garden you walk with a glowing white dog.
Tombstone plots line the garden paths among hedges, cypresses and a fountain,
an open field lies through the east gate, and each tombstone can be a pet's
memorial, with its photos, its story and notes from those who knew it.

## What this chapter teaches

- An arrival flight that hands the camera to the player without a jump.
- A companion that follows by the paths, not through the hedges.
- A single key light: the moon in its real phase, outlines and bloom for
  legibility in the dark.
- People's own media in a realm, uploaded, scanned and shared.

## The method

- **Arrival** ([chapters 1](ch001-a-frame-has-a-fixed-step.md),
  [10](ch010-the-camera-follows.md)): the flight's keys (out in space toward
  the planet, a cut down over the garden to the south gate) evaluated against
  the clock, skippable by any input, blended into the follow camera.
- **Ground and paths** ([chapters 7](ch007-ask-the-world-where-things-are.md),
  [8](ch008-ground-and-collision.md)): a heightfield levelled along the paths,
  plot and hedge colliders, the path graph the dog routes along.
- **Walking and the dog** ([chapters 6](ch006-entities-are-born-and-die.md),
  [9](ch009-a-character-moves.md)): the player controller; the dog an entity
  following along the path graph.
- **Look** ([chapters 12](ch012-materials.md)–[14](ch014-figures-and-animation.md)):
  low-poly faces lit flat, the moon the only key light, dim and blue, at its
  phase for the visitor's date; a pale outline post pass; bloom only on what
  glows; the player a black figure with fuzzy edges and a particle trail.
- **Memorials** ([chapters 50](ch050-shared-state-over-the-network.md),
  [53](ch053-bring-in-assets.md)): a pet's record and media through the
  upload protocol; notes as shared state; each memorial its own page.
- **Edit** ([chapter 52](ch052-edit-a-realm-in-place.md)): rows for the
  flight, fireflies, trail, tips and others.

## What exists today

- [villa.coax](../../apps/openagents-web/villa.coax): the layout, the
  controls, the flight
  ([verse/cinematic.coax](../../verse/cinematic.coax)), the HUD; it walks
  with [verse/player.coax](../../verse/player.coax), sees through
  [verse/camera.coax](../../verse/camera.coax)'s free orbit (pulled in where
  a tombstone, hedge or tree is in the way), and stands on
  [verse/terrain.coax](../../verse/terrain.coax)'s ground and paths.
- Its look: [verse/look.coax](../../verse/look.coax) for the outline,
  [verse/particles.coax](../../verse/particles.coax) for the trail; flat
  style since October 30.
- Pets: uploads through [coax/web/uploads.js](../../coax/web/uploads.js)
  (action 30, messages 52 and 53) to `/api/villa/pets/<id>/files`; each
  memorial's own page in
  [memorial.coax](../../apps/openagents-web/memorial.coax).
- The design: [verse/planetary-villa/README.md](../../verse/planetary-villa/README.md).
- Checks: `coax test apps/openagents-web/villa.coax --steps 200000000`.

## What it builds

- The garden, the dog, the memorials and the flight's keys over `aethyr/`;
  the private controller wiring, camera smoothing and ground deleted.

## Acceptance checks and oracles

- The flight hands over with no frame-to-frame camera jump above chapter
  10's bound, and a key, click or tap skips it at any frame.
- Over a recorded minute the dog never crosses a hedge or plot collider and
  stays within its follow distance of the player along the path graph.
- The moon's phase matches an independent phase computation for the
  visitor's date.
- An upload refused by the scan never appears in a memorial.
- `hecates-cemetery.png`'s golden changes only where a fix is visible; the
  route check in `villa.coax`'s tests passes.

## Dependencies

Chapters 1–16, 50, 52 and 53.
