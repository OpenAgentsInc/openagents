#+ Integrating External JSON APIs

This complements tool calling by showing how to reuse the same `@Generable` schemas with external providers (OpenAI, Anthropic, etc.) when Apple Intelligence is unavailable.

## Single Source of Truth

- Define response/argument types with `@Generable` and `@Guide`
- Export JSON Schema from these types and send it to external providers to get strict JSON outputs
- Keep Apple on‑device and external cloud paths aligned with identical schemas

## Architecture

- Try on‑device Foundation Models first
- If unavailable, fall back to provider API with the same schema and a compatible prompt
- Stream when supported; otherwise show a spinner with progressive disclosure

## OpenAgents Guidance

- Centralize `@Generable` models under `OpenAgentsCore` so both paths import the same types
- If using AIProxy or another gateway, pass the JSON Schema derived from the `@Generable` type; keep provider‑specific wiring out of app code
- Constrain outputs tightly (counts, ranges, enums) to reduce post‑processing

## Snippet (conceptual)

```swift
// Pseudocode: exporting schema
let schema = try RecipeRecommendation.generationSchema()
// Send schema alongside prompt to external provider
```

