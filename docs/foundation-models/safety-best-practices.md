#+ Safety and Best Practices

Summary of Apple’s multi‑layer safety approach and practical patterns for OpenAgents.

## Safety Layers

- Input filtering → checks instructions/prompts/tool calls
- Output filtering → inspects responses before returning
- Model‑level guardrails → trained reluctance for unsafe content
- Handle `GenerationError.guardrailViolation` with friendly UI and retry guidance

## Design for Limitations

- Knowledge cutoff (circa 2023): do not rely on world facts → use tools/search
- Math/precision: do explicit calculations outside the model
- Complexity: break into smaller steps; keep prompts short/clear

## Safer Instructions

- State scope, persona, and refusal policy explicitly
- Add disclaimers where appropriate (medical, legal)
- Prefer structured generation for critical paths; validate types, ranges, counts

## Input Risk Patterns

- Highest: free‑form user input → validate/sanitize, add confirmations
- Moderate: structured prompts → constrain arguments with `@Guide`
- Lowest: curated options → bounded choices in UI

## OpenAgents Guidance

- Map FM errors to ACP‑visible, user‑friendly messages; log diagnostics in development
- Use tools to ground responses in repository/workspace data instead of conjecture
- Keep hard caps/timeouts in tool implementations; avoid unbounded operations

