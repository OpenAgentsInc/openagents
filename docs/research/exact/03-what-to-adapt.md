# What Coax should adapt from Exact

This document lists the pieces of work, in the order to do them. Each one
names what to take from Exact, what we already have to build on, and what
the result looks like in Coax. Repository rules apply throughout:

- tools are written in Coax (`coax/tools-in-coax.md`);
- features are built engine-wide, never for one zone;
- Coaxial stays WebGPU-first.

The first three items make up the agent loop. They cost the least, reuse
the most, and help every realm, the Mac app and the iOS app at once.

## 1. One agent driver: `coax drive`

**Take from Exact:**

- A small, fixed set of operations that works the same on every host.
- Targets named by stable ids, never by coordinates.
- One operation per shell argument.
- Variations as arguments rather than new commands.
- Answers that come from the engine, not the host's mirror.
- Refusal to drive a build older than its sources.

**Build on:**

- `coax/lib/net/cdp.coax` and `coax/web/shot.coax`, for headless Chrome;
- `runProbe` and `?probe&do=` (`host.js:3690`);
- `globalThis.coaxial.*` probes;
- `coaxial_a11y`;
- `perf.ts` and `frametime.ts`;
- the MCP server in `apps/openagents-cli/mcp.coax`.

**Shape:**

```sh
coax drive web apps/openagents-web /worlds/everglade \
  tree "tap door-north" "type chat hello" "clock +2000" \
  "state player.zone" "screenshot out.png"
```

- **Operations.** Use Exact's ten, because agents may soon know them from
  their weights: `tree`, `screenshot`, `tap`, `type`, `state`, `layout`,
  `logs`, `clock`, `prefer`, `perf`.
- **Session.** It is a live session over CDP, replacing the steps encoded
  in a URL.
- **Engine exports.** The host calls three new exports:
  - `coaxial_tree(buf)`: ids, roles, labels and rectangles;
  - `coaxial_state(path, buf)`: reflection over the state arena using
    `field_info`, so apps no longer export probes one by one;
  - `coaxial_target(id) -> rect`: hands `tap` the centre of a node.
- **Stale builds.** Compare each source's hash with the build's
  provenance record (`coax/provenance.sh` already exists). Refuse with the
  command that rebuilds.
- **Other hosts.** Mac and iOS come second. Their Swift hosts answer the
  same requests over a local socket. They call the same Coax exports, so
  `tree` and `state` agree by construction.
- **MCP.** Expose the same operations as MCP tools in `mcp.coax`. An
  agent in Claude Code or Codex can then drive a realm without a shell.

## 2. An agent-owned clock and pinned agent sessions

**Take from Exact:** between two operations nothing moves. The forms are
`clock +ms`, `clock settle` (run until nothing is scheduled), `clock data`
(until pending fetches answer) and `clock until <state> within <ms>`.
Every agent session is pinned to one seed, locale, time zone and epoch.

**Build on:**

- typed clock domains (`coax/lib/clock.coax`);
- clip mode (`coaxialClipStep`, `coaxial_now_ms`);
- golden mode;
- replay, which already requires that simulation time comes only from the
  host.

**Shape:**

- **`?agent` mode in `host.js`.** It stops `requestAnimationFrame` from
  advancing simulation time. `clock` steps the engine with fixed `dt` up
  to the target.
- **`settle`.** It asks a new export, `coaxial_quiescent()`, whether
  anything is scheduled, such as animations, tweens or pending effects. It
  steps until the answer is yes or a bound is reached, and reports the
  bound when it hits it.
- **Pinned seed and epoch.** Set them through the existing state init.
  Document the pin in one place and use it on every host.

## 3. Driver test scripts that run on every host

**Take from Exact:** a test file is a list of driver operations and
expectations. It compiles to a driver script, never a second evaluator.
`on web, macos` limits the hosts. A `verify` step also checks the first
screen and each test's end in light and dark, at two widths, and at the
largest text size.

**Build on:**

- the test runner and evidence records of `coax test`;
- `goldens.ts`, for the pixel walk;
- `replay-hosts.coax`, for the cross-host harness.

**Shape:** a `*.drive` file, or `#drive` blocks in a `.coax` test file
so the compiler checks the target ids against the tree that the code can
emit.

