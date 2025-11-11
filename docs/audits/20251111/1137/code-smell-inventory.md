# OpenAgents Code Smell Inventory
**Date:** November 11, 2025
**Purpose:** Detailed catalog of specific code issues with actionable recommendations

## Overview

This document catalogs **specific code smells** found in the OpenAgents codebase. Each entry includes:
- **Location:** File path and line numbers
- **Smell Type:** Category (coupling, cohesion, complexity, naming, etc.)
- **Severity:** 🔥 Critical, 🟡 High, 🟢 Medium, 🔵 Low
- **Description:** What's wrong
- **Impact:** Why it matters
- **Recommendation:** How to fix it
- **Estimated Effort:** Time to fix

## Summary Statistics

| Severity | Count | % of Total |
|----------|-------|------------|
| 🔥 Critical | 0 | 0% |
| 🟡 High | 3 | 20% |
| 🟢 Medium | 8 | 53% |
| 🔵 Low | 4 | 27% |
| **Total** | **15** | **100%** |

**Good News:** No critical file size issues. The codebase is generally well-structured with reasonable file sizes.

---

## High-Severity Issues (🟡)

### H1: ChatAreaView Growing Too Large
**Severity:** 🟡 High
**Location:** `ios/OpenAgents/Views/macOS/ChatAreaView.swift` (672 LOC)
**Smell Type:** God Object / Lack of Cohesion
**Priority:** Address in Phase 1 (Week 2)

**Description:**
`ChatAreaView` handles too many responsibilities:
- Chat timeline layout
- Message row rendering for all message types
- Scroll position management
- Empty state handling
- Loading state handling

**Impact:**
- Hard to test individual message renderers
- SwiftUI preview compilation slowdown
- High cognitive load when modifying
- Risk of merge conflicts

**Recommendation:**
Extract message row components:
```swift
// Before
struct ChatAreaView: View {
  var body: some View {
    ScrollView {
      ForEach(messages) { message in
        switch message.type {
        case .assistant:
          // 50+ lines of rendering logic
        case .tool:
          // 50+ lines of rendering logic
        // ... more cases
        }
      }
    }
  }
}

// After
struct ChatAreaView: View {
  var body: some View {
    ScrollView {
      ForEach(messages) { message in
        MessageRowView(message: message)
      }
    }
  }
}

struct MessageRowView: View {
  let message: Message

  var body: some View {
    switch message.type {
    case .assistant: AssistantMessageRow(message: message)
    case .tool: ToolCallRow(message: message)
    // ... delegate to specialized renderers
    }
  }
}
```

**Files to Create:**
- `MessageRowView.swift` (< 100 LOC)
- `AssistantMessageRow.swift` (< 100 LOC)
- `ToolCallRow.swift` (< 100 LOC)
- `ThinkingBlockRow.swift` (< 100 LOC)

**Estimated Effort:** 2 days

---

### H2: BridgeManager Has Too Many Dependencies
**Severity:** 🟡 High
**Location:** `ios/OpenAgents/Bridge/BridgeManager.swift`
**Smell Type:** Feature Envy / Excessive Coupling
**Priority:** Address in Phase 1 (Week 2)

**Description:**
`BridgeManager` knows about too many dependencies:
- `PromptDispatcher`
- `TimelineStore`
- `ConnectionManager`
- `ConversationSummarizer`
- Multiple platform-specific services

**Impact:**
- Hard to test (need to mock all dependencies)
- Changes to any dependency may require BridgeManager changes
- Violates Single Responsibility Principle

**Recommendation:**
Introduce coordinator protocol and message bus:
```swift
// Define protocol
protocol BridgeCoordinator {
  func handlePrompt(_ prompt: String) async throws
  func handleSessionUpdate(_ update: ACP.Client.SessionUpdate) async
}

// Use message bus for decoupling
class BridgeManager: BridgeCoordinator {
  private let messageBus: MessageBus  // Combine publisher
  private let promptDispatcher: PromptDispatcher  // Keep minimal dependencies

  init(messageBus: MessageBus, promptDispatcher: PromptDispatcher) {
    self.messageBus = messageBus
    self.promptDispatcher = promptDispatcher
  }

  func handleSessionUpdate(_ update: ACP.Client.SessionUpdate) async {
    // Broadcast to message bus instead of calling dependents directly
    await messageBus.publish(event: .sessionUpdate(update))
  }
}

// Subscribers listen to message bus
class TimelineStore {
  init(messageBus: MessageBus) {
    messageBus.subscribe(to: .sessionUpdate) { [weak self] update in
      self?.handleUpdate(update)
    }
  }
}
```

