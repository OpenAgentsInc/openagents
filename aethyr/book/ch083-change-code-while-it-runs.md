# 83. Change code while it runs

> **Status: unimplemented — design draft.** Today `coax serve`
> ([`coax/web/serve.c`](../../coax/web/serve.c)) watches an app's files,
> rebuilds what changed and tells every open tab over `/coaxial/live` to
> swap: `wasm` for `.coax` and `.c`, `wgsl` for shaders, `page` for HTML,
> JavaScript and Markdown. A failed build sends `error` and the tab keeps
> the last good engine. Nothing checks layouts or migrates state when a
> struct changes. This chapter plans live iteration for realms. Nothing in
> `aethyr/` runs yet.

You change a realm and see the result in the running page, without losing
your place. This chapter defines what a realm can change while it runs, the
checks that guard each change, and the measured time from saving a file to
the next frame. It follows the four reload tiers in
[coax/web.md](../../coax/web.md) and the engine design in
[Coax chapter 19](../../coax/book/ch19-builds-hot-reload.md).

## What this chapter teaches

- The four tiers of a change: data, shader, logic and layout.
- Why state lives in the host's memory, so a code swap copies nothing.
- How layout fingerprints decide between a swap, a migration and a reset.
- Why production never loads code except at page load.

## Outline

1. **Data.** World layout, HUD layout, `home.textproto` and tuning are
   re-encoded, pushed and re-admitted through the load checks. Target:
   under 50 ms from save to frame.
2. **Shaders.** A changed `.wgsl` file or Coax shader procedure is
   re-emitted, `createShaderModule` runs, and the pipeline swaps at the next
   frame. Target: under 100 ms.
3. **Logic.** `coax lib`, clang to wasm32 and a push; the host instantiates
   the module against the same `WebAssembly.Memory` and swaps exports at a
   frame boundary in one JavaScript turn. Target: under 300 ms. Measured
   today: an engine rebuild in 118–135 ms.
4. **Layout.** When a state struct changes, its layout fingerprint
   differs. The subsystem migrates if a migration is declared, or resets
   to its defaults ([chapter 3](ch003-state-starts-from-its-defaults.md)).
   Target: under 500 ms.
5. **Failure.** A failed compile, a failed fingerprint check, or a trap in
   the first frame keeps the old module running and shows the error in the
   HUD with its `.coax` location. A newer request always replaces an older
   one.
6. **Memory layout.** The build reserves a fixed low region for module
   data and a fixed state base above it (`--global-base`, fixed initial
   memory and stack size), so a new module's data never overwrites state.
7. **Native reload.** On the Mac, Coax roadmap R26 replaces procedure
   bodies behind unchanged exports at a safe point; R27 adds versioned
   migration with rollback. iOS builds stay signed images and never load
   code.

## What exists today

- [coax/web/serve.c](../../coax/web/serve.c) and
  [serve.sh](../../coax/web/serve.sh): the watcher, the rebuild and the
  live channel.
- [coax/web/host.js](../../coax/web/host.js): state in one
  `WebAssembly.Memory`, code and shader swaps and the dev error note.
- [coax/web.md](../../coax/web.md), "Fast iteration": the tiers, their
  targets and how a swap works.
- [coax/web/place-reload.ts](../../coax/web/place-reload.ts): a page reload
  keeps your place, which a code swap must also do.

## What it builds

- `aethyr/live.coax`: per-subsystem layout fingerprints, declared
  migrations and the reset rule.
- A reload test harness that edits a realm in each tier and measures save
  to frame.

## Acceptance checks and oracles

The chapter is done when:

- Each tier meets its target on the Mac, measured over 20 edits of a real
  realm, with the median and the slowest reported.
- A logic swap with matching layouts leaves the session digest
  ([chapter 5](ch005-replay-a-session-exactly.md)) equal to a run that never
  swapped.
- A layout change with no migration resets only that subsystem; the
  player's position and the clock stay.
- A broken edit leaves the old module running, and the HUD names the file
  and line.
- An hour of edits in one tab does not grow memory without bound.
- The production host (`serve --static`) has no live channel.

## Dependencies

Chapters [1](ch001-a-frame-has-a-fixed-step.md) (the frame boundary),
[3](ch003-state-starts-from-its-defaults.md) (defaults for a reset),
[5](ch005-replay-a-session-exactly.md) (digests) and
[15](ch015-ui-and-the-hud.md) (the error note). Coax:
[chapter 19](../../coax/book/ch19-builds-hot-reload.md) and roadmap R26–R27
in [ROADMAP.md](../../coax/ROADMAP.md).
