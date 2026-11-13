# 82. Change code while it runs

> **Status: partly implemented.** On the dev server, a code reload keeps
> the game's state, including when the edit changes the state's layout.
> `coax serve` ([`coax/web/serve.c`](../../coax/web/serve.c)) rebuilds what
> changed and tells every open tab over `/coaxial/live` to swap: `wasm` for
> `.coax` and `.c`, `wgsl` for shaders, `page` for HTML, JavaScript and
> Markdown. A development module carries its state's field table, and
> [`coax/web/state-keep.js`](../../coax/web/state-keep.js) carries the running
> state into the new layout field by field (#13411). A failed build sends
> `error` and the tab keeps the last good engine. Not built yet: the data
> tier's push, per-subsystem state, declared migrations, a check of the
> first frame after a swap, and native reload. See
> [Still to do](#still-to-do). Nothing in `metaverse-engine/` holds this code
> yet; it lives in the page host.

You change a realm and see the result in the running page, without losing
your place. This chapter defines what a realm can change while it runs, the
checks that guard each change, and the measured time from saving a file to
the next frame. It follows the four reload tiers in
[coax/web.md](../../coax/web.md) and the engine design in
[Coax chapter 19](../../coax/book/ch19-builds-hot-reload.md).

## What this chapter teaches

- The four tiers of a change: data, shader, logic and layout.
- Why state lives in the host's memory, so a code swap copies nothing.
- How the state's field table decides what a swap keeps, converts and
  starts from its default.
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
4. **Layout.** When a state struct changes, the host carries the state into
   the new layout field by field ([Keep the state across a layout
   change](#keep-the-state-across-a-layout-change)). A field that can't be
   carried starts from its default
   ([chapter 3](ch003-state-starts-from-its-defaults.md)). Target: under
   500 ms.
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

## Keep the state across a layout change

A page's state is one struct in the host's memory. The build already prints
that struct's layout for `coax drive`: `coax layout` lists every struct with
its fields' offsets, sizes and types, and every enum with its members.
[`coax/web/wasm.sh`](../../coax/web/wasm.sh) writes it to
`build/.drive/APP.layout`. This text is the field table. Here is part of
Splice's:

```text
Splice  size 98448  align 8
       0     1  started: bool
       1     1  world_built: bool
       2     6  (padding)
       8     8  world_n: int
    ...
    1712  96736  game: [1] SpGame
```

A development build also puts the table in the module, in a custom section
named `coaxial.state` (`coax/web/tool.sh wasm-size state`). The table
travels with the code it describes, so a tab that is several builds behind
still has the right table for the engine it runs. A release module has no
table, because production loads code only at page load.

### What the host does on a swap

[`coax/web/host.js`](../../coax/web/host.js) `loadEngine` does these steps
when the running module and the new one both carry a table:

1. Copy the running state's bytes out of the shared memory. This happens
   before the new module is instantiated, because instantiating writes the
   new module's data into that memory.
2. Instantiate the new module and find the state's struct in each table: the
   struct whose size is the module's `coaxial_state_size()`, the same rule
   `coax drive` uses. If two structs tie, the host doesn't guess, and the
   layout fingerprint decides as it does for a release module.
3. If the state's layout is the same in both tables, keep the state where it
   is. This is the logic tier: nothing is copied.
4. Otherwise zero the state, call `coaxial_state_init` for the new
   defaults, and apply the plan below to the copied bytes.
5. Report what happened in the dev note and in `coaxial.stateKept`, for
   example `state kept: 257 fields; new game.e2_probe`.

### The rules

The plan pairs fields by name, starting at the state's struct.

| The old and new field | What the swap does |
| --- | --- |
| Same name and type | Copies it. The offset can change, so reordering fields loses nothing. |
| A struct | Carries it field by field with these rules. |
| `[N] T` and `[M] T` | Carries the first `min(N, M)` elements and reports the resize. |
| A number and a wider one | Converts it: `u8` to `u16` to `u32` to `u64`, `s8` to `s16` to `s32` to `s64` (`int`), unsigned to a wider signed type, `f32` to `f64`. |
| An enum whose members changed | Maps the value by member name. A value whose member is gone starts from the default. |
| Only in the new layout | Starts from its default. Reported as new. |
| Only in the old layout | Dropped. Reported as dropped. |
| Any other change of type | Starts from its default. Reported as reset, with both types. |
| A `string` or a view (`[] T`) | Starts from its default. It points into memory, and a swap can move what it points at. |

The plan depends only on the two tables, so the same edit always keeps the
same fields. A narrowing change, such as `int` to `s32`, is never converted,
because some values would not fit.

A new field's default comes from the face. `coaxial_state_init` must call
the compiler's `coax_default_T` for the state. Splice's face didn't, so a
new field with a declared default would have started at zero; it does now.

### Measured on Splice

The page ran on the dev server in headless Chrome, at a moment of the game
with Heat 20, two robot pieces, one battery cell and the player away from
the start. A field `e2_probe: int = 7` was added in the middle of `SpGame`
while it ran, then removed again.

| Edit | Reported | Kept |
| --- | --- | --- |
| Add `e2_probe` | `state kept: 257 fields; new game.e2_probe` | Position, facing, hearts, Heat, pieces, cells, `have`, `taken`, slots and cards were bit-for-bit equal one frame after the swap. `e2_probe` read 7. |
| Remove it | `state kept: 257 fields; dropped game.e2_probe` | The same fields, equal again. |
| Move `hearts`, `heat` and `hit_cool` up the struct | `state kept: 257 fields` | The same fields, equal at their new offsets. |
| Add a comment | No state report | The layout is unchanged, so the state stays where it is. |

Of the 204 fields that both layouts name, 198 were equal one frame after
the swap. The other 6 are the clock, the patrols' positions and the frame's
draw list, which the first new frame advanced. The swap itself took 3 to 9 ms.
The rebuild before it took 1.9 to 3.6 s on a quiet machine and up to 71 s
under load, which is the dev loop's cost, not the swap's.

## What exists today

- [coax/web/serve.c](../../coax/web/serve.c) and
  [serve.sh](../../coax/web/serve.sh): the watcher, the rebuild and the
  live channel.
- [coax/web/host.js](../../coax/web/host.js): state in one
  `WebAssembly.Memory`, code and shader swaps, the state carried across a
  layout change and the dev note.
- [coax/web/state-keep.js](../../coax/web/state-keep.js): the plan and its
  rules. [state-keep.test.ts](../../coax/web/state-keep.test.ts) checks
  reordered, added, removed, retyped and nested fields against tables the
  compiler printed for the fixtures in
  [coax/web/tests/state-keep/](../../coax/web/tests/state-keep/).
- [coax/web/wasm-size.coax](../../coax/web/wasm-size.coax) `state`: puts the
  table in a development module.
- [coax/web.md](../../coax/web.md), "Fast iteration": the tiers, their
  targets and how a swap works.
- [coax/web/place-reload.ts](../../coax/web/place-reload.ts): a page reload
  keeps your place.

## Still to do

- `metaverse-engine/live.coax`: per-subsystem state, so a change to one
  subsystem's layout can't touch another's, and declared migrations for
  changes the rules above can't infer, such as a renamed field or a split
  one.
- A check of the first frame after a swap: a trap there should put the old
  module and the old state back. Today a failed build keeps the old engine,
  but a module that instantiates and then traps does not roll back.
- The data tier's push, and native reload (outline items 1 and 7).
- A reload harness that edits a realm in each tier and measures save to
  frame over 20 edits.
- The page's own stores outside the state, such as a face's static
  buffers, are the new module's. A realm rebuilds them from the state, as
  Splice rebuilds its world store.

## Acceptance checks and oracles

Done:

- A layout change keeps every field both layouts name, and reports the new
  and dropped ones (`bun test coax/web/state-keep.test.ts`, 13 tests, and
  the Splice run above).
- A broken edit leaves the old module running, and the dev note shows the
  compiler's error.
- The production host (`serve --static`) has no live channel, and a release
  module has no field table.

Still open:

- Each tier meets its target on the Mac, measured over 20 edits of a real
  realm, with the median and the slowest reported.
- A logic swap with matching layouts leaves the session digest
  ([chapter 5](ch005-replay-a-session-exactly.md)) equal to a run that never
  swapped.
- A layout change resets only the subsystem it touches.
- An hour of edits in one tab does not grow memory without bound.

## Dependencies

Chapters [1](ch001-a-frame-has-a-fixed-step.md) (the frame boundary),
[3](ch003-state-starts-from-its-defaults.md) (defaults for a reset),
[5](ch005-replay-a-session-exactly.md) (digests) and
[15](ch015-ui-and-the-hud.md) (the error note). Coax:
[chapter 19](../../coax/book/ch19-builds-hot-reload.md) and roadmap R26–R27
in [ROADMAP.md](../../coax/ROADMAP.md).
