# 271. Run Inkling 2 Mini in OpenAgents Desktop

> **Status: unimplemented — design draft.** This chapter specifies full
> integration of local Inkling 2 Mini into `apps/openagents-desktop/`,
> including model acquisition, residency guardrails, model selection, and
> streaming agentic sessions. No Desktop Inkling integration is implemented
> here yet.

The goal of Part XXVII is not a standalone benchmark binary—it is full,
first-class Inkling 2 Mini support inside OpenAgents Desktop so a user on a
≥32 GB Mac can select Inkling 2 Mini, download and verify its original weights
once, and run local agentic coding and tool-use sessions with zero external
dependencies.

## What the chapter will build

1. **Model registry and verified download.** Register Inkling 2 Mini (and
   optional `TML-d24-varloop-research`) in OpenAgents Desktop's local model
   catalog with pinned revision, shard list, ~20 GB size estimate, and SHA-256
   verification on download and load.
2. **Hardware preflight and residency guardrails.** Inspect Apple Silicon
   unified memory before loading; show a clear refusal/warning on machines
   below 32 GB RAM, enforce chapter 269's memory budget, and unload cleanly on
   memory pressure or model switch.
3. **Resident local Psionic runner.** Launch and supervise the local Psionic
   Metal `/v1/responses` runner from [chapter 270](ch270-tool-calls-and-responses-for-inkling-2-mini.md)
   through the Desktop daemon (`openagents-environment`), exposing health,
   warm/cold state, and tokens/sec telemetry.
4. **Desktop UI and agentic session wiring.** Surface Inkling 2 Mini in the
   Desktop model picker, render streaming reasoning/CoT and tool-call chips,
   and support full multi-turn local tool execution and cancellation.

## Acceptance checks

- **Worked trace:** document the end-to-end Desktop lifecycle from model
  selection through SHA-256 admission, Metal residency check, `/v1/responses`
  streaming tool call, and clean unload.
- **Independent check:** run a real multi-turn coding task in OpenAgents
  Desktop backed by local Inkling 2 Mini and verify tool calls, reasoning
  display, cancellation, and memory reclamation.
- **Refused inputs:** verify that Desktop refuses corrupted/unverified weight
  files and refuses to load Inkling 2 Mini when available unified memory is
  below the admitted floor, with actionable UI diagnostics.
