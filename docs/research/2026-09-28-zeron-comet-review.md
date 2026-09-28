# Zeron (zeronsh/comet) review

This note reviews Zeron, a local-first controller for coding agents, and
ranks what OpenAgents can reuse for the phone chat, multi-provider agent
support, and remote control. It compares Zeron with Rust Native, the
OpenAgents and Coder iPhone apps, the Coder host and access crates, and
Microcoder.

| Field | Value |
| --- | --- |
| Source | `github.com/zeronsh/comet`, local clone at `~/work/projects/repos/comet` |
| Revision reviewed | `c2744c2f` (2026-09-27) |
| License | MIT, copyright 2026 Wing (`LICENSE`) |
| Method | Read-only reading of code, tests, and docs. Nothing was built or run. |

The MIT license lets you reuse, modify, and redistribute the code, including
in this repository, if you keep the copyright and permission notice with any
substantial copied portion. This repository's rule still applies: reimplement
designs here and name the source in the commit message. Don't vendor Zeron
code. Zeron's `THIRD_PARTY_NOTICES.md` lists bundled components, such as
Tree-sitter grammars and the Symbols icons, which keep their own licenses.
The text and markdown designs credit pretext, mugen, and streamdown in their
module docs (`crates/text/src/lib.rs`, `crates/markdown/src/mend.rs`), so
check those projects' licenses too before porting an idea that came from
them.

Paths that start with a Zeron directory, such as `crates/mobile/...` or
`apps/ios/...`, are relative to the comet clone. Paths to OpenAgents code are
named as such.

## Summary

Zeron runs a small Rust engine on every device. The engine drives agent
command-line tools as subprocesses: Claude Code, Codex, Cursor, OpenCode, and
several Agent Client Protocol (ACP) agents. It stores each chat as a Loro
conflict-free replicated data type (CRDT) document on that device.

- **Local and synced modes.** A new installation is local-only. Signing in to
  WorkOS selects a synced profile on the next start. The engine then syncs
  documents through Cloudflare Durable Objects written in TypeScript
  (`edge/`) and hosts a relay room that other devices use to control it
  (`ARCHITECTURE.md` sections 1–2).
- **Desktop.** The desktop app is one binary, `zeron`, built on GPUI from Zed.
  It runs headed, with the engine in-process and also served on a localhost
  port, or headless as a daemon (`zeron headless`, `zeron daemon`).
- **Phone.** The iOS app is a UIKit shell over a Rust core. `crates/mobile`
  exports that core through UniFFI 0.32. The phone runs no agent. It mirrors
  the workspace registry, joins per-chat rooms, and drives remote engines by
  appending durable commands to the chat document.

The iOS app was rewritten from SwiftUI to "Rust decides what to paint and
where; Swift paints, scrolls and handles gestures" in PR #570 on 2026-09-26
(`docs/mobile-rewrite.md`), two days before this review. The rewrite is
therefore new. `docs/mobile-polish.md` still describes the earlier
`UITableView` and `UIHostingConfiguration` app, and some other docs describe
the desktop app rather than iOS.

### Components

| Layer | Zeron crate or directory | Role |
| --- | --- | --- |
| Wire types and view derivations | `crates/proto` | `AgentEvent`, `ToolCall`, requests, and pure sort and staleness rules both frontends share (`proto/src/view.rs`) |
| Documents | `crates/doc` | Session document schema, command ledger, continuations, and the workspace registry document |
| Sync | `crates/sync` | Room clients for chats and the registry, outbox, and the SQLite store `DocsStore` |
| Agent adapters | `crates/harness` | The `Harness` trait and the Claude, Codex, Cursor, OpenCode, ACP, and mock drivers |
| Engine | `crates/engine` | Sessions, run journal, document host and command executor, repositories, terminals, and agent accounts |
| Remote procedure calls | `crates/rpc` | Typed RPC over WebSocket or in-memory, and virtual sockets over the device relay room |
| MCP server | `crates/mcp` | `zeron mcp`, which gives each agent tools to create and drive other chats |
| Viewer device | `crates/client` | An engine-free peer that the phone uses: rooms, relay RPCs, view models, and an offline demo host |
| Text layout | `crates/text`, `crates/markdown` | Analytic text measurement, and block markdown with incremental reparsing |
| Mobile core | `crates/mobile` | UniFFI facade and the transcript layout engine |
| Edge | `edge/` (TypeScript) | Worker, chat, device, and registry Durable Objects, R2 attachments, APNs push, and WorkOS auth |

