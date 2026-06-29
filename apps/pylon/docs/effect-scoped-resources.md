# Effect Scoped Resources

Pylon long-running runtime code should prefer Effect-scoped resources for
handles that must be released on success, failure, or interruption:

- subprocesses and streamed agent SDK sessions;
- WebSocket or SSE clients;
- local assignment leases, file locks, and workspace leases;
- temporary git worktrees and materialized checkouts.

Use `Effect.acquireRelease` for new scoped APIs. The acquire step should create
the local handle and write any lease/projection record; the release step should
use the same cleanup path as normal closeout so interruption and explicit
closeout cannot drift.

The workspace materializer exposes the local pattern:

- `materializeGitCheckoutWorkspaceWithLease(...)` is the legacy Promise API;
- `scopedMaterializedGitCheckoutWorkspace(...)` is the Effect API for new
  assignment runners and tests;
- `WORKSPACE_GIT_LOCK_RETRY_POLICY` is the named bounded retry policy for
  transient git lock collisions.

Tests for new scoped resources should run the resource inside `Effect.scoped`,
interrupt or close the scope, and assert that the local cleanup record changed
state. For expected typed failures, prefer `Effect.runPromiseExit` assertions so
tests cover the error channel instead of only thrown Promise exceptions.
