# What Exact is

Exact is Expo's new app framework. Charlie Cheever announced it in a
thread on X in November 2026. This document takes each claim from that
thread and checks it against the source in `projects/repos/exact` (the
public mirror of Expo's internal `exact2` repo). Paths below are relative
to that clone.

At a glance:

- About 1.7 million lines of tracked code, excluding `vendor/`:
  - Rust: 1.23M lines
  - Swift: 341K
  - JavaScript (`.mjs` and `.js`): 241K
  - Contract: 58K, in 748 files
  - Kotlin: 26K
- 298 crates.
- 59 apps under `apps/`.
- 16 vendored crates, including Taffy, Parley, Vello, wgpu-hal, Rapier and
  Hermes (through Ibex).

The public history starts with the initial release on 2026-11-01. The
README says it is pre-1.0, "changes daily", and has "no API stability, no
backwards compatibility, and no migration guide, by design."

## The shape of the system

```
app.contract (view, state, actions)    app.ts or a Rust crate (data sources)
        │                                         │
        ▼                                         │ answers, shape-checked
 Contract compiler: syntax → types → analyze → lower → bake
        │
        ▼
      plan  (43 tables + a 53-opcode bytecode pool, format.json v5)
        │
  ┌─────┼──────────────┬──────────────┬─────────────┐
  ▼     ▼              ▼              ▼             ▼
 Web   Rust runner + kernel (Taffy layout)
 (plan → JS, ~20 KB rt.js, real DOM)
        │              │              │             │
      macOS          iOS          Linux/Windows   Terminal
   AppKit+CoreText UIKit+CoreText  Ink GPU / CPU   cell grid
```

- **Contract** (`contract/`) is compiled in four passes. Each pass is its
  own crate that forbids `unsafe`. Lowering is byte-identical: two
  compilations of the same source give the same plan.
- **The plan** (`plan/tables/format.json`) has 43 relational tables:
  slots, derives, resources, mutations, actions, writes, timers, regions,
  nodes, bindings, routes, keyframes and more. It also has string,
  bytecode and data pools. Loading a plan is a validation pass, "never
  trusted indexing."
- **The runner** (`runner/`) is the plan's virtual machine. It handles
  keyed instances, events, timers on a seekable clock, and the data seam.
- **The kernel** (`kernel/`) is a columnar node arena. It applies binary
  command frames as transactions, so a rejected frame changes nothing. It
  lays out boxes with a patched Taffy 0.14.
- **Hosts** (`host/`) present the tree. `host/` is the largest
  directory at 822K lines; Apple alone is 398K.

## The announcement, point by point

### "Agent native", written in Rust, "extremely fast"

The repository has three principles: agent native, "speed is king", and
"write the web, ship the platform" (README). `rules/RULES.md` turns speed
into budgets:

| Budget | Target |
| --- | --- |
| Cold start to interactive first frame | 100 ms p50 |
| Dev restart, edit to present | 100 ms p50 |
| App JavaScript before first pixel | none |
| Touch one line, rebuild that crate | 30 s |
| Blocking check suite | 6 checks, 65 s total |

The rules file is itself the configuration. `scripts/caps.mjs` parses
its numbers with regular expressions and fails closed. `scripts/boot.mjs`
counts the module graph reachable before first pixel. A count cannot flake
the way a timer can. The per-commit time budgets have no CI gate; device
benchmarks run separately in `bench/board/`.

### Web performance "like a static website" (the Pokédex and RealWorld numbers)

The web host no longer runs the Rust runner. `host/web-js` compiles the
plan to an ES module:

- slots become signals;
- derives become memos;
- resources become `res(...)`;
- nodes become `h(...)` calls with static classes;
- each dynamic binding becomes one effect.

Rolldown tree-shakes it against `rt.js`, so a bundle carries only the
runtime its plan uses. Brotli sizes in `host/web-js/build.mjs` are:

| Bundle | Size |
| --- | --- |
| RealWorld | 23.6 KB |
| Feed benchmark | 27.9 KB |
| Grid | 18.1 KB |

Pages are pre-rendered at build time or per request by a Rust renderer.
They are then "adopted in place instead of hydrated." That is where the
"static site speed, React dynamism" claim comes from: the HTML is complete,
and the runtime attaches signals to existing nodes rather than
re-rendering them.

We did not reproduce the Pokédex numbers. The RealWorld comparison
against React 19 lives in `apps/realworld`.

### Agents author Contract well even though it is "not in the weights"

The strategy has three parts:

- **One required document.** `AGENTS.md` says `docs/start-here.md`
  (335 lines) is "the only required reading. The rest … is lookup only."
- **Lookup on demand.** The lookup material is:
  - 56 one-file recipes in `docs/recipes/`;
  - 6 tested starter patterns;
  - `contract vocab a b c`, which prints the card for any tag, attribute,
    function, recipe or pattern, and for an unknown name prints the
    compiler's refusal text;
  - `bun exact.mjs docs <heading words>`, which prints one section of the
    long guides.

  The long guides run to about 11,000 lines in total, and agents are told
  not to read them front to back.
- **Diagnostics that teach.** See below.

`docs/agent-pitfalls.md` is a symptom → cause → fix list. It admits only
pitfalls reproduced on `main`. An entry is deleted when a fix or a
diagnostic catches it, and entries marked **Candidate diagnostic** could
become a cheap check.

### Contract: "looks like Python", typed like Rust, describes UI and state together

Here is a complete todo view (README):

```contract
component Todos
  state draft = ""
  mutation changed as shape Change refreshes todos
  resource todos = todos() as shape Todo[]
  derive left = todos.filter(t => not t.done).length

  action add
    if draft.trim() != ""
      send changed = addTodo(draft.trim())
      draft = ""

  view
    list
      for t in todos key=t.id
        row testId=`todo-${t.id}`
          text t.title
```

The pieces map onto React and its state libraries:

- `state` is local state, like `useState`.
- `derive` is a memo.
- `resource` is a query, like TanStack Query.
- `mutation` and `send` are a mutation with `refreshes`.
- `action` is the only place state changes.
- `task` schedules work: `every(1000, tick)`, `after(ms, a)` or
  `every(frame, a)`.
- `routes` is a router that keeps a stack per tab.
- `style` and `class=` are named sets of CSS properties, with no cascade.

### "Constrained so it can be fully analyzed": no recursion, no effects in views

The compiler enforces the constraints:

- **No recursion.**
  - `type-fn-recursive`: "a fn is expanded where it is called, so it cannot
    recurse — a traversal is the data crate's."
  - `type-shape-recursive`: "plan values are finite trees."
  - Also `syntax-call-cycle` and `type-derive-cycle`.
- **No loops in actions, and no `await`.** A list is a view's keyed
  `for`; slow work belongs in a data source.
- **Writes are inferred.** A hand-written `writes` clause is refused
  (`syntax-writes-clause`). The inferred write set goes into the plan's
  `writes` table.
- **Every list is keyed.** A `for` without `key=` is refused, and the
  message names the field to use.
- **Expressions have no effects.** Data crosses one seam: a source answers,
  and the runner checks the answer against the declared `shape`.

There is a formal semantics of Contract in Lean 4 (`semantics/`, 23K
lines). It has an operational semantics, a Hoare logic for actions, and a
proof of type soundness. It also has a proof that the lowering to VM code
is correct. On top of that, the shipped VM machine is extracted to Lean
with Charon and Aeneas and proved to refine the model. The Rust runner is
tested against the semantics both differentially and at random.

### Error messages that teach agents on demand

Every diagnostic has the form `{pass, id, message, span, related[]}`.
`contract build --json` emits it for tools, and one run reports up to
twenty independent errors.

`contract/corpus/rejects.txt` holds one reject fixture per rule: 245
fixtures over 164 ids. Grepping the compiler finds about 540 id strings.

The messages repair web habits rather than just failing:

- `` `text` has no attribute `size`; `size` is spelled `font-size` here,
  the web's name ``
- `type-now-renamed` explains `performanceNow()` against
  `time.epochAtZero + performanceNow()`.
- Unknown names get "did you mean."
- Some warnings teach design. For example, `lower-screen-by-state`
  explains that switching screens on a state breaks Back, reload and deep
  links. It then points at `contract vocab screen` and the navigation
  recipe.

### The agent controls the clock

The runner never reads the wall clock. Time is a number the host supplies
through `Runner::advance(now_ms)`. Each due timer, `then` and queue
deadline fires as its own commit at its own due time.

| Host | How the clock is controlled |
| --- | --- |
| Web | Under `?agent`, `rt.js` takes the driver's clock. `agent.js` registers CSS animations and seeks them, so `clock settle` lands transitions too. |
| Apple | `Agent.swift` freezes the wall clock "before anything waits". It refuses to set the clock backwards (`ClockWentBackwards`). |
| Linux, terminal | Both dispatch `clock` to the runner. |

The driver accepts these clock forms: `clock 500`, `clock +60000`,
`clock settle`, `clock data` (wait for pending answers), `clock held`, and
`clock until state … within ms`.

Agent sessions are pinned on every host to seed 1, `en-US`, UTC, and the
epoch 2026-01-01. Two runs of the same script see the same world.

### Inspecting the app: screenshots, video, the abstract tree, layout

`scripts/agent.mjs` has ten operations: `tree · screenshot · tap · type ·
state · layout · logs · clock · prefer · perf`. The README says there are
ten "on purpose: an agent API grows one reasonable addition at a time until
no one can hold it in mind."

How the operations work:

- Variations are arguments, not new commands. For example,
  `"screenshot film.png over 600 every 50"` films a contact sheet, and an
  `.apng` name gives an animation.
- Targets are `testId`s, never coordinates.
- `tree`, `state` and `logs` come from the runner and kernel on every
  host, "never from a host's mirror." So they agree across hosts by
  construction.
- `layout`, `screenshot`, `tap` and `type` go through each host's real
  render and input path.

Other driver features:

- **Reference comparison.** `screenshot x.png compare ref.png` reports, in
  text and node by node, where two screens differ. An agent need not look
  at either picture.
- **Source maps.** `contract build --map` writes development-only source
  locations beside the plan, keyed by its SHA-256. Any node on screen
  leads back to the line that declared it.
- **Stale-build refusal.** The driver refuses a build older than its
  sources and names the command to rebuild (`refuseStale` in
  `scripts/agent-launch.mjs`). It exits with status 3, so `exact.mjs` can
  rebuild and retry.

### Tests as scripts that run on every host

```contract-test
test "add, finish and delete"
  expect text "count" == "0 left"
  type "new-todo" "Buy milk"
  tap "add"
  expect text "count" == "1 left"
  tap "toggle-1"
  expect tree missing "todo-1"
```

The steps compile "to a script the agent driver runs against the real
hosts; never a second evaluator." An optional `on web, macos` clause
limits a test to some hosts.

`bun exact.mjs verify` does more than run the tests:

- It checks the first screen and each test's end in light, dark and two
  widths.
- It checks them again at the largest text size, and reports spilled text
  with a fix.
- `verify ios` also plays real touches on a long list. It reports
  `scrolling: like UIKit`, or what a person would notice, against UIKit's
  values.

### Surgical updates, no virtual DOM, no separate hydration

When the native runner boots, it scans the bytecode of every binding,
derive and resource argument once. From that it builds a bitset table of
inputs (`runner/src/instance/deps.rs`).

An update then works like this:

- It skips any subtree whose reads did not change.
- It emits binding ops only for changed values.
- It emits `SetChildren` only for changed child lists.
- It creates or destroys only rows whose keys were added or removed.

The code says: "No tree diff is needed." `Update::full` is the reference
implementation that incremental updates are tested against.

There is one nuance: the dependency graph comes from compile-time bytecode,
but the runner builds it when it boots; the plan does not store it. The
web runtime tracks dependencies at run time with lazy, glitch-free signals.

### Live updates in milliseconds, and over the air

**In development:**

- **Native and Wasm.** `Runner::carry()` keeps every slot whose name and
  type still fit, plus the router, the store, the clock and any settled
  resource whose source and arguments match. Restarting a plan in place
  takes about 20 ms.
- **The JS web target.** The old page posts a checkpoint to the dev server
  before it reloads, and the new page consumes it. The README measures a
  saved edit in the page in 99 ms. The thread's "10–20 ms" refers to the
  plan swap itself.
- **TypeScript and Rust.** TypeScript modules reload logic on web, macOS
  and iOS. Rust reloads as a shared library on macOS and Linux, and as
  Wasm on iOS and the web.

**In production**, the `update/` crate handles delivery:

- A bundle is a signed envelope (Ed25519, at most 64 KB) plus files by
  digest.
- Updates are stored atomically, with anti-rollback through a `seq` floor
  and crash recovery.
- The embedded bundle is "entry zero", the fallback.
- `scripts/deploy.mjs` publishes the blobs first, then the signed head as
  a conditional put.
- Apple apps check every 15 minutes, off the boot path.

Signed delivery covers plans and assets. It does not cover TypeScript or
Rust code; app-store rules limit that anyway.

### Business logic in Rust or TypeScript, mixed, on any thread

The `DataSource` trait is "the one seam through which computation
enters." The data side works like this:

- **TypeScript** runs on Hermes on native hosts and in the browser's own
  engine on the web. `js/bake` compiles it to Hermes bytecode at build
  time, so no source is parsed at boot.
- **Generated types.** `contract types` generates the `.d.ts` the
  TypeScript side implements, and `contract rust` generates the Rust
  structs.
- **Shape checks.** Every answer is checked against the declared shape.
  The check is incremental: a 10,000-row list with one row inserted checks
  one row.

Grants are capability lines in `app.ts`, such as
`'net.fetch https://api.example.com'` and `'sqlite.open app:/data/x.db'`.
The design is "carried, not inferred." A module that was not given
`fetch` has no expression that can produce one (`grants/`).
`runner/src/device.rs` maps each device grant to every host's own form:
iOS `Info.plist` keys and the web's `Permissions-Policy`.

TypeScript cannot import npm package code yet; it can import only types.

### Escape hatches and "access hatches"

There are three levels:

- **Native modules.** A native module is a hyphenated tag, such as
  `exact-map`, backed by Swift, Kotlin or Rust. The kernel lays it out
  like any other box. MapKit on Apple and OpenStreetMap on the web are
  examples.
- **Access hatches.** A node marked `hatch="word"` hands its platform
  object (a `UIView`, a navigation controller, a DOM element) to native
  code. That code can configure what Exact made. It can change Contract
  state only by acting on authored nodes, "as a person would."
- **No escape from the plan.** Nothing steps outside it.

The thread's Liquid Glass long-press fix is this mechanism: Swift gets
the tile's real `UIView` and corrects its clipping.

### Native smoothness and startup

The README's third principle is "write the web, ship the platform":

- **Layout and names come from CSS.** The compiler refuses aliases and
  names the web's spelling.
- **Anything unsaid comes from the platform.** That covers controls,
  colour, type size, Back, the keyboard and accessibility.
- **Platform list rows.** A vertical `list` uses UIKit's rows on iOS and
  a table on the Mac.

`scripts/no-tells.mjs` flags literal colours and sizes that would make an
app look non-native.

Two mechanisms support the smoothness and startup claims:

- **Smoothness** comes from the kernel's layout and view recycling.
  `bench/board/` holds a device scoreboard against UIKit and SwiftUI.
- **Startup** comes from **baking**. `contract::bake` boots the runner
  once at build time, against the app's data source, and writes every
  resource's boot value into the plan. In the code's words, "The compiler
  is, for one frame, a host."

Baking skips values that depend on the network, randomness or the date. A
resource that reads the store bakes the empty-store answer, which is a
fresh install's first frame. At launch, Rust draws that frame before any
JavaScript runs.

### Layout with Taffy; text measured by the same line breaker

The kernel uses a patched Taffy for block, flex and grid layout. Patches
are in `vendor/taffy/EXACT-PATCHES.md`. The host measures text: CoreText
on Apple and cosmic-text on Linux. On the web the browser lays out the
same CSS itself, so the web build carries no layout engine.

The browser is the "parity oracle". Native layout, Canvas 2D, SVG and
motion are compared against what Chrome draws. Any unavoidable deviation
is declared with its reason; for example, Taffy has no `position:
static`.

`textflow/` implements CSS `shape-outside`, exclusion bands, line
breaking and balancing. Every host shares one Rust walker, so Rust code
can measure text exactly as the app will lay it out. That is what makes
the Pretext-style apps (`apps/reflow`, `apps/textflow`) possible.

### GPU, shaders over live UI, and the game engine

- **GPU.** `gpu/` says "wgpu is the one GPU API, on every host" (Metal,
  Vulkan, WebGPU). An app's GPU surfaces compile into one artifact, a
  dylib or Wasm. It loads on demand the first time a canvas is on screen,
  never on the boot path. A failure affects only its own canvas.
- **Game engine.** `game/` (155K lines) is an optional engine in its own
  Cargo workspace. No core crate depends on it. Gameplay is Rust, physics
  is vendored Rapier, and menus and HUDs are Contract.

### Ink, the desktop renderer

`host/ink` describes itself as "instanced UI geometry over a portable
device". One `ink.wgsl` draws rounded rectangles, rings, shadows, glyphs,
coverage, layers and blur, with an atlas and a tiny-skia CPU fallback.
Linux and Windows use it; on Windows it draws to a DirectX 11 composition
swap chain.

The thread compares Ink with GPUI and egui: "a bit faster" to start, with
more GPU and peak memory.

### TUIs

`host/terminal` is labelled "a spike". It is 12K lines with `unsafe`
denied. The runner and kernel run natively. After each commit the host
lays the tree out in a fixed 8×16 cell and paints it into a grid of
character cells, diffed and sent as escape sequences.

Headless, the same host answers the driver's operations and writes
`.txt` and `.ans` screenshots. It reads its own output through a built-in
terminal emulator (`vt.rs`). Images go out as kitty, iTerm2 or
half-blocks.

### Apps that agents use

Contract has a `tool` declaration (`contract/syntax/src/tool.rs`):

```
tool NAME reads|writes [additive]|destroys [idempotent] [closed]
  "DESCRIPTION" = ACTION [returns EXPR] [fails when EXPR] [available when EXPR]
```

A tool is a root action offered to a caller that is not the person:

- Its consequence word is `reads`, `writes` or `destroys`.
- Its clauses are derives, so the compiler type-checks them.

On the web, `host/web-js/calls.js` registers each tool through WebMCP
(`document.modelContext`) where the browser supports it:

- Its input schema comes from the plan's types.
- `reads` maps to `readOnlyHint`; `writes` and `destroys` map to
  `consequentialHint`.
- A tool is withdrawn while its `available when` is false.

A call runs as one commit with origin `tool`. It skips commands that would
move the person's view, such as `focus`, `scrollIntoView` and `haptic`.
Its answer is shape-checked.

Accessibility comes from HTML's own vocabulary: `role`, `aria-*`,
`testId`. A `testId` serves the driver, tests and accessibility tools at
once. Windows accessibility uses vendored `accesskit-windows`.

### Platform maturity

The README agrees with the thread:

- **iOS** is the most mature host.
- **Web and macOS** are good.
- **Linux** reads but cannot edit text, and has no word segmenter for
  Thai and related scripts.
- **Android** is early: it builds, runs and is driven by the agent, but
  has no delivery path.
- **Windows** has ink and CPU painters, and its general control parity is
  unfinished.

The README also lists work deliberately left out of v1:

- camera access;
- a gesture arena;
- general layout animation;
- grid virtualization;
- a devtools UI;
- a packaged CLI. Apps refer to an exact2 checkout by path.

### Templates

`expo/exact-templates` has three whole apps:

- `chat`: verified on web and on iOS 27 iPhone, iPad and iPhone Duo.
- `landscapes`: a port of Apple's Liquid Glass Landmarks sample, with 1 of
  4 tests passing.
- `softwear`: a store with a Bun and SQLite demo API, with 28 web UI tests
  passing.

Each has a keyword list. An agent picks a template by matching a request
against `INDEX.md`, and `new.mjs` scaffolds it.

### Agent feedback

`bun exact.mjs feedback note "<what happened>"` records friction in one
line. The record holds the commit, verb, exit code, duration and OS, and
never any output or code. It is sent to Expo only with consent.
Generated apps' `AGENTS.md` asks agents to use it, so Expo collects
friction reports from agents in the field.

## Development rules worth noting

`rules/RULES.md` is 84 lines:

- **Six blocking checks within 65 s.** A seventh requires deleting one in
  the same change.
- **Fail-fast is banned.** Every check reports all failures in one run.
- **Fix loops get three rounds**, then a human decides or the target is
  narrowed.
- **"Agents remove apparatus freely and add none."** An agent change that
  adds a check, script, registry, config file or design doc needs explicit
  human approval.
- **A spec needs an implementer and a date.**
- **Worktree hygiene.** Never `git stash`, and kill only PIDs you started.
  These match our own workspace rules.
