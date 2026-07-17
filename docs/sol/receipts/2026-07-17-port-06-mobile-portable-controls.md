# PORT-06 mobile portable-controls receipt

- Date: 2026-07-17
- Parents: #8751 (PORT-06), #8748 (PORT-03 prerequisite)
- Leaves: #8947, #8948, #8949
- Destination: `packages/khala-sync-client`,
  `apps/openagents-mobile`
- Status: deterministic source receipt; no physical-device or real host-move
  acceptance claim

## Landed boundary

The portable-session contract and Sync client expose one confirmed-only owner
projection for portable sessions, attachment generations, target membership,
accepted commands, and outcomes. Command requests create no optimistic entity.
Malformed rows, owner mismatch, orphaned refs, stale scope state, and local
pending mutations remain explicitly accounted.

OpenAgents mobile projects that authority onto an inspected controller session.
It shows the exact source target and attachment generation, requires an
explicit ready destination for Move and Failback, and admits typed Stop,
Checkpoint, Move, Resume, and Failback commands only when their current state
preconditions hold. Same-source, offline, stale, ambiguous, and in-flight
requests fail closed.

The Effect Native controller renders local queue state separately from server
acceptance and terminal outcomes. A queued tap says it is awaiting confirmed
reconciliation. Only a confirmed outcome may show `completed`, together with
its public-safe command ref and evidence count.

## Verification boundary

- portable contract and Sync-client projection/mutator tests pass;
- mobile admission, controller UI, authenticated Sync-host, and controller
  detail tests pass;
- mobile TypeScript and the complete bounded mobile suite pass;
- the server-side shared-constant change is import-only; its focused test was
  blocked before execution by exhausted local PostgreSQL shared-memory IDs and
  that limitation is recorded on #8947.

This evidence does not prove checkpoint creation, quiescence, transport,
destination attachment generation N+1, source cleanup, failback, crash
recovery, or an installed-device journey across two distinct hosts. Those
remain PORT-03 and PORT-06 acceptance work and must not be inferred from the
mobile command surface.
