# OpenAgents Desktop chat runtime reference

Date: 2026-07-21. Home: `docs/desktop/` (the established home for OpenAgents
Desktop architecture and cut receipts). Scope: `apps/openagents-desktop/src/`
plus the shared packages `packages/agent-turn-runtime`,
`packages/agent-runtime-schema`, and `packages/agent-harness-contract`.

This document is a MAP and reference for the custom chat and agent runtime that
OpenAgents Desktop runs. Read it to learn where each part lives and how the
parts connect. Every claim cites `path:line`. Paths are relative to the repo
root `/Users/christopherdavid/work/openagents`.

## Reading order

1. Read "Top-level architecture" for the main and renderer split and the
   Runtime Gateway.
2. Read "The two execution stacks" for the single most important idea. There
   are two turn engines. They meet at one function.
3. Use the "File and responsibility index" as the jump table.
4. Read a per-subsystem section when you touch that area.
5. Read "Sharp edges" before you change streaming, history, or the two stacks.

## Version note

This reference describes the on-disk code at HEAD `d4aa807106`. The
delegate-streaming and delegate-history change described below as "not in HEAD"
has since LANDED on `main` as commit `91de284512` (stream the delegate answer
live plus pass conversation history). Where the text marks that change as "not
in HEAD", read it as merged. The renderer promotion of a single delegate answer
to the primary assistant bubble is tracked by issue #9127.

## Companions

- [`../fable/2026-07-21-ai-sdk-and-effect-ai-streaming-harvest-audit.md`](../fable/2026-07-21-ai-sdk-and-effect-ai-streaming-harvest-audit.md)
  — the ranked STREAM-01..07 harvest plan for the live-to-UI streaming path
  this runtime lacks.
