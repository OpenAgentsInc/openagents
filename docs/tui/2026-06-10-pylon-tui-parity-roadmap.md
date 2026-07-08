# Pylon TUI Parity Roadmap — Evolving Toward the opencode Baseline

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-10
Author: agent audit (Claude Code)
Inputs:
- `docs/tui/2026-06-10-opencode-vs-pylon-tui-audit.md` (gap analysis, same framework)
- `docs/tui/2026-06-10-textual-vs-pylon-opentui-audit.md` (conceptual lessons)
Target surface: `apps/pylon/src/index.ts` and successors

Tracking issues: Phase 0 #4736 · Phase 1 #4737 · Phase 2 #4738 · Phase 3
\#4739 · Phase 4 #4740 · Phase 5 #4741 · Phase 6 #4742

Status: Phase 0 shipped (#4736, with quiet startup #4743). Phase 1 shipped
(#4737): the dashboard is Solid components on `@opentui/solid` under
`apps/pylon/src/tui/` (theme tokens, view store, Effect<->Solid bridge,
per-pane error boundaries), with the Solid transform loaded via bunfig
preload in dev/tests and a one-shot `--preload` re-exec for the packaged
bin. Phase 2 shipped (#4738): @opentui/keymap command registry, dialog
stack (alert/confirm/prompt/select + toasts), ctrl+k palette, f1 help,
footer hints, f2 verbose toggle, confirm-gated wallet commands, and
user keybinds via `keybinds.json`. Phase 3 shipped (#4739): the feed is
a virtualized 100k-line buffer (Textual Line-API style — only the visible
window renders), long lines collapse with elision, streaming responses
render as live markdown tails that flatten on finish, and every log entry
persists to `feed-log.jsonl` in the Pylon home dir (size-capped rotation)
with the previous session's tail restored at startup. Phase 4 shipped
(#4740): `pylon node` runs node-core headless with a token-authenticated
loopback control API (HTTP command endpoint + SSE event stream with a
snapshot-on-connect), `pylon attach [url]` mirrors a running node in the
full dashboard with 1s–30s reconnect backoff, wallet commands round-trip
the control API node-side, and detaching never interrupts the node. The
interactive dashboard serves the same API for second-terminal attach.
Phase 5 shipped (#4741): route system (f3 dashboard / f4 assignments /
f5 wallet, palette-discoverable), an assignments surface with lease rows
keyed by leaseRef and confirm-gated accept (local or over the control
API when attached), a wallet surface with readiness and session balance
history, and composer history/stash persisted across restarts (ctrl+p /
ctrl+n). Slot-style plugin composition was deliberately skipped — no
external TUI plugin authors yet, per the roadmap's non-goals. Phase 6
shipped (#4742): an owned headless render harness (`src/tui/harness.tsx`,
the Textual Pilot model on @opentui/solid's testRender) with programmatic
key injection, character-frame capture, bun:test snapshots at three
terminal sizes, dialog/palette interaction tests, and protocol tests that
drive the real runtime+bridge with fake PylonEvent streams. All six
roadmap phases are complete; the TUI is at the targeted opencode parity.

## Goal and non-goals

**Goal:** bring the Pylon TUI mostly into parity with opencode's TUI as the
canonical OpenTUI application — declarative view layer, real keybinding system
with discoverability, dialog stack, theming, event-stream architecture,
attach-capable process model — while keeping Effect as the service substrate
and diverging deliberately where Pylon's nature (unattended money-earning
node, unbounded operational log) demands it.

**Non-goals (explicitly out of scope for parity):**
- opencode's plugin runtime (no external TUI plugin authors yet)
- 39-theme catalog (one good theme on the right schema; catalog imports later)
- the full 2,500-LOC composer with parts/mentions (staged subset only)
- matching opencode's *absence* of feed virtualization and TUI tests — on both
  we target better than the baseline, per the Textual audit

**Definition of "mostly parity":** a Pylon operator gets the same interaction
grammar an opencode user gets — discoverable keybinds, command palette,
modal dialogs with confirmations, themed UI, multiple surfaces, attach to a
running node — even though the surfaces themselves (wallet, telemetry,
assignments, forum) are Pylon-specific.

## Architecture decisions locked in by the audits

These are the decisions the phases below assume; revisit them only with cause.

1. **Adopt `@opentui/solid` for the view layer.** Every harvestable opencode
   pattern is written against it; staying imperative means re-deriving each
   one. (opencode audit §2)
2. **Effect stops at the view boundary.** Node services remain Effect programs
   (house convention; `nostr-effect` for all Nostr behavior) exposing
   `SubscriptionRef`/`Stream` state. One adapter subscribes and writes Solid
   stores. No fibers inside components. (opencode audit §9)
3. **Event-stream seam between node-core and TUI.** In-process Effect
   `Stream`/`PubSub` first; HTTP/SSE attach transport later without changing
   the view. (opencode audit §1)
4. **Single command registry** feeding footer, help dialog, palette, and
   which-key. (opencode audit §3; Textual audit §6)
5. **Virtualize the log feed Textual-style**, not opencode-style bounding
   alone. (Textual audit §2/§5; opencode audit §6)

## Phase 0 — Seam and supervision (prerequisite for everything)

*Theme: make the current dashboard restructurable without changing what it
shows. No new user-visible features.*

1. **Extract domain state from the view.** Replace the module-global
   renderable refs in `apps/pylon/src/index.ts` with typed state owned by
   services: `WalletState`, `TelemetryState`, `OperatorState`, `LogFeedState`,
   each behind a `SubscriptionRef` (or `Stream` for the log). Service loops
   update state; nothing in a service touches a renderable.
   - Pattern source: opencode `context/sync.tsx` (normalized store fed by
     events), translated to Effect on the producer side.
2. **Define the node event stream.** A single typed `PylonEvent` union
   (wallet/telemetry/log/operator/inference events) published to a `PubSub`.
   This is the future attach wire format — design the types as if they will be
   serialized, because in Phase 4 they will be.
3. **Scoped supervision.** Replace `runBackgroundEffect` fire-and-forget with
   `Effect.forkScoped` inside one root `Scope`; Ctrl+C interrupts fibers,
   flushes the renderer, restores the terminal deliberately. Add an exit
   provider equivalent (opencode `context/exit.tsx`).
4. **First tests.** With state out of the view, test the state machines with
   no TTY: wallet poll failure → `WalletState` OFFLINE; heartbeat cadence;
   log ring-buffer behavior. `bun:test`, plain Effect tests.

**Exit criteria:** dashboard looks identical; `index.ts` no longer contains
business logic; killing the TUI is a clean interruption; ≥10 state-machine
tests pass in CI.

## Phase 1 — View layer migration to `@opentui/solid`

*Theme: rebuild the existing four-pane dashboard declaratively. Still no new
features — this phase buys leverage, not surface.*

1. Add `@opentui/solid` + `vite-plugin-solid` build wiring (reference:
   opencode `packages/opencode` build scripts; OpenTUI source at
   `projects/repos/opentui`).
2. **Bridge adapter:** `subscriptionRefToSignal` / `streamToStore` helpers —
   subscribe to Phase-0 state, apply with `batch()` on a ~16ms window
   (opencode `context/sdk.tsx` batching pattern).
3. Recreate the dashboard as components: `<Dashboard>`, `<LogFeed>`,
   `<WalletPane>`, `<TelemetryPane>`, `<OperatorPane>`, `<Composer>`; layout
   stays the same flexbox shape.
4. **Theme tokens:** port opencode's theme JSON schema (`context/theme.tsx`
   schema portion only — defs, ~10 semantic slots, `SyntaxStyle.fromTheme`)
   with a single `openagents` theme. Delete every inline `parseColor` from
   view code.
5. **Error boundary + root lifecycle:** Solid `ErrorBoundary` with a formatted
   fallback (opencode `component/error-component.tsx` pattern); resize via
   `useTerminalDimensions()`.

**Exit criteria:** feature-identical dashboard rendered from Solid components;
zero direct renderable mutation outside the bridge; all colors via theme
tokens; a thrown error in a pane shows a fallback instead of killing the node.

## Phase 2 — Interaction grammar: keymap, dialogs, palette

*Theme: this is the phase an operator actually feels. Highest user-visible
value; everything harvests near-verbatim from opencode.*

1. **`@opentui/keymap`** with the `opentui` addons: leader key, mode stack,
   sequence handling, managed-textarea commands (opencode `tui/keymap.tsx`,
   `config/keybind.ts`).
2. **Command registry:** every action registered once
   (`{ key, command, description, when }`). Pane focus switching, scroll
   commands, composer submit all move here.
3. **Footer hints** derived from the registry for the active mode/focus
   (opencode `routes/session/footer.tsx`).
4. **Dialog stack:** port the base (`ui/dialog.tsx` shape: stack provider,
   overlay, mode push/pop, focus restore) plus `DialogSelect` (fuzzy filter),
   `DialogConfirm`, `DialogAlert`, and toasts.
5. **Command palette** over the registry (opencode
   `component/command-palette.tsx`) and a **help dialog** listing all binds.
6. **Money confirmations:** wallet send and payout-target admission flows gated
   behind `DialogConfirm` with explicit amounts — the safety gap both audits
   flagged. Surface MDK wallet ops (balance detail, receive/send) as palette
   commands using these dialogs.
7. **User-configurable keybinds** file (validated with Effect schema, matching
   opencode's config approach) — low cost once the registry exists.

**Exit criteria:** no hardcoded `onKeyDown` dispatch outside the keymap; footer
shows live hints; palette and help dialog enumerate every command; no wallet
mutation reachable without a confirm dialog.

## Phase 3 — Feed engine: virtualization and richer rendering

*Theme: the one place we deliberately beat the opencode baseline.*

1. **Virtualized log feed:** replace per-entry `MarkdownRenderable`s with one
   feed component over a ring buffer of pre-rendered lines, rendering only the
   visible window (Textual Line-API concept; OpenTUI `TextBuffer` as
   substrate). Width-keyed render cache; re-render on resize only for visible
   window.
2. **Bounding tricks from opencode** as complements: collapse large entries by
   default (`util/collapse-tool-output.ts` idea), cap in-memory history with
   paged recall from disk.
3. **Streaming discipline:** inference/stream chunks append via the event
   stream and batched store updates instead of mutating a renderable mid-frame.
4. **Persist logs to disk** (JSONL alongside existing node state) so the TUI
   is a window onto durable history, not the history itself — prerequisite for
   attach mode showing scrollback.
5. Optional: syntax/markdown caching keyed on content+width.

**Exit criteria:** 100k-entry log session keeps a flat memory profile and
smooth scroll; restart shows persisted scrollback; streaming responses render
without full-feed reflow.

## Phase 4 — Process split and attach mode

*Theme: the node earns unattended; the TUI becomes a client.*

1. **Split binary roles:** `pylon` runs node-core headless by default
   (services + event stream + small control API); `pylon tui` (or default
   interactive launch) starts node-core and attaches in one process — the
   opencode `thread.ts` worker pattern, with node-core in the worker.
2. **Serialize the Phase-0 event stream** over HTTP+SSE (or WebSocket) with
   the reconnect/backoff pattern (1s→30s) from opencode `context/sdk.tsx`;
   commands go over a typed HTTP API. Auth via local token; over Tailnet this
   gives remote node supervision for free (aligns with the `control` iOS
   direction).
3. **`pylon attach <addr>`** — the opencode `attach.ts` equivalent.
4. **Detach-safe semantics:** killing an attached TUI never interrupts node
   fibers; confirm dialogs for money flows execute node-side with explicit
   acknowledgement events.

**Exit criteria:** node survives TUI exit; second terminal can attach to a
running node and see identical state including scrollback; a money confirm
round-trips through the protocol.

## Phase 5 — Surfaces and routing

*Theme: spend the leverage. New product surface on the now-cheap foundation.*

1. **Route system** (opencode `context/route.tsx` shape): `dashboard` |
   `assignments` | `wallet` | `forum`, plus the dialog stack for everything
   transient. Quick-switch bindings (1–9 style).
2. **Assignments/jobs table:** rows keyed by job id (Textual DataTable's
   key-vs-position separation), virtualized via the Phase-3 engine; accept/
   decline through confirm dialogs.
3. **Wallet surface:** balance history, receive/send, payout targets — all
   operations already dialog-gated from Phase 2.
4. **Composer upgrades, staged:** history + stash first (small, opencode
   `prompt/history.tsx`, `prompt/stash.tsx`), then cursor-positioned
   autocomplete for `@agent` / session / topic mentions (opencode
   `prompt/autocomplete.tsx` pattern) once there are targets to mention.
5. **Slot-style composition** (cheap subset of opencode's plugin slots) as a
   code-organization device for sidebar/footer extensions — no dynamic plugin
   loading.

**Exit criteria:** ≥2 non-dashboard surfaces shipped; navigation discoverable
via palette/help; composer history works across restarts.

## Phase 6 — Owned test harness

*Theme: close the gap both repos share. No upstreaming planned — our codebase
will diverge significantly from OpenTUI/opencode upstream, so the harness is
built and owned in this repo.*

1. **Headless render harness:** drive the OpenTUI renderer against a dumpable
   buffer (no TTY), inject key events, snapshot the text frame with
   `bun:test` snapshots — Textual's Pilot model, implemented in-repo.
   Spike first against what the Zig core already exposes
   (`projects/repos/opentui/packages/core`) versus what we wrap ourselves.
2. **Snapshot the dashboard and each surface** at 2–3 terminal sizes; snapshot
   dialog stacking and focus restore.
3. **Protocol tests:** fake event stream → assert rendered frames; this is the
   payoff of the Phase 4 split.

**Exit criteria:** TUI render regressions caught in CI; harness usable by
runtime package renderers (`opentui-renderer.ts`) too.

## Sequencing rationale and dependencies

```
Phase 0 (seam/supervision)
   └─► Phase 1 (Solid view)
          └─► Phase 2 (keymap/dialogs/palette)   ◄─ biggest operator-visible win
          └─► Phase 3 (virtualized feed)
                 └─► Phase 4 (split/attach)      ◄─ needs event types (P0) + persisted log (P3)
                        └─► Phase 5 (surfaces)   ◄─ needs routing + dialogs + table
   Phase 6 (tests) — start harness investigation early; full suite lands after P1
```

- **Phase 0 before Phase 1:** migrating the view while business logic still
  lives in it would port the architecture problem into JSX.
- **Phase 2 before Phase 3/4:** discoverable interaction is the largest
  usability jump per unit of work and de-risks every later surface (dialogs
  are reused everywhere).
- **Phase 4 after Phase 3:** attach mode without persisted/virtualized logs
  would attach to an empty screen.
- **Phase 6 is a thread, not a tail:** the headless-buffer investigation
  (one-day spike into the OpenTUI Zig core) should happen during Phase 1 so
  snapshot tests can accrete from Phase 2 onward; it is listed last only
  because its full payoff needs the seam and the split.

Rough effort ordering (not estimates): P0 and P2 are the high-leverage small
phases; P1 is mechanical but touchy; P3 and P4 are the real engineering; P5 is
product work on a paid-down foundation; P6 is a spike plus accretion.

## Standing rules for all phases

- Patterns from `projects/repos/opencode` and `projects/repos/textual` are
  ported, never vendored; cite the source file in the commit/doc when a
  pattern is adopted.
- All Nostr-facing behavior continues through `nostr-effect` — the TUI never
  grows parallel Nostr primitives.
- No fiber, layer, or Effect import in Solid component files; the bridge
  adapter is the only module that touches both worlds.
- Every new command is registered in the command registry (no ad hoc
  `onKeyDown` dispatch), and every state-mutating money command goes through a
  confirm dialog, including over the attach protocol.
- Keep `apps/pylon/README.md` honest: when Phase 2 lands, document the TUI for
  the first time (the README currently doesn't mention it at all).
