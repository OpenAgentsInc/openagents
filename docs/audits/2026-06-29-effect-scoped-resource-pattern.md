# Effect scoped resource and retry pattern

Issue: #7013

Date: 2026-06-29

This is the local pattern for migrating long-lived runtime resources toward
Effect without rewriting whole services in one pass.

## Scoped resources

Use `Scope` plus `Effect.addFinalizer`, `Effect.acquireRelease`, or
`Effect.acquireUseRelease` for resources that must be cleaned up on success,
failure, timeout, or fiber interruption:

- subprocess runners: kill the process and drain bounded output in the finalizer;
- WebSocket clients: remove listeners, close the socket, and reject pending
  commands in the finalizer;
- local leases and file locks: release the lease or remove the lock directory in
  the finalizer;
- temporary worktrees: remove assignment-scoped directories in the finalizer;
- long-running assignment runners: close local process markers and submit
  public-safe stale/interrupted closeout refs in the finalizer.

For resources that outlive one method call, create a retained
`Scope.Closeable`, acquire the resource into that scope, and close that scope
from the explicit lifecycle boundary such as `disconnect`, `reconnect`, or
assignment closeout. If acquisition can suspend before the resource is ready,
register the finalizer immediately after construction and close the retained
scope from `Effect.ensuring` when acquisition is interrupted.

## Retry schedules

Keep retry policy named and reusable. New code should import a domain schedule
instead of embedding one-off sleep loops:

- external HTTP/provider calls: short exponential retry, bounded attempts;
- Durable Object calls: tighter retry for transient coordination errors;
- wallet-adjacent calls: conservative retry only after balance/state proof;
- Git/GitHub operations: retry transient lock/network failures, never semantic
  failures such as missing commits;
- D1 transient failures: bounded retry around known transient errors only;
- public projection sync: wider exponential retry because lag is acceptable.

`packages/world-client/src/index.ts` now exposes `WorldClientRetrySchedules` for
the world transport boundary and migrates the browser WebSocket lifecycle to a
retained Effect scope with interruption cleanup coverage.

## Tests

New Effect-aware tests should prefer one Effect program per behavior, per-test
dependency injection, and `Effect.runPromiseExit` or `Fiber.interrupt` when
asserting typed failure and interruption behavior.
