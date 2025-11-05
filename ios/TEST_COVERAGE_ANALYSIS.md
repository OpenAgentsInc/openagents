# iOS OpenAgents Test Coverage Analysis

## Executive Summary

**Total Swift Files**: 87
**Total Test Files**: 20 (23% test file ratio)
**Test Frameworks**: XCTest, Testing (async)
**Current Coverage**: OpenAgentsCore extensively tested; OpenAgents app has minimal UI/integration tests

---

## 1. CURRENT TEST COVERAGE

### 1.1 OpenAgentsCore Tests (14 test files - WELL COVERED)

**Location**: `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/`

Existing tests cover:
- `ACPModelsTests.swift` - Message, tool call, plan state encoding/decoding
- `ACPTypesTests.swift` - ContentBlock variants, SessionNotification decoding
- `BridgeServerClientTests.swift` - Desktop/Mobile WebSocket handshake (macOS only)
- `BridgeEnvelopeTests.swift` - ThreadsList roundtrip serialization
- `ErrorsJSONRPCTests.swift` - JSONRPC error handling
- `SessionUpdateTests.swift` - Available commands, tool call updates
- `ClaudeAcpTranslatorTests.swift` - Claude JSONL to ACP translation
- `CodexAcpTranslatorTests.swift` - Codex JSONL to ACP translation
- `TranslatorTests.swift` - Generic translator utilities
- `ClaudeScannerTests.swift` - Claude provider discovery
- `CodexScannerTests.swift` - Codex provider discovery
- `CodexDiscoveryTests.swift` - Provider binary lookup
- `CodexUserDirsTests.swift` - User directory handling
- `ConversationSummarizerTests.swift` - Conversation summarization
- `FoundationSummarizerParsingTests.swift` - Model output parsing

### 1.2 OpenAgents App Tests (2 test files - MINIMAL)

**Location**: `/Users/christopherdavid/code/openagents/ios/OpenAgentsTests/`

Existing tests:
- `TimelineTests.swift` - Tests `AcpThreadView_computeTimeline` function for message grouping
- `OpenAgentsTests.swift` - Empty placeholder using new Testing framework

### 1.3 UI Tests (2 test files - MINIMAL)

**Location**: `/Users/christopherdavid/code/openagents/ios/OpenAgentsUITests/`

Existing tests:
- `OpenAgentsUITests.swift` - Empty placeholder
- `OpenAgentsUITestsLaunchTests.swift` - Launch performance measurement

### 1.4 Bridge Integration Tests (1 test file)

**Location**: `/Users/christopherdavid/code/openagents/ios/Tests/BridgeTests/`

Existing tests:
- `BridgeHandshakeTests.swift` - Desktop server / mobile client TCP handshake validation

---

## 2. CORE ACP PROTOCOL IMPLEMENTATION (All in OpenAgentsCore)

### 2.1 ACP Type System (in `/AgentClientProtocol/`)

**Files needing test coverage**:

| File | Size | Coverage | Critical Components |
|------|------|----------|---------------------|
| `acp.swift` | Small | NONE | ACP namespace container |
| `version.swift` | Small | NONE | Protocol version definitions |
| `client.swift` | Large | PARTIAL | ContentBlock, SessionNotification, ContentChunk |
| `agent.swift` | Large | PARTIAL | InitializeRequest/Response, SessionManagement |
| `session.swift` | Medium | NONE | ACPSessionId, SessionNew/Prompt/Execute |
| `rpc.swift` | Medium | NONE | JSONRPC Request/Response/Notification types |
| `errors.swift` | Small | PARTIAL | Error code definitions |
| `tool_call.swift` | Medium | NONE | ToolCallInput, ToolCallResult variants |
| `plan.swift` | Small | NONE | Plan management types |
| `services.swift` | Small | NONE | Service definitions |
| `ext.swift` | Medium | NONE | Extension utilities |

