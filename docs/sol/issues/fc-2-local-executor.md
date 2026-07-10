# FC-2: real mixed-harness Pylon supervisor and standing parallel executor

Parent: #8638

Status: **implementation complete; closure transfers the live-account receipt
to #8640 rather than claiming unspent provider work.**

## Outcome

A standing desktop Pylon claims a Sarah-created FleetRun and executes multiple
work units concurrently across real Codex, Claude, and Grok accounts.

## Grounded gap at issue creation

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

## Landed disposition

The implementation stack ending at `134d7d8ca3` closes the code and fixture
boundary:

- one Pylon-home SQLite authority owns accepted Sarah runs, claims, task state,
  activation, health breakers, and the execution outbox across restart;
- the standing node polls, imports, accepts, arms, refills, and reconciles
  without a supervising CLI or manually launched per-assignment shell;
- the live account inventory retains named Codex, Claude, and Grok custody,
  worker kind, declared cost truth, health, and counted slots;
- `auto` uses the shared typed fleet-intents resolver and records every bounded
  skip/fallback rather than silently substituting a harness;
- Codex and Claude terminal results require exact closeout/token refs; Grok
  produces a durable `not_measured` receipt with no invented token count;
- account quota/auth failures survive restart and rotate capacity, while task,
  verifier, workspace, and generic failures do not quarantine a healthy
  account;
- accepted Sarah runs journal execution locally even while delivery is
  offline, then upload frozen, gapless, byte-bounded batches through the
  registered-agent route;
- the server binds every append to the exact owner, Pylon, accepted claim, and
  known plan unit, persists append-only events plus coherent terminal
  closeouts, and updates the owner Sync post-image in the same Postgres
  transaction;
- one integrated mixed-harness fixture starts three unique claims, restarts the
  SQLite process boundary, reconciles without redispatch, and closes with
  `exact`, `exact`, and `not_measured` usage truth.

The first useful work through one real Codex account, one real Claude account,
and one real Grok account is deliberately not claimed by this receipt. That is
the live operational acceptance run in #8640 Phase A, after minimum-safe #8639
reconnect and control composition.

## Safety

- Automatic work requires named isolated account refs.
- No provider credentials, raw events, or worktree paths cross the public wire.
- Quota/auth failures update account health and rotate only to an allowed ready
  account.
- One executor failure cannot stop unrelated work units unless the run policy
  says fail-fast.

## Implementation exit for this issue

On one Pylon-home fixture, the production standing composition runs three
pinned units concurrently through the concrete Codex, Claude, and Grok adapter
ports. All three share one run/claim registry, produce zero double claims,
survive one daemon/SQLite restart in the test matrix, upload durable terminal
state, and close with honest per-harness usage evidence. The same source paths
are used by named live accounts; real credential spend and useful-work proof
remain #8640 and are not inferred from fixtures.
