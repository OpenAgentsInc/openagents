# Worktree And Workspace Materialization Audit

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-11

This is system #8 from the Bun/Effect terminal-agent systems list. It defines
how OpenAgents should create, retain, clean up, and project isolated task
workspaces for foreground work, background work, delegated loops, and
verification.

## Target

Build a workspace materialization system that can give each task a bounded,
auditable workspace without making the workspace path or branch name the source
of truth.

The system should support:

- Existing local workspace execution.
- Git checkout materialization.
- Native git worktree materialization.
- Temporary fixture workspaces.
- Background task workspaces.
- Retained dirty workspaces for operator inspection.
- Cleanup of unchanged workspaces.
- Public-safe artifact refs for patches, verification commands, and closeouts.

## User-Visible Capability

The user should be able to:

- Launch multiple independent task lanes without workspace collisions.
- See which lanes are running, completed, failed, or cancelled.
- Inspect retained workspaces only when they contain meaningful changes.
- Know when an unchanged workspace was cleaned up.
- Receive stable artifact refs instead of raw local paths in portable records.
- Merge or discard retained task work intentionally.

## Domain Model

Use typed records instead of ad hoc path strings.

- `WorkspaceMaterializationRequest`
  - task ref
  - source ref
  - materialization kind
  - base branch or commit ref
  - read/write policy ref
  - cleanup policy ref
  - verification command refs
- `WorkspaceMaterializationRecord`
  - workspace ref
  - task ref
  - materialization kind
  - local workspace handle
  - branch ref
  - base commit ref
  - created at
  - cleanup state
  - retained artifact refs
- `WorkspaceChangeSummary`
  - changed file refs
  - patch artifact refs
  - verification refs
  - dirty/clean state
  - commit refs when present
- `WorkspaceCleanupReceipt`
  - cleanup decision
  - reason ref
  - retained refs
  - deleted refs
  - generated at

Local paths stay local-only. Public or portable projections use workspace refs,
branch refs, patch refs, and verification refs.

## Materialization Modes

1. `existing_workspace`
   - Use the current working tree.
   - Require explicit policy because edits affect the caller's workspace.
2. `git_checkout`
   - Create a fresh checkout from a public or authorized repository ref.
   - Use for reproducible delegated coding work.
3. `git_worktree`
   - Create an isolated worktree from the active repository.
   - Use for local parallel task lanes.
4. `fixture_workspace`
   - Create deterministic test files under a temp root.
   - Use for CI-safe smokes.
5. `hosted_workspace`
   - Materialize inside a hosted container or remote runner.
   - Project only remote-safe refs back into OpenAgents.

## Cleanup Policy

Cleanup is policy-driven:

- Delete unchanged temporary workspaces.
- Retain dirty or committed workspaces until the user or operator acts.
- Retain workspaces with failed verification when the patch may be useful.
- Retain workspaces when cleanup cannot prove that deletion is safe.
- Delete fixture workspaces after test completion unless a failure fixture asks
  for retention.

Every cleanup decision emits a receipt. A missing cleanup receipt is a blocker,
not a silent success.

## Background Task Integration

Background lanes should use the same materialization service as foreground
work. A task supervisor should own the lifecycle:

1. Allocate task id and workspace ref.
2. Materialize workspace.
3. Start the task under a scoped runtime.
4. Stream typed runtime events.
5. Record artifacts and verification refs.
6. Decide cleanup or retention.
7. Emit completion notification to the parent lane.

The parent lane should receive structured task notifications, not raw child
transcripts. It can then summarize progress, merge evidence, or ask the user
for a decision.

## Projection Rules

Public and portable projections may include:

- task refs
- workspace refs
- branch refs
- base commit refs
- patch artifact refs
- verification command refs
- cleanup receipt refs
- retained-workspace status

They must not include:

- absolute local paths
- raw repository contents
- raw shell logs
- private branch names when policy marks them private
- provider payloads
- credentials or environment dumps

## Effect Boundary

Suggested services:

- `WorkspaceMaterializer`
  - validates request
  - creates workspace
  - returns a scoped record
- `WorkspaceChangeDetector`
  - computes clean/dirty state
  - records patch and commit refs
- `WorkspaceCleanupPolicy`
  - decides retain/delete
  - emits cleanup receipt
- `WorkspaceProjectionService`
  - produces public, operator, and local-only projections
- `TaskWorkspaceRegistry`
  - maps task refs to workspace records

Use `Scope` for lifecycle cleanup, `Effect.acquireRelease` for temp
workspace ownership, and typed errors for unsafe path, dirty workspace,
cleanup failure, and verification failure.

## Tests

Minimum regression coverage:

- Creates a fixture workspace and cleans it up when unchanged.
- Retains a dirty workspace and emits a retained-workspace receipt.
- Prevents nested or escaped workspaces outside allowed roots.
- Produces a public projection with refs and no local path.
- Runs two task workspaces in parallel without branch/path collision.
- Cancels a task and verifies scoped cleanup or retained evidence.
- Replays task notifications into the parent lane without raw transcript data.
- Verifies cleanup idempotency for already-deleted temp workspaces.

## Decision

Workspace materialization should be a first-class service under the runtime
kernel. Background tasks, delegated loops, hosted runners, and local fixture
tests should all consume the same materialization and cleanup contract. The
visible product surface should show task and artifact refs, while local paths
remain local-only diagnostics.