**Test Gap**: Missing comprehensive protocol validation tests for:
- All variant decoding/encoding paths
- Error handling for malformed ACP messages
- Protocol version negotiation
- Complex nested structures

### 2.2 ACP Data Models (in `/ACP/`)

| File | Size | Coverage | Critical Types |
|------|------|----------|-----------------|
| `ACPCommon.swift` | Small | NONE | ACPRole, ACPSummary, base types |
| `ACPMessage.swift` | Small | TESTED | ACPMessage structure (in ACPModelsTests) |
| `ACPEvent.swift` | Small | TESTED | ACPEvent variants (in ACPModelsTests) |
| `ACPThread.swift` | Small | TESTED | Thread container (in ACPModelsTests) |
| `ACPTool.swift` | Small | TESTED | ACPToolCall, ACPToolResult (in ACPModelsTests) |
| `ACPContent.swift` | Medium | NONE | JSONValue, content variants |
| `ACPPlanState.swift` | Small | TESTED | Plan state (in ACPModelsTests) |

**Test Gap**: Incomplete coverage of:
- JSONValue variant combinations
- Content block type conversions
- Edge cases in nested structures

### 2.3 Bridge Communication (in `/Bridge/`)

| File | Size | Coverage | Purpose |
|------|------|----------|---------|
| `BridgeConfig.swift` | Tiny | NONE | Default URLs, ports, service type |
| `BridgeMessages.swift` | Small | NONE | ThreadsListRequest, ThreadsListResponse |
| `JSONRPC.swift` | Small | PARTIAL | Request, Response, Notification (in ErrorsJSONRPCTests) |

**Test Gap**:
- No tests for BridgeMessages serialization
- Missing JSONRPC edge cases (invalid id, error codes)

### 2.4 WebSocket Bridge Implementations

**DesktopWebSocketServer** (`/DesktopBridge/DesktopWebSocketServer.swift`)
- **Lines**: ~400
- **Coverage**: ONE test (BridgeServerClientTests - happy path only)
- **Test Gaps**:
  - Connection lifecycle edge cases
  - Bonjour advertisement/discovery flow
  - Message fragmentation
  - Error handling (closed connection, invalid tokens)
  - Ping/pong frame handling
  - Multiple client management
  - Server restart scenarios

**MobileWebSocketClient** (`/MobileBridge/MobileWebSocketClient.swift`)
- **Lines**: ~300
- **Coverage**: ONE test (BridgeServerClientTests - happy path only)
- **Test Gaps**:
  - Reconnection logic
  - Initialization handshake variations
  - JSON-RPC request/response matching
  - Pending request timeout handling
  - Malformed message recovery
  - Connection state transitions
  - Delegate callback sequencing

### 2.5 Translator Modules (in `/Translators/`)

| File | Coverage | Test Files |
|------|----------|-----------|
| `ClaudeAcpTranslator.swift` | TESTED | ClaudeAcpTranslatorTests |
| `CodexAcpTranslator.swift` | TESTED | CodexAcpTranslatorTests |

**Test Status**: Both have tests but coverage depth unknown; should verify:
- All JSONL format variants
- Error handling for malformed input
- Incremental parsing
- State management across chunks

### 2.6 Providers (in `/Providers/`)

| File | Coverage | Notes |
|------|----------|-------|
| `ClaudeScanner.swift` | TESTED | ClaudeScannerTests |
| `ClaudeCodeScanner.swift` | NONE | New provider; no tests |
| `CodexScanner.swift` | TESTED | CodexScannerTests |
| `CodexDiscovery.swift` | TESTED | CodexDiscoveryTests |

**Test Gap**: ClaudeCodeScanner is new and untested

### 2.7 Utilities

| File | Coverage |
|------|----------|
| `HistoryLoader.swift` | NONE |
| `ThreadSummary.swift` | NONE |

---

## 3. UI COMPONENTS (OpenAgents App)

### 3.1 Main Application Files

