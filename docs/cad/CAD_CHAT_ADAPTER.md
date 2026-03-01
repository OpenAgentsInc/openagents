# CAD Chat-to-Intent Adapter

This document defines how free-form chat content is translated into strict `CadIntent` payloads.

## Module

- `crates/cad/src/chat_adapter.rs`

## Translation Order

1. Try strict JSON extraction + `CadIntent` schema validation.
2. Apply deterministic phrase patterns for supported commands.
3. Return machine-readable parse failure with recovery prompt.

## Supported Phrase Patterns

- `Set material <material_id>`
- `Set objective <objective>`
- `Select <selector>`
- `Compare <variantA,variantB,...>`
- `Export <format> <variant_id>`
- vent hole percent edits (`Make vent holes 20% larger`)
- direct value set (`Set wall_thickness_mm=6.0`)

## Failure Contract

`CadIntentTranslationOutcome::ParseFailure` includes:

- `code`
- `message`
- `recovery_prompt`

The adapter must never mutate CAD state directly; it only emits intent payloads or parse failures.
