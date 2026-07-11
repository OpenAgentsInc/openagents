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