**Root Level** (`/OpenAgents/`)
| File | Lines | Type | Coverage | Notes |
|------|-------|------|----------|-------|
| `OpenAgentsApp.swift` | 40 | App Root | NONE | SwiftUI app entry point |
| `ContentView.swift` | 142 | View | NONE | Main layout (split view / mobile) |
| `Theme.swift` | 40 | Utilities | NONE | Dark theme, colors |
| `Fonts.swift` | 25 | Utilities | NONE | Font definitions |
| `Features.swift` | 40 | Config | NONE | Feature flags (multicastEnabled) |
| `Item.swift` | 10 | Model | NONE | Generic Item model |

**Test Gaps**: 
- No theme/color validation tests
- No feature flag behavior tests

### 3.2 ACP Renderer Components (`/ACP/Renderers/`)

| File | Lines | Type | Coverage | Purpose |
|------|-------|------|----------|---------|
| `ToolCallView.swift` | 100 | View | NONE | Renders ACPToolCall with shell/JSON prettifying |
| `ToolResultView.swift` | ? | View | NONE | Renders ACPToolResult |
| `PlanStateView.swift` | ? | View | NONE | Renders ACPPlanState with steps |
| `RawEventView.swift` | ? | View | NONE | Raw JSON rendering |

**Test Gaps**:
- No view snapshot tests
- No shell command parsing tests (critical: `prettyShellCommand` function)
- No JSON prettification tests
- No error case rendering tests

### 3.3 Main Thread View (`/AcpThreadView.swift`)

**Lines**: 1550 (LARGEST COMPONENT)
**Coverage**: MINIMAL (only `TimelineTests.swift` tests timeline computation)

**Key Functions**:
1. `computeTimeline(lines:)` - TESTED (TimelineTests)
   - Message/event deduplication
   - Reasoning summary grouping
   - Timestamp fallback (ISO vs. Unix)
   - Markdown parsing
   
2. Message rendering - NONE
   - Text content rendering
   - Markdown to SwiftUI conversion
   - Code block syntax highlighting
   - Inline raw block handling
   
3. Timeline management - NONE
   - Scroll position tracking
   - Pagination logic
   - Update merging
   
4. State management - NONE
   - TimelineItem state
   - Deduplication tracking
   - Sheet/detail presentations

**Test Gaps** (CRITICAL):
- No snapshot tests for rendered markdown
- No edge case handling (empty messages, deeply nested markdown)
- No scroll behavior tests
- No memory/performance tests for large timelines (400 max messages)
- No update merging logic validation

### 3.4 Bridge UI Components (`/Bridge/`)

| File | Lines | Type | Coverage | Notes |
|------|-------|------|----------|-------|
| `BridgeManager.swift` | 255 | Manager | NONE | Connection management, state |
| `BonjourBrowser.swift` | ? | Service Discovery | NONE | mDNS/Bonjour discovery |
| `BridgeStatusChip.swift` | ? | View | NONE | Status indicator |
| `ManualConnectSheet.swift` | ? | View | NONE | Manual URL/port entry |

**Test Gaps**:
- No connection state transition tests
- No error recovery tests
- No Bonjour discovery simulation
- No UI state validation

### 3.5 History Components (`/History/`)

| File | Type | Coverage |
|------|------|----------|
| `LocalClaude.swift` | History Provider | NONE |
| `LocalCodex.swift` | History Provider | NONE |
| `HistorySidebar.swift` | View | NONE |

**Test Gaps**: No tests for history loading/display

### 3.6 Floating UI Components

| File | Type | Coverage |
|------|------|----------|
| `FloatingScrollButtons.swift` | View | NONE |
| `FloatingToolbar.swift` | View | NONE |
| `FloatingMenuButton.swift` | View | NONE |
| `FloatingMicButton.swift` | View | NONE |
| `FloatingToolbar.swift` | View | NONE |

**Test Gaps**: Gesture handling, state management not tested

### 3.7 Other UI Components

