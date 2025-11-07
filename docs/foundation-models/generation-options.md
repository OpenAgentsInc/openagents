# Generation Options and Sampling (Apple Foundation Models)

This document summarizes the book’s “Generation Options and Sampling Control” chapter and maps it to OpenAgents usage. It explains temperature, token limits, sampling modes, and how these differ from MLX Swift.

## Core Options

- `GenerationOptions(temperature:)`
  - 0.1–0.3: precise, predictable
  - 0.5–0.7: balanced
  - 0.8–1.0: creative, still coherent
  - `nil`: system default (recommended unless you need deterministic behavior)
- `maximumResponseTokens`
  - Cap response length to fit UI surfaces (cards, chat bubbles, long-form)
  - Prefer explicit formatting rules in instructions for UX consistency; use the cap as a guardrail

## Sampling Modes

- System default: lets the framework pick sane values for on‑device quality/latency
- Greedy: most likely token each step; stable but can be repetitive
- Top‑K: sample from the K most likely tokens (fixed pool)
- Top‑P (nucleus): sample from smallest set of tokens whose cumulative prob ≥ P

Note: Foundation Models exposes a simplified set of controls compared to many server LLM SDKs. Favor defaults unless you’re addressing a clear quality need.

## Patterns

- Creative copy: `temperature ~0.8`, moderate token cap (150–250)
- Technical/structured: `temperature ~0.2–0.4`, lower cap (50–150)
- Notifications/widgets: set explicit length rules in instructions; use small `maximumResponseTokens`

## Foundation Models vs MLX Swift

- Foundation Models prioritizes user‑friendly knobs (temperature, token cap, a few sampling choices) tuned for on‑device stability.
- MLX Swift exposes more low‑level controls for research/experimentation. Use MLX when you need model training/fine‑grained sampling work; use Foundation Models for production app features.

## OpenAgents Guidance

- Default to system sampling. Only tune temperature when you have a specific defect (e.g., repetitive or overly terse output).
- Keep token caps aligned with the consuming UI. Enforce length/format in instructions first; use caps to prevent overflow.
- For tests, pin temperature (e.g., 0.2) for reproducibility.

## Snippets

```swift
let defaults = GenerationOptions() // system‑tuned
let precise = GenerationOptions(temperature: 0.2, maximumResponseTokens: 120)
let creative = GenerationOptions(temperature: 0.9, maximumResponseTokens: 220)
```

