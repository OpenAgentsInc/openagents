# CUT-12 live-agent supervision UI receipt

- Date: 2026-07-11
- Issue: [#8692](https://github.com/OpenAgentsInc/openagents/issues/8692)
- Status: shared presentation model and Khala Mobile thread integration active;
  Desktop presentation plus physical iOS/Android receipts remain pending
- Contract: `khala_mobile.agent_graph.confirmed_hierarchy_and_safe_focus.v1`

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

## Verification

- Shared presentation focus: 5 pass, 0 fail, 12 expectations; typecheck passes.
- Mobile graph + thread + registry focus: 21 pass, 0 fail, 209 expectations.
- Full `@openagentsinc/khala-sync-client`: 178 pass, 3 opt-in live-smoke skips,
  0 fail, 12,741 expectations.
- Full Khala Mobile: 485 pass, 0 fail, 1,688 expectations.
- Mobile typecheck reaches only the pre-existing
  `expo-db-sqlite-persistence.ts` local-store interface mismatch; no CUT-12
  file reports an error.
- Mobile dependency-cruiser reaches only the pre-existing
  `khala-sync/index.ts -> local-authority.ts -> index.ts` cycle; no CUT-12
  dependency violation is reported.

## Residual

CUT-12 remains open. The next file-disjoint work is the equivalent Desktop
hierarchy/keyboard/pointer/screen-reader presentation over Runtime Gateway v8's
already-confirmed graph post-images. Physical iOS and Android interaction
receipts remain required and are not claimed by these mounted component tests.
The paired phone remains owner-deferred while it records video.