- [`../fable/2026-07-20-ai-sdk-harness-abstraction-harvest-analysis.md`](../fable/2026-07-20-ai-sdk-harness-abstraction-harvest-analysis.md)
  — the harness-layer harvest that produced `packages/agent-harness-contract`
  (HARN epic #9115).

---

## Architecture diagram

```
                          RENDERER (Electron webContents, React)
        renderer/react-timeline.tsx  ·  renderer/runtime-conversation.ts  ·  renderer/boot.ts
                 |  builds notes from events, projects records, memoized re-render
                 |  NEVER injects synthetic history. It only sends the new message.
   ======================================= IPC (preload.cts) =======================================
   invoke: claude-local:start / codex-local:start / apple-fm:startTurn / turn:submit / runtime-gateway/invoke
   on:     claude-local:event / codex-local:event / turn:event / usage-ledger:event / runtime-gateway/event
   ==================================================================================================
                          MAIN PROCESS (owns ALL runtime execution + history authority)
   +--------------------------------------------------------------------------------------------+
   |  Runtime Gateway (runtime-gateway.ts)  ---- durable Khala Sync conversation authority       |
   |     one typed request/response/event contract (runtime-gateway-contract.ts, protocol v12)   |
   +--------------------------------------------------------------------------------------------+
   |                                                                                            |
   |  STACK A: Provider Lane SPI (production coding turns)     STACK B: AFS turn kernel          |
   |  provider-lane.ts  makeProviderLaneDispatcher            packages/agent-turn-runtime        |
   |     admit -> journal.accept -> history assemble ->          TurnService (UI-neutral)        |
   |     runTurn(emit) -> project -> sender.send(eventChannel)   ProviderRegistry / TurnPolicy   |
   |                                                             ProviderStreamEvent.Chain       |
   |    ProviderLane values:                                     |                               |
   |      codexLocalLane   (codex-local-runtime.ts)             turn/desktop-turn-main.ts        |
   |      claudeLocalLane  (claude-local-runtime.ts)             installDesktopTurnKernel        |
   |      grokAcpLane / cursorAcpLane (provider-lane-acp.ts)    Providers in the kernel:         |
   |                                                              apple_fm router (desktop-apple-fm-provider.ts)
   |                                     \\                        codex/claude/grok delegates    |
   |                                      \\                       (desktop-codex-provider.ts)    |
   |                                       \\                              |                      |
   |                                        \\   runDelegateLaneTurn(lane) THE MEETING POINT      |
   |                                         \\====== main.ts:1431 =======//                      |
   |                                          the delegate provider calls lane.runTurn(emit)      |
   |                                          and redacts each ClaudeLocalEvent into a            |
   |                                          SafeMessageChainEntry chain for the kernel          |
   |                                                                                            |
   |  DURABLE STATE (main-owned, atomic 0600 writes)          NEUTRAL OBSERVER                   |
   |    thread-store.ts       (bounded composer cache)          harness-event-recorder.ts        |
   |    local-turn-journal.ts (exactly-once accept)             harness-projection.ts            |
   |    usage-ledger.ts       (per-session tokens)              -> KhalaRuntimeEvent log         |
   |    full-auto-*.ts        (multi-turn run graph consumer)                                    |
   +--------------------------------------------------------------------------------------------+
```

---

## File and responsibility index

### Runtime gateway and IPC

| File | Responsibility |
| --- | --- |
| `apps/openagents-desktop/src/runtime-gateway.ts` | The durable conversation gateway. Turns typed requests into service calls and publishes lifecycle plus Sync live updates. |
| `apps/openagents-desktop/src/runtime-gateway-contract.ts` | Frozen Effect Schema request, response, and event union. Protocol version 12. Channel names. |
| `apps/openagents-desktop/src/preload.cts` | The preload bridge. Maps main channels to renderer `bridge.*` calls. |

### Stack A — Provider Lane SPI (production coding turns)

| File | Responsibility |
| --- | --- |
| `apps/openagents-desktop/src/provider-lane.ts` | The `ProviderLane<Context>` interface and the shared `makeProviderLaneDispatcher` engine. |
| `apps/openagents-desktop/src/provider-lane-acp.ts` | Maps `AcpProjectionEvent` to `ClaudeLocalEvent` and builds the ACP `ProviderLane` for Grok and Cursor. |
| `apps/openagents-desktop/src/provider-lane-capabilities.ts` | L2 capability truth. Quarantines an over-claiming lane. Owns the `recovery` field type. |
| `apps/openagents-desktop/src/provider-lane-registry.ts` | Per-thread lane selection and typed switch refusals. |
| `apps/openagents-desktop/src/claude-local-runtime.ts` | The claude-local lane runtime over the Anthropic Agent SDK `query`. |
| `apps/openagents-desktop/src/claude-local-contract.ts` | The frozen `ClaudeLocalEvent` envelope. The renderer-facing vocabulary. |
| `apps/openagents-desktop/src/claude-history.ts` | Read-only projection of the on-disk Claude Code history tree. |
| `apps/openagents-desktop/src/codex-local-runtime.ts` | The codex-local lane runtime. Account discovery, exec and app-server paths, JSONL to envelope. |
| `apps/openagents-desktop/src/codex-app-server-supervisor.ts` | Pooled, reconnecting supervision of one `codex app-server` process per identity. |
| `apps/openagents-desktop/src/codex-app-server-client.ts` | Newline JSON-RPC client over the app-server child process. |
| `apps/openagents-desktop/src/codex-app-server-turn.ts` | Runs one turn through an app-server lease. Projects notifications to envelope events. |
| `apps/openagents-desktop/src/local-turn-recovery.ts` | `reconcileLocalTurns`. Restart recovery honoring the recovery mode. |
| `packages/grok-harness/src/*` | The Grok ACP lane over stdio JSON-RPC to the `grok` CLI. |
| `packages/cursor-agent-runtime/src/cursor-peer-runtime.ts` | The Cursor ACP lane over stdio to the `cursor-agent` CLI. |

### Stack B — AFS turn kernel plus Apple FM router and delegation

| File | Responsibility |
| --- | --- |
| `packages/agent-turn-runtime/src/turn-service.ts` | `TurnService`. The UI-neutral turn lifecycle kernel. |
| `packages/agent-turn-runtime/src/ports.ts` | The injected ports. `ProviderRegistry`, `TurnPolicy`, `ProviderStreamEvent`, and more. |
| `packages/agent-turn-runtime/src/projection.ts` | `deriveSafeProjection` and receipt derivation. The `SafeTurnProjection` card. |
| `packages/agent-turn-runtime/src/turn-state.ts` | The pure, total turn state machine. |
| `packages/agent-turn-runtime/src/event-gateway.ts` | The generation-fenced, bounded provider event gateway. |
| `apps/openagents-desktop/src/turn/desktop-turn-main.ts` | `installDesktopTurnKernel`. Composes the kernel over Desktop adapters. Owns `turn:submit`. |
| `apps/openagents-desktop/src/turn/desktop-apple-fm-provider.ts` | The Apple FM router provider. Guided generation over the on-device model. |
| `apps/openagents-desktop/src/turn/desktop-codex-provider.ts` | `makeDelegateProviderRegistry`. The redaction boundary. `ProviderStreamEvent.Chain`. |
| `apps/openagents-desktop/src/turn/desktop-delegation.ts` | `decideDelegation`. The host answer, delegate, or refuse decision. |
| `apps/openagents-desktop/src/turn/desktop-turn-policy.ts` | Context source, first-candidate policy, and artifact resolver layers. |
| `apps/openagents-desktop/src/turn/apple-fm-prompt.ts` | The host-owned router prompt with available agents and ambient context. |
| `packages/agent-runtime-schema/src/route.ts` | The frozen `RouteRecommendation` versus `RouteDecision` split. |
| `packages/apple-fm-runtime/src/recommendation.ts` | `decodeAppleFmRouteOutput`. Fail-closed decode of the model route output. |

### Event vocabularies and neutral log

| File | Responsibility |
| --- | --- |
| `apps/openagents-desktop/src/claude-local-contract.ts` | `ClaudeLocalEvent`. The desktop renderer surface. About 23 kinds. |
| `packages/agent-runtime-schema/src/index.ts` | `KhalaRuntimeEvent` neutral union and `RuntimeInteraction`. |
| `packages/agent-runtime-schema/src/presentation.ts` | `SafeTurnProjection` and `SafeMessageChainEntry`. |
| `packages/agent-client-runtime-bridge/src/projection.ts` | `AcpProjectionEvent` and the `AcpRuntimeProjector` bridge. |
| `apps/openagents-desktop/src/harness-projection.ts` | Projects `ClaudeLocalEvent` to the neutral `KhalaRuntimeEvent` stream. |
| `apps/openagents-desktop/src/harness-event-recorder.ts` | The durable cursor-exact neutral log per turn. Observer only. |
| `apps/openagents-desktop/src/harness-readiness-source.ts` | The one unified harness readiness projection. |

### State, durability, and renderer

| File | Responsibility |
| --- | --- |
| `apps/openagents-desktop/src/thread-store.ts` | The bounded composer cache. `open`, `upsert`, `append`, `maxNotes`. |
| `apps/openagents-desktop/src/local-turn-journal.ts` | The exactly-once local-turn journal and recovery record. |
| `apps/openagents-desktop/src/turn/desktop-turn-journal.ts` | The AFS kernel `TurnJournal` adapter. Mirrors legacy terminal state. |
| `apps/openagents-desktop/src/usage-ledger.ts` | Per-session token accounting per provider and account. |
| `apps/openagents-desktop/src/full-auto-lane.ts` | Full Auto lane policies and the Full-Auto prompt. |
| `apps/openagents-desktop/src/full-auto-run-registry.ts` | The durable multi-turn run graph and per-thread lease. |
| `apps/openagents-desktop/src/renderer/react-timeline.tsx` | The renderer timeline, the Worked group, and the Delegated Agents card. |
| `apps/openagents-desktop/src/renderer/runtime-conversation.ts` | Builds notes from events. Assistant text appends into one note by `messageRef`. |
| `packages/ui/src/workbench/agent-group.tsx` | The `DesktopAgentGroup` and `DesktopAgentRow` presentation. |

### Neutral harness substrate (the convergence target)

| File | Responsibility |
| --- | --- |
| `packages/agent-harness-contract/src/adapter.ts` | The `AgentHarness` adapter spec. One `start` entry. |
| `packages/agent-harness-contract/src/session.ts` | `HarnessSession` verbs and `HarnessPromptControl`. |
| `packages/agent-harness-contract/src/stream.ts` | `HarnessStreamEvent` equals `KhalaRuntimeEvent`. The cursor. |
| `packages/agent-harness-contract/src/event-log.ts` | The durable cursor-exact event log runtime. |
| `packages/agent-harness-contract/src/event-log-store.ts` | The persistence port. Append rejects a non-increasing sequence. |
| `packages/agent-harness-contract/src/slice-runner.ts` | The intra-turn slice runner. Suspend and resume at an exact cursor. |
| `packages/agent-harness-contract/src/readiness.ts` | `projectHarnessReadiness`. Candidates, admitted subset, capacity refs. |
| `packages/agent-harness-contract/src/sandbox.ts` | The fail-closed sandbox session and provider ports. |
| `packages/agent-harness-contract/src/acp-adapter.ts` | Generic ACP peer to `AgentHarness`. |
| `packages/agent-harness-contract/src/opencode-adapter.ts` | opencode events to the neutral stream. |

---

## 1. Top-level architecture

### Main owns execution. The renderer never injects history.

The Electron main process owns all runtime execution and all history authority.
The renderer sends only the new user message. It never supplies prior turns.
The dispatcher makes this explicit. History comes from the main-owned thread
store, and the just-appended user note is excluded from that history
(`provider-lane.ts:352-357`). The comment on those lines states the rule
directly. The AFS kernel enforces the same rule through the `ThreadRepository`
port, which the renderer cannot forge (`packages/agent-turn-runtime/src/ports.ts:160-178`).

### The Runtime Gateway

The Runtime Gateway is the durable conversation authority. It is not the turn
engine. It fronts the Khala Sync conversation state, the confirmed agent
timeline, Codex history, runtime interactions, session sign-in, and voice.

- The contract is a frozen Effect Schema union at `runtime-gateway-contract.ts`.
  The request union starts at `runtime-gateway-contract.ts:171`. The response
  union starts at `:326`. The event union starts at `:507`. The protocol version
  is 12 (`:71`).
- The two IPC channels are `openagents-desktop/runtime-gateway/invoke` and
  `openagents-desktop/runtime-gateway/event` (`:69-70`).
- `createDesktopRuntimeGateway` builds the gateway from injected service
  factories (`runtime-gateway.ts:178-209`). Each request kind routes to one
  service. Examples include `conversation.catalog`, `conversation.thread`,
  `agent.timeline`, `runtime.interactions`, and the command family
  `conversation.start` / `interrupt` / `continue` / `retry` / `close`
  (`runtime-gateway.ts:244-602`).
- The gateway publishes only lifecycle events and Khala Sync live updates
  (`runtime-gateway.ts:219-231`, contract `:507-517`). It does NOT carry the
  streamed `ClaudeLocalEvent` turn envelope. That envelope rides its own
  dedicated channels. See section 6.

The gateway is the authority for durable, confirmed, cross-device state. The
Provider Lane SPI and the AFS kernel are the authorities for live turn
execution. Keep these two roles separate in your mind.

---

## 2. The two execution stacks

This is the central idea of the whole runtime. There are two turn engines.

### Stack A — Provider Lane SPI (production coding turns)

`makeProviderLaneDispatcher` is the one shared engine for a normal coding chat
turn (`provider-lane.ts:305-531`). Each agent lane is a plain `ProviderLane`
value (`provider-lane.ts:149-209`). The lanes are `codexLocalLane`,
`claudeLocalLane`, and the ACP lanes `grokAcpLane` and `cursorAcpLane`, all
constructed in `main.ts`.

The dispatcher owns the plumbing that the lanes used to duplicate:

- content admission (`provider-lane.ts:317`),
- capability admission and quarantine (`:320-323`),
- typed request admission via `lane.admit` (`:324`),
- thread existence (`:328`),
- the exactly-once journal accept (`:339-346`),
- host-owned history assembly (`:352-357`),
- pre-turn and post-turn git checkpoints (`:394` and `:523`),
- streaming text persistence (`:368-373`),
- the live agent graph fold (`:386-401`),
- the exact usage ledger record (`:409-425`),
- shared tool-trace and effective-model notes (`:429-455`),
- the `onTurnEventProjected` observer hook (`:459`),
- and the renderer forward via `sender.send(lane.eventChannel, ...)` (`:464`).

A lane contributes only lane-specific behavior through typed hooks. The lane
runs the provider and emits the frozen `ClaudeLocalEvent` envelope through the
`emit` it receives (`provider-lane.ts:141` and `:466-478`).

Entry point: the renderer invokes `openagents:claude-local:start` or the codex
start channel. `main.ts:4481-4489` decodes the request, selects the lane by
thread (`providerLaneRegistry.selection`), and calls
`laneDispatcher.dispatchTurn(lane, request, event.sender)`.

### Stack B — the AFS turn kernel

`TurnService` is the UI-neutral turn kernel in `packages/agent-turn-runtime`
(`turn-service.ts:89-98`). It owns turn policy application, a deterministic state
machine, generation fencing, and a safe projection. It owns no provider
credentials, no store driver, no UI, and no platform API. It composes six
injected ports (`ports.ts`): `ContextSource`, `TurnPolicy`, `ProviderRegistry`,
`TurnJournal`, `ThreadRepository`, and `ActionBroker`.

The provider vocabulary of this kernel is `ProviderStreamEvent`, a five-variant
tagged enum (`ports.ts:68-75`): `Progress`, `Chain`, `Completed`, `Refused`,
`Failed`. This is not a raw SDK event and not the renderer projection. It is a
minimal advisory signal. The `Chain` variant carries the latest full,
already-redacted `SafeMessageChainEntry` snapshot. A `Chain` event never advances
the lifecycle. It is a live-only advisory signal (`ports.ts:53-67`).

The kernel install is `installDesktopTurnKernel` (`turn/desktop-turn-main.ts:194-512`).
It is live by default and disabled by `OPENAGENTS_DESKTOP_AFS_TURN_KERNEL=0`
(`:181-182`). It registers the IPC handlers `turn:start`, `turn:cancel`,
`turn:status`, and the one-shot `turn:submit` (`:257-501`).

### When each stack is used

- A normal chat coding turn uses STACK A. The renderer sends
  `claude-local:start` or `codex-local:start`, and the dispatcher runs the lane
  directly. The AFS kernel is not involved.
- An Apple FM routed turn uses STACK B. The renderer sends `turn:submit`. The
  on-device Apple FM model runs a router turn. If the router recommends a
  delegate, the kernel starts a second kernel turn on the delegate provider.
- The direct on-device Apple FM chat turn uses `apple-fm:startTurn` and calls
  `appleFmHost.runTurn(prompt)` directly (`main.ts:3996-3999`). This is neither
  stack. It is a plain on-device completion.

### Where the two stacks meet

They meet at `runDelegateLaneTurn` (`main.ts:1431-1460`). The delegate providers
in the kernel are built with `runTurn: (input) => runDelegateLaneTurn(<lane>, input)`
(`main.ts:1533`, `:1544`, `:1549`). Inside `runDelegateLaneTurn`, the code calls
`lane.admit(request)` and then `lane.runTurn({ ... emit })` on a STACK A
`ProviderLane` (`main.ts:1448-1458`). So the delegate provider of the kernel
invokes the Provider Lane directly.

The lane emits raw `ClaudeLocalEvent`s. The delegate provider redacts each one
and lifts it into the kernel vocabulary as `ProviderStreamEvent.Chain` and
`Progress` (`turn/desktop-codex-provider.ts:320-364`). The redaction boundary is
`redactCodexEvent` (`:224-251`). So:

- STACK B (the kernel) is always the orchestrator for a routed turn. It runs the
  router turn and each delegate turn.
- STACK A (the Provider Lane) is the delegate execution engine. It runs the real
  CLI or ACP session and owns account, model, and history.
- `runDelegateLaneTurn` plus the delegate provider redacting `emit` closure is
  the exact seam. Raw command args, output, paths, and tokens stay on the device.
  Only the redacted `SafeMessageChainEntry` chain crosses into the kernel and to
  the renderer.

One important caveat at HEAD. `runDelegateLaneTurn` passes `history: []`
(`main.ts:1454`). A delegated subagent turn starts with no thread history at
HEAD. The #9118 change (not in HEAD) feeds real thread history. See "Sharp edges".

---

## 3. Event vocabularies and how they map

The runtime uses four event vocabularies. Learn which is the renderer surface
and which is the durable neutral one.

- `ClaudeLocalEvent` is the desktop renderer surface. It is the rich,
  desktop-specific superset. Defined at `claude-local-contract.ts:226-503`. It
  crosses the `openagents:claude-local:event` and `openagents:codex-local:event`
  channels. Every Provider Lane emits it.
- `KhalaRuntimeEvent` is the neutral schema. It is the durable, redacted, replay
  substrate. Defined at `packages/agent-runtime-schema/src/index.ts:740-927`. Its
  `sequence` field is the durable cursor.
- `AcpProjectionEvent` is the ACP bridge output. It is either a
  `KhalaRuntimeEvent` or a bounded `AcpCanonicalStateEvent` snapshot
  (`packages/agent-client-runtime-bridge/src/projection.ts:35`). Grok and Cursor
  produce it, then a hand-mapper turns it back into `ClaudeLocalEvent`.
- `ProviderStreamEvent.Chain` carrying `SafeMessageChainEntry` is the delegate
  vocabulary. It is a redacted card snapshot. Defined at `ports.ts:68-75` and
  `presentation.ts:86-94`.

### The 23 `ClaudeLocalEvent` kinds

These are the renderer vocabulary. Kinds with a neutral origin are marked with a
star. Line numbers are in `claude-local-contract.ts`.

| Kind | Line | Purpose |
| --- | --- | --- |
| `composer_admission` | 227 | Composer and turn admission state plus a reason. |
| `turn_started` * | 233 | Turn began. May carry the persisted thread snapshot. |
| `text_delta` * | 242 | One streamed assistant text chunk. |
| `tool_use` * | 247 | A tool call started. |
| `tool_progress` | 255 | In-flight tool progress keyed by item ref. |
| `tool_result` * | 262 | A tool call finished with a typed completion item. |
| `model_effective` | 277 | The effective model the SDK reported. |
| `turn_completed` * | 281 | Turn finished. Carries token total and optional exact usage. |
| `turn_failed` * | 292 | Turn failed with a typed reason. |
| `reasoning` * | 303 | A completed reasoning summary line. |
| `lane_notice` | 313 | A visible lane notice. Account rotation is never silent. |
| `child_started` | 325 | A Codex delegate child started. |
| `child_activity` | 335 | Child lifecycle activity. |
| `child_completed` | 354 | Child finished with usage and duration. |
| `child_failed` | 365 | Child failed with a typed reason. |
| `question_pending` | 386 | An approval or question awaits the user. |
| `question_resolved` | 394 | The pending question settled. |
| `plan_updated` | 416 | Plan and todo progress from the SDK. |
| `child_steered` | 437 | Result of a steer-child control. |
| `followup_queued` | 451 | A follow-up was enqueued while the turn streams. |
| `followup_promoted` | 461 | A queued follow-up is ready to be the next turn. |
| `mcp_server_unavailable` | 474 | A configured MCP server could not be offered. |
| `meter_updated` | 489 | Context and rate-limit meter update. |

Only the starred kinds project to a neutral `KhalaRuntimeEvent`. The rest are
display-only and stay on the renderer envelope.

### The mapping table

"Dash" means no counterpart in that surface.

| Concept | `ClaudeLocalEvent` (renderer) | `KhalaRuntimeEvent` (neutral log) | `AcpProjectionEvent` (ACP) | `ProviderStreamEvent` (delegate) |
| --- | --- | --- | --- | --- |
| turn open | `turn_started` | `turn.started` | `turn.started` | folded into `Chain` |
| assistant text | `text_delta` | `text.delta` | `text.delta` | entry role `assistant` |
| reasoning | `reasoning` | `reasoning.delta` / `.completed` | `reasoning.delta` | dash at HEAD |
| tool start | `tool_use`, `tool_progress` | `tool.call`, `tool.input.*` | `tool.call` | entry `toolLabel` |
| tool end | `tool_result` | `tool.result` / `tool.error` | `tool.result` / `tool.error` | `fileChangeCount`, `commandOutputByteCount` |
| usage | `meter_updated`, `turn_completed.usage` | `usage.recorded`, `turn.finished.usage` | `usage-snapshot` | dash |
| turn done | `turn_completed` | `turn.finished` | `turn.finished` | `Completed` |
| turn fail | `turn_failed` | `turn.interrupted` | `turn.finished` cancelled | `Failed` |
| plan | `plan_updated` | dash | `plan-snapshot` | dash |
| child agent | `child_*` | `agent.child.*` | dash | dash |
| approval | `question_pending` / `_resolved` | not on stream, uses `RuntimeInteraction` | `session/request_permission` to `RuntimeInteraction` | dash |
| lane notice | `lane_notice` | dash | `degraded` | dash |

Note the approval row. Approvals never ride any event stream. They funnel to the
durable `RuntimeInteraction` model (`packages/agent-runtime-schema/src/index.ts:1002-1127`).
The Runtime Gateway reads and decides them through `runtime.interactions` and
`runtime.decideInteraction` (`runtime-gateway.ts:376-405` and `:496-532`).

### Where the hand-mappers live

The runtime hand-maps between these vocabularies in several places. Each mapper
is a place a fact can be lost or misrepresented.

1. `ClaudeLocalEvent` to `KhalaRuntimeEvent`: `makeClaudeLocalHarnessProjector`
   (`harness-projection.ts:106-210`). Covers the core kinds only. Display kinds
   project to nothing (`:201-207`).
2. `AcpProjectionEvent` to `ClaudeLocalEvent`: `acpProjectionEventToLaneEvent`
   (`provider-lane-acp.ts:55-191`).
3. Native ACP envelope to `AcpProjectionEvent`: the `AcpRuntimeProjector` class
   (`packages/agent-client-runtime-bridge/src/projection.ts:76-644`).
4. Generic ACP peer event to `KhalaRuntimeEvent`: `acpEventToKhalaEvents`
   (`packages/agent-harness-contract/src/acp-adapter.ts:214-308`).
5. opencode event to `KhalaRuntimeEvent`: `packages/agent-harness-contract/src/opencode-adapter.ts`.
6. `ClaudeLocalEvent` to `SafeMessageChainEntry`: `redactCodexEvent`
   (`turn/desktop-codex-provider.ts:224-251`), then `projectSafeMessageChain`
   (`packages/agent-surface/src/index.ts:158-162`).

### The neutral log per turn

`harness-projection.ts` and `harness-event-recorder.ts` add the new neutral
projection. The renderer envelope stays the renderer surface, but it becomes ONE
projection of the harness stream rather than a second source of truth
(`harness-projection.ts:1-15`).

`makeHarnessEventRecorder` is a pure observer attached to the dispatcher hook
`onTurnEventProjected` (`harness-event-recorder.ts:80-132`, wired at
`main.ts:4284-4292`). For each dispatched `ClaudeLocalEvent`, it projects to
neutral events and appends each to a per-turn store keyed by turn id. The cursor
is the neutral event `sequence`. The store rejects any sequence less than or
equal to the last, so replay is duplicate-free
(`packages/agent-harness-contract/src/event-log-store.ts:84-91`). The recorder
exposes cursor-exact liveness `{ cursor, lastEventKind, eventCount }` for Full
Auto (`harness-event-recorder.ts:117-126`). It never disturbs dispatch.

So the renderer surface is `ClaudeLocalEvent`. The durable neutral one is the
`KhalaRuntimeEvent` log.

---

## 4. The lanes

Each lane is a `ProviderLane` value. Each declares a `recovery` mode in its
capability report, which restart recovery enforces. The two modes are
`provider_session_replay` and `interrupt_on_restart`
(`provider-lane-capabilities.ts:58`).

### codex-local

- Runtime: `makeCodexLocalRuntime` (`codex-local-runtime.ts:324-1491`). Two
  dispatch paths inside `runAttempt` (`:492-1177`). The app-server path
  (`:520-817`) is the production path. The legacy exec path (`:818-1176`) is for
  deterministic fixtures.
- App server: `codex-app-server-supervisor.ts` supervises one `codex app-server`
  process per identity, with leases, reverse-RPC arbitration, and a bounded
  reconnect (`createCodexAppServerSupervisor`, `:230-665`). On reconnect it
  replays `thread/resume` for every registered visible thread (`:375-380`).
  `codex-app-server-turn.ts` runs one turn through a lease. It starts or resumes a
  thread, starts a turn, and projects notifications to envelope events through
  `handleNotification` (`:444-937`).
- Account and session: it prefers the user own logged-in Codex session and, when
  the app server is present, filters to `source === "current_session"`
  (`codex-local-runtime.ts:362-389`). Continuity lives in a `sessionByThread`
  map keyed by thread. A thread session is pinned to the account that created it.
- Recovery: mode is `provider_session_replay` (`main.ts:4725`). After a restart,
  `reconcileLocalTurns` replays the codex thread by its durable provider session
  ref (`local-turn-recovery.ts:183-262`).
- Streaming: emits `ClaudeLocalEvent` over `CodexLocalEventChannel`.

### claude-local

- Runtime: `makeClaudeLocalRuntime` (`claude-local-runtime.ts:735`). It uses the
  Anthropic Claude Agent SDK `query()` in-process (`:1308-1364`). No account
  yields a typed unavailable result, never a fall-through to another provider.
- History versus resume: an in-memory `sessionByThread` map drives resume
  (`:754`). Resume runs only when a stored session exists AND the account matches
  (`:1262-1269`). On resume the prompt is the bare message and `options.resume`
  is set. Otherwise `historyPrompt` prepends a bounded 12-message window
  (`:488-503`).
- Account and session: it prefers the ordinary `~/.claude` current session, then
  isolated `~/.claude-pylon-*` homes (`:456-486`). Admission never probes the
  macOS Keychain.
- Recovery: mode is `interrupt_on_restart` (`main.ts:4361`). A restart fails the
  turn closed to `interrupted_by_restart` with an owner-visible notice. Within a
  live turn, account rotation happens only when no content has streamed and the
  reason is `session_failed` or `account_reconnect_required` (`:1614-1633`).
- Streaming: emits `ClaudeLocalEvent` over `ClaudeLocalEventChannel`.
- History projection: `claude-history.ts` is a separate read-only projection of
  the on-disk Claude Code history tree. It is not the live continuity store.

### Grok and Cursor ACP lanes

Grok and Cursor share one ACP substrate. They differ in profile id, auth
methods, and identity-pin strength.

- Grok: `packages/grok-harness/src/`. The production path admits the `grok`
  executable via a trusted-peer profile and speaks ACP over stdio JSON-RPC
  (`grok-peer-runtime.ts:193-286`). Sessions persist to
  `~/.khala-code/grok-sessions.json` (`session-store.ts`). Auth prefers
  `xai.api_key`, then a cached token, then an external browser login. Interrupt
  is a graceful `peer.cancel(sessionId, "user")` with a grace window
  (`grok-peer-runtime.ts:432-457`).
- Cursor: `packages/cursor-agent-runtime/src/cursor-peer-runtime.ts`. It admits
  `cursor-agent` with both a file sha256 and an installation-closure sha256
  (`:114-169`). It re-verifies the closure on each transport start. Auth accepts
  only `cursor_login`. Interrupt mirrors Grok. Feature gates require live
  conformance evidence (`:261-288`).
- Bridge: both run over the shared `AcpSessionRuntime`. Updates fold through the
  `AcpRuntimeProjector` into `AcpProjectionEvent`. The desktop lane maps those
  back to `ClaudeLocalEvent` through `provider-lane-acp.ts`.
- Lane construction: `makeAcpProviderLane` builds the concrete `ProviderLane`
  (`provider-lane-acp.ts:216-278`). The chat lanes emit over
  `CodexLocalEventChannel`. The delegate variants re-point the channel to
  `ClaudeLocalEventChannel` (`main.ts:4929`, `:4937`).
- Recovery: Grok declares `provider_session_replay`, Cursor declares
  `interrupt_on_restart` (`main.ts:4902`).

---

## 5. The Apple FM router and delegation

The on-device Apple FM model is the router. The host is the decision authority.
The model proposes. The host acts and reports.

### The advisory recommendation versus the authoritative decision

The route contract freezes the split (`packages/agent-runtime-schema/src/route.ts:13-24`).
A model result can contain a `RouteRecommendation` (`:46-53`). It can never
contain an admitted `RouteDecision` (`:112-143`). The host derives the only
decision from owner policy, candidate order, capability, account readiness, data
destination, cost class, placement, privacy, and task needs.

The model boundary enforces the split. `decodeAppleFmRouteOutput`
(`packages/apple-fm-runtime/src/recommendation.ts:135-181`) can only return a
`Recommendation`, an `Answer`, or a `Reject`. A candidate outside the admitted
set becomes a `Reject` (`:164`). The model can never emit a `RouteDecision`.

### The router provider

`makeDesktopAppleFmProviderRegistry` is the router provider for the kernel
(`turn/desktop-apple-fm-provider.ts:117-168`). Readiness comes from
`host.status()` with no renderer input. The host builds the router prompt from
the canonical thread store and the available agents
(`resolveAppleFmAvailableAgents`, `main.ts:1468-1486`). The available agents
derive from the one unified harness readiness projection
(`harness-readiness-source.ts` and `main.ts:1484-1485`). When at least one agent
is ready and can delegate, the host runs guided generation with the admitted
candidates, so the route output can only name an admitted candidate and can
never be malformed. With no ready delegate, the model answers directly
(`desktop-apple-fm-provider.ts:158-166`).

### The host decision

`decideDelegation` is the host decision layer (`turn/desktop-delegation.ts:96-128`).
It decodes the router output fail-closed and combines it with main-owned
readiness. A non-recommendation answers locally. A recommendation for a
non-dispatchable candidate answers locally. A recommendation for a lane that is
not ready produces no start and an honest refusal. Only a ready, admitted,
host-owned lane produces a `delegate` decision.

### A routed turn becomes a delegated subagent turn

The flow lives in the `turn:submit` handler (`turn/desktop-turn-main.ts:369-501`).

1. Describe all providers. Split into router descriptors (`candidate === "apple_fm"`)
   and delegate descriptors (`isDelegateProvider`) (`:387-388`). The delegates
   are hand-off targets, never the router.
2. Run the kernel router turn on the Apple FM set (`:395-403`). The guided output
   becomes the answer text (`:408-409`).
3. If delegates exist and the router card is done with answer text, build the
   readiness map from main-owned descriptors and call `decideDelegation`
   (`:416-427`).
4. On a `delegate` decision, call `startDelegation` (`:428-457`). `startDelegation`
   builds a single-candidate owner-bound set and forks a SECOND kernel turn with
   `runtime.runFork` (`:330-367`). It returns a `delegated` result carrying the
   `delegationRequestRef`.

### How the delegate chain streams to the Delegated Agents card

The forked delegation turn is a normal kernel turn. As the delegate lane
`runTurn` emits `ClaudeLocalEvent`s, the delegate provider redacts them and emits
`ProviderStreamEvent.Chain` snapshots (`turn/desktop-codex-provider.ts:320-364`).
The kernel folds each `Chain` and publishes a `TurnProgressFrame` on
`service.progress`. The background forwarder sends each non-terminal frame,
carrying `projection.messageChain`, to the renderer over the `turn:event` channel
(`turn/desktop-turn-main.ts:226-255`). The renderer keys the card by the
`delegationRequestRef`.

History: the delegate starts with empty history at HEAD (`main.ts:1454`). The
#9118 change (not in HEAD) feeds real thread history.

---

## 6. Streaming to the UI

There are two streaming paths. They use different channels and different
vocabularies.

### The chat path (Stack A)

1. A lane emits a `ClaudeLocalEvent` through the `emit` it received.
2. `emit` is the dispatcher `emitTurnEvent` closure (`provider-lane.ts:395-465`).
3. The closure folds, persists, meters, and projects the event.
4. It forwards to the renderer with `sender.send(lane.eventChannel, { turnRef, event })`
   (`provider-lane.ts:464`). The channel is `ClaudeLocalEventChannel` for
   claude-local and `CodexLocalEventChannel` for codex-local and the chat ACP
   lanes.
5. The renderer receives the event through the preload bridge and appends the
   text into one note by `messageRef` (`renderer/runtime-conversation.ts:86-126`).

### The delegate path (Stack B)

1. The delegate lane emits a `ClaudeLocalEvent` into the delegate provider `emit`.
2. The provider redacts it with `redactCodexEvent` and rebuilds the full
   `SafeMessageChainEntry` chain (`turn/desktop-codex-provider.ts:325-340`).
3. It offers `ProviderStreamEvent.Chain` to the kernel.
4. The kernel folds the `Chain`, generation-fences it, and publishes a
   `TurnProgressFrame` (`turn-service.ts:233-241`).
5. The forwarder sends the frame on the `turn:event` channel
   (`turn/desktop-turn-main.ts:238-246`).
6. The renderer renders the chain inline in the Delegated Agents card.

### Where streaming can still be improved

At HEAD, the delegate path does NOT stream assistant text token by token.
`redactCodexEvent` returns `null` for `text_delta` (`turn/desktop-codex-provider.ts:248-250`),
so a text delta produces only a `Progress`, not a chain entry. The assistant
answer appears only when `result.text` is appended as the final chain entry at
completion (`:355-359`). The #9118 change (commit `23491d8d15`, not in HEAD)
coalesces `text_delta` into one growing assistant chain entry in the emit
closure, so the delegate answer streams live. The change also feeds real
delegate history. Until that change lands in HEAD, delegate answers appear in one
step at completion and delegate turns start context-free.

---

## 7. State and durability

All durable state is main-owned. Writes are atomic. Files are mode 0600 under a
mode 0700 parent.

### Thread store

`makeThreadStore` (`thread-store.ts:32-167`). It is a bounded composer cache, not
the sidebar catalog.

- `maxThreads = 5` ordinary LRU threads (`:14`).
- `maxAcceptanceVerdictThreads = 24` for `PASS/FAIL/BLOCKED · TEST NN` audit
  threads (`:18`).
- `maxNotes = 80` per thread. Every write slices to the last 80 (`:19`).
- Protected threads come from `options.protectedThreadIds` and cover nonterminal
  Full Auto runs (`:36-42`).
- API: `list`, `restoreThread`, `newThread`, `forkThread`, `open`, `rename`,
  `append`, `upsert`, `remove` (`:78-166`). `open` finds by id (`:115`).
  `append` pushes a note and slices (`:125-136`). `upsert` replaces one keyed
  note in place without changing order (`:138-153`).

### Local turn journal

`openLocalTurnJournal` (`local-turn-journal.ts:181-303`). The durable, atomic
record of in-flight local turns keyed by `(threadRef, turnRef, lane)`.

- Record shape at `:64-82`. Phases at `:30-41`. Dispositions at `:43-50`.
- Exactly-once accept: `accept` returns `{ accepted: false }` for a matching
  existing key, throws `conflicting_turn` for a key reused by a different turn,
  and returns `{ accepted: true }` only for a new key (`:206-237`). So exactly
  one caller ever accepts a given turn.
- Recovery: `beginRecovery` moves a first recovery to `recovering` with
  `recoveryGeneration = 1`. A second recovery forces terminal
  `interrupted_by_restart` (`:279-295`). It fails closed. It never resumes
  forever.
- Durability: `writePrivateAtomic` writes to `.pending` and renames
  (`:145-161`).

`turn/desktop-turn-journal.ts` is the AFS kernel `TurnJournal` adapter. On a
terminal kernel record it settles a pre-existing legacy row through
`mirrorLegacyTerminal` (`:92-104`). It never invents legacy identity.

### Usage ledger

`makeUsageLedger` (`usage-ledger.ts:50-126`). In-memory, per-session, aggregated
per `(provider, accountRef)`. `record` adds token fields and counts a turn or
child (`:99-110`). `markReconnectRequired` and `markVerified` toggle sticky
reconnect state (`:111-114`). Snapshots publish to subscribers.

### Runtime interactions

Approvals and questions live in the durable `RuntimeInteraction` model, not on
the event stream (`packages/agent-runtime-schema/src/index.ts:1002-1127`). Kinds
are `provider_question`, `tool_approval`, and `plan_review`. The Runtime Gateway
reads them and applies idempotent decisions.

### Full Auto as a consumer

Full Auto does not run turns itself. It is a policy and orchestration layer.
`full-auto-lane.ts` holds per-lane policy and the Full-Auto prompt
(`fullAutoPrompt`, `:50-53`). `full-auto-run-registry.ts` owns the durable
multi-turn run graph and a per-thread dispatch lease. It repeatedly frames a
Full-Auto prompt for the run bound lane and dispatches it as an ordinary local
turn through the same journal accept path and the same renderer projection. It
advances the run graph on completion until a cap, an owner control, or a failure
limit stops it.

---

## 8. The renderer transcript

The renderer timeline lives at `renderer/react-timeline.tsx` with presentation
in `packages/ui/src/workbench/agent-group.tsx`.

### The event flow into the renderer

Events arrive over Electron IPC through the preload bridge `preload.cts`. The key
channels are `openagents:claude-local:event`, `openagents:codex-local:event`,
`openagents:turn:event` for delegation frames, and the usage ledger channels
(`renderer/boot.ts:1044`, `:1078`, `:1734-1739`, `:1703-1704`). Each handler
updates renderer state. Notes are built in `renderer/runtime-conversation.ts`.
Assistant text appends into the same note by `messageRef`, which makes a streamed
message grow in place (`:95-99`).

### The timeline render path

`ReactTimeline` (`react-timeline.tsx:1934`) derives, all with `useMemo`, the
turns, the assistant meta keys, and the display rows through
`deriveReactTimelineDisplayRows` (`:1713-1716`). Rows are windowed for
virtualization, and each visible row renders through
`MemoTimelineItemBoundary` and `TimelineItem` (`:834`).

### The Worked activity group

A settled turn folds intermediate commentary and tool activity into one duration
row while keeping the prompt and the terminal answer visible. The live turn never
folds (`:1282-1287`). The label reads "Worked" or "Worked for {duration}"
(`:1236-1249`). The group is default-expanded (`useState(true)`) per the owner
directive that tools and activity must not hide behind a collapsed fold.

### The Delegated Agents card

`DesktopAgentGroup` renders the card (`packages/ui/src/workbench/agent-group.tsx`,
title default "Delegated agents", `:130`, `:152`). Each `DesktopAgentRow` is a
native details element that is default-expanded (`useState(true)`, `:44`). When a
record carries `runtimeChild`, `TimelineItem` passes the subagent bounded and
redacted message chain inline as the transcript, plus interrupt controls
(`react-timeline.tsx:876-893`). Each line renders as a bold label plus the text
(`agent-group.tsx:105-120`).

### The live re-render mechanism

The task calls this the transcript signature. The literal string does not appear
in the code. The mechanism is `runtimeChildTranscriptSignature` plus
`sameTimelineRecord`, which gate `MemoTimelineItemBoundary`.

- `runtimeChildTranscriptSignature(child)` is a cheap scalar signature
  `` `${length}:${lastRole}:${lastTextLength}` `` (`react-timeline.tsx:1129-1142`).
  It flips when a message is added or the last message grows during streaming,
  without stringifying kilobytes.
- `sameTimelineRecord(left, right)` is the memo equality. It compares the scalar
  fields and the two transcript signatures (`:1144-1177`, signatures at
  `:1169-1170`).
- `MemoTimelineItemBoundary = memo(..., (l, r) => l.report === r.report && sameTimelineRecord(l.record, r.record))`
  (`:1179-1182`).

So an IPC event grows a note, the reprojection produces a record with a changed
signature, `sameTimelineRecord` returns false for that one row, and only that row
re-renders. Every unaffected row stays cheap.

---

## 9. The harness contract layer

`packages/agent-harness-contract` is the neutral Effect harness substrate that
the lanes are converging toward. An `AgentHarness` adapter has one `start` entry
(`adapter.ts:61-94`). It yields a caller-owned `HarnessSession` whose
`promptTurn` streams neutral `HarnessStreamEvent`s, which are `KhalaRuntimeEvent`
verbatim (`stream.ts:20-21`). Those events flow into a durable, cursor-exact
`HarnessEventLog` over a swappable store port (`event-log.ts`,
`event-log-store.ts`). A slice runner drives a long turn as time-boxed slices
with non-destructive suspend and resume at an exact cursor (`slice-runner.ts`).
`projectHarnessReadiness` is the single readiness source (`readiness.ts`).
Sandbox providers are the fail-closed workspace substrate (`sandbox.ts`,
`local-process-sandbox-provider.ts`). The ACP and opencode adapters land those
runtimes on the same neutral stream (`acp-adapter.ts`, `opencode-adapter.ts`).
Approvals route off-stream into `RuntimeInteraction`. Capabilities are fail-closed
by method presence, refused with `HarnessCapabilityUnsupported` (`capability.ts`).

Today each lane hand-maps onto the renderer `ClaudeLocalEvent`. The harness
contract inverts that. Any admitted runtime becomes an `AgentHarness` emitting
one neutral `KhalaRuntimeEvent` stream through one factory. The desktop recorder
in `harness-event-recorder.ts` is the first live use of this substrate, running
as a pure observer of the existing dispatch path.

---

## 10. Gaps and sharp edges

These are the places a future change is likely to break something or where two
designs overlap. Read them before you touch streaming, history, or the stacks.

1. Delegate answers do not stream at HEAD. `redactCodexEvent` returns `null` for
   `text_delta` (`turn/desktop-codex-provider.ts:248-250`). The delegate answer
   appears in one step at completion. The #9118 change coalesces text deltas into
   the chain but is not in HEAD. If you rely on live delegate text, confirm the
   change landed first.

2. Delegated turns start context-free at HEAD. `runDelegateLaneTurn` passes
   `history: []` (`main.ts:1454`). A subagent turn has no thread history. The
   #9118 change feeds real history. Until then, a delegate cannot see the
   conversation that produced its objective.

3. Two turn engines with overlapping concerns. Stack A
   (`makeProviderLaneDispatcher`) and Stack B (`TurnService`) both own a turn
   lifecycle, a journal, and a thread repository. Stack A journals through
   `local-turn-journal.ts`. Stack B journals through `turn/desktop-turn-journal.ts`,
   which then mirrors terminal state back to the legacy journal
   (`:92-104`). A chat coding turn and an Apple FM delegated turn take different
   engines to reach the same lane. Keep the seam at `runDelegateLaneTurn` in mind
   when you change either engine.

4. Two hand-mapped ACP directions. The ACP native envelope maps to
   `AcpProjectionEvent` (`packages/agent-client-runtime-bridge/src/projection.ts`),
   then `AcpProjectionEvent` maps to `ClaudeLocalEvent`
   (`provider-lane-acp.ts:55-191`). A third mapper turns a generic ACP event into
   `KhalaRuntimeEvent` for the harness contract (`acp-adapter.ts:214-308`). Three
   mappers over one protocol is a maintenance risk. A mapping that returns `null`
   drops the fact from that surface by design, but a wrong `null` is a silent loss.

5. The neutral projection covers only the core kinds. `makeClaudeLocalHarnessProjector`
   projects seven kinds and drops the rest (`harness-projection.ts:201-207`). The
   durable neutral log therefore omits plan, meter, question, child, and notice
   facts. Do not treat the neutral log as a complete transcript. It is a core
   subset for a transcript, a usage accountant, and cursor-exact replay.

6. Two Apple FM entry points. `apple-fm:startTurn` runs a direct on-device chat
   turn through `appleFmHost.runTurn` (`main.ts:3996-3999`). `turn:submit` runs
   the on-device model as a router inside the kernel. The same model serves two
   different call paths. A change to Apple FM behavior must consider both.

7. Recovery is honest but asymmetric. Only codex-local declares
   `provider_session_replay`. Every other lane fails a restart closed to
   `interrupted_by_restart` (`local-turn-recovery.ts:177-182`). This is correct
   and honest, but it means a long claude-local or ACP turn cannot survive a
   desktop restart. The harness slice runner is the intended future path to
   durable long turns.

8. The renderer streaming coupling is by note key, not by a typed stream cursor.
   A streamed message grows because assistant text appends into one note by
   `messageRef` (`renderer/runtime-conversation.ts:95-99`) and the memo equality
   watches a scalar transcript signature (`react-timeline.tsx:1129-1142`). This is
   efficient, but it couples live re-render to a string shape rather than to the
   durable neutral cursor. The neutral log and the renderer stream are not yet the
   same source.
```
