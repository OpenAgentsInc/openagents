# 57. Made realms

> **Status: unimplemented — design draft.** Made realms run today on their
> own private code: [myrealm.coax](../../apps/openagents-web/myrealm.coax)
> plays any realm made at `/realms/new`, and
> [droprealm.coax](../../apps/openagents-web/droprealm.coax) a project's first
> realm from its dropped models. This chapter plans rebuilding both as one
> player of chapter 51's document over Aethyr's chapters. Nothing here runs
> yet.

A made realm has no code of its own. One module reads its document and lays
it out: land, water, trees, buildings, herds, lanterns, a flock, characters
who talk, under one of four cameras, three styles and four palettes. Because
thousands of realms share that module, every bug in it is a bug in all of
them, and every engine improvement reaches all of them at once.

## What this chapter teaches

- A realm as data played by one shared module.
- Composition for a first-time visitor: what was asked for, in view from the
  spawn, on a phone too.
- Four cameras over one controller: third person, first person, top-down and
  2D side-on.
- Characters who talk, from the document's names, roles and lore.

## The method

- **Document and layout** ([chapter 51](ch051-a-realm-is-a-document.md)): the
  realm is `layout(doc, seed)`; its entities
  ([chapter 6](ch006-entities-are-born-and-die.md)) go into one scene.
- **Composition**: headline slots ahead of the spawn for buildings, a hamlet,
  the giant tree and a camp; near slots for herds; fliers circle a point in
  view and climb over a height grid of whatever stands below.
- **Ground and movement** ([chapters 7](ch007-ask-the-world-where-things-are.md),
  [8](ch008-ground-and-collision.md), [9](ch009-a-character-moves.md)): a
  heightfield per ground type; in 2D the side camera's lane, where trees and
  characters stop you and rocks, crates and barrels can be jumped on.
- **Camera** ([chapter 10](ch010-the-camera-follows.md)): the document's
  camera picks a follow mode; none steps physics.
- **Look** ([chapters 11](ch011-the-wgsl-renderer.md)–[14](ch014-figures-and-animation.md)):
  the style preset and palette tint every piece, the sky and the ground;
  lanterns light pools after dark; figures are rigs drawn bone by bone.
- **Budgets** ([chapter 16](ch016-budgets-that-refuse.md)): 200 placed things,
  a vertex budget per tier; over budget refuses visibly.
- **Editing and forks** ([chapters 52](ch052-edit-a-realm-in-place.md)–[55](ch055-fork-a-realm.md)).
- **Dropped models** ([chapter 53](ch053-bring-in-assets.md)): a drop's
  models stand in a ring facing in, rescaled into range, colliders from their
  bounds.

## What exists today

- [myrealm.coax](../../apps/openagents-web/myrealm.coax),
  [myrealm-shapes.coax](../../apps/openagents-web/myrealm-shapes.coax)
  (`SLOT_A`, `NEAR_A`, `owl_home`, `roofs_out`, `wood`) and
  [myrealm-things.coax](../../apps/openagents-web/myrealm-things.coax), drawn
  by `myrealm.wgsl` with [verse/style-preset.coax](../../verse/style-preset.coax).
- The catalogue, cameras and palettes in
  [realm-builder.md](../../docs/verse/realm-builder.md); figures from
  [aethyr/figure.coax](../../aethyr/figure.coax).
- Talk through the character chat Everglade's people use
  (`POST /api/playtest/npc/ask`), refused for visitors with TALK off.
- [droprealm.coax](../../apps/openagents-web/droprealm.coax) (`MIN_SIZE`,
  `MAX_SIZE`, a carried lamp) and `/realms/drop` on the fixture pack.
- `/realms/built` and `/realms/drop` have goldens.

Each module carries its own controller wiring, camera modes and ground; the
side camera's collision is separate from the 3D ones.

## What it builds

- One made-realm player on `aethyr/` and `aethyr/forge/`, replacing the
  private loop, cameras and ground in `myrealm.coax` and `droprealm.coax`.

## Acceptance checks and oracles

- The same document and seed give the same realm for every visitor:
  identical entity tables natively and in the browser.
- From the spawn, every headline feature is inside the view frustum at 390
  CSS pixels wide and at 1440 (a check over the builder's prompt corpus).
- Each camera passes chapter 10's jump bound; the side camera's jumps onto
  rocks and crates land at the query's ground height.
- No flier intersects anything standing (checked against the height grid
  over a recorded minute).
- `/realms/built` and `/realms/drop` goldens change only where a fix is
  visible; the 60 FPS rule holds on a phone-sized walk.

## Dependencies

Chapters 1–16 and 49–55.
