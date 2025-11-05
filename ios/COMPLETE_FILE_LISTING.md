# Complete iOS Codebase File Listing

## OpenAgentsCore (Source Files)

### ACP Data Models (`/OpenAgentsCore/Sources/OpenAgentsCore/ACP/`)
- `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Sources/OpenAgentsCore/ACP/ACPCommon.swift`
- `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Sources/OpenAgentsCore/ACP/ACPContent.swift`
- `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Sources/OpenAgentsCore/ACP/ACPEvent.swift`
- `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Sources/OpenAgentsCore/ACP/ACPMessage.swift`
- `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Sources/OpenAgentsCore/ACP/ACPPlanState.swift`
- `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Sources/OpenAgentsCore/ACP/ACPThread.swift`
- `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Sources/OpenAgentsCore/ACP/ACPTool.swift`

### ACP Protocol Types (`/OpenAgentsCore/Sources/OpenAgentsCore/AgentClientProtocol/`)
- `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Sources/OpenAgentsCore/AgentClientProtocol/acp.swift`
- `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Sources/OpenAgentsCore/AgentClientProtocol/agent.swift`
- `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Sources/OpenAgentsCore/AgentClientProtocol/client.swift`
- `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Sources/OpenAgentsCore/AgentClientProtocol/errors.swift`
- `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Sources/OpenAgentsCore/AgentClientProtocol/ext.swift`
- `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Sources/OpenAgentsCore/AgentClientProtocol/plan.swift`
- `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Sources/OpenAgentsCore/AgentClientProtocol/rpc.swift`
- `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Sources/OpenAgentsCore/AgentClientProtocol/services.swift`
- `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Sources/OpenAgentsCore/AgentClientProtocol/session.swift`
- `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Sources/OpenAgentsCore/AgentClientProtocol/tool_call.swift`
- `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Sources/OpenAgentsCore/AgentClientProtocol/version.swift`

### Bridge Communication (`/OpenAgentsCore/Sources/OpenAgentsCore/Bridge/`)
- `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Sources/OpenAgentsCore/Bridge/BridgeConfig.swift`
- `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Sources/OpenAgentsCore/Bridge/BridgeMessages.swift`
- `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Sources/OpenAgentsCore/Bridge/JSONRPC.swift`

### Desktop Bridge (`/OpenAgentsCore/Sources/OpenAgentsCore/DesktopBridge/`)
- `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Sources/OpenAgentsCore/DesktopBridge/DesktopWebSocketServer.swift`

### Mobile Bridge (`/OpenAgentsCore/Sources/OpenAgentsCore/MobileBridge/`)
- `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Sources/OpenAgentsCore/MobileBridge/MobileWebSocketClient.swift`

### Providers (`/OpenAgentsCore/Sources/OpenAgentsCore/Providers/`)
- `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Sources/OpenAgentsCore/Providers/ClaudeCodeScanner.swift`
- `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Sources/OpenAgentsCore/Providers/ClaudeScanner.swift`
- `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Sources/OpenAgentsCore/Providers/CodexDiscovery.swift`
- `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Sources/OpenAgentsCore/Providers/CodexScanner.swift`

### Translators (`/OpenAgentsCore/Sources/OpenAgentsCore/Translators/`)
- `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Sources/OpenAgentsCore/Translators/ClaudeAcpTranslator.swift`
- `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Sources/OpenAgentsCore/Translators/CodexAcpTranslator.swift`

### Summarization (`/OpenAgentsCore/Sources/OpenAgentsCore/Summarization/`)
- `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Sources/OpenAgentsCore/Summarization/ConversationSummarizer.swift`
- `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Sources/OpenAgentsCore/Summarization/FoundationModelSummarizer.swift`

### Utilities (`/OpenAgentsCore/Sources/OpenAgentsCore/`)
- `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Sources/OpenAgentsCore/HistoryLoader.swift`
- `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Sources/OpenAgentsCore/ThreadSummary.swift`

---

## OpenAgentsCore (Test Files)

