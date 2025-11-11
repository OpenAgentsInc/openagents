# Heuristics Catalog (Code References)

This list highlights all deterministic heuristics found, with brief rationale and migration notes. File references use clickable paths with single line anchors.

Interpretation/Decision Heuristics (replace with FM)
- DecisionEngine heuristic thresholds and keywords:
  - ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/DecisionEngine.swift:129
  - ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/DecisionEngine.swift:131
  - ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/DecisionEngine.swift:132
  - ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/DecisionEngine.swift:200
  - ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/DecisionEngine.swift:220
  - ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/DecisionEngine.swift:221
  - ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/DecisionEngine.swift:244
  - ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/DecisionEngine.swift:247
  Summary: Hard‑coded “top file touched >20” + goal/intent keywords to choose refactor vs tests, compute confidence, and pick agent. This is precisely the type of logic the LLM‑First policy forbids.

- Conversation title and preface heuristics (fallbacks):
  - ios/OpenAgentsCore/Sources/OpenAgentsCore/Summarization/ConversationSummarizer.swift:20
  - ios/OpenAgentsCore/Sources/OpenAgentsCore/Summarization/ConversationSummarizer.swift:33
  - ios/OpenAgentsCore/Sources/OpenAgentsCore/Summarization/ConversationSummarizer.swift:59
  - ios/OpenAgentsCore/Sources/OpenAgentsCore/Summarization/ConversationSummarizer.swift:71
  Summary: “first user 5 words,” stopword trimming, and tag‑prefix checks to detect system prefaces. These should defer to FM classification/summarization whenever possible, with minimal sanitization retained.

- Agent selection from UI string (keyword match):
  - ios/OpenAgents/Views/NewChatView.swift:64
  - ios/OpenAgents/Views/NewChatView.swift:100
  - ios/OpenAgents/Views/NewChatView.swift:237
  Summary: Maps UI label containing “codex/claude” to ACPSessionModeId. For ad‑hoc user input, migrate to FM selection; for known menu items, keep deterministic mapping.

Structural/UI Heuristics (acceptable as validation/sanitization; keep or reduce)
- Title sanitization using regex to strip markdown link/emphasis:
  - ios/OpenAgents/ContentView.swift:154
  - ios/OpenAgents/HistorySidebar.swift:227
  Summary: UI sanitization, not interpretation. Leave as is, but ensure FM‑derived titles are primary.

- Provider preference/tie‑breakers in desktop tailer:
  - ios/OpenAgentsCore/Sources/OpenAgentsCore/DesktopBridge/DesktopWebSocketServer+Tailer.swift:57
  - ios/OpenAgents/HistorySidebar.swift:174
  Summary: Prefer Claude Code sessions, tie‑break on equal timestamps. Operational policy, not semantic interpretation. Keep, but consider making provider preference user‑configurable.

- Event‑type string matching for reasoning detection:
  - ios/OpenAgentsCore/Sources/OpenAgentsCore/DesktopBridge/DesktopWebSocketServer+Tailer.swift:368
  - ios/OpenAgentsCore/Sources/OpenAgentsCore/DesktopBridge/DesktopWebSocketServer+Tailer.swift:372
  Summary: Explicit protocol type checks. Appropriate.

- Regex‑based session search (user‑provided pattern):
  - ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/SessionTools.swift:210
  Summary: Deterministic search of logs by regex pattern. Appropriate.

Other deterministic logic (OK under policy)
- Config validation and parsing (cron/time patterns):
  - ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/OrchestrationConfig.swift:223
  - ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/OrchestrationConfig.swift:276
  Summary: API boundary validation; keep deterministic.

- ToolName mapping and argument formatting:
  - ios/OpenAgentsCore/Sources/OpenAgentsCore/ToolName.swift:40
  - ios/OpenAgentsCore/Sources/OpenAgentsCore/Utils/ShellCommandFormatter.swift:11
  Summary: Deterministic mapping/formatting; appropriate.

Notes
- Several FM paths already exist (e.g., ConversationSummarizer prefers FM, FMAnalysis pipeline, FMOrchestrator). The audit focuses on removing the remaining policy‑violating decision heuristics and replacing them with FM calls and light fallbacks.