## Comparison with OpenAgents

| Concern | Zeron | OpenAgents today |
| --- | --- | --- |
| Phone transcript rendering | Rust lays out every row at the viewport width with its own text engine. Swift draws `CTLine`s at Rust coordinates in a custom `UIScrollView`. | Rust builds a whole `rust-native.view.v2` JSON tree. Swift decodes it, diffs nodes by key, and hosts SwiftUI in `UICollectionView` cells with estimated heights and self-sizing `UITextView`s (OpenAgents `bins/coder-ios/host/App/NativeChat.swift`, lines 297–440 and 960–1030). |
| Update path to the phone | A Rust subscription feeds a layout thread, which publishes an immutable `LayoutFrame` and calls `frame_ready(revision)`. | Swift sends JSON requests to a C ABI (`openagents_mobile_call`) and receives full JSON packets. The conversation reader polls the history observer (OpenAgents `crates/openagents-mobile/src/conversation.rs`). |
| Markdown | `IncrementalParser` reparses only from the last top-level block and mends half-streamed inline markers for display. | `rust_native::markdown::parse` reparses the whole document. |
| Transcript size | Tested with 3,300 rows. | Capped at 240 rows. Messages are truncated at 6,000 bytes and tools at 1,500 bytes (`conversation.rs`, `MAX_ROWS`, `MESSAGE_BYTES`, `TOOL_BYTES`). |
| Phone control | Send, steer, queue, interrupt, and answering questions are all durable commands. | Create a task (NIP-HOST `task.create`) and cancel it. Follow-ups and steering are not built (OpenAgents `crates/openagents-mobile/src/coder_tab.rs`). NIP-CTRL and NIP-SESS specify steering as target contracts. |
| Agent engines | Drives vendor CLIs and servers as full agents, with native steering where the vendor offers it. | Microcoder runs its own loop and calls `claude --print` once per step as a model call (OpenAgents `crates/microcoder/src/claude.rs`). |
| Provider capacity | Probes usage windows per account for display. Doesn't fail over. | Doesn't record capacity. Issue #9831 asks for capacity-aware routing and failover. |
| Device trust | Any device signed in to the same WorkOS user gets the whole engine RPC surface. There is no pairing and no end-to-end encryption. | Keys per device, host-issued scoped grants with revocation (NIP-HOST), and authenticated direct channels (NIP-REACH). |
| Push | The edge compares session status rows and sends APNs alerts that include the chat title. | A push gateway and NIP-PL lease exist (OpenAgents `crates/coder-mobile/src/push.rs`, `crates/push-gateway`). |

## What to consider adapting

The items are in priority order. Effort is S (days), M (one to two weeks), or
L (several weeks).

### 1. Rust-measured transcript with display lists painted natively

**What Zeron does.**

- **Rows.** `TranscriptView` (`crates/mobile/src/layout/mod.rs`, lines
  142–250) owns one worker thread per transcript. The worker turns session
  entries into rows: one per top-level markdown block, user message, or tool
  group.
- **Measurement.** The worker measures each row with `zeron-text` at the
  viewport width and publishes an immutable `LayoutFrame` with exact heights
  and cumulative offsets. `rows_in(y0, y1)` is a `partition_point` binary
  search.
- **Display lists.** `display(i)` returns a `RowDisplay`
  (`layout/display.rs`) that holds:
  - text runs with UTF-16 ranges, a style ID, and an exact `x` and baseline;
  - boxes for code, quotes, tables, and bubbles;
  - link hit rectangles and horizontal scrollers;
  - fourteen kinds of native widget, such as copy, disclosure, and spinner.
- **Painting.** Swift's `RowModel` builds one `CTLine` per run and draws it at
  the Rust position (`apps/ios/Zeron/Transcript/RowModel.swift`).
  `TranscriptListView` is a plain `UIScrollView` that queries rows with a
  700 pt overscan. It reuses row views from a pool and caches models by key,
  version, and width, up to 500 entries. It builds the next model for the
  streaming row off the main thread while the old model keeps painting
  (`TranscriptListView.swift`, lines 349–464).

