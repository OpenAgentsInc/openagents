# Hook And Event System Audit

Date: 2026-06-11

This is system #32 from the Bun/Effect terminal-agent systems list. It defines
how a terminal coding agent should run lifecycle hooks, tool hooks, policy
hooks, observability hooks, and extension hooks.

## Target

Build a hook system that turns runtime lifecycle points into typed, ordered,
policy-governed events.

Hooks may observe, block, mutate bounded inputs, add context, request
permission changes, or emit diagnostics. They must not become an untracked
backdoor around the agent's normal authority model.

## User-Visible Capability

The user should be able to:

- Configure hooks at user, project, local, managed, plugin, and session scope.
- Browse hooks by event, matcher, and source.
- See which hooks can block work.
- See hook progress while long hooks run.
- Understand why a hook blocked or modified an action.
- Disable or restrict hooks by policy.
- Use shell, HTTP, model-prompt, agent, callback, or in-memory validation hook
  types where allowed.
- Run doctor checks for invalid hook config.
- Avoid hooks running in untrusted workspaces.

Hooks should be powerful but visible.

## Event Model

Define a typed event enum with categories:

- Before tool execution.
- After tool execution.
- After tool execution failure.
- Permission request.
- Permission denial.
- User prompt submitted.
- Session start and end.
- Main turn stop and stop failure.
- Subagent start and stop.
- Compaction before and after.
- Notification sent.
- Task created, idle, completed, or blocked.
- Elicitation request and result.
- Configuration changed.
- Instruction file loaded.
- Workspace created or removed.
- Current directory changed.
- Watched file changed.

Each event should declare input schema, matcher field, possible hook outputs,
blocking semantics, and whether output can reach the model.

## Hook Types

Support these hook types:

- Shell command hook.
- HTTP hook.
- Prompt hook.
- Agent hook.
- Runtime callback hook.
- In-memory validation hook.

Hook type should determine execution environment and output parsing, not event
semantics. All hook types should return through the same validated result
shape.

## Hook Output Model

Validated hook output may include:

- Continue or stop.
- Blocking reason.
- User-facing system message.
- Additional model context.
- Updated tool input.
- Permission behavior: allow, deny, ask, or passthrough.
- Permission decision reason.
- Updated permission refs.
- Updated remote-tool output where explicitly allowed.
- Initial user message for session-start events.
- File watch paths.
- Retry hint.
- Suppress output flag.
- Async marker and timeout.

Only specific events should accept mutation fields. For example, a notification
hook should not update tool input.

## Core Design

Define a `HookRuntimeService` that owns config loading, matcher resolution,
execution, aggregation, async response tracking, and policy enforcement.

Suggested service boundary:

```ts
interface HookRuntimeService {
  load(request: HookLoadRequest): Effect.Effect<HookCatalog, HookError>
  resolve(request: HookResolveRequest): Effect.Effect<ResolvedHookSet, HookError>
  execute(request: HookExecuteRequest): Effect.Effect<AggregatedHookResult, HookError>
  registerSessionHook(request: SessionHookRegisterRequest): Effect.Effect<SessionHookReceipt, HookError>
  checkAsync(request: AsyncHookCheckRequest): Effect.Effect<AsyncHookResultSet, HookError>
  finalize(request: HookFinalizeRequest): Effect.Effect<HookFinalizeReceipt, HookError>
}
```

The agent loop should call hooks through this service at explicit lifecycle
points. Hook implementations should not be called directly from tool code.

## Ordering And Scope

Hook scope priority should be explicit:

- Managed policy hooks.
- Local session hooks.
- Local settings hooks.
- Project settings hooks.
- User settings hooks.
- Plugin hooks.
- Built-in runtime hooks.

The exact order can differ, but it must be documented and tested. Duplicate
settings files should be deduplicated by resolved identity. Managed policy may
disable all non-managed hooks or allow only managed hooks.

## Execution Semantics

Hook execution should support:

- Per-hook timeout.
- Overall event timeout where needed.
- Abort signal composition.
- Parallel execution when order does not matter.
- Sequential execution when mutation ordering matters.
- Async hooks that return later with progress.
- Shell output capture.
- HTTP allowlist enforcement.
- Environment interpolation only from allowed variables.
- SSRF protection for HTTP hooks.
- Sandbox network routing where applicable.
- Transcript-safe output handling.

Hook failures should be isolated. A crashing hook should produce a typed
non-blocking error unless the event contract says otherwise.

## Aggregation

Aggregated results should preserve:

- Blocking errors.
- First or highest-priority stop reason.
- Additional context list.
- Updated input merge result.
- Permission decision and reason.
- User-visible messages.
- Async hook refs.
- Watch path updates.
- Retry hints.
- Hook source metadata.

Aggregation rules must be deterministic. If two hooks attempt conflicting
mutations, the system should choose by documented priority or reject the
conflict.

## Bun/Effect Boundary

Use these primitives:

- `Effect.Service` for hook runtime operations.
- `Schema` for events, inputs, hook configs, outputs, and aggregate results.
- `Layer` for shell, HTTP, model, agent, callback, settings, and policy
  executors.
- `Stream` for hook progress and async responses.
- `Queue` for async hook polling and notification injection.
- `Ref` for pending async hook registry and session hooks.
- `Schedule` for timeout, polling, and debounce.
- `Scope` for child process and HTTP request cleanup.

Every hook input and output should be schema-validated at the boundary.

## Safety Rules

- Do not run workspace hooks before workspace trust.
- Do not let hooks execute when policy disables them.
- Do not allow HTTP hooks outside URL allowlists when allowlists are set.
- Do not interpolate environment variables unless explicitly allowed.
- Do not follow HTTP redirects for hook calls unless policy allows them.
- Do not let hook output mutate fields unsupported by that event.
- Do not let a hook grant permissions outside policy refs.
- Do not let async hooks leak after session end.
- Do not include secrets in hook progress, logs, or public receipts.
- Do not let hook failures orphan child processes.

## Tests

Minimum regression coverage:

- Load hooks from all supported scopes.
- Group hooks by event and matcher.
- Sort and execute hooks by documented priority.
- Skip hooks before workspace trust.
- Enforce managed-only and disable-all policy.
- Execute shell, HTTP, prompt, agent, callback, and validation hooks.
- Parse sync output and async output.
- Track async hook progress and final response.
- Apply per-hook timeout and cancellation.
- Enforce HTTP URL and environment allowlists.
- Block SSRF-prone HTTP destinations.
- Aggregate blocking, context, permission, and input mutation results.
- Resolve conflicting mutations deterministically.
- Cleanup pending hooks on shutdown.

## OpenAgents Translation Notes

When promoted, map hook events to OpenAgents runtime events, policy refs,
approval refs, artifact refs, private diagnostics, and public-safe receipts.
Verify live issue state before claiming hook/event behavior is implemented.

## Decision

Hooks should be a typed event runtime, not arbitrary callbacks scattered
through the codebase. The agent should centralize hook execution, validate
inputs and outputs, enforce policy, and make every blocking or mutating hook
observable.
