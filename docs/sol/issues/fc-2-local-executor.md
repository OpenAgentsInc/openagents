# FC-2: real mixed-harness Pylon supervisor and standing parallel executor

Parent: #8638

## Outcome

A standing desktop Pylon claims a Sarah-created FleetRun and executes multiple
work units concurrently across real Codex, Claude, and Grok accounts.

## Grounded gap

The repository already has a mixed-kind supervisor model, but the default live
manager still inspects one requested provider at a time, drops per-account
`workerKind` when mapping real capacity, keeps its orchestration DB in memory,
and treats Grok through a separate spawn path. The headless assignment worker
can poll server leases, but its loop is serial unless multiple manual runners
are started. This issue joins those real paths.

## Scope

1. Move the production FleetRun store to the Pylon home/typed engine boundary;
   runs and claims survive desktop or daemon restart.
2. Build one real capacity projection containing named Codex, Claude, and Grok
   accounts with per-account worker kind, readiness, available slots, health,
   and measured/`not_measured` cost class.
3. Wire the real supervisor runner to the existing Codex, Claude, and Grok
   executors. Delete the Grok special-case spawn branch when parity is proven.
4. Make `auto` call the existing typed resolver across the live mixed pool;
   every skip/fallback event is recorded.
5. Run up to advertised capacity concurrently. Account-level serialization,
   global target concurrency, and work-claim uniqueness all bind.
6. Teach the standing `pylon node` worker to claim/refill a durable FleetRun
   without manual background shells. Arming remains owner-local and explicit.
7. Recover interrupted local work with typed stale closeout before claiming a
   replacement unit.
8. Stream lifecycle and exact closeout refs to the durable run projection.

## Safety

- Automatic work requires named isolated account refs.
- No provider credentials, raw events, or worktree paths cross the public wire.
- Quota/auth failures update account health and rotate only to an allowed ready
  account.
- One executor failure cannot stop unrelated work units unless the run policy
  says fail-fast.

## Exit

On one owner desktop, a single standing Pylon runs three pinned fixture units
concurrently using one real Codex account, one real Claude account, and one
real Grok account. All three share one run/claim registry, produce zero double
claims, survive one daemon restart in the test matrix, and close with honest
per-harness usage evidence.
