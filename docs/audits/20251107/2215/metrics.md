# Metrics & Hotspots (2025-11-07 22:15)

- Swift files: 208
- Markdown files: 113
- Swift total lines: 32,167
- App/source lines: 18,049
- Test lines: 14,118

## Top 20 Longest Swift Files (lines)

```
   1181 ios/OpenAgentsCore/Sources/OpenAgentsCore/DesktopBridge/DesktopWebSocketServer.swift
    762 ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/MobileWebSocketClientComprehensiveTests.swift
    666 ios/OpenAgentsTests/BridgeManagerTests.swift
    525 ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/ACPProtocolComprehensiveTests.swift
    515 ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/NetworkRecoveryTests.swift
    503 ios/OpenAgentsTests/ToolCallViewRenderingIntegrationTests.swift
    484 ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/JSONValueComprehensiveTests.swift
    477 ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/OrchestrationTypes.swift
    460 ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/JsonRpcRouterTests.swift
    460 ios/OpenAgentsCore/Sources/OpenAgentsCore/Agents/CLIAgentProvider.swift
    451 ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/HistoryApiTests.swift
    446 ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/SessionTools.swift
    438 ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/DesktopWebSocketServerComprehensiveTests.swift
    429 ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/PerformanceTests.swift
    417 ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/ClaudeCodeScannerComprehensiveTests.swift
    411 ios/OpenAgentsCore/Sources/OpenAgentsCore/DesktopBridge/DesktopWebSocketServer+Tailer.swift
    407 ios/OpenAgentsCore/Sources/OpenAgentsCore/MobileBridge/MobileWebSocketClient.swift
    401 ios/OpenAgentsTests/ToolCallViewTests.swift
    398 ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/ExploreOrchestrator.swift
    379 ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/ErrorScenarioTests.swift
```

Observations:
- The DesktopWebSocketServer is still a hotspot but has been slimmed significantly (from ~1620 lines → ~1181 lines) with extracted router/tailer/hubs; ongoing modularization is recommended (see recommendations.md).
- Tests remain heavy in orchestration/bridge areas — appropriate for recent refactors and ACP compliance validation.

