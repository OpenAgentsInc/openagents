# Comet vs Omega UI deep dive — 2026-07-31

Companion to the architecture teardown
[Comet teardown](./2026-07-31-comet-teardown.md). That report covers control
plane, multi-device fabric, harnesses, and disposition. This report is about
**pixels and interaction laws**: the composer (input bar), shell, transcript,
pickers, and how Omega's Zero Base / agent panel compare.

Read-only source audit. Pins:

| Tree | Path | Commit |
| --- | --- | --- |
| Comet | `~/work/projects/repos/comet` | `e5d8e9fb4c2ffe2350e4114db3bfd89979a2136d` |
| Omega | `~/work/omega` | `acd0f5324a570ef8de19b188f93c5e487abe760b` |

No builds, live sessions, or screenshot captures. Findings are from source
structure, pure layout functions with unit tests, and render paths. [source]
[limitation]

**Implementation follow-up.** This document preserves the original pinned
comparison. The subsequent Omega implementation receipts, current gap audit,
and ordered migration plan are maintained in
[Comet teardown §7.4.9](./2026-07-31-comet-teardown.md#749-implementation-update--2026-08-01).

**Naming.** "Ours" means **Omega**, the tracked Zed-fork desktop client — not
the retired Electron OpenAgents Desktop app.

## Executive judgment

**Comet's input bar is better as a coding-agent composer.** That is a product
statement about density, motion, and mid-turn controls — not about editor
power.

Comet designed a purpose-built composer with:

- pure, unit-tested layout math (compact↔expanded flip with hysteresis),
- a 180ms height morph that keeps controls pinned to the bottom edge,
- **one** primary button that is Send, Steer, or Stop by simple state,
- a question wizard that **replaces** the composer when the agent asks,
- inline harness / traits / repo / branch chips,
- staged attachment thumbs,
- per-chat drafts with optimistic send and failure return-to-draft.

Omega designed a **workbench field**: a full Zed `Editor` with mentions,
project creases, paste context, and a footer of authority chrome (executor
menu, model, pin, send). Mid-turn concurrency is handled by a separate
**message queue** with per-entry Steer toggles, plus typed
`SendDisposition` law in `omega_front_door`. That is stronger as an IDE
admission surface and stronger as concurrency *law*. It is weaker as a calm
primary input.

```text
Comet composer (winner for "type and drive an agent")

  ┌──────────────────────────────────────────────┐
  │ [thumbs…]                                    │  staged attachments
  │  Type a message…                    [chips] ●│  Send | Steer | Stop
  └──────────────────────────────────────────────┘
   compact pill (~49px)  ↔  expanded (124–308px)
   new chat: always expanded + full picker row
   AskUserQuestion: whole bar becomes paged wizard

Omega composer (winner for "admit project context + multi-lane authority")

  ┌──────────────────────────────────────────────┐
  │  [  full Editor · min ~96px · expand 80vh ]  │
  │  @mentions · creases · images · paste paths  │
  ├──────────────────────────────────────────────┤
  │ executor ▾ · model · pin · … · send          │
  └──────────────────────────────────────────────┘
   queue list above when mid-turn enqueue/steer
   elicitation cards in transcript, not composer swap
```

**Harvest direction for Omega:** make the Zero Base / agent composer **feel
like Comet's bar** while keeping Omega's Editor substrate, disposition law,
and executor disclosure. Do not replace MessageEditor with a hand-rolled
textarea. Do not copy Comet's monochrome product shell wholesale.

## 1. Scale of the UI code

| Surface | Comet | Omega (agent conversation) |
| --- | --- | --- |
| Composer / input | `crates/ui/src/composer.rs` ~3.6k lines (incl. pure math + tests) | `message_editor.rs` ~5.8k; chrome in `thread_view.rs` `render_message_editor` (~large) |
| Pickers / config chips | `pickers.rs` ~2.8k | executor menu, model selectors, voice admission — split across many modules |
| Shell | `shell.rs` ~4.1k + spaces/tabs | Workspace + AgentPanel (~24k) + Zero Base action gate |
| Transcript | `transcript.rs` ~2.9k | `thread_view.rs` ~15k |
| Theme / motion | owned monochrome + `motion.rs` | Zed theme system + ui crate |

Comet's UI is a **small closed product shell**. Omega's agent UI is a **slice
of a full IDE**. That asymmetry explains most of the density difference.
[source] [inferred]

## 2. The input bar — detailed contrast

### 2.1 What Comet built

Module header (`composer.rs`):

> hand-rolled multiline text input (adapted from gpui's `examples/input.rs`),
> the compact↔expanded flip, the Send/Steer/Stop morph, optimistic send with
> failure recovery, per-chat drafts, and the question wizard that replaces
> the composer while a run awaits input.
>
> Pure decision logic (flip, auto-grow math, button morph, wizard reducer,
> pending-input detection) lives in free functions/structs with unit tests;
> the gpui element only feeds them measurements.

That split — **pure layout law + thin render** — is the main craft difference.
Omega's layout behavior is mostly inline in large `Render` methods and
commented product deltas (`omega#99`, `omega#100`, …). [source]

#### Compact ↔ expanded flip

Comet ports the prior Electron composer's geometry exactly:

| Constant | Value | Role |
| --- | --- | --- |
| Compact pill | ~49px border-box | one line + hairline |
| Expanded empty | 124px | 76px textarea floor + 46px actions + 2px border |
| Expanded max | 308px | 260px content cap + actions |
| Collapse hysteresis | separate thresholds | no oscillation at the boundary |
| Resize freeze | settle window | no flip while the user is dragging the window |
| New chat | always expanded | repo/branch chips need the actions row |

`composer_flip` is a pure function. Flip morph (`FlipMorph`, 180ms ease-out)
animates **height only**: layout commits immediately so the caret never
remounts; the pill clips toward the live target; controls stay bottom-anchored;
text glides with the top edge. Route/session changes **snap** (no morph on
nav). Reduced motion disables morphs. [source] [test]

Omega has expand/minimize (`ExpandMessageEditor`) that jumps the field toward
`vh(0.8)` when expanded, and a fixed `min_h` of ~96px so empty threads do not
collapse to a caret. There is **no content-width-driven compact pill**, no
hysteresis, and no continuous auto-grow morph between two product modes.
Growing with typed text uses the underlying Editor, not a designed bar
geometry. [source]

#### Send / Steer / Stop as one button

```text
send_button_mode(run_live, has_text):
  !run_live          → Send   (up arrow on filled circle)
  run_live + text    → Steer  (same affordance; steers the live run)
  run_live + empty   → Stop   (stop square on the same circle)
```

One 28px control. One click target. The user does not choose "steer" as a
separate mode for ordinary mid-turn text — typing while live **is** steer.
Empty + live is stop. [source] [test]

Omega's law is richer and more honest for multi-executor reality:

- `SendCommand::{Steer, Enqueue}` are distinct intentions.
- `disposition(...)` returns a total `SendDisposition` per executor class
  (native loop can steer at boundary; external ACP depends on capability;
  engine/Full Auto refuses composer steer with a durable-hold fallback).
- The UI surfaces that as a **queue list** with per-entry "Steer" toggles,
  "Send Now", edit, delete — not as one morphing primary button.

So Omega is **better law**, Comet is **better default gesture**. A user who
types during a Claude/Codex run in Comet gets the right thing with one Enter.
In Omega they must understand queue vs steer, and for some executors a steer
becomes a stated refusal + hold. [source] [inferred]

**Owner-facing claim validated by this audit:** Comet's input bar is better
for the common path. Omega should absorb that path without discarding
disposition typing.

#### Question wizard vs elicitation cards

Comet: when the last assistant message has an unresolved `Input` part, the
composer **swaps** to a paged wizard (`1/3`): number keys 1–9, single-select
auto-advances after 220ms, multi-select stays, Back pages back. The panel
stays answerable even if the run aborted (resume path). A steer prompt that
appends a user entry does **not** hide the question (forensics fix vs
last-entry-only reads). [source]

Omega: ACP elicitations render as **cards in the thread**
(`conversation_view/elicitation.rs`) with full form schemas, URL modes, and
submit/cancel actions. The MessageEditor remains the message field. That is
correct for general ACP schemas and richer forms. It is heavier and more
fragmented for the common Claude "pick one of four" case. [source]

#### Pickers as composer chips

Comet's expanded actions row is the product configuration surface:

| Chip | Role |
| --- | --- |
| Harness | Claude / Codex / (mock only under env) |
| Traits | reasoning ladder + model options summary (`High · 1M · Fast`) |
| Repo / folder | browser with breadcrumbs, clone/create |
| Branch | refs + worktree toggle |

Defaults remember last harness/model/reasoning in
`composer-defaults.json` (atomic write). Resolved run config never sends a
silent "default" once the catalog is loaded. [source]

Omega's footer row is **authority and routing**:

- executor dropdown (Omega Agent, ACP peers, …),
- model / tier selection,
- pin / warmth / disclosure surfaces,
- send,
- plus expand control on the field.

That matches Omega Agent as a **router** (front door owns no execution). It
puts more cognitive load on every keystroke. Comet assumes "you already
picked Claude on this space" and keeps chips secondary. [source] [inferred]

#### Attachments and drafts

Comet: paste/drop/picker → staged strip (`size-14` thumbs, wrap height pure
function) → chunked upload to host → `withAttachments` in prompt + inline
image blocks for Claude. Failure returns text and staged files. Per-chat
draft map swaps on session change without morph flicker. [source]

Omega: MessageEditor paste path builds creases and content blocks (files,
images, mentions) into ACP `ContentBlock`s. Stronger project integration;
less "staged strip above a pill" product polish. [source]

#### Optimistic send

Comet queues a doc command and echoes the user row immediately; on failure
restores draft and staged files with a dismissible Notice chip (amber offline /
red error). [source]

Omega sends through thread/queue machinery with stronger generation fencing;
the empty-thread and loading-composer handoff (`OMEGA-DELTA-0122`) is careful
about not losing text while the executor connects. Less "failure notice as
first-class chip" chrome. [source]

### 2.2 What Omega already fixed that Comet never had to

Comet is a single-product shell. Omega absorbed full-window Zero Base and had
to correct IDE inheritance:

| Omega delta (in source comments) | Problem | Fix |
| --- | --- | --- |
| omega#99 | Empty thread composer filled the whole panel | Always bottom-hug content unless expanded |
| omega#100 | Composer band painted edge-to-edge under a narrow message column | Colour/border on the same max-width column as messages |
| omega#100 | Field collapsed to caret on empty thread | `min_h` ~96px |
| omega#100 | Expand control only after first message | Expand always available |
| OMEGA-DELTA-0122 | Loading composer vs live MessageEditor style drift | Shared `composer_editor_style` |
| OMEGA-DELTA-0118 | Zero Base refusal toasts never showed | Fix MultiWorkspace root for toast path |

Those are real quality repairs. They still leave the **primary control** as a
tall Editor box + authority footer, not a morphing agent bar. [source]

### 2.3 Scorecard — input bar

| Criterion | Comet | Omega | Prefer |
| --- | --- | --- | --- |
| Calm default density | compact pill | min ~96px Editor box | **Comet** |
| Content-driven growth | measured auto-grow + morph | Editor growth + manual expand | **Comet** |
| Mid-turn send gesture | one button Send/Steer/Stop | queue + steer toggles + dispositions | **Comet** for UX; **Omega** for multi-executor law |
| Agent questions | composer becomes wizard | elicitation cards | **Comet** for simple picks; **Omega** for general schemas |
| Project mentions / creases | path pickers, not in-buffer mentions | first-class Editor mentions | **Omega** |
| Executor / model honesty | harness chips | front-door disclosure | **Omega** for authority; Comet clearer for dual-CLI product |
| Pure layout tests | extensive | sparse in chrome path | **Comet** |
| Attachment strip polish | designed thumbs strip | paste/crease path | **Comet** |
| Failure recovery in UI | Notice chip + restore draft | generation / queue recovery | **Comet** for bar UX |
| New-chat configuration | always-expanded + chips + remembered defaults | executor menu + project state | Split: Comet clearer for "start a run" |

**Bottom line:** for the thing the user stares at every turn — the input bar —
Comet wins. Omega wins the surrounding IDE admission surface. [inferred]

## 3. Shell and navigation

### 3.1 Comet shell

- Sidebar 208–400px (default 256), 200ms width tween, glass edge-fade on scroll.
- Main header h-11; reserved h-6 status strip so content never jumps.
- Right Changes pane 360–760 (default 520), hidden by default; per-session
  terminal/changes open flags (in memory).
- Browser-style **NavHistory** (back/forward) for routes (chat vs settings
  sections).
- Sidebar **resort glide** (FLIP offsets, 260ms curve) when attention order
  changes.
- Widths/collapsed state → `ui-settings.json`.
- Spaces list + global Active attention list; sessions as horizontal tabs
  (close = archive). [source]

### 3.2 Omega shell

- Full Zed workspace: docks, tabs, project panel, terminal panel, git UI,
  multi-window MultiWorkspace.
- Agent lives in AgentPanel / conversation view; Zero Base admits a reduced
  action set so legacy editor actions stay unreachable.
- Threads sidebar and composer executor menu are separate authority surfaces.
- No Comet-style space-first device+folder sidebar as the primary IA.
  [source]

### 3.3 Shell judgment

Comet's shell is the right shape for **agent supervision product**. Omega's
shell is the right shape for **all work**. Zero Base tries to make Omega feel
like the former without deleting the latter. The remaining gap is mostly the
composer density and the missing space-centric multi-host index, not a missing
file tree. [inferred]

## 4. Transcript

### 4.1 Comet

- Virtualized list, block-granularity rows (`msgId#blockId`), stick-to-bottom
  spring with feed-forward streaming (mugen / use-stick-to-bottom lineage).
- Restick is **direction-aware** (pure tested): wheel-up near bottom does not
  re-pin; return toward bottom inside 70px band does.
- Tool folding (`ToolGroup` / `ToolChip`), MessageRail minimap with hover
  preview (hidden under 48rem width).
- Streaming markdown: background parse, block-level incremental re-parse,
  paint-only fade veil on new text, monospace code height independent of
  highlight. [source] [test]

### 4.2 Omega

- Full conversation view with ACP thread entries, diffs, elicitations, file
  peeks, queue UI, executor disclosure.
- Far more entry kinds and workbench links into the editor.
- Stick/scroll behavior inherits Editor/list patterns rather than a dedicated
  spring controller with pure restick tests. [source] [inferred]

### 4.3 Transcript judgment

Comet is better at **streaming chat ergonomics**. Omega is better at
**connecting a message to a buffer, diff, and receipt**. Harvest Comet's
stick/restick and tool-group density into Omega's agent transcript without
thinning Omega's workbench entry types. [inferred]

## 5. Motion and theme

| | Comet | Omega |
| --- | --- | --- |
| Theme | Always-dark monochrome, oklch neutrals, Geist | Full Zed theme system, user themes |
| Motion kit | Explicit catalog: fade-in, splash-out, pulse, menu-in, resort glide, flip morph | Scattered animations; product polish uneven across agent surfaces |
| Reduced motion | Honored in morph paths (gap called out in PARITY for some surfaces) | Theme/OS dependent via platform |

Comet's motion is a **parity catalog** with timings ported from the prior
product. Omega should not copy the monochrome brand, but should adopt
**one motion kit** for agent list resort, composer mode changes, and stick
scroll. [source] [inferred]

## 6. TUI as a second surface

Comet ships `comet-tui` with **the same pure view module** as the desktop
(`comet_proto::view`) so row order never diverges, and a fingerprinted
transcript cache with a no-tick coalescing loop. The TUI never embeds an
engine. [source]

Omega has no first-party agent TUI peer of Zero Base. Headless work is
effectd / CLI / remote — not a shared-view TUI. If Omega wants SSH attach
parity, Comet's "same derivations, no gpui" pattern is the template. [inferred]

## 7. Recommended Omega product moves

Ordered by leverage. None of these admit Cloudflare rooms or Comet as a
dependency. All are UI/interaction packets against Omega source.

### P0 — Composer density and primary control (input bar)

1. **Compact↔expanded geometry for Zero Base / agent composer**
   - Keep `MessageEditor` (Editor) as the field.
   - Add measured content height + hysteresis modes: calm one-line/short bar
     when empty/short; expand for multiline and for new-thread config.
   - Unit-test the pure flip the way Comet does; do not bury it in render.
2. **One primary control: Send / Steer / Stop**
   - Map empty+live → stop (cancel generation).
   - Map text+live → **declared** steer-or-enqueue via existing
     `disposition` — prefer steer when capability is CanSteer / native
     boundary; otherwise enqueue with a one-line disclosure chip ("Queued —
     this executor cannot steer").
   - Do not invent a third silent cancel-then-send path.
3. **Optimistic echo + failure Notice**
   - User bubble appears immediately; restore draft on failure with a
     dismissible chip.

### P1 — Mid-turn and questions

4. **Simple question takeover for common schemas**
   - When elicitation is single/multi select with short options, render a
     Comet-style paged wizard **in the composer slot**; keep full form cards
     for complex ACP schemas.
5. **Queue as secondary, not primary**
   - Keep the queue for multi-item and edit/send-now power users.
   - Default mid-turn text should not require opening queue chrome first.

### P2 — Configuration chrome

6. **Demote executor chrome on Zero Base**
   - Show a single clear "talking to X" chip; put full executor menu one click
     deeper once a default is set.
   - Remember last executor/model per project (Comet's
     `composer-defaults.json` pattern).
7. **New-thread always "ready tall"**
   - Empty thread composer should match post-first-message height (Omega
     already moved this way with min_h); add config chips row only when
     expanded/new.

### P3 — Transcript and shell polish

8. Direction-aware stick-to-bottom spring for agent transcript.
9. Tool-group folding density for ACP tool bursts.
10. Optional FLIP resort on threads sidebar attention order.
11. Shared pure view crate for desktop + future TUI/mobile list order.

### Explicit non-goals

- Do not hand-roll away Zed Editor (lose mentions, IME depth, accessibility).
- Do not adopt Comet monochrome as Omega brand.
- Do not couple composer polish to Cloudflare session docs.
- Do not weaken `SendDisposition` total function for a prettier button.

## 8. Relationship to the architecture teardown

| Architecture harvest | UI harvest here |
| --- | --- |
| Detach ≠ kill | TUI / second viewport can look like Comet-tui later |
| Durable host-only command ledger | Optimistic send still needs durable queue under the bar |
| Pure `view` module | Pure `composer_flip` / stick / resort modules |
| Spaces as device+folder | Sidebar IA; out of scope for input bar P0 |

The architecture report said Comet is not an IDE. This report says **Comet is
a better agent input product**. Omega should remain the IDE and still ship an
input bar that feels like Comet's. [inferred]

## 9. Evidence anchors

### Comet

- `crates/ui/src/composer.rs` — flip, morph, SendButtonMode, wizard, drafts
- `crates/ui/src/pickers.rs` — DraftConfig, traits summary, harness chips
- `crates/ui/src/settings/composer.rs` — remembered defaults
- `crates/ui/src/attachments.rs` — staged strip
- `crates/ui/src/transcript.rs` — stick spring, ToolGroup, MessageRail
- `crates/ui/src/shell.rs` — layout, NavHistory, resort offsets
- `crates/ui/src/motion.rs` — motion catalog
- `docs/PARITY.md` §1.7–1.12

### Omega

- `crates/agent_ui/src/message_editor.rs` — Editor-backed field, mentions
- `crates/agent_ui/src/conversation_view/thread_view.rs` —
  `render_message_editor`, queue, expand
- `crates/agent_ui/src/conversation_view/elicitation.rs` — question cards
- `crates/omega_front_door/src/send_during_turn.rs` — disposition law
- `crates/omega_front_door/src/omega_front_door.rs` — router, no execution
- `crates/omega/src/omega_zero_base_ui.rs` — admitted-action gate
- `crates/omega_zero_base/src/omega_zero_base.rs` — Zero Base inventory

## 10. Decision sentence

**Admit the product fact: Comet's composer is the better agent input bar.
Plan Omega P0 work to match its density and Send/Steer/Stop primary control
on top of MessageEditor and existing disposition law. Keep Omega's IDE
admission, executor honesty, and workbench transcript depth.**

---

*Design evidence only. Not ProductSpec, not release authority, not a claim
that Omega is "behind" as an IDE.*
