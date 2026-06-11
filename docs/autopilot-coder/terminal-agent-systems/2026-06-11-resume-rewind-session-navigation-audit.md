# Resume, Rewind, And Session Navigation Audit

Date: 2026-06-11

This is system #26 from the Bun/Effect terminal-agent systems list. It defines
how a terminal coding agent should persist transcripts, list sessions, resume
work, fork prior conversations, rewind to checkpoints, restore workspace state,
and recover from corrupted session records.

## Target

Build session navigation as a durable state system, not a transcript browser
bolted onto the UI.

The agent should be able to restore enough session state to continue safely,
while making clear which state is durable, which state is reconstructed, and
which state cannot be restored.

## User-Visible Capability

The user should be able to:

- List recent sessions for the current workspace.
- Search sessions by title, content summary, branch, issue, or date.
- Resume a prior session.
- Fork a prior session into a new line of work.
- Rewind to a previous user message or checkpoint.
- Choose whether rewind restores conversation only, files only, or both.
- See when a session belongs to another workspace.
- Export or share a public-safe session summary.
- Recover gracefully when transcript files are partial or malformed.

Session navigation should make destructive restore choices explicit.

## Durable Session Model

Persist a typed session log:

- Session id.
- Workspace ref.
- Created and updated timestamps.
- Runtime mode.
- Model/provider refs.
- User messages.
- Assistant messages.
- Tool-call events.
- Tool-result events.
- Approval events.
- Task refs.
- Cost and token summaries.
- File-history checkpoints.
- Context-compaction snapshots.
- Agent and workflow configuration refs.
- Public-safe title and summary.
- Corruption and repair metadata.

Do not assume the rendered transcript is enough to resume. Resume needs the
runtime state that affects future tool calls, policies, and context assembly.

## Core Design

Define a `SessionNavigationService` that owns listing, searching, resuming,
forking, rewinding, exporting, and repair.

Suggested service boundary:

```ts
interface SessionNavigationService {
  list(request: SessionListRequest): Effect.Effect<SessionListResult, SessionNavigationError>
  search(request: SessionSearchRequest): Effect.Effect<SessionSearchResult, SessionNavigationError>
  resume(request: SessionResumeRequest): Effect.Effect<SessionResumePlan, SessionNavigationError>
  fork(request: SessionForkRequest): Effect.Effect<SessionForkReceipt, SessionNavigationError>
  rewind(request: SessionRewindRequest): Effect.Effect<SessionRewindReceipt, SessionNavigationError>
  export(request: SessionExportRequest): Effect.Effect<SessionExportReceipt, SessionNavigationError>
  repair(request: SessionRepairRequest): Effect.Effect<SessionRepairReceipt, SessionNavigationError>
}
```

The service should return a plan before mutating active runtime state. The UI
can render the plan and ask for confirmation when files or policies are
affected.

## Resume Semantics

Resume should restore:

- Durable messages and compacted context snapshots.
- Active workspace ref and repository identity.
- Runtime mode.
- Agent and workflow refs.
- Cost accounting where available.
- File-history state.
- Read-file cache refs where still valid.
- Background task summaries, not necessarily running processes.
- Session metadata and title.

Resume should not silently restore:

- Stale process handles.
- Expired approvals.
- Secrets.
- Terminal-only focus state.
- External service sessions that need fresh auth.
- Workspace paths that no longer exist.

When a session points to another workspace, the system should either open a
clear handoff command or require the user to switch context explicitly.

## Fork Semantics

Forking should:

- Create a new session id.
- Preserve chosen message history.
- Record parent session ref.
- Preserve or replace workspace ref according to policy.
- Mark inherited summaries and checkpoints.
- Avoid mutating the original session.
- Make future receipts point to the fork, not the parent.

Forks are useful for alternative approaches, parallel issue work, or replaying
a saved state with changed model settings.

## Rewind Semantics

Rewind should treat conversation and file state separately.

Supported modes:

- Conversation only.
- Files only.
- Conversation and files.
- Summarize from selected point.
- Summarize up to selected point.
- Cancel.

When file history exists, show diff stats before restoring. When file history
does not exist, the UI must say that only conversation restore is available.

Rewind should cancel active generation before mutating state.

## Corruption Recovery

Session loading should tolerate:

- Partial JSON lines.
- Missing tool result events.
- Unknown future event versions.
- Missing file checkpoints.
- Deleted workspace directories.
- Truncated summaries.
- Unreadable transcript files.

Recovery should produce an explicit repair receipt and should preserve the raw
record out of band where policy allows.

## Bun/Effect Boundary

Use these primitives:

- `Effect.Service` for session navigation operations.
- `Schema` for session events, snapshots, plans, receipts, and repair records.
- `Stream` for progressive session listing and transcript loading.
- `Layer` for session store, file-history store, git provider, and renderer.
- `Ref` for selected session, visible list, and rewind selection.
- `Queue` for navigation actions.
- `Scope` for temporary export and restore operations.

Use append-only event storage for the main transcript where practical. Derived
session indexes should be rebuildable.

## Safety Rules

- Do not restore files without explicit user confirmation.
- Do not reuse expired approvals after resume.
- Do not resume into a different workspace silently.
- Do not overwrite the active prompt without stashing or confirmation.
- Do not hide corrupted transcript recovery from the user.
- Do not include private transcript content in public exports by default.
- Do not treat background task processes as alive after resume unless verified.
- Do not let rewind produce a transcript that falsely claims skipped work ran.
- Do not delete parent sessions during fork.

## Tests

Minimum regression coverage:

- List current-workspace sessions and optionally all-workspace sessions.
- Search sessions by title, content summary, date, issue ref, and branch ref.
- Resume a session with messages, summaries, cost state, and metadata.
- Refuse or hand off cross-workspace resume.
- Fork a session without mutating the parent.
- Rewind conversation only.
- Rewind files only from a checkpoint.
- Rewind both conversation and files with diff confirmation.
- Cancel active generation before restore.
- Load partial or malformed session records with repair receipts.
- Export a public-safe summary without private payloads.
- Rebuild session index from append-only records.

## OpenAgents Translation Notes

When promoted, map sessions to OpenAgents workroom refs, artifact refs,
workspace refs, task refs, projection freshness state, and public-safe closeout
receipts. Verify live issue state before claiming resume, fork, rewind, or
session export behavior is implemented.

## Decision

Resume and rewind should be explicit state transitions over typed session logs.
The agent should restore durable state carefully, distinguish conversation from
file state, and make forks and repairs visible so resumed work remains honest.
