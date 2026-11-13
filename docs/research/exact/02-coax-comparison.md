# Exact compared with Coax and Coaxial

This document sets each Exact capability against what exists in this
repository today, as of 2026-11-09. "Coax" means the language and
compiler (`coax/`). "Coaxial" means the web framework and browser host
(`coax/web/`, `apps/openagents-web/`).

## The basic difference

The two systems start from different shapes:

| | Exact | Coax and Coaxial |
| --- | --- | --- |
| What you write | Contract for views and state; TypeScript or Rust for data | Coax for everything; Swift and JS hosts for platform effects only |
| Compiled form | A plan of tables and bytecode, run by a Rust VM, or compiled to a JS module on the web | One self-contained C11 file per program, compiled natively or to wasm32 |
| UI model | Retained and keyed: a node tree with bindings, updated surgically | Immediate mode: `coaxial_frame(...)` rewrites the vertex buffer every frame |
| Pixels | The platform's own: DOM, AppKit, UIKit; Ink only where there is no standard | Ours: WebGPU in a full-window canvas, Metal on Mac; HTML only where a canvas can't do the job |
| Layout | CSS through Taffy; the browser is the oracle | Coordinates computed by hand; text measured in Coax (`font.coax`, `md_wrap`) |
| Main consumer | Apps: forms, lists, stores, chat | Worlds: realms, the Grid, Everglade; also the Zeron chat UI |

These differences are deliberate. The Coaxial decisions of 2026-10-28
(`coax/web.md`) choose WebGPU only, put the GPU in charge of the screen,
and make "a DOM component system" a non-goal. An honest comparison asks
which of Exact's ideas survive those decisions. Most of the agent-loop
ideas do. The platform-native-pixels idea does not.

## Capability by capability

### Constrained language that can be fully analyzed

**Exact.** Contract has no recursion, no loops in actions, no effects in
expressions, inferred write sets, keyed lists and closed shapes. A Lean 4
semantics proves the lowering and the VM correct.

**Coax: exists, in a broader form.** Coax is a general systems language,
yet it is already constrained in the ways that matter for analysis:

- no globals and no heap; state lives in storage the caller owns;
- compile-time execution with step, call and depth budgets (`#run`,
  `#assert`);
- typed clocks and units (`coax/lib/clock.coax`, `QUANTITIES.md`);
- a conservative transitive effects inventory
  (`coax deps --effects`, `EFFECTS.md`) and partial `#effects(...)`
  enforcement (0.52.0);
- `#authority`, which requires a check on every procedure (E0380);
- mutation testing (`coax mutate`), so every mutant must fail a check.

**Gap.** Coax has no view profile: no rule that a procedure which draws
UI may not loop without bound or recurse. Exact gets surgical updates and
baking from exactly that restriction.

### Teaching diagnostics

**Exact.** About 540 stable ids, each with a reject fixture. The messages
repair habits from other languages: "`size` is spelled `font-size` here."
`contract vocab NAME` prints the card for any name, and one required
document points to recipes.

**Coax: exists for the language.** `coax/src/errors.cpp` has 229 E-codes,
each with a title and fix text. Other pieces:

- `coax explain CODE` and `--all --markdown`;
- `--json` diagnostics;
- call-site notes on compile-time failures;
- in the browser, compiler errors reach the tab with their `.coax`
  location (`serve.c`, the `host.js` dev note).

**Gap.** Coax has no repairs for idioms agents bring from C, Rust, Go or
JS: "Coax spells this `cast,trunc`", "there is no `while`; use …". It has
no `vocab`-style lookup for library entry points, and no single "start
here" document for building a Coaxial page or realm. `coax/LIBRARIES.md`
and the book exist, but they are reference material, not an on-ramp.

### Agent driver: one tool, ten operations, every host

**Exact.** `scripts/agent.mjs <host> "op" "op" …` drives web, macOS,
iOS, Linux, Android and the terminal. Targets are `testId`s. `tree`,
`state` and `logs` come from the runner, so every host agrees.

**Coaxial: partial, and split across tools.**

| Exact op | Closest thing in Coaxial |
| --- | --- |
| `screenshot` | `coax/web/tool.sh shot URL OUT.png` (CDP-driven headless Chrome); `openagents labs shot`; `?probe` GPU readback |
| `tap`, `type` | `?probe&do=STEPS` (`host.js:291`, `runProbe` at `:3690`): keys, named keys, clicks at pixel coordinates. These are steps encoded in a URL before one probed frame, not a live session |
| `state` | `globalThis.coaxial.probe()`, `fieldProbe`, `facts()`; numeric probes an app chooses to export, read with `shot --eval` |
| `tree` | `coaxial_a11y` lines (id, role, state, label), mirrored into hidden DOM; no dump for agents |
| `layout` | None for UI. `coax layout` reports struct memory layout |
| `logs` | Page errors forwarded as message 604; build errors pushed over `/coaxial/live` |
| `clock` | See the next section |
| `prefer` | URL flags: `?clock=`, `?variant=`, `?phone`, `?golden`, reduced motion |
| `perf` | `perf.ts`, `frametime.ts`, `coaxial.takeFrameTrace()`, `takePassTimes()`; the strongest of the set |