```
drive "open the north door"
  tap "door-north"
  clock settle
  expect state "player.zone" == "glade"
  expect tree has "zone-title"
  screenshot "north.png" compare "goldens/north.png"
```

- **Pixel comparisons.** Reports should be in text, node by node, as in
  Exact's `screenshot … compare`, so an agent does not have to look at
  PNGs.
- **Evidence.** Results go into `coax.evidence.v1` beside the logic checks.
- **Releases.** `release.sh` can run the scripts as a check against
  production's baseline. Under the "releases never blocked" rule, only a
  new regression stops a release.

## 4. A retained UI layer with ids and CSS-named flex layout

This is the largest item. It is what makes `tree`, `layout`, real
accessibility and baking possible.

**Take from Exact:**

- A keyed node tree. Rows keep their state by key.
- Bindings updated only when their inputs change.
- Layout and property names taken from CSS, because agents know them.
- The browser as the layout oracle.

**Leave out:**

- **Platform controls and DOM output.** Coaxial draws everything on the
  GPU by decision, so we keep that.
- **A separate language.** Coax is already constrained enough.

**Build on:**

- `aethyr/ui/tree.coax` (roles, hit-testing, focus, labels required);
- `aethyr/ui/hud.coax`;
- `apps/shared/zeron/` (GPUI components);
- `font.coax`, for measuring and wrapping;
- the accessibility mirror.

**Shape:**

- **Library.** Grow `aethyr/ui/tree.coax` into an engine-wide library:
  - nodes in a fixed-capacity arena owned by the caller;
  - a stable `id` string per node, which replaces Exact's `testId`;
  - a role and a label;
  - style properties with CSS names (`flex-direction`, `gap`, `padding`,
    `align-items`).
- **Flex layout in Coax.** Write it in Coax, not by vendoring Taffy.
  Limit the scope to block and flex at first. Test it with checks and
  mutation, and compare it against Chrome with a differential tool that
  lays out the same tree as HTML in headless Chrome and diffs the boxes.
- **Drawing.** HUD and app code builds the tree; the library lays it out
  and emits vertices. The immediate-mode world renderer stays as it is.
- **Accessibility.** The mirror reads the tree directly, which brings
  every ARIA role, not just buttons and checkboxes.
- **View profile.** Add a profile in `coax/PROFILES.md` for procedures
  that build the tree: no recursion, and loops bounded by a collection's
  `.count`. The checker refuses anything else with a teaching
  diagnostic. This is the Contract restriction expressed inside Coax.

## 5. Field-by-field state carry on reload

**Take from Exact:** `Runner::carry()` keeps every slot whose name and
type still fit, and resets only the rest.

**Build on:** the shared `WebAssembly.Memory` arena, `coaxial_layout()`
fingerprints, compile-time reflection (`type_info`, `field_info`),
`coax compat`, and the R26–R27 roadmap items.

**Shape:**

- **Layout table.** Each build exports a table of its state layout
  (field path, type fingerprint, offset, size) next to the single
  fingerprint.
- **Copy.** On reload, `host.js` copies each field whose path and type
  fingerprint match from the old region to the new one, and zeroes the
  rest.
- **Report.** It names the reset fields in the dev note, for example
  "kept 41 fields, reset `player.buffs` (type changed)".
- **Migrations later.** Declared migrations (R26) can then grow out of
  this.

This is a small change that fixes the most common reason a reload throws
the player out of the world.

## 6. A baked first frame

**Take from Exact:** at build time the compiler acts as a host for one
frame and writes the result into the artifact. The first pixel needs no
network and no data.

**Build on:**

- `#run` with its budgets;
- the vertex-buffer contract of `coaxial_frame`;
- the `first-image` measurement;
- the loader card, which this replaces.

**Shape:**

- **Build step.** It runs the realm's first frame at compile time with
  the empty save and the pinned agent clock. The vertex buffer and
  uniforms it produces go into a small sidecar file next to `index.html`,
  or into the Wasm's data segment.
- **Startup.** `host.js` draws the sidecar with the shared pipeline
  before `engine.wasm` arrives, then hands over.
