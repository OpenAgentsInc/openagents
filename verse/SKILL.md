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

Start from the template: `verse/realm-template/` is a whole small realm
(Star Catcher, about 460 lines of Coax with four `#test` checks that play
it, no assets) that a fresh agent made from this skill. Copy its three files
to `apps/openagents-web/<module>.coax`, `<module>-face.c` and
`<module>.wgsl`, rename `starcatch`/`Starcatch` to your module throughout,
then make your own game in it. `verse/realm-template/row.json` is its
`realms.json` row and `index.md` its page text. For bigger ideas, read Hold
the Stair (`stair.coax`, a tower defense with buttons and phone taps) or
Orbit League (`orbitleague.coax`, an arcade game with two thumbs); their
faces also send analytics (`analytics-face.h`, `track_drain`), which your
realm can leave out.

- `<module>.coax`: the game, its state type and its drawing. Every
  procedure the face calls is `#export_c`. `<module>_frame(state, vertices,
  uniforms, w, h, dpr, dt, still) -> int` steps and draws a frame, HUD
  included, and returns the vertex count. Read input in `<module>_key`,
  `<module>_key_up`, `<module>_pointer` (phase 0 down, 1 move, 2 up) and
  `<module>_blur`. A key is its code point, or a named key below zero:
  Escape -1, Enter -2, Backspace -3, ArrowLeft -4, ArrowRight -5, ArrowUp
  -6, ArrowDown -7 (`KEYS` in `coax/web/host.js`). Message 600 is the page facts (`realm=<slug>` and more,
  one `key=value` a line); action 1 with a URL in the outbox navigates
  (`/verse/galaxy` to leave). Coax is not C or Rust: read
  `coax/README.md` and copy idioms from the module you started from.
  Loaded files (`#load "font.coax"` for text) share one namespace with
  your module: a top-level name of yours must not clash with a name in
  them (errors then point into the loaded file), and `font.coax` uses
  `lo`/`hi`, which your module defines (the template does).
- Draw a way back: Escape and a drawn button go to `/verse/galaxy`
  (action 1); every realm has one.
- `<module>-face.c`: the browser exports over the module (`coaxial_layout`,
  `coaxial_state_size`, `coaxial_frame`, input, inbox/outbox). Copy the
  face you started from and rename; it decides nothing. The compiler
  writes `<module>.h`, where your state struct `Foo` is `coax_t_Foo` and a
  view of it `coax_v_t_Foo`.
- `<module>.wgsl`: `bg_vs`/`bg_fs` (a full-screen pass) and `fx_vs`/`fx_fs`
  (the vertices your frame writes). The fx pass adds light (a stroke's
  colour times its light), it does not blend over. Copy and trim.
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
<module>=$!      # a shell variable named after the module; then `wait "$<module>"` with the others
```

If the realm is `flat` or `painterly`, add its WGSL to the matching
`coax/web/wgsl-block.coax ... --into` line at the top of `build.sh` (Orbit League is
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

Write `apps/openagents-web/realms/<slug>/index.md` by hand (the generator
refuses a realm without it): the realm as text
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

`coax/web/check-visuals.sh apps/openagents-web` must pass too (no DOM UI).

Play it locally. Serve without the whole site's build (`build.sh` builds
every realm and rewrites tracked project files, and the server would rebuild
forever); rebuild your wasm with `wasm.sh` after each change:

```sh
coax/web/serve.sh apps/openagents-web --build true     # http://127.0.0.1:8800/realms/<slug>
coax/web/tool.sh shot http://127.0.0.1:8800/realms/<slug> <slug>.png 1440x900 4000
coax/web/tool.sh shot http://127.0.0.1:8800/realms/<slug> <slug>-phone.png 390x844 4000
```

The first `tool.sh shot` builds the shot tool (about a minute). Look at both shots: it draws, it is lit, the HUD fits a 390 px phone.

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

Stage only your realm's files (and a test line, if you added one); `git
status` should show nothing else. If you ran `build.sh`, put back what it
rewrote (`git checkout -- apps/openagents-web/projects
apps/openagents-web/newland-data.coax`). Don't commit
`apps/openagents-web/build/`. The team builds it,
makes its golden (the release walk opens every realm and compares a shot)
and lists it on the galaxy.

## After it ships

The realm's maker can be named and tipped in Bitcoin: the team sends a
one-time claim link (`docs/verse/makers.md`).

## Asks waiting on code

Every realm's edit mode keeps the list of what its owner asked for
(`docs/verse/realm-edit.md`, "What you asked for"). An ask the realm's code can't do yet is NOT
APPLIED and waits for someone to build it. To work one, as an agent with a site admin key:

1. Read the list: `GET https://openagents.com/api/playtest/realms/<slug>/asks`, signed with NIP-98
   (`ask ID STATE KIND WORDS`, newest first). `waiting ask` is to build; `waiting remove` is a row
   whose code is to come out; `removing ask` is built code to take out again.
2. Build it (or take it out) in the realm's files as this skill says, with `#test`s, a new board
   row in `realms.json` and its migration when the owner should be able to switch it, and push to
   `main`. Taking a row out: its code, its `realms.json` row and a migration that drops it from
   `verse.realm_rights` and `verse.realm_edit.switched`. Taking built code out: revert its commit.
3. Mark it: `POST /api/playtest/realms/<slug>/asks/<id>/done {"commit": "<the pushed 40-hex hash>"}`,
   signed with NIP-98. A built ask becomes APPLIED; a remove (or a removing ask) leaves the list.