| File | Type | Coverage |
|------|------|----------|
| `GlassBar.swift` | View | NONE |
| `GlassHeader.swift` | View | NONE |
| `TopEdgeGradient.swift` | View | NONE |
| `ThreadHeaderView.swift` | View | NONE |
| `RawThreadView.swift` | View | NONE |

### 3.8 Supporting Components

| File | Type | Coverage |
|------|------|----------|
| `FontLoader.swift` | Utility | NONE |
| `TitleCache.swift` | Cache | NONE |
| `FMProbe.swift` | System Check | NONE |

### 3.9 Examples (Not for testing)

| File | Purpose |
|------|---------|
| `ChatTabsDemo.swift` | Feature demo |
| `GlassTerminalCard.swift` | Component showcase |

---

## 4. DATA FLOW & ARCHITECTURE

### 4.1 Connection Flow (Mobile)

```
OpenAgentsApp
  ↓
ContentView
  ├→ BridgeManager.start()
  │   ├→ MobileWebSocketClient.connect()
  │   │   ├→ sendInitialize() [ACP protocol]
  │   │   ├→ waitForInitializeResponse()
  │   │   └→ receiveLoop() [JSON-RPC]
  │   │
  │   └→ BonjourBrowser.start() [if multicast enabled]
  │
  └→ AcpThreadView
      ├→ Fetch latest thread
      └→ computeTimeline()
          ├→ Parse JSONL
          ├→ Deduplicate
          ├→ Group reasoning
          └→ Render to SwiftUI
```

**Test Gaps**:
- No integration tests for full connection flow
- No mock WebSocket server for offline testing
- No state synchronization tests

### 4.2 Message Update Flow

```
Bridge (JSON-RPC notification)
  ↓
MobileWebSocketClient.didReceiveJSONRPCNotification()
  ↓
BridgeManager (parses SessionNotificationWire)
  ├→ Updates: @Published updates property
  └→ UI observes changes
      ↓
      AcpThreadView observes updates
      └→ Merges into timeline
```

**Test Gaps**:
- No end-to-end message flow tests
- No update ordering/race condition tests
- No observable binding tests

### 4.3 Data Models Hierarchy

```
ACP Protocol (Rust SDK mirror)
  ├→ ACP.Agent (outbound: requests)
  ├→ ACP.Client (inbound: updates, requests to handle)
  └→ JSONRPC (transport wrapper)

App Models (OpenAgents)
  ├→ ACPMessage, ACPEvent, ACPThread
  ├→ ACPToolCall, ACPToolResult
  ├→ ACPPlanState
  └→ ThreadSummary
```

---

## 5. KEY FILES NEEDING TEST COVERAGE

### Priority 1: CRITICAL (Communication & Protocol)

1. **MobileWebSocketClient.swift** (300 lines)
   - Initialize handshake
   - Message send/receive loop
   - JSON-RPC request/response matching
   - Connection state machine
   - Error recovery

2. **DesktopWebSocketServer.swift** (400 lines)
   - Client acceptance
   - Handshake validation
   - Message broadcast
   - Connection cleanup

3. **BridgeManager.swift** (255 lines)
   - Connection state transitions
   - Update buffering (ring buffer)
   - Delegate callbacks
   - iOS: Bonjour discovery
   - macOS: Server management

4. **AcpThreadView.swift** (1550 lines)
   - Full timeline computation and rendering
   - Message deduplication
   - Markdown parsing and rendering
   - Update merging
   - Scroll behavior
   - State management

### Priority 2: HIGH (Protocol Compliance & Data)

1. **ACP Protocol Files** (all of `/AgentClientProtocol/`)
   - Encoding/decoding all variants
   - Protocol version negotiation
   - Error handling
   - Session lifecycle

2. **ACP Models** (all of `/ACP/`)
   - JSONValue codec completeness
   - Content variant conversions
   - Nested structure handling

