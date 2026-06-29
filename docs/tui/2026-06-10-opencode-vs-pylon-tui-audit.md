# opencode TUI vs. Pylon TUI — Same-Framework Gap Analysis

Date: 2026-06-10
Author: agent audit (Claude Code)
Reference repo: `projects/repos/opencode` (workspace reference lane, read-only)
Our surface: `apps/pylon/src/index.ts` (+ `apps/pylon/packages/runtime/src/opentui-renderer.ts`)
Companion doc: `docs/tui/2026-06-10-textual-vs-pylon-opentui-audit.md` (Textual comparison)
Roadmap doc: `docs/tui/2026-06-10-pylon-tui-parity-roadmap.md`

## Why this comparison matters more than the Textual one

The Textual audit compared us against a different language and a different
framework; lessons there are conceptual and need translation. opencode is the
opposite case: it is built **on the same framework we use** — OpenTUI — by the
same team that *wrote* OpenTUI. Its TUI (`packages/opencode/src/cli/cmd/tui/`,
~29,450 LOC across 136 files) is effectively the reference implementation of
how an OpenTUI application is supposed to be structured. We have already taken
specific architecture from opencode; this audit identifies what else should be
harvested directly, where harvesting needs adaptation to our Effect-first house
style, and where we should deliberately diverge.

The standing rule for `projects/repos/*` applies: study and port ideas, do not
vendor code wholesale. But because the framework is shared, "porting" here is
often pattern-for-pattern rather than concept-for-concept.

## Scale snapshot

| Dimension | opencode TUI | Pylon TUI |
|---|---|---|
| TUI LOC / files | ~29,450 / 136 | ~1,700 / 1 (`index.ts`) + 157 (`opentui-renderer.ts`) |
| OpenTUI packages | `@opentui/core`, `@opentui/solid`, `@opentui/keymap` (+ addons), `opentui-spinner` | `@opentui/core` only |
| View paradigm | Declarative Solid JSX + signals/stores | Imperative renderable construction + direct mutation |
| Routes/screens | 2 routes (home, session) + 12+ dialogs + plugin slots | 1 fixed dashboard, 0 dialogs |
| Context providers | 19 nested providers | 0 (module-level globals) |
| Keybinding system | `@opentui/keymap`: leader key, mode stack, user-config keybinds, command palette, which-key, help dialog | hardcoded `onKeyDown` handlers, undocumented |
| Theming | 39 themes, plugin-injectable, dark/light detect, hot-swap | inline `parseColor` literals, one hardcoded SyntaxStyle |
| Client/server split | Yes — TUI client over Worker RPC or HTTP/SSE to engine | No — TUI fused with node services in one process |
| Plugin system | Slots + scoped keymaps + theme injection (~1,131 LOC runtime) | none |
| Composer | 1,713 LOC: mentions, autocomplete (804 LOC), history, frecency, stash, paste decode | TextareaRenderable with onSubmit |
| Effect usage | Scoped: config loading, plugin lifecycle, validation only | Everywhere: all service loops are Effect.gen fibers |
| TUI tests | none (manual + unit tests on logic) | none |

## Architecture comparison, layer by layer

### 1. Process model: client/server split vs. fused monolith

**opencode.** The TUI is a *client*. The engine runs either in a Worker
(`tui/worker.ts`, local mode — HTTP-over-Worker-RPC via `createWorkerFetch()`
and a custom `createEventSource()`) or as a remote server (`tui/attach.ts`,
plain HTTP + SSE with optional basic auth). Either way the TUI consumes one
typed SDK (`@opencode-ai/sdk/v2`) and one global SSE event stream with
exponential-backoff reconnect (1s → 30s).

**Pylon.** `runPylonNode` (`apps/pylon/src/index.ts:666`) constructs the
renderer and launches all node services (wallet poll, presence heartbeat,
NIP-90 provider loop, telemetry, OpenCode inference) as fibers *in the same
process and module scope as the view*. There is no protocol boundary; services
write to the screen by mutating shared renderable refs.

**Assessment.** This is the deepest structural difference and the root cause of
most downstream gaps (testability, attach/detach, multi-surface). The
client/server split buys opencode three things we want badly:

- **`pylon attach`**: watch/drive a running node from another terminal — or
  over Tailnet — without restarting it. For a contributor node that earns money
  while unattended, headless-with-attachable-TUI is the obviously correct
  shape; today, killing the Pylon TUI kills the node.
- **Testability**: the TUI can be tested against a fake event stream; the
  engine can be tested with no TTY.
