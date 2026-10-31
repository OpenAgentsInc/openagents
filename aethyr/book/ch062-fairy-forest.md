# 62. Fairy Forest

> **Status: unimplemented — design draft.** Fairy Forest runs today on its
> own private code at `/realms/fairy-forest`:
> [fairy.coax](../../apps/openagents-web/fairy.coax) over
> [verse/fairy-forest/](../../verse/fairy-forest/README.md). This chapter
> plans rebuilding it only from Aethyr's earlier chapters. Nothing here runs
> yet.

Fairy Forest is the realm players asked for when asked what game they would
make. You are a newly woken fairy in a valley faded to grey. Bloom a faded
patch and colour spreads from it; bloom a glade and its creature is happy,
and one of the heart tree's five roots wakes. When all five wake the heart
tree blooms, colour returns to the whole park, and the forest stays yours to
wander.

## What this chapter teaches

- Flight as part of character movement: one wing beat per press, holding
  climbs, letting go glides.
- Progress as saved state that survives devices: in the browser and, with an
  account, on the server.
- A world whose look is state: colour spreading from bloomed patches.
- Characters with troubles, memory and owner-written lore and quests.

## The method

- **State and saves** ([chapter 3](ch003-state-starts-from-its-defaults.md)):
  each patch's bloom, each creature's mood and each root are declared fields
  with defaults; what you restored is saved locally and on the server.
- **Input** ([chapter 4](ch004-input-becomes-typed-actions.md)): BLOOM (E, or
  a button on a phone), the wing beat (Space), talk.
- **Movement** ([chapters 8](ch008-ground-and-collision.md),
  [9](ch009-a-character-moves.md)): the controller walks and runs; flight adds
  an upward impulse per beat and a glide, both inside the controller rather
  than in the page's face.
- **Camera** ([chapter 10](ch010-the-camera-follows.md)): the free orbit.
- **Water** ([chapter 24](ch024-liquids-from-particles.md) for physics where
  it matters; the game layer's river body otherwise): the stream and pond
  along a spline, lily pads, what water you are in.
- **Look** ([chapters 11](ch011-the-wgsl-renderer.md)–[13](ch013-light-and-shadow.md)):
  the flat style, faceted low poly in the `FLAT_FAIRY` palette, flat
  translucent shadows, bloom only on what glows; colour per cell blended from
  grey by the bloom field.
- **Talk and panes** ([chapters 15](ch015-ui-and-the-hud.md),
  [52](ch052-edit-a-realm-in-place.md)): creatures answer through the
  character chat; with EDIT open, LORE and QUEST panes for whoever you stand
  by.

```jai
bloom_at :: (f: *Forest, p: Vec3W) {
    patch := nearest_patch(*f.patches, p, BLOOM_REACH);   // chapter 7's query
    if patch != NONE then patch_bloom(f, patch);           // spreads over steps, saved
}
```

## What exists today

- [verse/fairy-forest/README.md](../../verse/fairy-forest/README.md): the
  story, Thornfall and the Old Ring, the five roots, the panes rule.
- [forest.coax](../../verse/fairy-forest/forest.coax) (layout, fade,
  troubles), [stream.coax](../../verse/fairy-forest/stream.coax) (one
  [verse/water.coax](../../verse/water.coax) river body),
  [talk.coax](../../verse/fairy-forest/talk.coax),
  [life.coax](../../verse/fairy-forest/life.coax),
  [night.coax](../../verse/fairy-forest/night.coax),
  [park.coax](../../verse/fairy-forest/park.coax).
- [fairy.coax](../../apps/openagents-web/fairy.coax) and `fairy.wgsl`:
  the page, drawn from the engine's five shapes (a ground of cells in store
  0, objects in store 1, the stream's nodes in store 2); the face does the
  flight. Quests from the panes: `apps/openagents-web/fairyquest.coax`.
- Edit rows: sparkles, grass, ridges; visitor tool: fly.

Flight lives in the face, outside the controller, so its physics is not in
the realm's checks or its replay.

## What it builds

- The forest's story, troubles and bloom over `aethyr/`, with flight moved
  into the shared controller as a declared mode.

## Acceptance checks and oracles

- A scripted session blooms every glade, wakes all five roots and reaches the
  heart tree's bloom; it replays bit-identically natively and in the browser.
- Flight: a held beat climbs, a release glides; the same action stream gives
  the same path at 30, 60 and 144 FPS (chapter 1's rule).
- A save written in one browser and loaded on the server's copy restores the
  same fields; a missing field takes its declared default.
- `fairy-forest.png`'s golden changes only where a fix is visible.

## Dependencies

Chapters 1–15, 24, 49 and 52.
