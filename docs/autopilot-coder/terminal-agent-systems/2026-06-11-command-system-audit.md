# Command System Audit

Date: 2026-06-11

This is system #23 from the Bun/Effect terminal-agent systems list. It defines
how a terminal coding agent should expose slash commands, command-palette
entries, prompt shortcuts, local actions, interactive panels, and agent-runtime
composition.

## Target

Build a command system that treats commands as typed runtime capabilities, not
as loose string handlers.

Commands should be discoverable, permission-aware, testable, and composable
with the main agent loop. A command may run entirely in the terminal UI, inject
messages into the conversation, call a local service, open a modal, or produce
a prompt for the model runtime.

## User-Visible Capability

The user should be able to:

- Type slash commands or open a command palette.
- See available commands with descriptions and argument hints.
- Invoke built-in, workspace-defined, plugin-defined, and workflow-defined
  commands from one list.
- Pass arguments without fragile parsing.
- Use aliases where they are intentionally exposed.
- Run noninteractive commands from scripts when marked safe.
- Open interactive command panels for settings, help, history, diagnostics,
  and review workflows.
- See when a command is hidden, disabled, unavailable, or blocked by policy.
- Keep command history separate from normal prompt history where useful.

The command surface should be fast enough to refresh on each prompt render so
auth, settings, workspace, and policy changes are reflected without restarting.

## Command Families

Use three abstract command families:

- Prompt command: expands user intent into a model-facing prompt or workflow.
- Local command: performs a bounded local action and returns a text, compact,
  or skipped transcript result.
- Interactive command: opens a terminal panel or modal and reports completion
  back to the shell.

All command families should share a common command descriptor:

- Stable id.
- User-facing name.
- Optional aliases.
- Description.
- Argument schema or hint.
- Visibility predicate.
- Availability predicate.
- Noninteractive support flag.
- Invocation policy.
- Runtime family.
- Origin label.
- Version.
- Feature gates.
- Sensitivity flag.

The descriptor is public-safe. The command implementation is not part of help
or session exports unless explicitly allowed.

## Core Design

Define a `CommandRegistryService` that owns discovery, filtering, resolution,
and invocation planning.

Suggested service boundary:

```ts
interface CommandRegistryService {
  discover(request: CommandDiscoverRequest): Effect.Effect<CommandCatalog, CommandError>
  resolve(request: CommandResolveRequest): Effect.Effect<CommandResolution, CommandError>
  plan(request: CommandInvocationRequest): Effect.Effect<CommandInvocationPlan, CommandError>
  run(request: CommandRunRequest): Effect.Effect<CommandRunResult, CommandError>
  clearCaches(scope: CommandCacheScope): Effect.Effect<void, CommandError>
}
```

Discovery should return descriptors. Invocation should happen through a plan
that names the command family, permissions, runtime effects, and transcript
display result.

## Registry Inputs

The registry should merge command descriptors from:

- Built-in command modules.
- Workspace-defined workflow files.
- User-defined command files.
- Installed extension manifests.
- Bundled skills or recipes.
- Remote-control adapters with explicit allowlists.

Discovery failures from optional inputs should produce diagnostics and skip the
broken input. They should not make the whole command system unavailable.

Command names should be deduplicated deterministically. Prefer local,
workspace-specific commands only when the override is explicit and visible.

## Resolution And Filtering

Resolution should support:

- Exact name.
- Slash-prefixed name.
- Alias.
- User-facing display name.
- Case policy defined by schema.
- Argument validation before execution.

Filtering should apply in this order:

1. Runtime mode constraints.
2. Policy and permission constraints.
3. Feature flags and capability refs.
4. Visibility rules.
5. User-invocable versus model-invocable rules.
6. Noninteractive support.

Remote and bridge-like modes should have explicit allowlists. Interactive
commands should be blocked from noninteractive channels unless a command has a
purpose-built remote-safe adapter.

## Execution Semantics

Invocation should produce one of these outcomes:

- Transcript message.
- System-only status.
- Prompt injection into the next agent turn.
- Modal or panel state.
- Background task.
- No transcript output.
- Failure with typed reason.

Interactive commands should support:

- Lazy loading.
- Cancellation through an abort signal.
- Returning system messages.
- Asking the shell to query the model after completion.
- Restoring a stashed prompt.
- Emitting public-safe metadata messages.
- Declaring whether completion should produce a notification.

Prompt commands should be able to provide allowed tool scopes, model effort,
context behavior, and workflow metadata without bypassing policy.

## Argument Model

Avoid ad hoc string parsing for rich arguments.

Use:

- Schema-validated positional arguments for simple commands.
- Structured option parsing for flags.
- Deterministic parsers for bounded fields such as ids, file paths, amounts,
  and dates.
- Typed command-specific input forms for complex interactive commands.

Arguments that may include user prose should remain plain text and should not
be parsed for intent by keywords after command resolution.

## Bun/Effect Boundary

Use these primitives:

- `Effect.Service` for registry discovery and invocation planning.
- `Schema` for descriptors, arguments, plans, results, and errors.
- `Layer` for built-in, workspace, user, extension, and remote command
  providers.
- `Cache` for command catalogs with explicit invalidation.
- `Ref` for current runtime mode and command-palette state.
- `Queue` for command invocation events.
- `Stream` for command-output events when a command is long-running.
- `Scope` for interactive command lifecycle and cleanup.

Command implementations should depend on services, not globals. That keeps the
same command available in terminal, headless, and test runners when policy
allows it.

## Safety Rules

- Do not let a hidden command become invocable through an alias.
- Do not let model-invoked commands run when marked user-only.
- Do not allow noninteractive use unless a command explicitly supports it.
- Do not let command descriptors include secrets or private paths.
- Do not let optional command-provider failures crash the registry.
- Do not execute a remote command unless it is in the current channel
  allowlist.
- Do not let prompt commands expand allowed authority beyond policy refs.
- Do not treat display text as an executable command id.
- Do not let command completion submit a prompt unless the command explicitly
  requests it.

## Tests

Minimum regression coverage:

- Discover built-in, workspace, user, extension, and workflow commands.
- Skip broken optional providers while surfacing diagnostics.
- Deduplicate name collisions deterministically.
- Resolve exact names, slash names, display names, and aliases.
- Validate arguments and reject malformed input with actionable errors.
- Filter commands by runtime mode, policy, feature gates, visibility, and
  noninteractive support.
- Run prompt, local, and interactive command families.
- Cancel an interactive command and restore the prompt.
- Preserve command history separately from normal prompt history.
- Block user-only commands from model invocation.
- Block interactive commands from noninteractive channels by default.
- Clear registry caches after extension or workflow changes.

## OpenAgents Translation Notes

When promoted, map commands to OpenAgents capability refs, policy refs,
operator UX actions, Autopilot workflow entries, and public-safe receipts.
Verify live issue state before claiming command-palette, extension-command, or
noninteractive command behavior is implemented.

## Decision

The command system should be a typed registry and invocation planner. It should
make command authority, visibility, runtime family, and transcript effects
explicit so terminal, headless, and remote-control surfaces can share one safe
shape.
