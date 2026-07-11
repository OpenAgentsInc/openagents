# CUT-06 Fleet supervisor ordering receipt

Date: 2026-07-11

Issue: [#8686](https://github.com/OpenAgentsInc/openagents/issues/8686)

Implementation: `d98abda795`

Status: deterministic implementation accepted; named-account parent receipt
still pending

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

## Remaining live rung

This receipt does not close CUT-06 or #8640. The leaf close rule still requires
one owner-authorized production-path FleetRun with simultaneous named `codex`
and named Claude execution, two accepted useful closeouts, the exact pinned
verifiers, no default-home or provider substitution, and one durable typed
steer or approval round trip. That run must use the authenticated human start
authority plus the registered-Pylon claim/execution adapters; a local fixture or
rejected attempt is not a substitute.
