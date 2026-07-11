# CUT-06 Fleet supervisor ordering receipt

Date: 2026-07-11

Issue: [#8686](https://github.com/OpenAgentsInc/openagents/issues/8686)

Implementation: `d98abda795`, compatibility stack through `54934f05f5`

Status: accepted; CUT-06 and the bounded #8640 Phase A proof are complete

## Result

Fleet supervisors are now owned by the exact run scope that starts them. The
scope interrupts the supervisor, propagates cancellation into assignment HTTP
and local Codex/Claude execution, joins the loop, and only then releases the
one-supervisor guard. A concurrent restart waits for that teardown instead of
overlapping the prior generation, and late terminal lifecycle evidence remains
available while release is in progress.

The publication order is also explicit. A completed assignment result remains
withheld until the exact closeout, usage evidence, verifier result, assignment
identity, and correlation refs are readable. Delayed verification cannot expose
an accepted terminal result, rejected verification cannot be promoted after a
restart, and cancellation produces a typed rejected lifecycle rather than a
fabricated completion.

The behavior contract
`background_agents.fleet.supervisor_scope_and_publication_order.v1` records the
boundary. Its registry version is `2026-07-11.2`.

## Race and leak evidence

The focused suites cover:

- simultaneous manager dispatch and exact one-supervisor admission;
- cancellation while a dispatch is still running;
- teardown/restart overlap and guard retention until loop join;
- success, failure, and late lifecycle retention during release;
- delayed closeout/verifier evidence;
- rejected verifier evidence across restart;
- Codex SDK cancellation and outer-deadline ordering;
- Claude SDK cancellation through the same supervisor signal; and
- the Khala Fleet manager leak that previously left the supervisor guard held.

The original Khala regression is green at 40/40. The combined focused run is
green at 131 tests and 745 expectations.

## Integrated verification

```bash
bun test apps/pylon/tests/fleet-run-manager.test.ts \
  apps/pylon/tests/fleet-run-owned-runner.test.ts \
  apps/pylon/tests/codex-agent-executor.test.ts \
  apps/pylon/tests/claude-agent-executor.test.ts \
  clients/khala-code-desktop/tests/khala-fleet-tools.test.ts
bun test packages/behavior-contracts/src/behavior-contracts.test.ts
bun run --cwd packages/pylon-core test
bun run --cwd packages/pylon-core typecheck
bun run --cwd apps/pylon test
bun run --cwd apps/pylon typecheck
bun run check:deploy
```

Passed from the clean worktree:

- 36 behavior-contract tests and 284 expectations;
- 65 Pylon-core tests and 210 expectations, plus typecheck;
- 2,368 Pylon tests, 12,048 expectations, three explicitly gated skips, and
  zero failures, plus typecheck;
- the full deploy gate, including security, architecture, contract, Khala Sync,
  web, and Worker API suites; and
- `git diff --check`.

## Accepted production-path rung

The authenticated administrator start authority created
`fleet_run.sarah.666432631ce5e88a47a5` against pinned public source
`e3f9ad11a39702630a91e111ddf61a6b879fc2d5`. The registered standing Pylon
claimed both dependency-free units in one supervisor tick at target concurrency
two. Public execution finished at sequence 44 with:

- state `completed`;
- two work units, zero active assignments;
- two accepted assignments, zero failed assignments, zero stale assignments;
- named Codex assignment
  `assignment.public.khala_coding.chatcmpl_60ee4448ec634d4ca093fed348682dd9`
  on `account.pylon.codex.d0b50dc586f60fbb3099a335`; and
- named Claude assignment
  `assignment.public.khala_coding.chatcmpl_453bc3f50e884ca89246053760e01287`
  on `account.pylon.claude_agent.9bf9d93a5996e04c3f27cb12`.

Both workers ran exactly
`bun test apps/pylon/tests/fleet-run-manager.test.ts`: four tests passed, zero
failed, and neither checkout was modified. Both materializations recorded a
prepared-baseline cache hit for the same pinned commit. No default Codex home,
provider substitution, manually launched assignment shell, duplicate claim, or
unproven terminal promotion entered the accepted run.

Exact own-capacity usage was retained without normalizing provider truth:

| Harness | Input | Output | Reasoning | Cache read | Total |
| --- | ---: | ---: | ---: | ---: | ---: |
| Codex | 49,103 | 165 | 0 | 25,344 | 49,268 |
| Claude | 12 | 2,013 | 0 | 579,960 | 2,025 |

Claude's cache-read counter is independent of uncached input; total remains
input plus output. Migration `0060_fleet_attempt_claude_cache_usage.sql` now
enforces that same invariant in Postgres while retaining nonnegative counters,
reasoning/output coherence, positive rows, and exact total arithmetic.

One typed `fleet.dispatchSteerMessage` targeted only the live Codex assignment.
It produced exactly one durable outcome,
`outcome.pylon.fleet_steering.3f0976e141c46c45a208e8aa`, with state
`skipped_stale`; no second delivery or invented effective state was recorded.

## Deployment and publication evidence

The burn exposed three compatibility defects after the deterministic CUT-06
implementation: mixed closeout SQL portability, order-independent proof refs /
projection convergence, and Claude cache-usage truth. The repair stack is:

- `9a0fb3c500` — portable mixed closeout evidence and harness-aware truth;
- `79cc12c9bb` — order-independent closeout refs;
- `d4e69e1509` — bounded closeout projection convergence;
- `e3f9ad11a3` — Claude cache counters accepted by Pylon and Sync schemas; and
- `54934f05f5` — matching Postgres invariant and migration `0060`.

Migration dry-runs and applies completed in staging before production. Both
environments applied pending migrations `0059` and `0060`. Production Cloud Run
revision `openagents-monolith-00084-tnv` serves 100% of traffic; its health,
Sarah tombstone, portal browser smoke, public root document, and referenced
holding image all passed. The exact frozen terminal outbox prefix then replayed
idempotently from sequence 40 through 44, with acknowledgements at every
sequence and final execution state `completed`.

The human authority acknowledged the run in about 2.9 seconds and both named
assignments were accepted about 7.4–8.1 seconds after creation. Local terminal
evidence was durable by 14:31:57Z. Public completion arrived at 14:42:55Z
because this burn discovered and repaired the stale production database
constraint; that delay is recorded as compatibility evidence, not presented as
normal fleet latency.

## Boundary after acceptance

CUT-06 and #8640 close at this receipt. It does not claim Desktop/mobile
two-client control, fault convergence, portable-session movement, managed-cloud
execution, Grok capacity, or sustained dogfood. Those remain in their existing
R0–R7 and remote-workroom issues. The Fable streamlining audit's protected-core
rule governed this burn; broader Pylon cleanup still requires its own bounded
post-proof issue and does not displace the ordered next leaf, CUT-07 #8687.