**Why it's good.**

- Heights are exact before a row appears, so there's no estimated-height
  correction, no self-sizing pass, and no jump when a cell is measured.
- A streamed token re-measures one row. The doc reports 0.19 ms per token,
  30 ms for a cold layout of 3,300 rows, and 0 hitches while flinging and
  streaming on the simulator (`docs/mobile-rewrite.md`). These are simulator
  numbers from the authors, not device measurements.
- Zeron abandoned the design we use now. The SwiftUI app hosted SwiftUI in
  every cell, self-sized rows, and rebuilt every row per token
  (`docs/mobile-rewrite.md`, "Why"). Our `NativeTranscript` is that same
  design: `UIHostingConfiguration` cells, `.estimated(80)` heights, and a
  `UITextView` measured in `sizeThatFits`.

**How it maps.**

- `crates/rust-native` gains an optional layout layer beside the semantic
  contract.
- `crates/openagents-mobile` and `crates/coder-mobile` host one
  `TranscriptView`-like engine per open chat.
- `bins/openagents-ios/host` and `bins/coder-ios/host` replace the
  `NativeTranscript` cell tree with a CoreText painter.

**Adaptation sketch.**

1. Keep `Transcript`, `Message`, `Markdown`, and `Tool` as the semantic
   contract that the application produces and validates.
2. Add a Rust layout stage that consumes those nodes, plus a viewport width
   and a text scale, and produces frames: row keys, versions, heights,
   offsets, and display lists. The stage can live in a new
   `rust-native-layout` crate or in `crates/rust-native/src/layout`.
3. Let the adapter pull row displays by index, never the whole tree. Rows
   then don't cross the FFI boundary on every revision, and the 512 KiB and
   1,024-node view bounds stop constraining transcript length.
4. Stage the work. Start with measurement from system fonts through a
   platform callback. Adopt a bundled font and in-Rust shaping (item 2) only
   when accuracy tests pass.

**Effort.** L.

**Conflicts with our rules.** None. This moves more decisions into Rust,
which matches `AGENTS.md`. `crates/rust-native/docs/spec.md` currently says
the core "does not mount" views and that "adapters lay out the blocks", so
this is a spec change. Record the new boundary in the spec and in
`docs/coder/rust-native/architecture.md`. Keep accessibility in the semantic
tree: Zeron exposes links as accessibility elements, but a painted transcript
isn't accessible by default. The spec's "Drawing surfaces" section already
warns about this.

### 2. Analytic text engine with a CoreText ground-truth test

**What Zeron does.**

