# CAD Intent Execution Path

This document defines the deterministic execution pipeline for intent-based modeling in `openagents-cad`.

## Module

- `crates/cad/src/intent_execution.rs`

## Entry Points

- `plan_intent_execution(&str) -> Result<CadIntentExecutionPlan, CadIntentExecutionClarification>`
- `execute_intent_input(&str, &mut CadDispatchState, CadIntentExecutionPolicy, bool) -> CadIntentExecutionDecision`

## Stages

- `parsed`: strict `CadIntent` JSON parsed and validated.
- `inferred`: natural-language prompt mapped to typed `CadIntent`.
- `confirmation_required`: natural-language plan gated until explicit confirmation.
- `applied`: typed intent dispatched through `dispatch_cad_intent`.
- `clarification_required`: parse/translation ambiguity surfaced with recovery prompt.
- `dispatch_failed`: typed dispatch rejected due state/parameter constraints.

## Execution Contract

- Strict JSON intent payloads execute without confirmation gate.
- Natural-language intent payloads require confirmation by default.
- Confirmation gate can be disabled only via explicit policy (`CadIntentExecutionPolicy`).
- Ambiguous inputs return deterministic clarification metadata (`code`, `message`, `recovery_prompt`).
- State mutation authority remains in typed dispatcher (`crates/cad/src/dispatch.rs`).

## Failure Contract

- JSON schema failures use `CAD-INTENT-*` validation codes.
- Natural-language ambiguity uses `CAD-CHAT-*` clarification codes.
- Dispatch failures return explicit stage `dispatch_failed` with original error text.

## Determinism

- Identical input + policy produce identical plans.
- Confirmed execution increments `CadDispatchState.revision` monotonically.
- Replay behavior is covered by parity fixture checks (`VCAD-PARITY-087`).
