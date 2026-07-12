# CUT-12 live-agent supervision UI receipt

- Date: 2026-07-11
- Issue: [#8692](https://github.com/OpenAgentsInc/openagents/issues/8692)
- Status: shared presentation model plus Khala Mobile and OpenAgents Desktop
  thread integration active; physical iOS/Android receipts remain pending
- Contracts: `khala_mobile.agent_graph.confirmed_hierarchy_and_safe_focus.v1`,
  `openagents_desktop.agent_graph.pointer_keyboard_focus_equivalence.v1`

## Shared presentation model

`@openagentsinc/khala-sync-client` now converts one validated canonical graph
post-image into a provider-neutral supervision model. It preserves:

- parent/subagent depth and stable agent/graph refs;
- status, semantic state tone, current action, attention, elapsed time, and
  terminal reason;
- provider, runtime, session, and worktree facts, including explicit named
  unavailability rather than blank or fabricated parity;
- live versus historical authority; and
- deterministic selection fallback and newest attachment/cursor selection.

Traversal is stable across source ordering. A configurable hard bound reports
the exact hidden remainder rather than silently dropping a busy graph.
Historical rows remain inspectable but `canControl` is false for every node.

## Mobile thread surface

Khala Mobile reads `live_agent_graph` through the same exact thread-scope,
local-first Sync hook as the transcript and turns. The thread view adds one
compact agent stack above the transcript:

- up to four agents open as a hierarchy by default; larger graphs start with
  a compact summary so the chat does not lose its task surface;
- any attention state opens the hierarchy automatically;
- tapping a row selects the same typed agent ref and reveals provider/runtime,
  session/worktree, elapsed, tool, attention, and terminal details inline;
- state color is semantic only and every row carries a complete screen-reader
  label, selected state, and ordinary button semantics; and
- mobile renders at most 40 rows and announces the exact hidden remainder.

The component accepts typed `inspect_agent` / `focus_agent` actions. The
current thread integration uses selection/inspection only; focus is local UI
focus, not execution movement or provider authority. Historical authority is
labeled `Historical import · controls unavailable` and removes the focus
control.

## Desktop thread surface

OpenAgents Desktop consumes the same validated Runtime Gateway v8 graph
post-image already carried by its fenced conversation subscription. The
renderer projects the newest attachment/cursor through the shared presentation
model; an opened existing thread uses one brief subscribe/current/unsubscribe
cycle to hydrate its graph. Sends retain the existing live subscription. There
is no second timeline query loop or graph poller.

The Effect Native chat surface places one compact agent stack above the
transcript:

- graphs of up to eight agents open by default, attention opens the hierarchy,
  and larger graphs retain the task surface behind an explicit summary;
- rows preserve parent/subagent depth, semantic status and attention, and an
  accessible label containing the same facts available to sighted operators;
- selecting a row opens an inline table with provider, runtime, session,
  worktree, action, elapsed, attention, and terminal facts;
- pointer, keyboard, and screen-reader activation dispatch the same schema-
  checked `DesktopAgentAction` with explicit `inspect_agent` or `focus_agent`
  kind and stable agent ref; and
- Escape deselects through that registry, rapid replacement falls back through
  the shared selector, historical graphs omit focus, and the 200-row Desktop
  bound names its exact hidden remainder.

Desktop focus remains local inspection state. It does not move execution,
change provider authority, infer parentage from transcript/tool text, or expose
provider payloads and handles.

## Verification

- Shared presentation focus: 5 pass, 0 fail, 12 expectations; typecheck passes.
- Mobile graph + thread + registry focus: 21 pass, 0 fail, 209 expectations.
- Full `@openagentsinc/khala-sync-client`: 178 pass, 3 opt-in live-smoke skips,
  0 fail, 12,741 expectations.
- Full Khala Mobile: 485 pass, 0 fail, 1,688 expectations.
- Desktop graph/conversation/shell focus: 59 pass, 0 fail, 294 expectations;
  Desktop typecheck passes.
- Full OpenAgents Desktop: 346 pass, 0 fail, 1,753 expectations; production
  bundle and Electron smoke pass at Runtime Gateway protocol v8.
- Mobile typecheck reaches only the pre-existing
  `expo-db-sqlite-persistence.ts` local-store interface mismatch; no CUT-12
  file reports an error.
- Mobile dependency-cruiser reaches only the pre-existing
  `khala-sync/index.ts -> local-authority.ts -> index.ts` cycle; no CUT-12
  dependency violation is reported.

## Residual

CUT-12 remains open only for the required physical iOS and Android interaction
receipts. The mounted component, built Desktop, and Electron smoke evidence do
not substitute for those devices. The paired phone remains owner-deferred while
it records video; no device completion is claimed here.

## 2026-07-12 tranche — greenfield OpenAgents Mobile surface and exact token attribution

The greenfield product app `apps/openagents-mobile` now renders the same
canonical supervision boundary the deprecated Khala Mobile reference proved:

- the mobile Sync host exposes the confirmed thread-scope
  `live_agent_graph` reader (`createKhalaSyncLiveAgentGraph`) with the same
  live-phase gating as the timeline;
- the conversation adapter passes that reader into
  `openKhalaConversationLive` and the confirmed-thread read, so every thread
  snapshot carries the exact confirmed graph post-images — no parallel shape
  and no poller;
- the Effect Native conversation surface mounts one accessible agent stack
  above the transcript: authority badge, summary toggle, parent/subagent
  depth rows with status badges, attention auto-open, tap select/inspect of
  the exact typed agent ref, deterministic replacement fallback through the
  shared selector, an inline inspector with provider/runtime/session/
  worktree/elapsed/token/action/attention/terminal facts, and a named 40-row
  remainder;
- no runtime-control or execution-movement intent is reachable from a graph
  row, and historical authority renders `Historical import · controls
  unavailable`.

Exact token attribution is now part of the shared presentation contract.
`projectLiveAgentGraphPresentation` accepts a typed per-node attribution
ledger (the shape produced by the desktop-local fold's `usageAttributions()`);
each row carries `tokenTruth` and `tokensLabel` where `exact` requires every
recorded attribution to hold a complete well-formed usage split, a mix names
the exact recorded total plus the unreported turn count, and missing or
malformed claims stay loss-accounted `Unreported` — never synthesized. Both
inspectors (Desktop table, mobile stack) and both accessible row labels
surface the token fact. The live Desktop and mobile paths currently provide no
attribution ledger, so every live row honestly reads `Unreported` until the
CUT-11 local-fold wiring or a gateway attribution source lands.

Verification (2026-07-12, worktree at `origin/main` 375a8997ff):

- Shared presentation focus: 7 pass, 0 fail, 16 expectations.
- Full `@openagentsinc/khala-sync-client`: 192 pass, 3 opt-in live skips,
  0 fail, 12,803 expectations.
- Mobile agent-graph oracle: 6 pass, 0 fail, 32 expectations.
- Full OpenAgents Mobile: 104 pass, 0 fail, 533 expectations; mobile
  typecheck passes.
- Full OpenAgents Desktop: 985 pass, 3 skip, 0 fail, 5,275 expectations;
  Desktop typecheck passes; built Electron smoke passes end-to-end.
- Rendering proof is deterministic view-program oracles plus the built
  Electron smoke; no new pixel/screenshot claim is made for the mobile
  surface, and physical iOS/Android receipts remain open.