- **Engine.** `crates/text` ports pretext's split between `prepare` and
  `layout`:
  - ICU4X line-break opportunities (UAX #14) with Apple's tailoring;
  - rustybuzz shaping on the bundled Geist faces;
  - a width cache keyed by style and segment;
  - greedy line fitting in `f64` with CoreText's 0.0002 pt slack.
  Re-layout at a new width is arithmetic only (`crates/text/src/lib.rs`,
  lines 1–75).
- **Fallback glyphs.** Glyphs the bundled faces don't cover go to a
  `PlatformMeasurer` callback that Swift implements with CoreText
  (`layout/mod.rs`, lines 36–74; `apps/ios/Zeron/Core/Fonts.swift`).
- **Fonts.** Swift registers the same font bytes it hands to Rust.
- **Accuracy tests.** `crates/text/tests/coretext.rs` (a macOS
  `cargo test`) and `ZeronTests/LineBreakAccuracyTests.swift` compare Rust
  line starts with `CTFramesetter` across three faces, three sizes, and many
  widths. The library doc claims 100% of 48,000 cases.

**Why it's good.** Measurement and painting can't disagree about line breaks,
because the platform draws the lines Rust chose. The ground-truth test makes
that claim checkable. It also runs from `cargo test` on a Mac without Xcode.

**How it maps.**

- The engine is a new crate, such as `crates/rust-native-text`, used by
  item 1.
- Android (`bins/coder-android/host`) can implement the same fallback
  callback with its text APIs.

**Adaptation sketch.**

1. Reimplement the prepare and layout split. Don't copy it.
2. Bundle one sans and one mono face in the iOS hosts.
3. Add the `CTFramesetter` comparison test before shipping, and gate on it
   the same way Zeron does: zero line-count difference and at least 99.9%
   exact matches.

**Effort.** L. This is the riskiest item. Zeron's text crate is about 3,700
lines with extensive fidelity tests.

**Conflicts with our rules.** None known. Check the licenses of the fonts you
bundle. The ICU4X data size adds to the app binary.

### 3. Incremental markdown with display-only mending

**What Zeron does.**

- **Incremental reparsing.** `IncrementalParser`
  (`crates/markdown/src/parser.rs`, lines 1–12 and 611–628) keeps the block
  tree and reparses only from the start of the last top-level block. A source
  that contains a link-reference definition falls back to full parses.
  Parity tests stream corpora through both paths and assert that the results
  are equal.
- **Mending.** `mend.rs` appends synthetic closers to the display copy of the
  streaming tail only: `**a` becomes `**a**`, and `[text](partial` becomes a
  link with a placeholder URL. Styling therefore doesn't flip and reflow when
  the closing marker arrives. The canonical tree stays exact.
- **Stable prefix.** The parser reports `stable_prefix_blocks` so render
  caches for unchanged blocks stay valid.

**Why it's good.** Per-token work becomes proportional to the tail instead
of the whole message. Mending removes a visible flicker that any streaming
markdown renderer has.

**How it maps.** Extend `crates/rust-native/src/markdown.rs`, which uses
`pulldown-cmark` just as Zeron does, with an incremental parser and a mend
pass. The parser is useful without item 1: it cuts the work that
`openagents-mobile` does per poll or update.

**Adaptation sketch.**

1. Add `IncrementalMarkdown { source, blocks, stable_prefix }` with
   `append(&str)` and `display_blocks()`.
2. Port the parity test design: streaming results must equal a full parse.
3. Keep links inert, as the spec requires.

**Effort.** S to M.

**Conflicts with our rules.** None.

### 4. A durable command ledger for phone control: send, steer, queue, interrupt, and answer

**What Zeron does.** Each chat document has a `commands` list
(`crates/doc/src/commands.rs`, lines 1–9 and 138–173):

- **Commands.** The kinds are `Run`, `Steer`, `Interrupt`, and
  `RespondInput`.
- **Writers.** Each device appends only its own entries. Only the host writes
  outcomes: `Applied`, `Rejected`, `Expired`, `Superseded`, or `Cancelled`.
- **Evaluation order.**
  1. An entry already in the processed ledger is skipped.
  2. An entry past its time to live (TTL, 24 hours) expires.
  3. A newer interrupt supersedes an older interrupt only.
  4. An interrupt based on a past turn is superseded.
- **Execution.** The host marks a command processed before it executes it
  (`crates/sync/src/store.rs`, `mark_processed`), so a crash can't run a
  command twice.
- **Offline sends.** Sends made while offline wait in a durable outbox and
  replay with the same batch IDs.
- **Waking the host.** A "nudge" to the host's relay room wakes it
  (`crates/client/src/client.rs`, lines 243–275).
- **Queue.** The phone composer has one action button that morphs between
  Send, Queue or Steer, and Stop. A long-press menu offers "Queue for next
  turn", "Steer now", and "Stop and send"
  (`apps/ios/Zeron/Composer/ComposerBar.swift`, lines 578–637). A queue panel
  supports edit, reorder, and send-now, with a host edit lease renewed every
  20 seconds.

**Why it's good.** It's a small, tested set of rules that makes remote
control safe across restarts and poor connectivity. The evaluation order,
especially "interrupts supersede only interrupts" and "never supersede runs
or steers", avoids ambiguous cases.

**How it maps.** NIP-CTRL already defines `observe`, `steer`, and `cancel`
rights, and NIP-SESS defines `submit`, `queue`, `reorder`, `steer`, and
interrupt with an expected turn revision. What's missing is the
implementation for the phone:

- `crates/openagents-mobile/src/coder_tab.rs` says "follow-ups that continue
  one task are not built yet".
- The host side belongs in `crates/coder-host` and the `crates/coder` task
  owner.
- Transport is NIP-REACH direct channels or relay events, not a CRDT.

**Adaptation sketch.**

1. Define a host-side command journal per task, keyed by a device-minted
   command ID and the device key.
2. Apply Zeron's evaluation order as a pure function with a TTL and
   supersession tests.
3. Mark a command processed durably before dispatching it.
4. Expose a single composer action in the Rust view (send, queue or steer,
   or stop). Its mode comes from task state and the grant's rights, never
   from parsing the text.
5. Add a `Composer` extension to Rust Native for a queue or steer choice
   instead of the single `stop` intent.

**Effort.** M.

**Conflicts with our rules.** Zeron trusts `issued_by` as the client writes
it (see "What not to adopt"). Our version must check each command against the
sender's NIP-HOST or NIP-CTRL grant and the revocation epoch.
`INVARIANTS.md` auto-start rows still bound what a follow-up may run.

### 5. Steering semantics for each engine, made explicit

**What Zeron does.** The `Harness` trait (`crates/harness/src/lib.rs`, lines
82–168) declares `supports_steering()` and a
`SteeringMode::{StepBoundary, TurnBoundary}`. Each driver implements
steering differently.

| Engine | Steering implementation |
| --- | --- |
| Claude Code | Writes a user line to stdin with `priority: "now"` when no tool is open, or `"next"` while a tool runs, because `now` aborts in-flight MCP calls. A steer counts as delivered only when the CLI replays it (`--replay-user-messages`), which emits `AgentEvent::Steered` (`claude/wire.rs`, lines 226–230). |
| Codex | `turn/steer` with `expectedTurnId`. If the turn has ended, the steer becomes the next `turn/start` (`codex/mod.rs`, lines 1503–1510 and 1644). |
| ACP agents | Uses an advertised `_session/steering` extension, or cancels and re-prompts at a step boundary, or waits for the turn boundary. |

The engine keeps a ledger of routed steers. If a run dies before a steer is
confirmed, the engine re-dispatches the steer as a new turn
(`crates/engine/src/sessions.rs`, lines 667–756).

**Why it's good.** It records how each vendor surface actually behaves.
Examples are the `now` versus `next` MCP hazard and replay-based
acknowledgment. It separates "accepted" from "consumed".

**How it maps.** NIP-SESS already requires this distinction: "Starting
another turn, enqueueing, or killing/restarting a process is not native
steering. The host refuses unsupported steering unless the caller explicitly
chose the pinned emulated operation." Zeron's driver table is a ready list of
adapter capability rows for NIP-SESS, and it names the evidence each engine
gives for consumption. Add these facts to the NIP-SESS adapter
documentation. Use them when `crates/coder` gains Claude Code or Codex
session adapters.

**Adaptation sketch.** Add a steering capability to our engine adapter
description: native mid-turn, emulated by cancel and continue, or turn
boundary only. Add an acknowledgment source for each. Record the consumption
event in ATIF as a separate step.

**Effort.** S for the documentation and types. M for each engine adapter.

**Conflicts with our rules.** Zeron's emulated steering (cancel and
re-prompt) runs by default. NIP-SESS requires the caller to choose emulation
explicitly. Keep our rule.