The MCP server in `apps/openagents-cli/mcp.coax` exposes realm commands
(`realm_shot`, `realm_look`) but not UI driving. Mac and iOS have no
driver at all.

### Agent-owned clock

**Exact.** The runner never reads the wall clock. `clock +ms`,
`clock settle` and `clock data` work on every host. Sessions are pinned
to seed 1, `en-US`, UTC, and the epoch 2026-01-01.

**Coax: partial, with stronger foundations.** Coax already has these
pieces:

- typed clock domains, where mixing clocks is a type error (E0348);
- clip mode: `coaxialClipStep(frames, fps)` advances fixed game time, and
  `coaxial_now_ms` returns clip time;
- `?probe&at=SECONDS`, which steps forward before a probe;
- `?clock=T`, which pins time of day, and golden mode, which holds effects
  still;
- replay of the same input stream across hosts, matching bit for bit
  (`metaverse-engine/core/replay.coax`, `coax/web/replay-hosts.coax`).

**Gap.** There is no interactive "freeze, then advance on command" mode
that a driver controls. There is no `settle` (run until nothing is
scheduled), and no single pinned agent environment (seed, locale, zone,
epoch) shared by every host.

### Tests as scripts that run on every host

**Exact.** `app.test.contract` holds `type`, `tap`, `clock` and
`expect` steps. The same file runs on every host; there is no second
evaluator.

**Coax: partial.** Coax has three kinds of tests, but none of them is a
driver script:

- **Logic.** `#test`, `#forall` and mutation testing are stronger than
  anything in Exact, but they run natively and never touch a host.
- **Pixels.** `goldens.ts` walks every realm in headless Chrome and
  compares 68 golden PNGs.
- **Determinism.** `replay-hosts.coax` runs scripted input natively and in
  Chrome and compares digests at every step.

**Gap.** There is no authored script of operations and expectations,
such as "type, tap, expect the HUD says X", that runs on the browser, Mac
and iOS.

### Retained UI tree, keyed rows, surgical updates

**Exact.** A keyed instance tree. A bitset of dependencies from the
bytecode decides what to update. No tree diff.

**Coax: different by design.** Immediate mode has nothing to diff,
because it redraws. That is fine for worlds at 60 fps. It costs three
things:

- no `tree` or `layout` answers for agents;
- no first frame to bake;
- an accessibility mirror limited to what the app re-emits as text lines
  each frame.

The pieces of a retained layer exist:

- `metaverse-engine/ui/tree.coax`: a fixed table of 128 nodes with roles, the same
  rectangles for drawing, hit-testing and focus, labels required, and
  focus trapped in dialogs.
- `metaverse-engine/ui/hud.coax`: a component-library foundation.
- `apps/shared/zeron/`: about 5.7K lines of GPUI-style components shared
  by the Mac app and openagents.com/chat.

### Layout engine

**Exact.** Taffy for block, flex and grid. The browser is the parity
oracle, and deviations are declared.

**Coax: missing.** Coax has no flex or grid layout and no Taffy. Layout
is coordinate arithmetic, plus GPUI pixel snapping in `gpui.coax`.

### Baked first frame

**Exact.** The compiler runs the app for one frame at build time and
writes the data into the plan. No JavaScript runs before the first pixel.

**Coaxial: missing, though measured.** The first thing painted is an HTML
loading card (`loader.js`), which fades out when the world draws. The
`first-image` tool measures time and bytes to the first world frame. Coax
already has the main ingredient: bounded compile-time execution (`#run`).

### Hot reload that keeps state

**Exact.** `Runner::carry()` keeps every slot whose name and type still
fit. A logic edit asks resources again.

**Coaxial: exists, but coarser.** `serve.c` pushes rebuilt Wasm in
118–135 ms. `host.js:474–491` instantiates the new module against the
same `WebAssembly.Memory`, with this rule:

