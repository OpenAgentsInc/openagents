# Tools (Schema + Execution Contract)

This doc defines the canonical requirements for tools used by OpenAgents runtimes.

Normative references:
- `docs/adr/ADR-0007-tool-execution-contract.md` (runtime vs adapters vs refine)
- `docs/execution/ARTIFACTS.md` (receipt fields)
- `docs/execution/REPLAY.md` (ToolCall/ToolResult events)

## Tool Definition Requirements

Each tool MUST have:
- A stable `tool` name (string id).
- A JSON schema for params.
- Deterministic failure modes (bounded errors, no silent partial success).
- Bounded outputs (truncate/limit where necessary).
- A timeout.

## Runtime Requirements

The execution runtime MUST:
- Validate tool params against schemas **before execution**.
- Execute tools deterministically (bounded output, timeouts).
- Emit replay events:
  - `ToolCall` (includes `params` + `params_hash`)
  - `ToolResult` (includes `output_hash`, `latency_ms`, `side_effects`, `step_utility?`)
- Emit receipt entries with:
  - `params_hash`, `output_hash`, `latency_ms`, `side_effects`

## Side Effects

Tool results MUST declare side effects to support policy/auditing.

Recommended side effect tags:
- `fs_read`, `fs_write`
- `network`
- `process_spawn`
- `deploy`
- `payment`

## Canonical Tool Registries (Current)

These are the highest-signal locations to look for tool contracts/handlers:
- `apps/openagents.com/app/AI/Tools/` (web control-plane tool contracts + handlers)
- `apps/openagents-runtime/lib/openagents_runtime/` (runtime execution orchestration)
- `packages/lightning-effect/` and `packages/dse/` (shared typed contracts/services)
- `crates/` (desktop/local execution tool surfaces)
