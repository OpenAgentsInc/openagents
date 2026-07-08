# Context Assembly System Audit

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-11

This is system #12 from the Bun/Effect terminal-agent systems list. It defines
how the agent should assemble model context from instructions, user input,
workspace state, prior turns, tool outputs, memory, diagnostics, tasks, and
retrieved files.

## Target

Build a context assembler that creates bounded, auditable model-context
snapshots. The model should receive the right information for the step, with
known provenance, priority, redaction, token cost, and freshness.

Context assembly should be a service, not a scattered collection of string
concatenation helpers.

## User-Visible Capability

The user should be able to trust that:

- Project instructions and invariants are considered.
- The current prompt is not buried under stale history.
- Relevant files and tool results are included within budget.
- Private or secret-bearing data is not exposed accidentally.
- Large outputs are summarized, referenced, or truncated predictably.
- Context compaction preserves the state needed to continue safely.
- The agent can explain what kind of context it used when debugging.

## Core Design

Define a `ContextAssembler` service that consumes a context request and emits a
versioned context snapshot plus model-specific projections.

Suggested service boundary:

```ts
interface ContextAssembler {
  assemble(request: ContextRequest): Effect.Effect<ContextSnapshot, ContextError>
  project(snapshot: ContextSnapshot, target: ModelTarget): Effect.Effect<ModelContext, ContextError>
  estimate(request: ContextRequest): Effect.Effect<ContextBudgetEstimate, ContextError>
}
```

The snapshot should be durable enough to audit and replay decisions. The final
provider prompt projection can remain ephemeral because it may contain
provider-specific formatting.

## Context Sources

Represent each input as a typed source:

- System instructions.
- Developer or operator instructions.
- Project instructions.
- Current user turn.
- Prior conversation summary.
- Recent transcript window.
- Tool-call and tool-result pairs.
- Task and progress state.
- Workspace metadata.
- Git or repository status.
- File snippets and search results.
- Diagnostics and test failures.
- User, project, or team memory.
- Retrieved knowledge.
- Artifact summaries.
- External adapter closeouts.

Each source should include provenance, freshness, priority, visibility, token
estimate, and redaction class.

## Priority Model

Use explicit priority tiers:

1. Safety and policy instructions.
2. Current user request.
3. Active task, plan, and approval state.
4. Required tool-use/result consistency.
5. Recently referenced files and artifacts.
6. Project instructions and invariants.
7. Relevant diagnostics and repository state.
8. Retrieved memory and knowledge.
9. Older transcript summary.
10. Low-priority background context.

The assembler should trim or summarize lower-priority context before violating
tool consistency, safety instructions, or current user intent.

## Snapshot Shape

A `ContextSnapshot` should include:

- Snapshot id.
- Run and turn refs.
- Created timestamp.
- Model target.
- Budget estimate.
- Included source refs.
- Excluded source refs with reasons.
- Redaction policy.
- Truncation and summarization records.
- Freshness markers.
- Tool inventory summary.
- Prompt projection checksum or version ref.

This allows debugging without storing raw provider payloads as the canonical
record.

## Assembly Flow

1. Load policy, instruction, and runtime state.
2. Normalize the current user turn and attachments.
3. Gather active plan, task, permission, and approval state.
4. Add required tool-use/result pairs.
5. Pull relevant file snippets, diagnostics, and artifact summaries.
6. Retrieve memory and knowledge through semantic or structured selectors.
7. Estimate token cost and source priority.
8. Apply redaction and exposure policy.
9. Summarize or trim low-priority sources.
10. Emit a snapshot with included and excluded source refs.
11. Project the snapshot into the target model or adapter format.

The projection step should be deterministic for a given snapshot and target.

## Bun/Effect Boundary

Use these primitives:

- `Effect.Service` for context assembly.
- `Schema` for source refs, snapshots, and budget estimates.
- `Layer` for repository, memory, retrieval, and diagnostics providers.
- `Stream` for large source scanning or incremental retrieval.
- `Ref` for per-run context cache.
- `Schedule` for bounded refresh of slow diagnostics.
- `Either` or tagged failures for skipped optional sources.

Optional context sources should fail soft with recorded exclusion reasons.
Mandatory sources should fail the assembly step.

## Redaction And Exposure

Every source should carry exposure metadata:

- Local-only.
- Model-safe.
- Public-safe.
- Operator-only.
- Secret-bearing.
- Raw-log.
- Derived summary.

Model-safe does not mean public-safe. Public receipts should use separate
projections that prefer artifact refs and summaries.

Secret scanners and boundary decisions should run before a source can be marked
model-safe. If classification is uncertain, fail closed or include only a
derived summary.

## Tool And Transcript Consistency

The assembler must preserve tool-use and tool-result pairing. It should not
drop a tool result while retaining the corresponding tool request, or retain a
tool result without enough request context for the model to understand it.

When old turns are compacted, tool pairs should be represented by verified
summary records or omitted together with an explicit boundary event.

## Tests

Minimum regression coverage:

- Assemble a context snapshot with current turn, instructions, files, and tool
  results.
- Preserve required tool-use/result pairs under budget pressure.
- Trim low-priority memory before safety instructions.
- Record excluded sources with reasons.
- Redact a secret-bearing fixture before model projection.
- Distinguish model-safe and public-safe projections.
- Include active task and approval state.
- Summarize large tool output with artifact refs.
- Produce deterministic projection for the same snapshot and target.
- Fail mandatory source load and soft-skip optional source load.

## OpenAgents Translation Notes

When promoted, map sources to OpenAgents capability refs, policy refs,
artifact refs, memory contracts, and projection-freshness requirements. Verify
current issue and roadmap state before making claims about live behavior.

## Decision

Context assembly should produce typed snapshots with provenance, priority,
budget, and redaction metadata. Provider prompts are projections from those
snapshots, not the durable source of truth.