- If `coaxial_layout()` is unchanged, all state survives.
- Otherwise the whole arena resets ("state layout changed: … starts
  over").

`fast-edit.coax` replaces a single procedure body as an experiment.
Declared migrations are planned (ROADMAP R26–R27, spec §16.3) but not
built.

**Gap.** Coaxial cannot carry state field by field. Adding one field to
one struct throws away the player's place in the world.

### Over-the-air updates

**Exact.** Signed envelopes, files addressed by digest, anti-rollback,
crash recovery and an embedded fallback bundle. Plans and assets only.

**Coax: different.** The web host has a stronger release pipeline. It is
built on baselines: production's existing failures are the baseline,
there is a release lock, the same bytes are promoted, and a regression
triggers automatic rollback (`release.sh`). A deploy takes about 1.2 s.

Native iOS code ships through the App Store. The Grid on iOS loads in a
WKWebView, so it updates with web deploys. No signed bundle format exists
for native Coax apps. Under app-store rules that is the right default
for code.

### Native hosts, access hatches and native pixels

**Exact.** AppKit and UIKit controls. Access hatches hand the platform
object to Swift.

**Coax: has hosts, with a different stance.** The hosts are the Mac app
(Swift, Metal, CoreText through `MetalUI.swift`), the iOS app (UIKit,
with Coax compiled to C in `Generated/`), and the browser
(`host.js`, "the browser's Swift"). Swift calls Coax through generated
headers. Coax draws its own controls, so there is no platform object to
hatch into.

### GPU and shaders

**Exact.** wgpu everywhere, loaded on demand after first pixel. HTML can
be put inside a GPU surface.

**Coax: stronger.** Coax emits shaders from checked source: `coax wgsl`,
`coax spirv`, `coax metal`, `coax metal-compute` and `coax cuda`, with
reflection records. Render plans admitted in `metaverse-engine/render/` run in
`render.js` with a ledger of GPU handles. The GPU is the whole screen,
not an add-on.

### Accessibility, and apps that agents use

**Exact.** HTML roles and `aria-*` on every host. `testId` is shared by
the driver, tests and accessibility tools. `tool` declarations become
WebMCP tools with schemas and consequence hints.

**Coaxial: partial.**

- **Accessibility mirror.** `a11yMirror` (`host.js:2791`) turns
  `coaxial_a11y` lines into hidden buttons and checkboxes, plus an
  `aria-live` status. It covers only button, checkbox and status roles.
- **Text for crawlers.** Pages serve `index.md` and `llms.txt` as text.
- **Tools.** There is no per-app declaration of agent-callable tools.

### TUIs

**Exact.** The same tree is laid out into terminal cells. Headless runs
give the driver `.txt` screenshots.

**Coax: partial.** `coax/lib/tui/tui.coax` is a terminal kit: escape
sequences, UTF-8 widths, a key decoder with bracketed paste, a composer
and an inline live region. Programs write to it directly; it is not a
host for a shared UI tree.

### Formal semantics

**Exact.** Lean 4, with type soundness, a proof that lowering is correct,
and the VM extracted with Aeneas and proved to refine the model.

**Coax: a different method.** Coax relies on differential verification
of the checked AST against native execution
(`verification-differential.md`), bounded conformance probes, checks
plus mutation as evidence (`coax.evidence.v1`), and a strict-math profile
shared by the evaluator and generated C. These checks are practical
rather than proofs. Exact's Lean work is the more ambitious and is worth
watching. Its main trick, keeping the formal model of the VM in step with
the shipped VM through extraction, could apply to the Coax evaluator
later.

## Where Coax is ahead

- **Checks and mutation in the language.** Coax ships evidence records;
  Exact ships tests.
- **An effects inventory with witness paths.** It is close to what
  Exact's grants want to be, and it is computed from the code, not
  declared by hand.
- **Layout fingerprints and `coax compat`.** These cover ABI, effects and
  saved fields, which is the basis for safe state carry and safe updates.
- **No runtime in the output.** C11 to native or Wasm: the splash is
  24 KB and the fixture realm 55 KB. Exact's web bundle is about 20 KB of
  runtime plus the plan.
- **Shader emission from the same checked language.**
- **Cross-host bit-exact replay.**
- **A release pipeline built on baselines, with automatic rollback.**

## Where Exact is ahead

- **One agent driver** with a small fixed set of operations on every host,
  stable `testId` targets, and a stale-build refusal.
- **An agent clock** with `settle`, and pinned agent sessions.
- **Driver test scripts** that run on every host, plus `verify`, which
  also checks dark mode, two widths and the largest text size.
- **A retained, keyed UI tree.** It answers `tree` and `layout`, gives
  surgical updates, and backs real accessibility.
- **A baked first frame.**
- **State carried by name and type** across reloads.
- **An agent on-ramp:** one required document, a lookup command, recipes,
  templates, a pitfalls list with a deletion rule, and agent feedback
  notes.
- **Typed `tool` declarations** exposed through WebMCP.
- **CSS layout** that agents already know.