### 6. Provider usage probes as capacity inputs for issue #9831

**What Zeron does.** `crates/engine/src/agent_accounts.rs` stores several
saved accounts per provider as credential slots and swaps them by rewriting
the CLI's credential store. It probes usage windows over HTTP and caches the
results with stale-while-revalidate, a 30-second minimum interval, and
`Retry-After`:

| Provider | Usage endpoint | Windows read |
| --- | --- | --- |
| Claude | `GET https://api.anthropic.com/api/oauth/usage` with header `anthropic-beta: oauth-2025-04-20` | `five_hour` and `seven_day`, each with `utilization` and `resets_at` |
| Codex | `GET https://chatgpt.com/backend-api/wham/usage` with header `chatgpt-account-id` | Primary and secondary windows with `used_percent` and `limit_window_seconds` |

The Claude normalizer turns a `rate_limit_event` with status `rejected` into
an error that names the window (`claude/normalize.rs`, lines 536–548).
Zeron doesn't fail over. Switching accounts is manual and affects only new
sessions (`crates/ui/src/settings/accounts.rs`, line 2205).

**Why it's good.** It shows where a host can learn a provider's remaining
capacity and reset time before a run starts, not only after a 429 response.

**How it maps.** Issue #9831 asks for durable capacity state, routing among
admitted providers with capacity, failover during a run, and a `no_capacity`
ending. Zeron covers only the first part: knowing capacity ahead of time.

