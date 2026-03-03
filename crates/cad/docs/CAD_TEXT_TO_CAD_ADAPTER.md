# CAD Text-to-CAD Adapter

This document defines the deterministic prompt-to-CAD adapter surface in `openagents-cad`.

## Module

- `crates/cad/src/text_to_cad.rs`

## Entry Points

- `text_to_cad(TextToCadRequest) -> CadResult<TextToCadOutcome>`
- `text_to_cad_from_prompt(&str) -> CadResult<TextToCadOutcome>`

## Model Profiles

- `cad0` (default): higher-fidelity template path.
- `cad0-mini`: smaller/offline fallback profile.

## Output Contract

`TextToCadOutcome::Generated` includes:

- selected `model` profile
- generated `compact_ir` text (vcad-style compact IR)
- canonical `ir` object (`CadMcpDocument`)
- `operation_count`
- deterministic generation signature

`TextToCadOutcome::Clarification` includes:

- `code`
- `message`
- deterministic clarification questions for under-specified prompts

## Failure/Clarification Codes

- `CAD0-EMPTY-PROMPT`
- `CAD0-AMBIGUOUS-PROMPT`

## Determinism

- Prompt + profile produce deterministic compact IR output.
- Canonical IR is round-tripped through compact IR parser.
- Replay signatures are stable across repeated runs.