**Estimated Effort:** 2 days

---

### H3: Missing Plugin, Marketplace, Payment Infrastructure
**Severity:** 🟡 High
**Location:** N/A (missing code)
**Smell Type:** Missing Abstraction
**Priority:** Address in Phases 2-4 (Weeks 3-12)

**Description:**
No infrastructure exists for:
- Dynamic plugin loading and sandboxing
- Marketplace discovery and installation
- Billing, usage tracking, and revenue sharing

**Impact:**
- Can't deliver features #6, #9, #10 from Episode 195
- Can't grow plugin ecosystem
- Can't monetize

**Recommendation:**
Build from scratch following the roadmap in `refactoring-roadmap.md`:
- Phase 2: Plugin system (4 weeks)
- Phase 3: Marketplace (3 weeks)
- Phase 4: Payments (3 weeks)

**Estimated Effort:** 10 weeks

---

## Medium-Severity Issues (🟢)

### M1: DesktopWebSocketServer Growing Large
**Severity:** 🟢 Medium
**Location:** `OpenAgentsCore/DesktopBridge/DesktopWebSocketServer.swift` (880 LOC)
**Smell Type:** God Object
**Priority:** Monitor, refactor if grows beyond 1000 LOC

**Description:**
Main file is 880 LOC. Already split via extensions, but still dense.

**Current Structure:**
```
DesktopWebSocketServer.swift           # 880 LOC
DesktopWebSocketServer+Session.swift   # Session handlers
DesktopWebSocketServer+Threads.swift   # Thread handlers
... (6 more extensions)
```

**Impact:**
- Moderate cognitive load
- Acceptable for now, but watch for growth

**Recommendation:**
Current extension pattern is good. Consider further extraction if main file exceeds 1000 LOC:
```swift
// Extract protocol
protocol WebSocketServerDelegate {
  func handle(request: JSONRPCRequest) async throws -> JSONRPCResponse
}

// Delegate to separate handlers
class DesktopWebSocketServer {
  private let sessionHandler: SessionHandler
  private let threadHandler: ThreadHandler
  // ...

  func handle(request: JSONRPCRequest) async throws -> JSONRPCResponse {
    switch request.method {
    case "session/new": return await sessionHandler.handle(request)
    case "session/prompt": return await sessionHandler.handle(request)
    // ...
    }
  }
}
```

**Estimated Effort:** 1 day (only if needed)

---

### M2: OrchestrationConfig Growing Complex
**Severity:** 🟢 Medium
**Location:** `OpenAgentsCore/Orchestration/OrchestrationConfig.swift` (441 LOC)
**Smell Type:** Data Clump / Complex Configuration
**Priority:** Monitor, consider splitting at 600 LOC

**Description:**
`OrchestrationConfig` has many nested structs:
- `Schedule`
- `Constraints`
- `AgentBias`
- `Goals`
- `PRAutomation`
- `TimeWindow`

**Impact:**
- Hard to understand full config schema
- Changes affect large surface area
- Acceptable for now

**Recommendation:**
If it grows beyond 600 LOC, split into separate files:
```
OrchestrationConfig/
├── OrchestrationConfig.swift      # Main config (< 200 LOC)
├── ScheduleConfig.swift           # Schedule settings
├── ConstraintsConfig.swift        # Constraints settings
├── AgentBiasConfig.swift          # Agent preferences
├── GoalsConfig.swift              # Goals settings
└── PRAutomationConfig.swift       # PR automation settings
```

**Estimated Effort:** 1 day (only if needed)

---

### M3: No Workspace Memory System
**Severity:** 🟢 Medium
**Location:** N/A (missing code)
**Smell Type:** Missing Abstraction
**Priority:** Address in Phase 5 (Weeks 13-15)

**Description:**
No persistent workspace-level memory:
- No knowledge graph of entities and relations
- No convention detection
- No long-term goal tracking
- No cross-session insights

**Impact:**
- Agents can't learn workspace conventions
- Can't remember "Alice usually works on auth"
- Can't track progress on long-term goals

**Recommendation:**
Build workspace memory system following roadmap:
- `WorkspaceKnowledgeGraph` - Entity-relation graph
- `ConventionDetector` - Detect code style, architecture
- `WorkspaceGoals` - Track long-term goals
- `WorkspaceSummarizer` - FM-powered understanding

**Estimated Effort:** 3 weeks (Phase 5)