- Capacity state belongs in `crates/capability` or the delegation surface
  (`docs/coder/runtime/delegate.md`).
- Recording a 429 with `resets_at` belongs in `crates/microcoder`.
- Routing belongs in the auto-start policy.

**Adaptation sketch.**

1. Model `ProviderCapacity { provider, window, used_fraction, resets_at,
   source: probe | refusal, observed_at }`.
2. Fill it from 429 refusals first. That part is required and has no
   undocumented dependency.
3. Optionally add a probe. Both endpoints are private and undocumented, and
   the Claude one needs a beta header, so treat a probe result as advisory
   and expect it to break.
4. Route with a typed policy over admitted models, never by matching strings
   in error text. Match on structured fields such as the error code and
   `resets_at`.

**Effort.** M. The probe is S on top of that.

**Conflicts with our rules.**

- The probes need OAuth tokens. Microcoder's documentation says it "never
  reads the credential file". A probe changes that boundary, so it needs an
  explicit decision and an `INVARIANTS.md` entry.
- Swapping credential slots rewrites another tool's credential store and
  Keychain item. Don't adopt that part.

### 7. Push rules computed from status transitions, with a minimal payload

**What Zeron does.**

- **Trigger rules.** The edge compares each session row before and after a
  write (`edge/src/push-notify.ts`, lines 46–69). It sends **failed** when a
  row becomes `errored`, **input** when it becomes `awaitingInput`, and
  **done** when `lastCompletedTurn` changes and the row is less than
  45 seconds old.
  - The first time a row appears, it only sets the baseline.
  - Side chats and archived chats never notify.
  - Each device keeps its own `done`, `input`, and `failed` preferences.
- **Client behavior.** The app asks for permission only after the user starts
  the first session from the phone. It shows nothing while the app is in the
  foreground. Tapping a notification opens the chat by `chatId`
  (`apps/ios/Zeron/App/PushNotifications.swift`).

**Why it's good.** Three meaningful events, a staleness guard, a baseline on
first sight, and permission asked in context. These are the product rules
that decide whether push is useful or noisy.

**How it maps.** OpenAgents `crates/coder-mobile/src/push.rs` and
`crates/push-gateway` already provide the delivery path through NIP-PL
leases. The missing piece is the rule for when the host wakes the phone.
Derive it on the host from signed activity summary phase transitions, such
as `nostr::activity_summary::Phase` in OpenAgents
`crates/openagents-mobile/src/coder_tab.rs`.

**Adaptation sketch.** A pure function `wake_for(previous, next, now) ->
Option<WakeKind>` with the same three kinds, the baseline rule, and the
staleness guard. Put the preferences in the device's grant or enrollment
record.

**Effort.** S to M.

**Conflicts with our rules.** Zeron puts the chat title in the APNs alert,
so Apple sees it. Our payload should carry only an opaque wake. The phone
then fetches the summary over its authenticated channel and posts a local
notification.

### 8. Follow-the-tail, anchor, and runway scrolling rules

**What Zeron does.** `TranscriptListView.swift` defines these rules:

- **Follow state.** Following starts on. Any user pan releases it. Releasing
  within 70 pt of the bottom with a low downward velocity re-engages it, and
  so does momentum that carries the list into that band.
- **Spring.** The follow spring is critically damped,
  `delta * (1 - exp(-dt*16))` with `dt` capped at 1/30 second, and runs on
  `CADisplayLink`.
- **Anchor.** When the reader isn't following, the view keeps the top row's
  key and offset. After a new frame it shifts `bounds.origin` instead of
  `contentOffset`, so momentum survives (lines 290–330).
- **Runway.** After your own send, the view reserves a runway that holds your
  prompt at the top until the reply fills the screen (lines 51–140).

**Why it's good.** The rules are small and precise, and Zeron measured them.
Our spec already requires following while at the bottom, stopping on scroll
up, and offering a jump. Zeron adds the re-engage band, momentum-preserving
anchoring, and the send runway.

**How it maps.** OpenAgents `bins/coder-ios/host/App/NativeChat.swift`
(`NativeTranscriptCoordinator` and `NativeTranscriptView.pinToBottom`) and
the `Transcript` element in `crates/rust-native/docs/spec.md`.

