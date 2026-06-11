# LSP And Diagnostics System Audit

Date: 2026-06-11

This is system #20 from the Bun/Effect terminal-agent systems list. It defines
how a terminal coding agent should manage language servers, diagnostics, code
navigation, and post-edit feedback without letting editor noise overwhelm the
agent loop.

## Target

Build a language-intelligence service that gives the agent typed code facts:
definitions, references, symbols, hover text, call hierarchy, and diagnostics.

The service should be optional, capability-driven, and safe to ignore when a
language server is missing or unhealthy.

## User-Visible Capability

The user should be able to:

- Ask where a symbol is defined.
- Ask for references or call hierarchy.
- Get type or syntax diagnostics after edits.
- See whether a language service is active.
- Continue working when language services are unavailable.
- Avoid unrelated diagnostics drowning out the current task.
- Get plugin or setup suggestions when a useful language service is missing.

Language intelligence should improve precision without becoming a hard runtime
dependency.

## Core Design

Define a `LanguageIntelligenceService` that owns server lifecycle, file
synchronization, read-only code intelligence operations, and diagnostic
projection.

Suggested service boundary:

```ts
interface LanguageIntelligenceService {
  ensure(request: LanguageServerEnsureRequest): Effect.Effect<LanguageServerHandle, LanguageError>
  query(request: LanguageQueryRequest): Effect.Effect<LanguageQueryResult, LanguageError>
  diagnostics(request: DiagnosticsRequest): Effect.Effect<DiagnosticsResult, LanguageError>
  observe(request: DiagnosticsObserveRequest): Stream.Stream<DiagnosticEvent, LanguageError>
  shutdown(request: LanguageShutdownRequest): Effect.Effect<LanguageShutdownReceipt, LanguageError>
}
```

File editing systems should notify this service about open, change, save, and
close events. The agent loop should consume compact diagnostic summaries, not
raw protocol payloads.

## Server Lifecycle

Use an explicit lifecycle:

- `stopped`
- `starting`
- `running`
- `degraded`
- `stopping`
- `failed`

Each server instance should carry:

- Server id.
- Workspace root.
- Supported language ids and file extensions.
- Launch command or adapter ref.
- Capability set.
- Health state.
- Restart count.
- Last error summary.
- Open file refs.

Server crashes should be recoverable up to a configured limit. One failed
language service should not disable unrelated languages.

## Query Operations

Support a bounded operation set:

- Go to definition.
- Find references.
- Hover.
- Document symbols.
- Workspace symbols.
- Go to implementation.
- Prepare call hierarchy.
- Incoming and outgoing calls.
- Optional code actions when policy allows.

Every operation should validate file path, file size, workspace boundary, and
server capability before sending a request.

## Diagnostics Model

Represent diagnostics as domain records:

- Diagnostic id.
- File ref.
- Range.
- Severity.
- Source.
- Code.
- Message.
- Related information refs.
- First seen timestamp.
- Last seen timestamp.
- Delivered flag.
- Baseline classification.

The agent needs three diagnostic views:

- Current diagnostics for a file.
- New diagnostics since a baseline.
- Pending passive diagnostics worth surfacing.

Baseline comparison is critical after edits: the agent should distinguish
pre-existing repo noise from diagnostics introduced by its own change.

## Context Projection

Diagnostics should feed context carefully:

- Include only relevant files by default.
- Summarize repeated messages.
- Cap diagnostics per file and total diagnostics.
- Prefer new or changed diagnostics over stale ones.
- Preserve exact location data.
- Redact private path prefixes when projecting publicly.
- Attach setup recommendations separately from code facts.

Diagnostics are evidence, not instructions. The model should not get unlimited
raw diagnostics every turn.

## Bun/Effect Boundary

Use these primitives:

- `Effect.Service` for language intelligence.
- `Schema` for server configs, query operations, locations, symbols, and
  diagnostics.
- `Layer` for protocol client implementations and plugin-provided server
  adapters.
- `Stream` for diagnostics notifications.
- `Queue` for passive diagnostic events.
- `Schedule` for startup retries and crash recovery.
- `Scope` for server process lifetime.

All protocol details should stay inside adapters. The runtime should persist
only normalized records and receipts.

## Safety Rules

- Do not start arbitrary language-server commands without policy.
- Do not query files outside the workspace boundary.
- Do not open very large files in language services.
- Do not expose private absolute paths in public projections.
- Do not treat language-service failure as a code failure.
- Do not mix diagnostics from different workspace roots.
- Do not let passive diagnostics interrupt critical user prompts.
- Do not run code actions that mutate files without normal edit permissions.

## Tests

Minimum regression coverage:

- Start a fixture language service for one language.
- Keep unrelated languages working when one service fails.
- Validate path and file-size limits before query execution.
- Query definition, references, hover, symbols, and call hierarchy from
  fixtures.
- Filter locations outside the workspace or ignored paths.
- Record baseline diagnostics before an edit.
- Report only new diagnostics after an edit.
- Deduplicate repeated passive diagnostics.
- Cap diagnostics per file and total diagnostics.
- Restart a crashed server up to the configured limit.
- Shut down all server processes on scope close.

## OpenAgents Translation Notes

When promoted, map language intelligence to OpenAgents capability refs,
artifact refs, verification receipts, policy refs, and private diagnostic
projections. Verify live issue state before claiming language-service behavior
is implemented.

## Decision

Language services should be optional typed adapters. They should provide
bounded code facts and diagnostic deltas, recover from failures, and feed the
agent through compact normalized records rather than raw protocol output.