---

### M4: No Semantic History Search
**Severity:** 🟢 Medium
**Location:** N/A (missing code)
**Smell Type:** Missing Abstraction
**Priority:** Address in Phase 6 (Weeks 16-17)

**Description:**
History search is basic (ID, date range only):
- No semantic search ("show me sessions about auth")
- No natural language queries ("what did I work on last Monday?")
- No cross-workspace queries

**Impact:**
- Users can't find relevant sessions easily
- Poor discoverability of past work

**Recommendation:**
Enhance history system following roadmap:
- `SemanticHistorySearch` - Embedding-based search
- `SmartHistoryQuery` - Natural language to SQL/semantic
- `HybridSearch` - Combine FTS + embeddings
- `TemporalQueryParser` - Parse "last Monday", "past 2 weeks"

**Estimated Effort:** 2 weeks (Phase 6)

---

### M5: No PR Automation Implementation
**Severity:** 🟢 Medium
**Location:** `OpenAgentsCore/Orchestration/OrchestrationConfig.swift`
**Smell Type:** Dead Code / Incomplete Feature
**Priority:** Address in Phase 7 (Weeks 17-18)

**Description:**
`OrchestrationConfig.PRAutomation` exists but has no implementation:
- No git operations (branch, commit, push)
- No GitHub/GitLab API integration
- No PR monitoring

**Impact:**
- Can't automatically create PRs after overnight runs
- Manual PR creation required

**Recommendation:**
Implement PR automation following roadmap:
- `GitOperations` - Shell out to git commands
- `GitHubClient` - GitHub API integration
- `PRService` - High-level PR operations
- `PRMonitor` - Track PR status

**Estimated Effort:** 1 week (Phase 7)

---

### M6: Platform-Specific Code Scattered
**Severity:** 🟢 Medium
**Location:** Throughout `OpenAgentsCore`
**Smell Type:** Platform Coupling
**Priority:** Accept for now, refactor if iOS orchestration needed

**Description:**
`#if os(macOS)` guards scattered throughout:
- Orchestration is macOS-only
- Some file system operations have platform guards
- Terminal operations are macOS-only

**Example:**
```swift
#if os(macOS)
func startOrchestration() async throws {
  // macOS-only orchestration
}
#endif
```

**Impact:**
- Limits iOS functionality
- Makes code harder to reason about
- Acceptable for now (orchestration is desktop-first)

**Recommendation:**
If iOS orchestration is needed:
1. Extract protocol:
```swift
protocol OrchestrationPlatform {
  func checkConstraints() async -> Bool
  func executeTask(_ task: OvernightTask) async throws
}

class MacOSOrchestrationPlatform: OrchestrationPlatform { ... }
class IOSOrchestrationPlatform: OrchestrationPlatform { ... }
```

2. Inject platform at runtime:
```swift
let platform: OrchestrationPlatform = {
  #if os(macOS)
  return MacOSOrchestrationPlatform()
  #else
  return IOSOrchestrationPlatform()
  #endif
}()
```

**Estimated Effort:** 2 days (only if iOS orchestration needed)

---

### M7: Missing Test Coverage for Settings
**Severity:** 🟢 Medium
**Location:** `ios/OpenAgents/Views/macOS/Settings/`
**Smell Type:** Untested Code
**Priority:** Address in Phase 1 (Week 2)

**Description:**
Settings views have no tests:
- `SettingsView` - No tests
- Connection tab - No tests
- Workspace tab - No tests
- Agents tab - No tests
- Orchestration tab - No tests

**Impact:**
- Can't verify settings persistence
- Can't verify validation logic
- Risk of regressions

**Recommendation:**
Add focused tests:
```swift
class ConnectionTabTests: XCTestCase {
  func testConnectionSettingsPersistence() {
    // Given: User changes connection settings
    // When: Settings saved
    // Then: Settings persisted to UserDefaults
  }
}

class WorkspaceTabTests: XCTestCase {
  func testWorkspacePathValidation() {
    // Given: User enters invalid workspace path
    // When: User saves
    // Then: Error shown, settings not saved
  }
}
```

**Estimated Effort:** 1 day (Phase 1)

---

### M8: No Embeddings Test Coverage
**Severity:** 🟢 Medium
**Location:** `OpenAgentsCore/Embeddings/`
**Smell Type:** Untested Code
**Priority:** Address in Phase 8 (Week 18-19)

