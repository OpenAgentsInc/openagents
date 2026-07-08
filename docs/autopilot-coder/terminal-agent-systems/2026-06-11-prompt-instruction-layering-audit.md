# Prompt And Instruction Layering Audit

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-11

This is system #16 from the Bun/Effect terminal-agent systems list. It defines
how system prompts, developer instructions, project instructions, user memory,
agent definitions, skills, command prompts, output styles, and runtime mode
prompts should be layered without losing precedence or auditability.

## Target

Build an instruction system that makes prompt precedence explicit. The runtime
should know which instruction source won, which sources were appended, which
were skipped, and which policy boundaries cannot be overridden.

Prompt assembly should be deterministic for a given run, model target, tools,
workspace, mode, and instruction snapshot.

## User-Visible Capability

The user should get behavior that respects:

- Global safety and policy instructions.
- Workspace instructions.
- User preferences.
- Current prompt.
- Active agent or mode.
- Skill and command instructions.
- Output style.
- Tool and provider limitations.

When behavior is surprising, the agent should be able to explain which
instruction source was active without exposing private prompt internals.

## Core Design

Define an `InstructionLayeringService` that produces an instruction snapshot
for each model step.

Suggested service boundary:

```ts
interface InstructionLayeringService {
  assemble(request: InstructionRequest): Effect.Effect<InstructionSnapshot, InstructionError>
  project(snapshot: InstructionSnapshot, target: ModelTarget): Effect.Effect<SystemPromptProjection, InstructionError>
}
```

The context assembler should consume the snapshot. The model gateway should
only receive the final provider projection.

## Layer Taxonomy

Use typed instruction layers:

- Runtime policy.
- Product default.
- Mode instruction.
- Agent instruction.
- Command instruction.
- Skill instruction.
- Tool instruction.
- Provider capability instruction.
- Workspace instruction.
- Local private instruction.
- User global instruction.
- Team or organization instruction.
- Memory-derived instruction.
- Append-only operator instruction.
- Override instruction for special execution modes.

Each layer should include source, scope, precedence, visibility, freshness,
token estimate, and redaction class.

## Precedence Model

Define precedence as policy, not convention:

1. Non-overridable runtime safety policy.
2. Developer or operator instruction.
3. Explicit execution-mode override.
4. Active coordinator or mode instruction.
5. Active agent instruction.
6. Custom system prompt.
7. Product default prompt.
8. Workspace and project instructions.
9. User and team memory instructions.
10. Skill and command additions.
11. Append-only prompts.
12. Output style and formatting preferences.

Some modes replace lower layers; others append. Replacement and append behavior
must be recorded in the snapshot.

## Frontmatter And Metadata

Markdown-defined commands, skills, agents, or memories can carry metadata:

- Description.
- Allowed tools.
- Model preference.
- Skills to preload.
- User-invocable flag.
- Hooks.
- Effort or reasoning setting.
- Inline versus forked execution.
- Conditional path globs.
- Shell preference for command blocks.
- Memory type.

Parse metadata with a structured parser. Invalid metadata should degrade with a
warning and not crash prompt assembly unless the field is required.

## Snapshot Shape

An `InstructionSnapshot` should include:

- Snapshot id.
- Run and turn refs.
- Layer records.
- Applied precedence decisions.
- Skipped layers and reasons.
- Replaced layers and replacement source.
- Appended layers.
- Tool and model capability deltas.
- Token estimate.
- Redaction policy.
- Projection checksum or version.

This lets resume, side queries, background tasks, and tests rebuild the same
effective prompt.

## Bun/Effect Boundary

Use these primitives:

- `Effect.Service` for instruction assembly.
- `Schema` for instruction layers, frontmatter, and snapshots.
- `Layer` for workspace, memory, skill, command, and provider metadata
  sources.
- `Cache` for expensive instruction discovery.
- `Stream` for instruction-loaded events.
- `Ref` for per-session prompt overrides.

Instruction discovery should be cancellable and observable, but the final
snapshot should be immutable for a model step.

## Safety Rules

- Runtime safety policy cannot be overridden by project or memory files.
- User-visible project instructions should not silently override managed
  policy.
- Private local instructions must not leak into public receipts.
- Prompt overrides should clearly record that they replaced default behavior.
- Skill or command metadata cannot grant tools or permissions beyond policy.
- Conditional path rules must be evaluated against normalized workspace paths.
- Instruction snapshots should be refreshed after compaction, workspace switch,
  settings changes, and memory edits.
- Provider prompts are projections, not the durable source of truth.

## Tests

Minimum regression coverage:

- Assemble default instructions with workspace and user layers.
- Override lower layers with an execution-mode prompt.
- Append an operator instruction without replacing policy.
- Apply an agent instruction in replace mode and append mode.
- Parse valid frontmatter metadata.
- Degrade invalid optional frontmatter safely.
- Match conditional path instructions after path normalization.
- Refresh instruction snapshot after memory cache invalidation.
- Redact private layers from public projection.
- Rebuild the same provider prompt from the same snapshot.

## OpenAgents Translation Notes

When promoted, map instruction layers to OpenAgents policy refs, capability
refs, memory contracts, adapter refs, and public/private projections. Verify
current issue and roadmap state before claiming a prompt path is live.

## Decision

Instruction layering should be a typed precedence system with immutable
snapshots. The runtime should never depend on scattered prompt concatenation as
the only record of what instructions were active.