3. **ToolCallView.swift** (100 lines)
   - Shell command parsing
   - JSON prettification
   - Edge cases (null args, empty calls)

### Priority 3: MEDIUM (Integration & UI)

1. **ContentView.swift** (142 lines)
   - Layout composition (mobile vs. macOS)
   - Title handling
   - Markdown stripping

2. **BonjourBrowser.swift**
   - Service discovery
   - Timeout handling
   - Error recovery

3. **UI Component Tests**
   - Snapshot tests for views
   - Gesture handling
   - State management

### Priority 4: LOW (Utilities & Supporting)

1. **Translators** (Claude/Codex)
   - JSONL format edge cases
   - Incremental parsing
   - Error handling

2. **Providers** (Scanners, Discovery)
   - Binary discovery
   - Fallback mechanisms

3. **History Components**
   - Loading/caching
   - Display logic

---

## 6. TESTING GAPS BY CATEGORY

### A. Unit Tests

**Missing (Priority 1)**:
- [ ] MobileWebSocketClient: handshake variants, reconnection, timeout
- [ ] DesktopWebSocketServer: client lifecycle, message ordering, broadcast
- [ ] BridgeManager: state transitions, update buffering, thread safety
- [ ] ToolCallView: shell parsing edge cases, JSON formatting
- [ ] ACP Protocol: all variant encoding/decoding paths

**Missing (Priority 2)**:
- [ ] ACP Models: JSONValue codec, nested structures
- [ ] ContentView: title sanitization, layout logic
- [ ] Translators: malformed input, incremental parsing
- [ ] Providers: discovery failures, fallbacks

### B. Integration Tests

**Missing**:
- [ ] Full connection flow (handshake → load → update stream)
- [ ] Message update ordering and deduplication
- [ ] State synchronization (BridgeManager → AcpThreadView)
- [ ] Markdown rendering pipeline
- [ ] History loading and display

### C. UI Tests

**Missing**:
- [ ] Snapshot tests for message rendering
- [ ] Scroll behavior and pagination
- [ ] Gesture handling (floating buttons, taps)
- [ ] Sheet/modal presentations
- [ ] Accessibility testing

### D. Performance Tests

**Missing**:
- [ ] Timeline computation with max 400 messages
- [ ] Update merging performance
- [ ] Memory usage with long-running connections
- [ ] Scroll performance with large lists

### E. Protocol Compliance Tests

**Missing**:
- [ ] ACP version negotiation
- [ ] All ContentBlock variants
- [ ] All SessionUpdate variants
- [ ] Error response handling
- [ ] Timeout scenarios

---

## 7. ARCHITECTURE OBSERVATIONS

### Strengths
1. **Strong OpenAgentsCore tests** - Protocol layer well-validated
2. **Separation of concerns** - Bridge, ACP, UI layers distinct
3. **Type safety** - Swift Codable for all protocol types
4. **Observable pattern** - SwiftUI binding via @Published

### Weaknesses
1. **Minimal UI testing** - Large components (AcpThreadView) lack tests
2. **No integration tests** - End-to-end flows untested
3. **Single large view** - AcpThreadView (1550 lines) needs decomposition
4. **Limited error testing** - Edge cases and failures not covered
5. **No mocking framework** - Would need for bridge simulation
6. **Performance untested** - Memory and scroll performance unknown

---

## 8. RECOMMENDED TEST STRATEGY

### Phase 1: Critical Infrastructure (Week 1-2)

1. **Mock WebSocket Framework**
   - Create MockMobileWebSocketClient
   - Create MockDesktopWebSocketServer
   - Shared test fixtures for messages

2. **BridgeManager Unit Tests**
   - State transitions
   - Update buffering
   - iOS/macOS divergent paths

3. **MobileWebSocketClient Tests**
   - Initialization sequence
   - Message send/receive
   - JSON-RPC request/response

### Phase 2: Protocol & Model Validation (Week 2-3)

