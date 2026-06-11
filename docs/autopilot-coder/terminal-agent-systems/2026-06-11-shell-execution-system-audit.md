# Shell Execution System Audit

Date: 2026-06-11

This is system #5 from the Bun/Effect terminal-agent systems list. It covers
shell command execution, command parsing, PTY versus non-PTY behavior, progress
streaming, background tasks, sandbox mode, path validation, timeouts,
interruption, and output persistence.

## Target

The shell system should let the agent run real commands while treating shell
execution as a major authority boundary. Commands can read files, mutate the
workspace, use the network, spawn long-running processes, change directories,
or exfiltrate secrets. The shell tool must therefore be stricter than a generic
process wrapper.

## User-Visible Capability

The user should see:

- The command and a plain-language description before or during execution.
- Permission prompts for dangerous or untrusted commands.
- Streaming progress after a short threshold.
- Clear timeout and background-task behavior.
- Output summaries that distinguish no output, expected no output, errors,
  interrupted commands, image output, and large persisted output.
- Background task ids and output paths when work continues after the turn.
- Receipts for exit status, duration, sandbox state, output truncation, and
  artifact refs.

## Core Design

Define shell execution as an Effect service, not a direct tool implementation.

Suggested service boundary:

```ts
interface ShellExecutionService {
  classify(request: ShellClassifyRequest): Effect.Effect<ShellClassification, ShellPolicyError>
  execute(request: ShellExecutionRequest): Stream.Stream<ShellExecutionEvent, ShellExecutionError>
  cancel(taskId: ShellTaskId): Effect.Effect<void, ShellExecutionError>
  background(taskId: ShellTaskId): Effect.Effect<ShellTaskRef, ShellExecutionError>
}
```

The shell tool should become a thin adapter that validates input, asks the
permission service, then delegates to this service.

## Command Input Shape

The model-facing shell input should include:

- `command`
- `description`
- optional timeout
- optional background flag
- optional sandbox override flag

Internal-only fields should never appear in the model-facing schema. If an
approval UI previews an edit or transformation, the post-approval execution
path should consume a sealed internal plan that the model cannot forge.

## Execution Events

The service should emit:

- `shell.command_validated`
- `shell.permission_requested`
- `shell.started`
- `shell.progress`
- `shell.backgrounded`
- `shell.completed`
- `shell.failed`
- `shell.interrupted`
- `shell.output_persisted`
- `shell.cwd_reset`
- `shell.sandbox_violation_detected`

Progress events should include elapsed time, recent output preview, total
lines/bytes, task id if available, and timeout metadata.

## Command Semantics

The shell system should classify command behavior before deciding permission
and UI treatment:

- Read/search/list command.
- Write/edit command.
- Destructive command.
- Network command.
- Long-running command.
- Directory-changing command.
- VCS operation.
- Compound command.
- Command with output redirection.
- Command with unsafe env vars or wrappers.
- Command whose paths cannot be statically validated.

Read-only commands may be concurrency-safe. Mutating or unclassifiable commands
should run serially and ask or deny according to policy.

## Path Validation

The shell path validator should understand common path-bearing commands:

- Directory navigation and listing.
- Reads such as cat/head/tail/stat/file/wc.
- Search such as grep/rg/find.
- Writes such as mkdir/touch/rm/mv/cp.
- Redirections.
- Sed-style edits.
- VCS path operations where statically knowable.

Validation must account for:

- `--` end-of-options.
- Quoting and escaping.
- Globs and path base extraction.
- Pipelines and compound operators.
- Environment-variable prefixes.
- Wrapper commands such as timeout/nohup/time/nice.
- Symlink resolution.
- Dangerous removal paths.
- Commands that change cwd before a path operation.

When validation cannot prove safety, it should ask or deny, not allow.

## Sandbox Model

Sandbox state should be explicit:

- `sandboxed`
- `unsandboxed_by_policy`
- `unsandboxed_by_user_override`
- `sandbox_unavailable`
- `sandbox_failed_closed`
- `sandbox_failed_open_with_prompt`

The shell service should annotate outputs when sandbox policy blocks behavior.
Sandbox override requests should be permission decisions with durable reasons,
not plain boolean flags that bypass the normal approval path.

## Background Tasks

Long-running shell commands need first-class task state:

- Foreground task id.
- Background task id.
- Output file ref.
- Completion notification.
- Cancel/kill capability.
- Reattach/read-output capability.
- Distinguish user-backgrounded, timeout-backgrounded, and agent-policy
  backgrounded.

The runtime should avoid blocking the main agent loop on long-running commands
when a background path is available and safe.

## Output Handling

Shell output should be bounded:

- Stream recent output previews during execution.
- Keep full output in a scoped task output file when needed.
- Persist large final output to an artifact ref with preview and size.
- Cap persisted output size.
- Detect and resize supported image output.
- Interpret non-zero exit codes with command-specific semantics where useful.
- Strip internal hint/protocol side channels before model-visible output.

## Bun/Effect Boundary

Use:

- `Schema` for command input, classification, progress, result, and errors.
- `Effect.Service` for shell execution and task registry.
- `Layer` for bash, PowerShell, fake shell, sandbox, and task output storage.
- `Stream` for progress and final result.
- `Scope` for process handles, output files, polling, and cleanup.
- `Queue` for background task notifications.
- `Schedule` for progress polling and timeout behavior.
- `Ref` for active foreground/background tasks.

## Safety Rules

- Permission happens after validation and before execution.
- Internal execution plans are not model-writable.
- Commands with unvalidated paths ask or deny.
- Deny/ask rules should be harder to bypass than allow rules.
- Dangerous removals require explicit approval and should not generate
  remembered allow suggestions.
- Prefix allow rules must not match compound commands that append unsafe work.
- Environment variables that affect binary resolution or code loading must not
  be stripped for allow matching.
- Background tasks must be killable and discoverable.
- Subagent shell work should not permanently change the parent cwd.
- Output files and previews must respect redaction/public-safety policy.

## Tests

Minimum tests:

- Read-only command classifies as concurrency-safe.
- Mutating command requires permission.
- Compound command with allowed prefix plus unsafe suffix is not auto-allowed.
- Denied command remains denied when prefixed with env vars or wrappers.
- Output redirection path is validated.
- Dangerous removal asks or denies even with broad allow rules.
- Unknown or unparseable path command asks rather than allows.
- Timeout backgrounds allowed long-running command and records task ref.
- User interrupt kills foreground process and records interruption.
- Large output persists to artifact ref with preview.
- Sandbox override produces an approval event.
- Subagent command cannot escape by changing parent cwd.

## Decision

The shell execution system should be a typed, policy-aware process runtime with
command semantics, path validation, sandbox state, background tasks, bounded
output, and cancellation. Treating shell as just another process spawn is not
sufficient for a coding agent.