**Adaptation sketch.** Add the re-engage band and the runway to the spec as
adapter behavior. Apply the `bounds.origin` shift in the current
`restore(anchor)`. This is useful before item 1 lands.

**Effort.** S.

**Conflicts with our rules.** None. This is adapter behavior, not state.

### 9. Frame coalescing and a single latest-frame pull

**What Zeron does.** The layout worker drains its channel with `try_iter()`
and acts only on the latest input (`layout/mod.rs`, `Worker::run`). The
client coalesces events to at most one burst every 16 ms
(`crates/client/src/events.rs`, line 21). Swift's `FrameRelay` schedules one
main-queue hop per burst and then pulls `engine.frame()`, which is always the
newest (`apps/ios/Zeron/Transcript/FrameRelay.swift`). The docs describe this
as per-display-frame coalescing. The code coalesces once per main-queue hop.

**Why it's good.** A streaming burst can't queue a backlog of stale renders
on the main thread.

**How it maps.** OpenAgents `bins/openagents-ios/host/App/MobileBridge.swift`
publishes every packet it receives. A Rust-side "latest revision wins" rule
and a Swift-side single pending hop give the same guarantee without item 1.

**Effort.** S.

**Conflicts with our rules.** None.

### 10. Performance lab, hitch meter, and scripted scroll benchmark

**What Zeron does.**

- **Launch arguments.** `-lab -turns N -autostream -bench -meter` open a
  transcript lab over fixture markdown with the real engine.
- **Hitch meter.** `HitchMeter` counts frames later than 1.5 times their
  budget.
- **Scroll benchmark.** `ScrollBench` flings at 5,200 pt/s while idle and
  while streaming, then writes `Documents/bench.json` with a hitch ratio in
  ms/s, the worst frame, and layout, apply, and build maxima from signposts.
- **Tests.** `ZeronUITests/ScrollPerformanceTests.swift` asserts a hitch
  ratio below 5 ms/s. `-measureopen` records how long a session takes to
  open. `-demo` runs an offline demo host in Rust.

**Why it's good.** It turns "streaming feels janky" into numbers that a test
can assert. Our phone work has no equivalent.

**How it maps.** Add these to the OpenAgents `bins/openagents-ios` and
`bins/coder-ios` hosts, with fixture transcripts produced by
`crates/openagents-mobile` (an existing `NativeFixture.swift` is a start).
Record the numbers under `docs/coder/measurements/`.

**Effort.** S to M.

**Conflicts with our rules.** None. The workspace has no GitHub workflows, so
run the tests manually.

### 11. An MCP tool surface that lets an agent drive other chats

**What Zeron does.** `zeron mcp` is a stdio MCP server that the engine injects
into every run (`docs/mcp.md`; `crates/mcp/src/tools.rs`, lines 61–197). Its
tools include `list_harnesses`, `list_models`, `create_chat`,
`send_message` (modes `auto`, `run`, `steer`, and `queue`), `wait_for_turn`,
`interrupt_chat`, and `respond_to_input`. Child chats record
`parent_chat_id`, and only one level of nesting is allowed. An agent can
therefore delegate to a different provider.

**Why it's good.** Cross-provider delegation takes a small tool surface, and
the one-level limit is simple to enforce.

**How it maps.** Delegation in `docs/coder/runtime/delegate.md` and
`crates/coder`. Our delegation must pass through the task owner, policy, and
cost accounting. Zeron's tools are a checklist of operations an agent
needs, not a design to copy.

**Effort.** M.

**Conflicts with our rules.** Tool selection must stay typed. Spend and
approvals must stay under POL and auto-start policy.

### 12. Crash recovery for runs

**What Zeron does.** Each chat has an append-only JSONL run journal. On
restart, the engine stamps a run that didn't end with `Done` as `aborted`,
closes it, and resumes it at most three times within 12 hours
(`crates/engine/src/sessions.rs`, lines 822–948). It has no stall timeout by
design. A turn-quiesce watchdog parks a silent turn after 120 seconds, and is
disabled for engines with a deterministic turn end.

**Why it's good.** The limits on automatic resumption are explicit, and the
reasons for having no stall timeout are written down.