- **What to skip.** Skip anything that depends on the network or the
  date, as Exact does.
- **Proof.** `first-image` already proves the gain in milliseconds and
  bytes.

## 7. Agent-callable tools declared in Coax, exposed through WebMCP and MCP

**Take from Exact:** `tool NAME reads|writes|destroys "description" =
action [returns …] [fails when …] [available when …]`. Input schemas come
from types. Consequence words become hints, and a tool is withdrawn while
it is unavailable. A tool call runs as one transaction and never moves
the person's view.

**Build on:** `apps/openagents-cli/mcp.coax`, `#export_c` attributes, and
the analytics contract (`analytics-contract.json`, a model of a checked
contract between page and host).

**Shape:**

- **Declaration.** Use a `#tool(reads|writes|destroys, "description")`
  attribute on an exported procedure. The compiler checks the consequence
  word against `coax deps --effects`: a `reads` tool with a write effect is
  a compile error. That check is one step beyond Exact, which trusts the
  author's word.
- **Web.** `host.js` registers the tools through `document.modelContext`
  where the browser supports it.
- **CLI.** The openagents CLI serves the same tools over MCP.
- **Agents in the Verse.** A realm becomes something an agent can operate
  through typed calls, not only through pixels. This fits our agent
  products directly.

## 8. The agent on-ramp for Coaxial

**Take from Exact:**

- One required document, with everything else as lookup.
- A `vocab` command that prints the card for any name, and for an unknown
  name prints the refusal text.
- Diagnostics that repair idioms from other languages.
- A pitfalls list with a deletion rule.
- Templates indexed by keywords.
- One-line agent feedback notes.

**Shape:**

- **`coax/start-here.md`.** A single page for building a Coaxial page or
  realm: the workflow, the drive commands, the five most expensive
  pitfalls.
- **`coax vocab NAME…`.** It prints the entry from `LIBRARIES.md` or the
  book for a library procedure, type or diagnostic, and for an unknown
  name suggests the nearest one.
- **Idiom repairs.** Add them to `errors.cpp` for the spellings agents
  get wrong most often. Mine the first set from agent transcripts.
- **Pitfalls.** Keep `coax/DEBUGGING.md` as symptom → cause → fix, and
  delete each entry when a diagnostic catches it.

## What not to adopt

- **A second UI language.** Contract exists because TypeScript cannot be
  analyzed. Coax already can be. A view profile inside Coax (item 4) gets
  the analysis without a second compiler, grammar and set of docs.
- **TypeScript and Hermes for business logic.** The thread argues that
  TypeScript iterates faster for agents. Our rules put logic in Coax, and
  Coax already reloads in about 120 ms. Exact's own numbers do not show a
  gap we need to close.
- **Platform-native controls and DOM rendering.** These conflict with the
  2026-10-28 Coaxial decisions. The cost is real: we give up the free
  accessibility and crawler text that Exact gets from the DOM. Items 4
  and 7 recover the agent-facing half of that.
- **Vendoring Taffy.** A Coax flex subset, checked against Chrome, fits
  our rules and can be tested in our own way.
- **Exact's governance numbers as such.** "Six checks in 65 seconds" and
  "agents remove apparatus freely and add none" are good instincts. Our
  "releases never blocked", lean-verification and no-shortcut-phases
  rules already cover the same ground.

## Sequencing

| Order | Item | Depends on | Size |
| --- | --- | --- | --- |
| 1 | `coax drive` on web (state through reflection, screenshot, tap and type by id, logs, perf) | `coaxial_tree` from the existing `coaxial_a11y` lines at first | Small to medium |
| 2 | `?agent` clock and `settle` | 1 | Small |
| 3 | Drive scripts and evidence | 1, 2 | Medium |
| 5 | Field-by-field state carry | none | Small |
| 8 | `start-here`, `vocab`, idiom repairs | none | Small |
| 7 | `#tool` and WebMCP | 1 for testing | Medium |
| 4 | Retained UI tree and flex layout | 1 to prove it | Large |
| 6 | Baked first frame | 4 for UI; possible for worlds now | Medium |
| — | Mac and iOS drive hosts | 1, 4 | Medium |

Items 5 and 8 have no dependencies and can run in parallel with 1–3.
