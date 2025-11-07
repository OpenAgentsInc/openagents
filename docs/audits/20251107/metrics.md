# Metrics & Hotspots

Quantitative scan of `ios/OpenAgents` and `ios/OpenAgentsCore/Sources/OpenAgentsCore` (app + core sources only).

## Summary

- Approx source lines (Swift, app+core): ~15,706
- Largest files (lines):
  1. `ios/OpenAgents/AcpThreadView.swift` — 1,759
  2. `ios/OpenAgentsCore/Sources/OpenAgentsCore/DesktopBridge/DesktopWebSocketServer.swift` — 1,464
  3. `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/ExploreOrchestrator.swift` — 1,150
  4. `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/OrchestrationTypes.swift` — 477
  5. `ios/OpenAgents/Views/ChatHomeView.swift` — 457
  6. `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/SessionTools.swift` — 446
  7. `ios/OpenAgents/SimplifiedIOSView.swift` — 440
  8. `ios/OpenAgents/Bridge/BridgeManager.swift` — 419
  9. `ios/OpenAgentsCore/Sources/OpenAgentsCore/MobileBridge/MobileWebSocketClient.swift` — 404
  10. `ios/OpenAgents/SimplifiedMacOSView.swift` — 388
  11. `ios/OpenAgentsCore/Sources/OpenAgentsCore/AgentClientProtocol/client.swift` — 376
  12. `ios/OpenAgentsCore/Sources/OpenAgentsCore/Providers/CodexScanner.swift` — 338
  13. `ios/OpenAgents/HistorySidebar.swift` — 335
  14. `ios/OpenAgentsCore/Sources/OpenAgentsCore/AgentClientProtocol/agent.swift` — 318
  15. `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/GrepTool.swift` — 281
  16. `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/WorkspaceScanner.swift` — 279
  17. `ios/OpenAgents/History/LocalCodex.swift` — 277
  18. `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/OrchestrationPlan.swift` — 260
  19. `ios/OpenAgents/ACP/Renderers/ToolCallView.swift` — 259
  20. `ios/OpenAgents/Views/NewChatView.swift` — 256
  21. `ios/OpenAgents/ACP/Renderers/ToolCallDetailSheet.swift` — 247
  22. `ios/OpenAgentsCore/Sources/OpenAgentsCore/Translators/CodexAcpTranslator.swift` — 245
  23. `ios/OpenAgentsCore/Sources/OpenAgentsCore/Providers/ClaudeCodeScanner.swift` — 206
  24. `ios/OpenAgents/History/LocalClaude.swift` — 197
  25. `ios/OpenAgents/Bridge/BridgeSetupInstructionsSheet.swift` — 192

- Print/log statements: 160+ `print`/`debugPrint` occurrences across app+core sources.
- TODO/FIXME/HACK markers (in sources): minimal; notable TODO in `FloatingMicButton` for voice input, and a TODO in a regression test documenting future behavior.

## Notes

- The `ios/OpenAgentsCore/.build/` tree (SwiftPM derived) is ignored by `.gitignore` and not relevant to source metrics.
- A local `ios/build/` directory was present; ensure it is in `.gitignore` to avoid accidental commits.

