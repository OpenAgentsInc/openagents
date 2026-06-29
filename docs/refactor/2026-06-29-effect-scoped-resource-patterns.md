# Effect scoped resource patterns

Issue: #7013

Use Effect `Scope` / `acquireRelease` for runtime resources whose cleanup must
run on success, failure, or interruption. In this repo that includes:

- subprocess runners and long-running assignment runners,
- WebSocket clients and stream subscriptions,
- local leases, process locks, and file locks,
- temporary checkouts and worktrees.

The Pylon reference helper is `apps/pylon/src/effect-runtime-patterns.ts`.
Prefer `scopedResource(acquire, release)` when a lower-level API needs to
return a resource effect, and `withScopedResource(acquire, release, use)` when a
caller can keep the full use-site inside one scoped effect. Promise-facing
entry points may bridge with `Effect.runPromise`, but the acquire/release pair
should remain in Effect so interruption closes the scope.

Retry and timeout policy should be named at the boundary instead of encoded as
ad hoc loops. The same helper module names reusable schedules for:

- Git and GitHub operations,
- external HTTP/provider calls,
- Durable Object calls,
- wallet-adjacent calls,
- public projection sync.

Tests for new Effect services should use per-test layers for stateful
dependencies and `Effect.runPromiseExit` for expected typed failures. Use
`TestClock` or bounded injected schedules for time-sensitive behavior so tests
do not depend on wall-clock sleeps.
