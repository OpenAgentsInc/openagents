# ADR-0007: Tool Execution Contract (Adapters vs Runtime vs Refine)

## Status

**Proposed**

## Date

2026-01-13

## Context

OpenAgents uses three layers that are often conflated:
- **Adapters** (LM I/O formatting/parsing)
- **Execution runtime** (tool validation, tool execution, retries, receipts)
- **Meta-operators** like `Refine` (retry/selection policy for LM calls)

We need clear boundaries to prevent:
- tool schema validation duplicated in adapters,
- retry logic split unpredictably between layers,
- missing receipts/replay events,
- "maybe it worked" failure modes.

## Decision

**Adapters serialize/parse only. The execution runtime owns validation + tool execution + retries + receipts. `Refine` applies to LM calls, not tool execution.**

### Responsibilities (Normative)

#### Adapters
Adapters MUST:
- Render signature inputs into provider format
- Parse provider outputs into typed fields (or raise parse errors)
- Normalize provider-specific tool-call conventions (format only)

Adapters MUST NOT:
- Validate tool params against schemas
- Execute tools
- Implement retry policies (except purely parse-level signaling; retries happen elsewhere)

#### Execution Runtime
Runtime MUST:
- Validate tool params against tool JSON schemas **before execution**
- Execute tools deterministically (bounded output, timeouts, clear errors)
- Emit:
  - replay events (`ToolCall`, `ToolResult`) per REPLAY.md
  - receipt fields (`params_hash`, `output_hash`, `latency_ms`, side_effects) per ARTIFACTS.md / ADR-0002
- Apply retry/circuit-breaker policy for tool execution (not adapters)

#### Refine (and other predictors/operators)
Refine MUST:
- Wrap LM/predictor calls with retries, best-of-N, reward functions
- Never "retry a tool" by re-running it directly; tool retries are runtime-governed
- When used for tool selection signatures, it may retry *selection*, not execution

## Scope

What this ADR covers:
- Clear separation of adapter vs runtime vs refine
- Canonical location of tool schema validation
- Required logging/receipt emission around tool execution

What this ADR does NOT cover:
- Specific tool schemas (TOOLS.md)
- Signature design for ToolCall/ToolResult (SIGNATURES.md)
- Marketplace job execution (NIP-90) beyond "tools are logged and receipted"

## Invariants / Compatibility

| Invariant | Guarantee |
|-----------|-----------|
| Adapters do not validate/retry | Stable constraint |
| Runtime validates tool params | Stable constraint |
| Hashing on full outputs | Stable constraint (see ADR-0006) |
| Tool execution emits replay + receipt | Stable constraint |
| Tool failures are deterministic | Stable constraint |

Backward compatibility:
- If existing code violates this separation, fix is treated as implementation work unless it changes public contracts.

## Consequences

**Positive:**
- One place to reason about safety, retries, receipts, and tool correctness
- Prevents silent tool hallucination and inconsistent validation

**Negative:**
- Requires refactors where adapters currently do "too much"

**Neutral:**
- Some metrics (e.g., ToolParamsSchemaMetric) remain evaluators, not enforcers

## Alternatives Considered

1. **Adapters validate tool params** — rejected (mixes concerns; duplicates runtime enforcement).
2. **Refine retries tools** — rejected (breaks replayability and deterministic accounting).
3. **Each product crate defines its own tool policy** — rejected (fragmentation and drift).

## References

- [GLOSSARY.md](../../GLOSSARY.md) — Adapter vs Execution Runtime definitions
- [crates/dsrs/docs/ADAPTERS.md](../../crates/dsrs/docs/ADAPTERS.md)
- [crates/dsrs/docs/TOOLS.md](../../crates/dsrs/docs/TOOLS.md)
- [crates/dsrs/docs/REPLAY.md](../../crates/dsrs/docs/REPLAY.md)
- [crates/dsrs/docs/ARTIFACTS.md](../../crates/dsrs/docs/ARTIFACTS.md)
- `crates/adjutant/src/tools.rs`, `crates/rlm/src/tools/*` (tool implementations)
- `crates/dsrs/src/adapter/*`, `crates/dsrs/src/predictors/refine.rs` (adapter + refine implementations)
