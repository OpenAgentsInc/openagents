# Sandbox And Workspace Boundary Audit

Date: 2026-06-11

This is system #7 from the Bun/Effect terminal-agent systems list. It defines
the authority boundary for files, directories, temporary state, network access,
secrets, process execution, and workspace escape prevention.

## Target

Build one workspace-boundary service that every tool, task, subagent, and
external adapter must use before reading, writing, executing, or exposing local
state.

The boundary should be independent of any single tool. File edits, shell
commands, background tasks, remote adapters, and model context assembly should
all consume the same policy snapshot and produce the same audit events.

## User-Visible Capability

The user should be able to:

- See which workspace is active.
- Add an extra readable or writable directory deliberately.
- Understand why an action was blocked.
- Run read-only exploration without granting mutation.
- Run mutation only inside approved roots.
- Keep secrets and agent configuration out of accidental model context.
- Use temporary directories without broadening the whole system.
- Know whether network access is allowed, denied, or constrained.

The agent should never rely on chat text as the authority boundary. The
boundary must be enforced before the action runs.

## Core Design

Define a `WorkspaceBoundaryService` that normalizes paths, resolves symlinks,
classifies authority, and produces policy decisions.

Suggested service boundary:

```ts
interface WorkspaceBoundaryService {
  snapshot(request: BoundarySnapshotRequest): Effect.Effect<BoundarySnapshot, BoundaryError>
  decide(request: BoundaryDecisionRequest): Effect.Effect<BoundaryDecision, BoundaryError>
  addRoot(update: WorkspaceRootUpdate): Effect.Effect<BoundarySnapshot, BoundaryError>
  explain(decision: BoundaryDecision): Effect.Effect<BoundaryExplanation, never>
}
```

The concrete implementation should use Effect Schema, branded identifiers, and
typed error classes. Raw paths should be normalized into path refs before they
cross service boundaries.

## Policy Model

Represent workspace policy as data:

- Primary workspace root.
- Additional read roots.
- Additional write roots.
- Temporary directories owned by the current run.
- Explicit read deny paths.
- Explicit write deny paths.
- Secret-bearing paths.
- Agent configuration paths.
- Internal runtime storage paths.
- Project metadata paths that need special handling.
- Network allow or deny policy.
- Process execution policy.
- Sandbox availability and mode.
- Prompt availability for interactive escalation.

Policy should separate read, write, execute, network, and expose-to-model
authority. A path that can be read by a local tool is not automatically safe to
show to the model or include in a public receipt.

## Decision Shape

Every boundary decision should include:

- `allow`, `deny`, or `ask`.
- Authority requested: read, write, execute, network, expose, delete, or
  materialize.
- Canonical target ref.
- Policy source.
- Reason code.
- Redaction class.
- Suggested narrower alternative when available.
- Whether the decision can be remembered.

Denies should win over allows. If a path resolves through a symlink, policy
must apply to the final canonical target and preserve the original display path
for user explanation.

## Admission Flow

1. Normalize the requested cwd, path, command, or network target.
2. Resolve symlinks and platform-specific path aliases.
3. Classify the action authority.
4. Check hard denies and secret paths.
5. Check workspace roots and temporary directories.
6. Check additional user-approved roots.
7. Check sandbox and network mode.
8. Ask the permission service only when the action is eligible for user
   escalation.
9. Emit a boundary decision event.
10. Pass only an approved, canonicalized target to the executing service.

This keeps permission prompting and physical containment separate: approval can
grant authority, but it should not bypass non-negotiable escape checks.

## Sandbox Shape

The sandbox should be configured from the boundary snapshot, not separately by
each tool.

Sandbox policy dimensions:

- Read allowlist.
- Write allowlist.
- Read denylist.
- Write denylist.
- Network allowlist or denylist.
- Process restrictions.
- Temporary directory grants.
- Runtime-internal storage grants.
- Fail-closed versus best-effort behavior.
- Violation reporting and post-run inspection.

If a platform sandbox is unavailable, the service should emit that fact and
fall back to deterministic preflight checks. The fallback must be visible in
events and receipts.

## Bun/Effect Boundary

Use these primitives:

- `Effect.Service` for boundary and sandbox policy.
- `Layer` for platform-specific sandbox implementations.
- `Schema` for path refs, authority requests, and decision events.
- `Ref` for session policy state.
- `Scope` for temporary directory lifetimes.
- `Stream` for sandbox violation events.
- `Schedule` for bounded retry of transient platform probes.

Tests should run against a deterministic in-memory filesystem model and a real
filesystem fixture suite.

## Safety Rules

- Deny beats allow.
- Read authority does not imply write authority.
- Local read authority does not imply model-exposure authority.
- Temporary directory authority expires with the run scope.
- Secret-bearing paths require explicit redaction even when locally readable.
- Symlink resolution is mandatory before mutation.
- Workspace metadata and runtime settings need explicit policy treatment.
- Network access should be a separate authority, not hidden inside shell
  approval.
- Background tasks inherit a frozen policy snapshot unless explicitly
  re-authorized.
- Public receipts must use artifact refs, not raw private paths.

## Tests

Minimum regression coverage:

- Normalize relative, absolute, tilde, and platform-specific paths.
- Deny mutation outside the active workspace.
- Allow mutation inside an approved write root.
- Deny a symlink that points outside the write boundary.
- Deny secret and runtime configuration exposure to model context.
- Allow scoped temporary directory writes and revoke after scope close.
- Preserve deny-over-allow precedence.
- Block network access when network mode is disabled.
- Produce a user-readable explanation for each denial reason.
- Emit audit events for allowed, denied, and asked decisions.

## OpenAgents Translation Notes

When this is promoted, map the policy model onto OpenAgents capability refs,
policy refs, artifact refs, and projection-freshness rules. Verify live issue
state before making claims about what is already implemented.

## Decision

Workspace boundaries should be a central typed service, not per-tool custom
logic. Tools may request authority, but one boundary service should normalize,
decide, audit, and explain every local filesystem, process, and network action.
