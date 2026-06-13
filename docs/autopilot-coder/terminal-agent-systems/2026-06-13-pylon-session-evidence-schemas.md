# Pylon Session Evidence Schemas

Date: 2026-06-13

This reference enumerates the typed evidence schema ids used by the Pylon
multi-session runner and control-session runner. Field lists below are limited
to refs, digests, and retained aggregate evidence fields.

Source files:

- `apps/pylon/scripts/multi-session-run.ts`
- `apps/pylon/src/node/control-sessions.ts`

## Multi-Session Schemas

### `openagents.pylon.multi_session_plan.v0.1`

This schema id is exported as `MULTI_SESSION_PLAN_SCHEMA`, but
`multi-session-run.ts` does not currently write a retained plan artifact with
this schema. The parsed plan input accepts these ref-shaped selectors:

- `accountRef`
- `repoRef`

### `openagents.pylon.multi_session_heartbeat.v0.1`

Emitted as JSONL heartbeat records.

- `runRef`
- `sessionRef`

### `openagents.pylon.multi_session_summary.v0.1`

Emitted as `multi-session-summary.json`.

- `runRef`
- `runIdRef`
- `concurrency`
- `totalSessions`
- `completedCount`
- `failedCount`
- `totalDurationMs`
- `totalTokens`
- `artifactRefs`
- `heartbeatRef`
- `outcomes`
- `outcomes[].sessionRef`
- `outcomes[].account`
- `outcomes[].account.accountRefHash`
- `outcomes[].workspaceRef`
- `outcomes[].resultRef`
- `outcomes[].errorDigestRef`
- `outcomes[].durationMs`
- `deviations`

### `openagents.pylon.multi_session_failure.v0.1`

Emitted as per-session failure JSON, and as a quarantine JSON object if a
failure payload does not pass the redaction scan.

- `sessionRef`
- `account`
- `account.accountRefHash`
- `workspaceRef`
- `errorDigestRef`
- `violationRefs`
- `artifactDigestRef`

## Control-Session Schemas

### `openagents.pylon.control_session_event.v0.1`

Emitted to the in-memory event list and server-sent event stream.

- `sessionRef`
- `account`
- `account.accountRefHash`
- `workspaceRef`
- `messageRef`
- `artifactRef`
- `resultRef`
- `errorDigestRef`
- `violationRefs`

### `openagents.pylon.control_session_artifact.v0.1`

Emitted as the retained control-session proof artifact.

- `sessionRef`
- `account`
- `account.accountRefHash`
- `workspaceRef`
- `task`
- `task.objectiveDigestRef`
- `task.verifyRef`
- `executor`
- `executor.executionPathRef`
- `executor.eventCount`
- `executor.commandCount`
- `executor.editedFileCount`
- `executor.totalTokens`
- `executor.externalSessionRef`
- `executor.responseDigestRef`
- `devCheck`
- `redactionScan`
- `redactionScan.patternRefs`
- `deviations`

### `openagents.pylon.control_session_failure.v0.1`

Emitted as the retained control-session failure artifact.

- `sessionRef`
- `account`
- `account.accountRefHash`
- `workspaceRef`
- `errorDigestRef`
- `redactionScan`
- `redactionScan.patternRefs`

## Redaction Guarantees

- Account identity appears only as hashed `accountRef`/`accountRefHash`.
- Workspace identity is retained as refs.
- No raw credential paths, tokens, or prompts are retained.
- Every retained artifact passes `scanProofSerialization`.
