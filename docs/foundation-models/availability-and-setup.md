#+ Availability, Setup, and Prewarming

Foundation Models is available on iOS 26+, macOS 15+ where Apple Intelligence is supported. Always check availability and present clear UI states.

## Availability Checks

- Check `SystemLanguageModel.default.availability` (exact API subject to SDK)
- Common states: available, downloading, restricted, unavailable
- Build UI for each: spinner/progress for downloading, explanatory copy for restricted/unavailable, CTA to enable Apple Intelligence where applicable

## Prewarming

- Call `session.prewarm(promptPrefix:)` when a screen appears or the user starts typing
- Improves time‑to‑first‑token/snapshot by priming tokenizer/caches

## Instructions vs Prompt

- Put durable behavior/safety in `Instructions`
- Keep the per‑turn `prompt` focused and concise

## OpenAgents Guidance

- Gate features in UI based on availability; fail soft to deterministic logic if unavailable
- Prewarm in ExploreOrchestrator right before expected generations (plan/analysis)
- Serialize access per session to avoid `concurrentRequests` errors

