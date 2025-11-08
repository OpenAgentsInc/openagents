# Code Smells

Captured via ripgrep scans. Counts exclude non-Swift files.

## Summary Counts

- Forced cast `as!`: 6
- Forced try `try!`: 2
- Forced unwrap after call `)!`: 89
- Identifier forced unwrap `x!`: 66
- `fatalError(...)`: 4
- TODO/FIXME/HACK markers: 6
- `print(...)` debug calls in app/source: 142

## Logging: `print(...)`

- Total in app/source: 142
- Top files by `print(...)` occurrences:
  - ios/OpenAgentsCore/Sources/OpenAgentsCore/MobileBridge/MobileWebSocketClient.swift (25)
  - ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/ExploreOrchestrator.swift (24)
  - ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/SessionTools.swift (13)
  - ios/OpenAgentsCore/Sources/OpenAgentsCore/Agents/CLIAgentProvider.swift (12)
  - ios/OpenAgentsCore/Sources/OpenAgentsCore/DesktopBridge/JsonRpcRouter.swift (11)
  - ios/OpenAgentsCore/Sources/OpenAgentsCore/DesktopBridge/HistoryApi.swift (7)
  - ios/OpenAgents/Bridge/BridgeManager.swift (7)

Recommendation: Introduce a shared `Logger` wrapper over `os.Logger` for production‑grade logging with levels; gate verbose logs behind `#if DEBUG` and feature flags. Add SwiftLint rules to ban `print`.

## Force unwraps/casts

Examples (non‑exhaustive):

- as! (tests):
  - ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/ACPPlanStateTests.swift:245
  - ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/ACPMessageTypeComplianceTests.swift:126, 139, 306–308

- try! (tests):
  - ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/ACPPlanStateTests.swift:220–221

- Forced unwrap after call `)!` (code + tests):
  - ios/OpenAgents/History/LocalClaude.swift:165
  - ios/OpenAgentsCore/Sources/OpenAgentsCore/Tinyvex/Server/TinyvexServer.swift:26
  - ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/ErrorScenarioTests.swift:18, 30, 53, 208, 354

- Identifier forced unwrap `x!` (code + tests):
  - ios/OpenAgents/TinyvexManager.swift:91
  - ios/OpenAgentsUITests/ConversationContinuationUITests.swift:8
  - ios/Tests/BridgeTests/BridgeHandshakeTests.swift:9–10

- fatalError:
  - ios/OpenAgents/OpenAgentsApp.swift:30
  - ios/OpenAgentsCore/Sources/OpenAgentsCore/Agents/CLIAgentProvider.swift:354, 365
  - ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/HistoryApiTests.swift:446

Recommendations:

- Replace `x!` and `)!` with `guard let`/`if let` or optional chaining; when failure is exceptional, surface structured errors instead of crashing.
- Replace `as!` with conditional casts + error handling in production code; in tests, prefer `XCTUnwrap` and strongly typed helpers.
- Review `fatalError` usage; if unrecoverable, ensure app state/logs are captured prior to termination, else prefer thrown errors.

## TODO/FIXME markers

- Total markers found: 6
- Concentrated in:
  - ios/OpenAgentsTests/ToolCallViewRenderingIntegrationTests.swift (2)
  - ios/OpenAgentsTests/MessageClassificationRegressionTests.swift (2)
  - ios/OpenAgentsCore/Sources/OpenAgentsCore/Agents/CodexAgentProvider.swift (1)
  - ios/OpenAgents/FloatingMicButton.swift (1)

Recommendation: Convert TODOs into GitHub issues and reference IDs inline (e.g., `// TODO(#1234): ...`).

