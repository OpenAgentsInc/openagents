# 60. Ice Cream Man

> **Status: unimplemented — design draft.** Ice Cream Man runs today on its
> own private code at `/realms/ice-cream-man`: the rules, run, camera and
> house in [verse/icecream/](../../verse/icecream/play.coax) and the HUD in
> [icecream.coax](../../apps/openagents-web/icecream.coax). This chapter plans
> rebuilding it only from Aethyr's earlier chapters. Nothing here runs yet.

Ice Cream Man is OpenAgents' seeded house game: a kid finds three keys and
escapes a house the Ice Cream Man and his minions patrol. Its rules were
ported step for step from a Rust build, and its parity is proven by replaying
that build's recorded input traces, step for step, against the Coax rules.
That makes it the realm where replay is not a feature but the oracle.

## What this chapter teaches

- A rules step slower than the engine step: 30 rules steps a second inside
  the fixed loop.
- Parity by trace: a recorded input stream, a state fingerprint per step and
  the first divergence named.
- A seeded procedural house as level geometry, queries and draws at once.
- A collision-aware third-person boom inside small rooms.

## The method

- **Time** ([chapter 1](ch001-a-frame-has-a-fixed-step.md)): the engine
  steps at its fixed rate; the rules step every second engine step, as the
  demo did.
- **Input** ([chapter 4](ch004-input-becomes-typed-actions.md)): keys, the
  mouse, two thumbs and the HUD's buttons become the rules' `Input` once a
  rules step.
- **Replay** ([chapter 5](ch005-replay-a-session-exactly.md)): every step's
  input is a trace line in the Rust recorder's format with the state
  fingerprint after it, so a round played in the browser replays in the
  native rules tests.
- **House** ([chapters 6](ch006-entities-are-born-and-die.md)–[8](ch008-ground-and-collision.md)):
  the seed's rooms, doors, furniture and keys as entities and colliders; the
  same geometry the camera's sweep and the kid's movement query.
- **Camera** ([chapter 10](ch010-the-camera-follows.md)): Aethyr's follow,
  stepped once a rules step, its target on a spring to the kid's head,
  pulled in short of walls, lintels, floor and ceiling (`follow_room`); drag
  to orbit, Shift Lock, V for first person.
- **Look and HUD** ([chapters 11](ch011-the-wgsl-renderer.md),
  [12](ch012-materials.md), [15](ch015-ui-and-the-hud.md)): per-room light,
  ink outlines, the house's button style for health, keys, timer, boss bar,
  prompts and the phone's Fire, Jump, Use, Sprint and View.

```jai
icecream_tick :: (g: *IceCream, a: Actions, step: u64) {
    if step % 2 == 0 {
        input := rules_input(a);                 // once per rules step
        rules_step(*g.rules, input);
        trace_line(*g.trace, input, fingerprint(*g.rules));   // chapter 5
    }
}
```

## What exists today

- [verse/icecream/rules.coax](../../verse/icecream/rules.coax),
  [play.coax](../../verse/icecream/play.coax) (the play loop, 30 steps a
  second, trace lines), [trace.coax](../../verse/icecream/trace.coax),
  [trace-text.coax](../../verse/icecream/trace-text.coax),
  [replay.c](../../verse/icecream/replay.c),
  [mesh.coax](../../verse/icecream/mesh.coax) (the seeded house),
  [scene.coax](../../verse/icecream/scene.coax) (stores 0 and 1),
  [camera.coax](../../verse/icecream/camera.coax) (from the demo's
  `camera.cpp`).
- [verse/icecream/test.sh](../../verse/icecream/test.sh): the Coax checks,
  then every recorded Rust trace replayed natively under sanitizers (the
  checked-in `fixtures/*.trace.gz` always; all thirteen when the demo's
  fixtures are present).
- [icecream.coax](../../apps/openagents-web/icecream.coax): the HUD in Coax.
- Edit rows: minions, weapons, backrooms
  ([realm-edit.md](../../docs/verse/realm-edit.md)).

Its loop, input mapping and camera are its own and match no other realm's.

## What it builds

- The rules, house and HUD over `aethyr/`'s loop, actions, replay, queries
  and follow camera; the private camera and input path deleted.

## Acceptance checks and oracles

- Every recorded Rust trace still replays step for step, fingerprint for
  fingerprint, after the move (`verse/icecream/test.sh`).
- A round played in the browser replays natively to the same fingerprints.
- The boom never puts the eye inside a wall, a lintel, the floor or the
  ceiling over a recorded round (a swept-sphere check per frame).
- `ice-cream-man.png`'s golden changes only where a fix is visible; the
  60 FPS rule holds.

## Dependencies

Chapters 1–12 and 15.