**How it maps.** The OpenAgents `crates/coder` task owner and ATIF logs
(`crates/atif`). Compare the rule with our host auto-start and task recovery
behavior before copying the numbers.

**Effort.** S to M.

**Conflicts with our rules.** An automatic resume must not relax auto-start
spending or admission rules.

## What not to adopt

| Zeron choice | Why not |
| --- | --- |
| Same-account trust: every device signed in to one WorkOS user can call the engine's whole RPC surface, including terminals, file writes, account login, and updates (`crates/engine/src/lib.rs`, lines 433–451; `README.md`) | Conflicts with NIP-HOST scoped grants, revocation epochs, and per-operation checks. Our phone must stay a device with limited rights. |
| No end-to-end encryption (`docs/PARITY.md`, lines 101–102): the edge sees every document row, checkpoint, relay payload, and plaintext R2 backup | Our relay model uses NIP-44 encryption and authenticated direct channels (NIP-REACH). A central relay that can read everything is a regression. |
| Loro CRDT documents synced through Cloudflare Durable Objects, and a TypeScript edge | We don't add TypeScript, and our protocols are Nostr NIPs. The command ledger rules (item 4) carry over without a CRDT. |
| Commands trusted by their `issued_by` field; the host doesn't verify the issuer (`crates/doc/src/commands.rs`) | Every command must be signed by a device key and checked against a grant. |
| `POST /chat2/{id}/reset` deletes the room's `owner` key, which appears to let the next joiner claim the room (`edge/src/chat-room.ts`, lines 317–332) | We didn't verify this, but it shows the risk of owner-on-first-join rooms. Keep host-issued identity. |
| Auto-approving every tool call: Claude runs with its permission prompt answered "allow" (and `--dangerously-skip-permissions` when auto-approve is set); Codex runs with `DangerFullAccess` and approval policy `never` | Conflicts with POL exact approvals and our sandbox policy. |
| Swapping credential slots by rewriting `~/.claude/.credentials.json`, Keychain items, and `$CODEX_HOME/auth.json` | Changes another tool's state out of band. Account choice should be an explicit, reversible configuration of each engine process. |
| The chat title in APNs alerts | Discloses content to Apple. Use an opaque wake. |
| GPUI desktop and bundled Geist visual design | Our desktop direction and styling are separate decisions. Zeron's `ARCHITECTURE.md` notes that it avoids Zed's GPL crates, so reusing Zed code beyond GPUI needs the same care. |
| Driving the Cursor SDK through a pinned Node shim (`crates/harness/src/cursor`) | Adds a Node runtime to a product path. Revisit only if Cursor ships a stable non-JavaScript protocol. |

## Open questions

1. **Scope of the layout rewrite.** Should item 1 replace `NativeTranscript`
   in both iOS hosts at once, or start in the OpenAgents app, where the
   conversation reader is newest? Android would reuse the Rust layout
   engine, so decide the FFI boundary once. Zeron uses UniFFI objects and
   callbacks. We use a JSON C ABI.
2. **Fonts.** Do we accept bundling fonts and measuring in Rust, or do we
   first try system fonts with a CoreText measurement callback for every
   segment and measure the cost?
3. **Accessibility.** How does a painted transcript meet the Rust Native
   accessibility requirements? Zeron offers "Select Text" in a separate
   `UITextView` and doesn't support selection in place. Is that enough?
4. **Engine adapters.** Should `crates/coder` drive Claude Code and Codex as
   full session engines, as Zeron does, for NIP-SESS? Or should Microcoder
   stay the only engine, with vendor CLIs used only as model calls? This
   decides whether items 5 and 11 apply.
5. **Capacity probes.** Is reading an OAuth token to call an undocumented
   usage endpoint acceptable for #9831, or should capacity come only from
   refusals and documented rate-limit headers?
6. **Transcript bounds.** With exact layout and paged rows, do the 240-row
   and 6,000-byte limits in `conversation.rs` still need to exist, or only as
   bounds on network reads?
7. **Doc drift in Zeron.** `docs/chat2-sync.md` says "PLANNED" but is mostly
   built. `docs/mobile-polish.md` describes the removed SwiftUI app.
   `docs/research/acp.md` says Claude and Codex moved to ACP, but the code
   reversed that. Before porting from a Zeron doc, check the code.