- **Multiple frontends later** (web operator surface, `control` iOS) consuming
  the same event stream.

Pylon already half-has the backend for this: services are Effect programs.
What's missing is the seam — a typed event stream between node-core and view.
Note the Textual audit reached the same conclusion from a different direction
(state/view separation was its top finding); opencode shows the concrete wire
format for it.

**Harvest directly:** the event-stream client pattern — one SDK, one SSE/stream
subscription, reconnect with backoff, batched application to state. In our
idiom the transport can be an Effect `Stream`/`PubSub` in-process first, with
the HTTP/SSE attach mode added later without changing the view.

### 2. View paradigm: Solid JSX vs. imperative renderables

**opencode.** Components are Solid JSX (`<box>`, `<scrollbox>`) compiled via
`vite-plugin-solid`; `@opentui/solid` provides `useRenderer()` and
`useTerminalDimensions()`. State lives in Solid stores updated with
`produce()`; derived values are `createMemo()`; SSE events are applied inside
`batch()` on a 16ms window so a burst of server events causes one coherent
re-render. Nothing in component code mutates a renderable directly.

**Pylon.** Imperative `@opentui/core`: construct `BoxRenderable`s, `.add()`
children, hold refs in module globals, assign `.content` from service loops.

**Assessment.** This is the largest *leverage* gap: every other opencode
pattern we want (dialogs, command palette, autocomplete, plugin slots) is
written against the Solid layer, so adopting `@opentui/solid` makes those
patterns copyable nearly verbatim, while staying imperative means re-deriving
each one by hand. The integration question for us is Effect↔Solid, and
opencode itself demonstrates the answer (§9): keep the reactive view layer
free of Effect, bridge at the edge. Effect `SubscriptionRef`/`Stream` on the
service side maps cleanly onto Solid signals via a small adapter (subscribe →
`setStore`), which is exactly the role opencode's `SyncProvider` plays for SSE
events (`context/sync.tsx`, 628 LOC).

**Harvest directly:** `@opentui/solid` + the three-layer sync pattern
(event source → event emitter → store reducer), `batch()`-windowed event
application, normalized store schema keyed by ID, `createMemo` for all
filtered/derived lists.

### 3. Keybindings and discoverability

**opencode.** `@opentui/keymap` with the `opentui` addon set
(`tui/keymap.tsx`, 284 LOC + `config/keybind.ts`, 465 LOC): leader key with
configurable timeout, vim-style sequences, **mode stack** (dialog opens → push
"modal" mode → escape pops; keys never leak into the surface below), token
aliases, managed-textarea command registration when an editor has focus.
Keybinds are **user-configurable** (`~/.config/opencode/tui.json`, validated
with Effect schemas). Discoverability is layered: footer hints
(`routes/session/footer.tsx`), a which-key overlay
(`feature-plugins/system/which-key.tsx`, 608 LOC), a help dialog
(`ui/dialog-help.tsx`), and a fuzzy command palette
(`component/command-palette.tsx`) generated from the same keymap registry.

**Pylon.** Tab toggles between two focusables; arrows/page keys forwarded
manually inside `onKeyDown`; nothing user-visible documents any of it.

**Assessment.** Zero reason to diverge. `@opentui/keymap` is a sibling package
of the framework we already depend on. The deciding design idea to copy is
**commands as a registry**: every action is registered once with key,
name, and description, and the footer, help dialog, palette, and which-key all
*derive* from that single registry. (The Textual audit recommended
"bindings-as-data"; opencode ships the finished version.)

**Harvest directly:** the whole subsystem — `@opentui/keymap` + addons, the
mode-stack discipline, command registry, footer hints, help dialog, command
palette. Adaptation needed: none of substance.

### 4. Dialog/overlay system

**opencode.** A stack-based modal system (`ui/dialog.tsx`, 163 LOC):
`DialogProvider` holds a stack with `show/replace/alert/confirm/clear`;
each push also pushes a keymap mode; a semi-transparent overlay
(`RGBA(0,0,0,150)`, z-index 3000) captures outside-clicks to dismiss; focus is
restored on close. On top of the base sit `DialogSelect` (600 LOC — fuzzy
filter via fuzzysort, grouped categories, footer hints, scrollbox
integration) and 12+ concrete dialogs (model/provider/theme/session pickers,
confirm, prompt, help, MCP, status). Toasts (`ui/toast.tsx`, 89 LOC) handle
transient notices.