**Description:**
Embeddings module has minimal tests:
- `EmbeddingService` - No integration tests
- `MLXEmbeddingProvider` - No tests
- `VectorStore` - Basic tests only

**Impact:**
- Can't verify MLX-Swift integration works
- Can't verify vector search accuracy
- Risk of silent failures

**Recommendation:**
Add comprehensive tests:
```swift
class EmbeddingServiceIntegrationTests: XCTestCase {
  func testEmbedAndSearch() async throws {
    let service = EmbeddingService()

    // Index documents
    try await service.index(id: "doc1", text: "Swift programming language")
    try await service.index(id: "doc2", text: "Python programming language")
    try await service.index(id: "doc3", text: "Apple fruit nutrition")

    // Search
    let results = try await service.search(query: "coding languages", topK: 2)

    // Verify
    XCTAssertEqual(results.count, 2)
    XCTAssertTrue(results.contains { $0.id == "doc1" })
    XCTAssertTrue(results.contains { $0.id == "doc2" })
  }
}
```

**Estimated Effort:** 1 day (Phase 8)

---

## Low-Severity Issues (🔵)

### L1: Naming Inconsistency: "Manager" Suffix
**Severity:** 🔵 Low
**Location:** Various
**Smell Type:** Inconsistent Naming
**Priority:** Low (cosmetic)

**Description:**
Inconsistent use of "Manager" suffix:
- `BridgeManager` ✓
- `TinyvexManager` ✓
- `ConnectionManager` ✓
- `AgentRegistry` (not `AgentManager`)
- `TaskQueue` (not `TaskManager`)

**Impact:**
- Minor cognitive load
- Not a functional issue

**Recommendation:**
Accept inconsistency or standardize:
- Use "Manager" for coordinators (BridgeManager, TinyvexManager)
- Use specific names for specialized roles (Registry, Queue, Service)

**Estimated Effort:** N/A (accept as-is)

---

### L2: No UI Tests
**Severity:** 🔵 Low
**Location:** `ios/OpenAgentsTests/`
**Smell Type:** Missing Tests
**Priority:** Low (nice-to-have)

**Description:**
No UI tests (XCTest UI Testing):
- No tests for critical flows (install plugin, create session, etc.)
- Only unit and integration tests

**Impact:**
- Can't verify end-to-end user flows
- Risk of UI regressions

**Recommendation:**
Add UI tests for critical paths:
```swift
class MarketplaceFlowUITests: XCTestCase {
  func testInstallPlugin() throws {
    let app = XCUIApplication()
    app.launch()

    // Navigate to marketplace
    app.buttons["Marketplace"].tap()

    // Search
    let searchField = app.searchFields["Search"]
    searchField.tap()
    searchField.typeText("Slack")

    // Install
    app.buttons["Install"].tap()

    // Verify
    XCTAssertTrue(app.staticTexts["Slack Integration"].exists)
  }
}
```

**Estimated Effort:** 3 days (Phase 8, optional)

---

### L3: No Nostr Test Coverage
**Severity:** 🔵 Low
**Location:** `OpenAgentsCore/Nostr/`
**Smell Type:** Untested Code
**Priority:** Low (Nostr is low-priority feature)

**Description:**
Nostr integration has minimal tests:
- `NostrRelayManager` - No tests
- `NostrEventFeedManager` - No tests

**Impact:**
- Can't verify Nostr integration works
- Low priority (Nostr is not critical path)

**Recommendation:**
Add tests if Nostr becomes more important:
```swift
class NostrRelayManagerTests: XCTestCase {
  func testConnectToRelay() async throws {
    let manager = NostrRelayManager()
    try await manager.connect(relay: "wss://relay.example.com")
    XCTAssertTrue(manager.isConnected)
  }
}
```

**Estimated Effort:** 1 day (low priority)

---

### L4: Magic Numbers in Code
**Severity:** 🔵 Low
**Location:** Various
**Smell Type:** Magic Numbers
**Priority:** Low (cosmetic)

**Description:**
Some magic numbers without named constants:
```swift
// Example (hypothetical)
let timeout = 30.0  // 30 seconds
let maxRetries = 3
let defaultPort = 8765
```

**Impact:**
- Reduces readability
- Harder to change values consistently

**Recommendation:**
Extract to named constants:
```swift
enum Constants {
  static let defaultTimeout: TimeInterval = 30.0
  static let maxRetries = 3
  static let defaultPort = 8765
}

let timeout = Constants.defaultTimeout
```

**Estimated Effort:** 1 day (low priority, optional)

---

## Positive Patterns (Keep Doing)

