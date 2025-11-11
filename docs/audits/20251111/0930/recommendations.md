# Recommendations (Prioritized)

P0 — Replace heuristic DecisionEngine with FM decisions
- Implement FM‑driven decision selection for refactor vs tests vs exploration, using on‑device models.
- Inputs: recent session summaries (SessionAnalyzeResult), file frequency, goals; Output: DecisionOutput.
- Retain deterministic fallback only when FM unavailable; remove fixed thresholds/keyword lists.
- Owners: OpenAgentsCore Orchestration
- References: ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/DecisionEngine.swift:129

P0 — Use FM for conversation titles and “system preface” detection
- ConversationSummarizer: always attempt FM; demote “first user 5 words” to legacy fallback only.
- Add FM classifier for isSystemPreface instead of tag prefix checks; keep tag checks as last resort.
- Owners: OpenAgentsCore Summarization
- References: ios/OpenAgentsCore/Sources/OpenAgentsCore/Summarization/ConversationSummarizer.swift:33

P1 — FM‑based agent mode selection for ad‑hoc input
- When user types arbitrary agent text, use FM to infer desired provider/mode from context; for fixed menu entries, keep deterministic mapping.
- Owners: iOS UI + Bridge
- References: ios/OpenAgents/Views/NewChatView.swift:64

P1 — Make provider preference policy explicit/configurable
- Keep deterministic “prefer Claude Code” operational rule, but surface as user setting (default on). Document in Settings.
- Owners: DesktopBridge + Settings
- References: ios/OpenAgentsCore/Sources/OpenAgentsCore/DesktopBridge/DesktopWebSocketServer+Tailer.swift:57

P2 — Keep regex sanitization and config validation
- Title sanitation regex and cron/time validation remain deterministic; clearly scoped as boundary checks.
- Owners: UI + Orchestration

Guardrails
- All FM calls must no‑op to deterministic fallback when on‑device FM unavailable.
- Add telemetry counters: fm_used vs fallback_used for each surface.

