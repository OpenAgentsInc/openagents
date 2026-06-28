# Artanis FRLM Conductor Core Scheduler

Status: implementation note for #6679. This is not public product copy and
does not enable live scheduler authority.

## Contract

`apps/openagents.com/workers/api/src/artanis-frlm-conductor.ts` introduces the
pure `FrlmConductor` core. It models the FRLM pattern from the retained
architecture notes as:

- Environment: typed context vars plus bounded context fragments.
- Scheduler policy: allowed executors, fanout limit, parallelism limit, depth
  limit, quorum, budget, Blueprint signature refs, and evidence refs.
- Planner: returns governed subqueries.
- Leaf executor: runs a typed Local/Swarm/Remote/Codex subquery.
- Composer: receives only returned subquery outputs and builds the final answer.
- Trace emitter: records `Run.Init`, `SubQuery.Submit`, `SubQuery.Return`, and
  `Run.Done`.

The conductor does not call providers directly and does not grant execution,
payout, public-claim, or training-promotion authority. It is a composition
kernel that can be wired into the owner-only Artanis operator path later.

## Blueprint Boundary

Every subquery must carry a signature ref included by the scheduler policy.
The default Artanis policy includes:

- `program_signature.frlm_conductor.v1`
- `program_signature.rlm_leaf_executor.v1`
- `program_signature.blueprint_action_submission.evidence_only.v1`

Planner output using an unregistered signature is rejected before any leaf
executor is called. Failed leaf executor returns are recorded as trace events,
but the run only composes if quorum is met.

## Why This Is The Core Scheduler

The previous long-answer safety net continued a single completion after length
stops. This core makes the architectural move required by #6654/#6679: a long
operator answer can be produced by decomposition, fanout, quorum, and
composition instead of being bounded by one model call. The first landed surface
is deliberately pure and testable so later wiring can attach real Artanis
operator memory, Pylon/Codex state, NIP-90 swarm calls, or remote model calls
without changing the authority boundary.

## Verification

Focused regression coverage lives in
`apps/openagents.com/workers/api/src/artanis-frlm-conductor.test.ts` and asserts:

- governed subquery fanout emits the FRLM trace event sequence;
- quorum success composes and sums token usage;
- quorum failure blocks composition while preserving failure evidence;
- ungoverned Blueprint signatures fail closed before execution.
