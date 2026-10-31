---
name: verse-realm
description: Make a new Verse realm (a world at openagents.com/realms/<slug>) in the OpenAgents repository from a shallow clone, check it locally and open a pull request. Use when asked to build, add or remix a realm, a game or a world for the Verse.
---

# Make a Verse realm

A realm is a page at `https://openagents.com/realms/<slug>` that runs one
Coax module, compiled to WebAssembly and drawn with WebGPU. You add four
things and one table row; a generator writes the page. Nothing else in the
site changes. The full contract is `docs/verse/realm-host.md`; this file is
the short path.

## 1. Get the code

```sh
git clone --depth 1 https://github.com/OpenAgentsInc/openagents.git
cd openagents
```

One level of history is all you need. Tools (macOS: Homebrew; Linux: your
package manager): a C11/C++20 compiler (clang), `llvm` and `lld` with
`wasi-libc` (for `wasm32`), `bun`, `python3`, and Chrome or Chromium for a
headless look.

Build the Coax compiler once (a few minutes; it lands in `coax/bin/coax`):

```sh
coax/build.sh
```

## 2. Pick a slug and a module name

- slug: lowercase words with dashes, e.g. `lantern-maze` (the URL and the
  save slot).
- module: one short lowercase word, e.g. `lanternmaze` (the file names).
- Read `apps/openagents-web/realms.json` and pick a slug, module, world `id`
  and orbit nobody uses.

## 3. Write the module (three files in `apps/openagents-web/`)

Start from the smallest realm with no assets that is close to your idea and
copy its shape, not its game. Good ones to read first:

| Realm | Files | What it shows |
| --- | --- | --- |
| Hold the Stair | `stair.coax`, `stair-face.c`, `stair.wgsl` | 2D-ish game, buttons, phone taps, `#test` checks that play the game |
| Orbit League | `orbitleague.coax`, `orbitleague-face.c`, `orbitleague.wgsl` | arcade game, keyboard and two thumbs, flat style |
| Water lab | `waterlab.coax`, `waterlab-face.c`, `waterlab.wgsl` | a traced scene in a full-screen shader, camera from the pointer |

- `<module>.coax`: the game, its state type and its drawing. Every
  procedure the face calls is `#export_c`. `<module>_frame(state, vertices,
  uniforms, w, h, dpr, dt, still) -> int` steps and draws a frame, HUD
  included, and returns the vertex count. Read input in `<module>_key`,
  `<module>_key_up`, `<module>_pointer` (phase 0 down, 1 move, 2 up) and
  `<module>_blur`. Message 600 is the page facts (`realm=<slug>` and more,
  one `key=value` a line); action 1 with a URL in the outbox navigates
  (`/verse/galaxy` to leave). Coax is not C or Rust: read
  `coax/README.md` and copy idioms from the module you started from.
- `<module>-face.c`: the browser exports over the module (`coaxial_layout`,
  `coaxial_state_size`, `coaxial_frame`, input, inbox/outbox). Copy the
  face you started from and rename; it decides nothing.
- `<module>.wgsl`: `bg_vs`/`bg_fs` (a full-screen pass) and `fx_vs`/`fx_fs`
  (the vertices your frame writes). Copy and trim.
- Checks: put `#test` blocks in the `.coax` file that play the game (a
  frame draws, input does what it says, the game can be won and lost). They
  run with `coax test`.
- Every visible thing is drawn in Coax and WGSL: no HTML, CSS or DOM UI
  (`coax/web/check-visuals.sh` refuses it). Text from players is drawn as
  glyphs, never interpreted. Orange is for Bitcoin only. Write "Log in",
  never "Sign in".

Add one build line to `apps/openagents-web/build.sh` next to the other
realms (and its `wait`):

```sh
"$here/../../coax/web/wasm.sh" "$here/<module>.coax" "$here/<module>-face.c" "$here/build/<module>.wasm" &
<module>=$!
```

If the realm is `flat` or `painterly`, add its WGSL to the matching
`coax/web/wgsl.py ... --into` line at the top of `build.sh` (Orbit League is
flat; Everglade painterly); `lowpoly` needs nothing.

## 4. Add the table row and the page text

In `apps/openagents-web/realms.json`, add a world at the end of `worlds`
(copy Hold the Stair's row):

```json
{ "id": "maze", "name": "LANTERN MAZE", "kind": "open", "listed": false, "orbit": { "r": 0.34, "a": 2.6, "size": 0.024 },
  "realm": { "slug": "lantern-maze", "style": "lowpoly", "clock": "fixed:0.0",
             "title": "Lantern Maze", "description": "One or two sentences for search and link previews.",
             "alt": "What the link preview's image shows", "module": "lanternmaze", "slot": "lantern-maze",
             "golden": { "lit": 0.02, "settle": 3000, "blocks": 6 }, "assets": [] } }
```

Keep `"listed": false`: the team lists it at launch (then it needs `added`
and `line`). `style` is `lowpoly`, `painterly`, `photoreal` or `flat`;
`clock` is `cycle`, `real` or `fixed:T`.

Write `apps/openagents-web/realms/<slug>/index.md` by hand: the realm as text
(what it is, how to play with keys and on a phone, how it is made). Copy
`realms/hold-the-stair/index.md`'s headings. Then write the page and the
galaxy's table:

```sh
bun coax/web/realms.ts
```

Never edit `realms.coax` or `realms/<slug>/index.html` by hand.

## 5. Check it

All of these pass before a pull request:

```sh
coax/bin/coax check apps/openagents-web/<module>.coax
coax/bin/coax test apps/openagents-web/<module>.coax --steps 2000000000
bun coax/web/realms.ts --check
bun test coax/web/realms.test.ts
coax/web/wasm.sh apps/openagents-web/<module>.coax apps/openagents-web/<module>-face.c apps/openagents-web/build/<module>.wasm
```

If `realms.test.ts` lists realms by name (styles, rulesets) and yours is
not lowpoly or has a ruleset, add yours to that test, nothing else.

Play it locally:

```sh
coax/web/serve.sh apps/openagents-web     # http://127.0.0.1:8800/realms/<slug>
coax/web/tool.sh shot http://127.0.0.1:8800/realms/<slug> /tmp/<slug>.png 1440x900 4000
coax/web/tool.sh shot http://127.0.0.1:8800/realms/<slug> /tmp/<slug>-phone.png 390x844 4000
```

Look at both shots: it draws, it is lit, the HUD fits a 390 px phone.

## 6. Open the pull request

```sh
git checkout -b realm-<slug>
git add apps/openagents-web/<module>.coax apps/openagents-web/<module>-face.c apps/openagents-web/<module>.wgsl \
  apps/openagents-web/build.sh apps/openagents-web/realms.json apps/openagents-web/realms.coax \
  apps/openagents-web/realms/<slug>/
git commit -m "Verse: <Title>, a new realm at /realms/<slug>"
gh repo fork --remote && git push -u origin realm-<slug>
gh pr create --title "Verse: <Title>" --body "What it is, how to play, the checks you ran and the two shots."
```

Stage only your realm's files (and a test line, if you added one). Don't
commit `apps/openagents-web/build/` or anything else. The team builds it,
makes its golden (the release walk opens every realm and compares a shot)
and lists it on the galaxy.

## After it ships

The realm's maker can be named and tipped in Bitcoin: the team sends a
one-time claim link (`docs/verse/makers.md`).