1. **Comprehensive ACP Protocol Tests**
   - All variant paths
   - Encoding/decoding roundtrips
   - Error cases

2. **Data Model Tests**
   - JSONValue codec
   - Content variants
   - Nested structures

3. **ToolCallView Tests**
   - Shell command parsing
   - JSON prettification
   - Edge cases

### Phase 3: Integration & UI (Week 3-4)

1. **Integration Tests**
   - Full connection flow
   - Message pipeline
   - Timeline computation

2. **UI Snapshot Tests**
   - Message rendering
   - Tool/result display
   - Plan states

3. **Interaction Tests**
   - Scroll behavior
   - Gesture handling
   - State updates

### Phase 4: Performance & Compliance (Week 4+)

1. **Performance Tests**
   - Large timeline rendering
   - Memory profiling
   - Scroll performance

2. **Protocol Compliance**
   - Version negotiation
   - Error handling
   - Timeout scenarios

---

## 9. TEST FILE CHECKLIST

### Must Create

**OpenAgentsCore** (unit tests):
- [ ] `AgentClientProtocol/ACPProtocolTests.swift` - All protocol variants
- [ ] `AgentClientProtocol/ACPRPCTests.swift` - JSON-RPC completeness
- [ ] `AgentClientProtocol/ACPSessionTests.swift` - Session lifecycle
- [ ] `Bridge/BridgeConfigTests.swift` - Configuration validation
- [ ] `Providers/ClaudeCodeScannerTests.swift` - New provider

**OpenAgents** (app unit + integration):
- [ ] `Bridge/BridgeManagerTests.swift` - State machine, update buffer
- [ ] `Bridge/BridgeIntegrationTests.swift` - Connection flow
- [ ] `Bridge/MobileWebSocketClientTests.swift` - WebSocket lifecycle
- [ ] `ACP/ToolCallViewTests.swift` - Shell/JSON parsing
- [ ] `AcpThreadViewTests.swift` - Timeline computation deep dive
- [ ] `AcpThreadViewRenderingTests.swift` - Markdown rendering snapshots
- [ ] `ContentViewTests.swift` - Layout and title logic

**UI Tests**:
- [ ] `MessageRenderingUITests.swift` - Snapshot tests
- [ ] `ScrollBehaviorUITests.swift` - Scroll and pagination
- [ ] `FloatingUITests.swift` - Gesture handling

**Supporting**:
- [ ] `Mocks/MockWebSocketClient.swift` - Test double
- [ ] `Mocks/MockWebSocketServer.swift` - Test double
- [ ] `Fixtures/TestData.swift` - Shared test data

---

## 10. SUMMARY TABLE

| Category | Coverage | Priority | Gap Severity |
|----------|----------|----------|--------------|
| ACP Protocol Core | PARTIAL | HIGH | Medium |
| Bridge Communication | MINIMAL | CRITICAL | High |
| BridgeManager | NONE | CRITICAL | High |
| AcpThreadView | MINIMAL | CRITICAL | High |
| UI Components | NONE | HIGH | Medium |
| Data Models | TESTED | MEDIUM | Low |
| Translators | TESTED | MEDIUM | Low |
| Providers | PARTIAL | MEDIUM | Low |
| Integration Flow | NONE | CRITICAL | High |
| Performance | NONE | MEDIUM | Medium |

---

## Conclusion

The iOS codebase has strong test coverage for the OpenAgentsCore framework (ACP protocol types, translators, providers), with 14 dedicated test files. However, the OpenAgents app layer (UI, bridge management, integration) has minimal test coverage with only 2 functional test files focusing on a single timeline function.

**Critical gaps** are in:
1. Bridge communication layer (WebSocket client/server)
2. BridgeManager state management
3. AcpThreadView rendering (1550 lines, 23% of app code)
4. Message/update pipeline integration
5. UI interaction testing

**Recommended action**: Focus Phase 1-2 effort on bridge infrastructure tests and protocol compliance, then move to integration and UI tests.
