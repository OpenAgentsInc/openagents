# Actions (Concrete Next Steps)

P0 — Orchestration decisions via FM
- Add FMDecider actor under `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/` that:
  - Takes `SessionAnalyzeResult + OrchestrationConfig` and returns `DecisionOutput`.
  - Uses on‑device FM with a prompt seeded by goals, file frequency summary, and recent tool patterns.
  - Streams progress via existing ACP `tool_call` updates (`fm.analysis`).
- Wire DesktopWebSocketServer `orchestrate.*` to use FMDecider; deprecate `DecisionEngine.decideNextTask` heuristic path behind a feature flag.

P0 — Summarization/classification via FM
- ConversationSummarizer:
  - Always try FM first for title; log fm_used/fallback_used counters.
  - Add FM classifier `isSystemPrefaceFM(_:)` and gate existing tag prefix checks behind fallback.
- Update tests to treat FM path as default when `Features.foundationModelsEnabled`.

P1 — Agent selection via FM (ad‑hoc)
- Add a small FM prompt to infer provider (`.claude_code` vs `.codex`) from free‑form user agent text.
- In `NewChatView`, use FM inference when `selectedAgent` is not from the known list; keep direct mapping for known menu items.

P1 — Provider preference setting
- Add a user setting “Prefer Claude Code” (default on) consumed by Desktop tailer and History sidebar sort tie‑breakers.

P2 — Telemetry + docs
- Add counters (per session): `fm.used`, `fm.fallback` for titles, decisions.
- Document the LLM‑First enforcement and fallbacks in `docs/foundation-models/`.

Validation
- Unit tests covering FM present vs absent for: decision making, title generation, system preface detection.
- Integration test for `orchestrate/coordinator.run_once` streaming plan + decisions via ACP.