**Pylon.** Nothing. Errors scroll by in the log; wallet sends and payout-target
admissions have no confirmation surface (flagged as a money-handling risk in
the Textual audit).

**Harvest directly:** the dialog stack + `DialogSelect` + confirm/alert + toast
quartet. This is the highest-value bundle in opencode for us per line of code,
and it is the prerequisite for putting any wallet operation in the TUI safely.

### 5. Composer/prompt

**opencode.** `component/prompt/index.tsx` (1,713 LOC) plus
`autocomplete.tsx` (804 LOC): multi-line textarea with full editing commands,
**parts system** (file/agent mentions as typed parts with virtual inline
text), cursor-positioned autocomplete popup, paste decoding
(`decodePasteBytes`), prompt history with frecency, and a **stash** that
preserves drafts >20 chars across navigation (`PromptStashProvider`).

**Pylon.** A `TextareaRenderable` with placeholder + `onSubmit` → OpenCode
inference (`index.ts:794`).

**Assessment.** Harvest selectively, by stages. History + stash are small and
immediately useful. The parts/mentions system matters once the composer can
target things — agents, sessions, forum topics, payout targets — which is where
Pylon is heading. The full 2,500-LOC composer is overkill until then.

### 6. Feed rendering, streaming, and long transcripts

**opencode.** Renders the session feed in a
`<scrollbox stickyScroll stickyStart="bottom">` with custom scroll
acceleration (`util/scroll.ts`). Markdown via `marked` + shiki highlighting;
diffs via `@pierre/diffs`. Streaming chunks arrive as SSE events and flow
through the batched store, so Solid re-renders only the affected message.
**Notably: no virtualization** — all of a session's messages stay live, with
older history lazy-loaded from the DB on session restore, and tool output
collapsed by default (`util/collapse-tool-output.ts`).

**Pylon.** Also unvirtualized (up to 1,000 live MarkdownRenderables), also
sticky-scroll, with `MacOSScrollAccel`.

**Assessment.** This is the one place the canonical consumer does **not** have
the answer — opencode bounds the problem (per-session feeds, lazy history,
collapsed tool output) instead of solving it. The Textual audit's Line-API
virtualization remains the better long-term design for Pylon's log feed, which
is an unbounded append-only stream rather than a bounded conversation. Near
term, harvest opencode's *bounding* tricks (collapse large entries, cap and
page history); long term, virtualize per the Textual doc.

### 7. Theming

**opencode.** `context/theme.tsx` (1,339 LOC): 39 JSON themes with a small
schema (`defs` + ~10 semantic slots), ref-name color resolution, dark/light
auto-detection, plugin-injected and user-file themes, live switching via
dialog, and `SyntaxStyle.fromTheme()` so code highlighting follows the theme.

**Pylon.** Inline color literals; one hardcoded GitHub-Dark `SyntaxStyle`.

**Harvest directly:** the theme JSON schema and resolution layer — even if we
ship exactly one OpenAgents theme initially, routing all color through semantic
tokens (the Textual audit's `theme.ts` recommendation) is strictly better, and
adopting opencode's *schema* keeps the door open to importing its 39 themes
later for free.

### 8. Routing, sessions, and plugin slots

**opencode.** A deliberately tiny route system (`context/route.tsx`: `home` |
`session(sessionID)`), a session sidebar, quick-switch (1–9), session list
dialog, and a **plugin slot system** (`plugin/slots.ts`, runtime in
`plugin/runtime.ts`, 1,131 LOC): named extension points (`home_footer`,
`session_v2_messages`, sidebar slots) with replace/single-winner/stack
semantics, scoped keymap registration with auto-cleanup on plugin unload, and
theme injection.

**Pylon.** One screen, no routing, no extension points.

**Assessment.** The two-route + dialog-stack shape is the right amount of
routing for a TUI — harvest the shape when Pylon grows a second surface
(assignments table, forum, wallet ops). The plugin system is the one large
subsystem to **defer**: it earns its 1,100+ LOC because opencode has external
plugin authors; Pylon doesn't yet. Take the cheap subset only — named slots as
a code-organization device — and skip dynamic loading.

### 9. Effect: where the reference consumer draws the line

opencode **does** use Effect — but only for config loading/validation
(`config/tui.ts`, exposed as a `TuiConfig.Service`), plugin lifecycle, and
clipboard/process wrappers. The render loop, stores, and event handling are
pure Solid; Effect never runs per-frame. The split is explicit: typed errors
and DI where startup order and I/O failure matter; plain reactive primitives
in the hot UI path.

For Pylon the lesson cuts the other way too. Our backend services (wallet,
NIP-90, presence, telemetry) are *correctly* Effect programs and must stay
that way — that's also house convention (and `nostr-effect` is mandatory for
Nostr behavior). What opencode tells us is to **stop Effect at the view
boundary**: services own `SubscriptionRef`s/`Stream`s; one thin adapter
subscribes and writes into Solid stores; components never see a fiber. This
resolves the tension the Textual audit left open about where reactive state
should live.