### ✅ P1: Actor-Based Concurrency
**Location:** Throughout `OpenAgentsCore`
**Pattern:** Use Swift actors for thread-safe state management

**Examples:**
- `AgentRegistry`
- `AgentCoordinator`
- `TaskQueue`
- `DecisionEngine`
- `SessionUpdateHub`

**Why It's Good:**
- Compile-time thread safety guarantees
- No manual locking
- Clean async/await integration

**Recommendation:** Continue using actors for all shared mutable state.

---

### ✅ P2: Protocol-Oriented Design
**Location:** `OpenAgentsCore/Agents/`, `OpenAgentsCore/Payments/`
**Pattern:** Define protocols for extensibility

**Examples:**
- `AgentProvider` - Base protocol for all agents
- `PaymentProvider` - Protocol for payment methods
- `EmbeddingProvider` - Pluggable embedding backends

**Why It's Good:**
- Easy to add new implementations
- Testable (can mock protocols)
- Swifty

**Recommendation:** Continue defining protocols for all extensible systems.

---

### ✅ P3: Dependency Injection
**Location:** Throughout codebase
**Pattern:** Inject dependencies via initializers, not singletons

**Example:**
```swift
let coordinator = AgentCoordinator(
  taskQueue: taskQueue,
  decisionEngine: decisionEngine,
  agentRegistry: agentRegistry
)
```

**Why It's Good:**
- Testable (can inject mocks)
- Explicit dependencies
- No hidden global state

**Recommendation:** Continue injecting all dependencies explicitly.

---

### ✅ P4: LLM-First Policy
**Location:** Orchestration, Summarization, Decision-making
**Pattern:** Use Foundation Models for interpretation, not deterministic heuristics

**Examples:**
- `DecisionEngine` - FM-powered task selection
- `ConversationSummarizer` - FM-powered title generation
- `FMOrchestrator` - FM-powered tool calling

**Why It's Good:**
- More intelligent than rule-based systems
- Future-proof (models improve over time)
- Privacy-preserving (on-device)

**Recommendation:** Continue using FM for all interpretation. Avoid deterministic heuristics unless explicitly requested.

---

### ✅ P5: Comprehensive Test Coverage
**Location:** `OpenAgentsCoreTests/`
**Pattern:** ~70% test coverage on core modules

**Examples:**
- `BridgeServerClientTests` - E2E bridge tests
- `AgentRegistryTests` - Unit tests
- `MessageClassificationRegressionTests` - Regression tests

**Why It's Good:**
- Prevents regressions
- Enables confident refactoring
- Documents expected behavior

**Recommendation:** Maintain 70%+ coverage on all new code.

---

## Action Plan Summary

### Phase 1 (Week 2) - Code Quality
1. Extract `ChatAreaView` components (2 days)
2. Decouple `BridgeManager` (2 days)
3. Add Settings test coverage (1 day)

### Phases 2-4 (Weeks 3-12) - Core Infrastructure
4. Build plugin system (4 weeks)
5. Build marketplace (3 weeks)
6. Build payments (3 weeks)

### Phase 5 (Weeks 13-15) - Workspace Memory
7. Build workspace memory system (3 weeks)

### Phase 6 (Weeks 16-17) - Smart History
8. Enhance history with semantic search (2 weeks)

### Phase 7 (Weeks 17-18) - PR Automation
9. Implement PR automation (1 week)

### Phase 8 (Weeks 18-19) - Polish
10. Add embeddings tests (1 day)
11. Add UI tests (optional, 3 days)

---

## Conclusion

The OpenAgents codebase is in **good health** with no critical issues. Key findings:

**Strengths:**
- ✅ Well-sized files (no 1000+ LOC monsters)
- ✅ Actor-based concurrency for thread safety
- ✅ Protocol-oriented design for extensibility
- ✅ LLM-first policy for intelligence
- ✅ Comprehensive test coverage (~70%)

**Gaps:**
- Missing plugin/marketplace/payment infrastructure (expected for v0.3)
- Missing workspace memory and smart history (roadmap items)
- Some code quality improvements needed (ChatAreaView, BridgeManager)

**Next Steps:**
1. Review this inventory with team
2. Prioritize issues (focus on H1, H2 first)
3. Create tracking issues for each item
4. Begin refactoring per `refactoring-roadmap.md`

---

**Document Version:** 1.0
**Last Updated:** November 11, 2025
**Total Issues:** 15 (0 critical, 3 high, 8 medium, 4 low)