### Core Tests (`/OpenAgentsCore/Tests/OpenAgentsCoreTests/`)
- `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/ACPModelsTests.swift` ✓
- `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/ACPTypesTests.swift` ✓
- `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/BridgeEnvelopeTests.swift` ✓
- `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/BridgeServerClientTests.swift` ✓
- `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/ClaudeAcpTranslatorTests.swift` ✓
- `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/ClaudeScannerTests.swift` ✓
- `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/CodexAcpTranslatorTests.swift` ✓
- `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/CodexDiscoveryTests.swift` ✓
- `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/CodexScannerTests.swift` ✓
- `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/CodexUserDirsTests.swift` ✓
- `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/ConversationSummarizerTests.swift` ✓
- `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/ErrorsJSONRPCTests.swift` ✓
- `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/FoundationSummarizerParsingTests.swift` ✓
- `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/SessionUpdateTests.swift` ✓
- `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/TranslatorTests.swift` ✓

---

## OpenAgents App (UI Source Files)

### Root Level (`/OpenAgents/`)
- `/Users/christopherdavid/code/openagents/ios/OpenAgents/OpenAgentsApp.swift`
- `/Users/christopherdavid/code/openagents/ios/OpenAgents/ContentView.swift`
- `/Users/christopherdavid/code/openagents/ios/OpenAgents/AcpThreadView.swift` (1,550 lines)
- `/Users/christopherdavid/code/openagents/ios/OpenAgents/RawThreadView.swift`
- `/Users/christopherdavid/code/openagents/ios/OpenAgents/Theme.swift`
- `/Users/christopherdavid/code/openagents/ios/OpenAgents/Fonts.swift`
- `/Users/christopherdavid/code/openagents/ios/OpenAgents/Features.swift`
- `/Users/christopherdavid/code/openagents/ios/OpenAgents/Item.swift`

### Bridge Components (`/OpenAgents/Bridge/`)
- `/Users/christopherdavid/code/openagents/ios/OpenAgents/Bridge/BridgeManager.swift` (255 lines)
- `/Users/christopherdavid/code/openagents/ios/OpenAgents/Bridge/BonjourBrowser.swift`
- `/Users/christopherdavid/code/openagents/ios/OpenAgents/Bridge/BridgeStatusChip.swift`
- `/Users/christopherdavid/code/openagents/ios/OpenAgents/Bridge/ManualConnectSheet.swift`

### ACP Renderers (`/OpenAgents/ACP/Renderers/`)
- `/Users/christopherdavid/code/openagents/ios/OpenAgents/ACP/Renderers/ToolCallView.swift` (100 lines)
- `/Users/christopherdavid/code/openagents/ios/OpenAgents/ACP/Renderers/ToolResultView.swift`
- `/Users/christopherdavid/code/openagents/ios/OpenAgents/ACP/Renderers/PlanStateView.swift`
- `/Users/christopherdavid/code/openagents/ios/OpenAgents/ACP/Renderers/RawEventView.swift`

### History Components (`/OpenAgents/History/`)
- `/Users/christopherdavid/code/openagents/ios/OpenAgents/History/LocalClaude.swift`
- `/Users/christopherdavid/code/openagents/ios/OpenAgents/History/LocalCodex.swift`
- `/Users/christopherdavid/code/openagents/ios/OpenAgents/HistorySidebar.swift`

### Floating UI (`/OpenAgents/`)
- `/Users/christopherdavid/code/openagents/ios/OpenAgents/FloatingMenuButton.swift`
- `/Users/christopherdavid/code/openagents/ios/OpenAgents/FloatingMicButton.swift`
- `/Users/christopherdavid/code/openagents/ios/OpenAgents/FloatingScrollButtons.swift`
- `/Users/christopherdavid/code/openagents/ios/OpenAgents/FloatingToolbar.swift`

### UI Utilities (`/OpenAgents/`)
- `/Users/christopherdavid/code/openagents/ios/OpenAgents/GlassBar.swift`
- `/Users/christopherdavid/code/openagents/ios/OpenAgents/GlassHeader.swift`
- `/Users/christopherdavid/code/openagents/ios/OpenAgents/TopEdgeGradient.swift`
- `/Users/christopherdavid/code/openagents/ios/OpenAgents/ThreadHeaderView.swift`
- `/Users/christopherdavid/code/openagents/ios/OpenAgents/FontLoader.swift`
- `/Users/christopherdavid/code/openagents/ios/OpenAgents/TitleCache.swift`
- `/Users/christopherdavid/code/openagents/ios/OpenAgents/FMProbe.swift`