### 10. Lifecycle, errors, dev experience

**opencode.** `ExitProvider` centralizes shutdown; `ErrorBoundary` +
`ErrorComponent` catch component crashes with formatted errors; hot reload via
SIGUSR2 (`thread.ts:172`); debug commands expose heap snapshots and a console;
Windows quirks isolated in `win32.ts`.

**Pylon.** `Effect.never` holds the process; cleanup is process death; an
exception in a UI callback is unhandled.

**Harvest directly:** exit provider pattern (pairs with the Scope-based
supervision the Textual audit prescribed), error boundary at the view root,
SIGUSR2 hot reload for dev.

## Gap table

| Capability | opencode | Pylon today | Harvestability | Severity |
|---|---|---|---|---|
| Client/server split + attach mode | Worker RPC / HTTP+SSE, typed SDK, reconnect | fused single process | Pattern ports; transport can start in-process (Effect Stream) | **High** |
| Declarative view layer | `@opentui/solid`, stores, batch, memo | imperative `@opentui/core` + globals | Direct (same framework family) | **High** |
| Keybinding system | `@opentui/keymap`, leader, mode stack, user config | hardcoded handlers | Direct (shared dependency) | **High** |
| Discoverability (footer/help/palette/which-key) | all four, derived from one command registry | none | Direct once keymap lands | **High** |
| Dialog stack + select/confirm/toast | 12+ dialogs on a 163-LOC base | none | Direct | **High** (blocks safe wallet UX) |
| Theming | 39 themes, schema, plugin-injectable | inline literals | Schema ports directly | Medium |
| Composer (history/stash/mentions/autocomplete) | 2,500+ LOC, mature | bare textarea | Staged harvest | Medium |
| Streaming feed updates | SSE → batched store → reactive re-render | direct renderable mutation per poll | Direct with view-layer adoption | Medium |
| Session/route system | 2 routes + sidebar + quick-switch | single screen | Shape ports when needed | Medium |
| Error boundaries + exit lifecycle | yes | no | Direct | Medium |
| Feed virtualization | **absent** (bounded instead) | absent | Not harvestable — see Textual doc | Medium-High (Pylon-specific) |
| Plugin/slot system | full runtime, scoped cleanup | none | Defer; take slots-as-organization only | Low (for now) |
| TUI test harness | **absent** | absent | Not harvestable — see Textual doc (Pilot model) | High (both repos share the gap) |

## Where we should *not* follow opencode

1. **Unvirtualized feeds.** opencode's transcript is bounded per session;
   Pylon's log is an unbounded operational stream. Follow Textual here, not
   opencode (companion doc §2/§5).
2. **No TUI tests.** The reference consumer ships without a UI harness; that is
   a gap in the OpenTUI ecosystem, not a pattern. Textual's Pilot +
   headless-snapshot model remains our target, and a headless buffer-dump
   harness is a candidate upstream OpenTUI contribution.
3. **Effect minimalism in the backend.** opencode keeps its engine in plain TS;
   our node services stay Effect (house convention, `nostr-effect`
   requirement, and they genuinely need supervision). We copy opencode's
   *boundary placement*, not its backend style.
4. **The full plugin runtime.** Premature for a product with no external TUI
   plugin authors.

## Bottom line

opencode is the missing instruction manual for the framework we already chose.
The Textual audit told us *what* good TUI architecture looks like in the
abstract; opencode shows the same conclusions already implemented on OpenTUI:
state/view separation (their SSE→store sync), bindings-as-data (their command
registry), modal discipline (their dialog/mode stack), theming-as-tokens
(their theme schema). The cheapest path to a credible Pylon TUI is to adopt
`@opentui/solid` + `@opentui/keymap`, port the dialog stack and command
registry patterns, put an event-stream seam between node-core and view, and
keep Effect on the service side of that seam — then diverge from opencode only
where Pylon's nature demands it (virtualized operational log, attach-first
process model, money-confirmation flows). The ordered plan is in
`docs/tui/2026-06-10-pylon-tui-parity-roadmap.md`.
