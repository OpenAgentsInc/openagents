# autopilot.decision_queue.v1 — vertex-fleet worker note

Promise state: **planned** (unchanged — no green/yellow flips in this change).

## Blocker advanced

`blocker.product_promises.decision_queue_api_missing` — *partially advanced* (not
cleared; left listed).

The promise claim names **eight** decision-queue actions — continue, steer,
provide context, rerun tests, retry with another account, stop, accept, and
create a follow-up mission. Until now the only command/verb surface in the
control protocol was the 3-verb `decision.resolve` wire form
(`approve | deny | answer`, see `decision-resolve-command.ts`). There was no typed
API that enumerated the richer eight-action vocabulary, declared which actions
exercise elevated authority, or produced the exactly-once command handle the
route layer needs.

## What was built

- `packages/autopilot-control-protocol/src/decision-queue-action.ts` — a pure,
  transport-agnostic module that:
  - declares the explicit `DecisionQueueAction` enum (the eight promised actions)
    and `DECISION_QUEUE_ACTION_SPECS` describing each;
  - classifies the **authority** each action exercises
    (`none | continuation | spend | account | mission_creation`) and derives
    `requiresOwnerApproval` from it, honoring the promise's authorityBoundary
    (a visible decision grants no continuation/account/spend authority by itself);
  - derives a deterministic, cross-client **idempotency key**
    (`decisionQueueIdempotencyKey`) — the exactly-once command handle;
  - lowers each action to the existing receipt-backed `decision.resolve` wire
    command via `buildDecisionQueueCommand`, refusing (`ok:false`) on blank
    requestId, unknown action, missing required payload, or an authority-bearing
    action without owner approval. It grants **no** new authority — resolution
    still flows through the capability-scoped bridge enforced node-side.
- `packages/autopilot-control-protocol/src/decision-queue-action.test.ts` —
  16 passing tests covering the enum surface, authority/approval gating,
  idempotency-key determinism, payload validation, full eight-action coverage,
  and wire-format parity with `buildDecisionResolve`.
- Exported from the package `index.ts`.

This composes with the already-built `remote-decision-queue.ts` (the transport
that relays a resolution over the bridge and classifies the receipt): the new
module is the action-vocabulary + authority-gating front-half that maps a UI
action onto the verb that queue resolves.

## What genuinely remains (blocker NOT cleared)

- Authenticated server command APIs (a real route per action) that consume these
  commands and enforce the idempotency key server-side.
- Persisted owner-approval records for the authority-bearing actions.
- Receipt closeout wired end-to-end to a real mission
  (`receipt_backed_command_closeout_missing`) and UI projection across
  desktop / web / Expo (`cross_client_exactly_once_decisions_missing`).
- A dereferenceable receipt of a real cross-client decision resolved over a
  paired node, plus owner sign-off, before any state change.

All three blockers remain listed on the promise; only the action-enum/idempotency
piece of the first was advanced.