### Examples/Demos (`/OpenAgents/`)
- `/Users/christopherdavid/code/openagents/ios/OpenAgents/Examples/ChatTabsDemo.swift`
- `/Users/christopherdavid/code/openagents/ios/OpenAgents/Examples/GlassTerminalCard.swift`

---

## OpenAgents App (Test Files)

### Unit Tests (`/OpenAgentsTests/`)
- `/Users/christopherdavid/code/openagents/ios/OpenAgentsTests/TimelineTests.swift` ✓
- `/Users/christopherdavid/code/openagents/ios/OpenAgentsTests/OpenAgentsTests.swift` (empty)

### UI Tests (`/OpenAgentsUITests/`)
- `/Users/christopherdavid/code/openagents/ios/OpenAgentsUITests/OpenAgentsUITests.swift` (empty)
- `/Users/christopherdavid/code/openagents/ios/OpenAgentsUITests/OpenAgentsUITestsLaunchTests.swift` (launch metrics)

### Bridge Integration Tests (`/Tests/BridgeTests/`)
- `/Users/christopherdavid/code/openagents/ios/Tests/BridgeTests/BridgeHandshakeTests.swift` ✓

---

## Summary by Status

### Well-Tested (✓)
1. `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/ACPModelsTests.swift`
2. `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/ACPTypesTests.swift`
3. `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/BridgeEnvelopeTests.swift`
4. `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/BridgeServerClientTests.swift`
5. `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/ClaudeAcpTranslatorTests.swift`
6. `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/ClaudeScannerTests.swift`
7. `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/CodexAcpTranslatorTests.swift`
8. `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/CodexDiscoveryTests.swift`
9. `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/CodexScannerTests.swift`
10. `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/CodexUserDirsTests.swift`
11. `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/ConversationSummarizerTests.swift`
12. `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/ErrorsJSONRPCTests.swift`
13. `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/FoundationSummarizerParsingTests.swift`
14. `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/SessionUpdateTests.swift`
15. `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/TranslatorTests.swift`
16. `/Users/christopherdavid/code/openagents/ios/Tests/BridgeTests/BridgeHandshakeTests.swift`
17. `/Users/christopherdavid/code/openagents/ios/OpenAgentsTests/TimelineTests.swift`

### Critically Under-Tested (Major gaps)
1. `/Users/christopherdavid/code/openagents/ios/OpenAgents/AcpThreadView.swift` (1,550 lines)
2. `/Users/christopherdavid/code/openagents/ios/OpenAgents/Bridge/BridgeManager.swift` (255 lines)
3. `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Sources/OpenAgentsCore/DesktopBridge/DesktopWebSocketServer.swift` (400 lines)
4. `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Sources/OpenAgentsCore/MobileBridge/MobileWebSocketClient.swift` (300 lines)

### Completely Untested (Priority: High)
1. `/Users/christopherdavid/code/openagents/ios/OpenAgents/ACP/Renderers/ToolCallView.swift` (100 lines)
2. `/Users/christopherdavid/code/openagents/ios/OpenAgents/ContentView.swift` (142 lines)
3. `/Users/christopherdavid/code/openagents/ios/OpenAgents/Bridge/BonjourBrowser.swift`
4. `/Users/christopherdavid/code/openagents/ios/OpenAgents/History/LocalClaude.swift`
5. `/Users/christopherdavid/code/openagents/ios/OpenAgents/History/LocalCodex.swift`
6. `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Sources/OpenAgentsCore/Providers/ClaudeCodeScanner.swift`
7. All ACP Protocol files in `/AgentClientProtocol/` (11 files)
8. All UI component files (15+ views)

### No Tests Created (Empty files)
1. `/Users/christopherdavid/code/openagents/ios/OpenAgentsTests/OpenAgentsTests.swift`
2. `/Users/christopherdavid/code/openagents/ios/OpenAgentsUITests/OpenAgentsUITests.swift`
3. `/Users/christopherdavid/code/openagents/ios/OpenAgentsUITests/OpenAgentsUITestsLaunchTests.swift`
