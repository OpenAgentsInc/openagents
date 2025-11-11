# OpenAgents Module Boundaries & Responsibilities
**Date:** November 11, 2025
**Purpose:** Define clear ownership, responsibilities, and dependencies for each module

## Overview

This document establishes **clear module boundaries** for the OpenAgents codebase. Each module has:
- **Single Responsibility** - Does one thing well
- **Clear Interface** - Well-defined public API
- **Minimal Dependencies** - Depends on few other modules
- **High Cohesion** - Components within module are tightly related
- **Low Coupling** - Modules are loosely coupled to each other

## Module Dependency Graph

```
┌─────────────────────────────────────────────────────────────────┐
│                          App Layer                               │
│  Views → ViewModels → Managers (Bridge, Tinyvex, Plugin, etc.)  │
└─────────────────────────────────────────────────────────────────┘
                               ↓
┌─────────────────────────────────────────────────────────────────┐
│                     Core Business Logic                          │
│                                                                  │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐     │
│  │ ACP Protocol │ ← │ Agents       │ ← │ Orchestration│     │
│  └──────────────┘    └──────────────┘    └──────────────┘     │
│         ↑                    ↑                     ↑            │
│         │                    │                     │            │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐     │
│  │ Bridge       │    │ Plugins      │    │ Marketplace  │     │
│  └──────────────┘    └──────────────┘    └──────────────┘     │
│         ↑                    ↑                     ↑            │
│         │                    │                     │            │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐     │
│  │ History      │    │WorkspaceMem  │    │ Payments     │     │
│  └──────────────┘    └──────────────┘    └──────────────┘     │
│         ↑                    ↑                     ↑            │
│         │                    │                     │            │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │         Foundation Modules (Used by all above)           │  │
│  │  Tinyvex | Embeddings | Summarization | Utils | Log     │  │
│  └──────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

## Core Modules (Existing)

### ACP (Agent Client Protocol)

**Location:** `OpenAgentsCore/AgentClientProtocol/`, `OpenAgentsCore/ACP/`

**Responsibility:** Define canonical types and protocols for agent communication.

**Public Interface:**
```swift
// Core types
struct ACP.Client.SessionUpdate
struct ACP.Agent.ToolCall
struct ACP.Agent.ContentBlock
enum ACP.Agent.SessionMode

// Extensions
extension ACP.Client.SessionUpdate {
  var content: [ContentBlock]
  var toolCalls: [ToolCall]
}
```

**Dependencies:** None (foundation module)

**Dependents:** Agents, Bridge, Translators, Orchestration

**Key Principles:**
- ✅ **Canonical:** Source of truth for ACP types
- ✅ **Immutable:** Types should be value types (structs, enums)
- ✅ **Codable:** All types conform to Codable for JSON serialization
- ❌ **No Business Logic:** Pure data types only

---

### Bridge

**Location:** `OpenAgentsCore/Bridge/`, `OpenAgentsCore/DesktopBridge/`, `OpenAgentsCore/MobileBridge/`

**Responsibility:** JSON-RPC 2.0 transport over WebSocket for iOS ↔ macOS communication.

**Public Interface:**
```swift
// Server (macOS)
actor DesktopWebSocketServer {
  func start(port: Int) async throws
  func stop() async
  func register(method: String, handler: (JSON) async throws -> JSON)
}

// Client (iOS)
actor MobileWebSocketClient {
  func connect(host: String, port: Int) async throws
  func sendRequest(_ request: JSONRPCRequest) async throws -> JSONRPCResponse
  func subscribe(notification: String, handler: (JSON) -> Void)
}

// Types
struct JSONRPCRequest: Codable
struct JSONRPCResponse: Codable
struct JSONRPCNotification: Codable
```

**Dependencies:** ACP (for session updates), Tinyvex (for persistence)

**Dependents:** BridgeManager (app), Agents (for session updates)

**Key Principles:**
- ✅ **Protocol-Agnostic:** Can swap WebSocket for other transports
- ✅ **Typed:** Use Swift types, not raw JSON
- ✅ **Async/Await:** All methods are async
- ✅ **Actor-Safe:** Actors prevent data races

**Boundaries:**
- ✅ **IN SCOPE:** Transport, serialization, method routing
- ❌ **OUT OF SCOPE:** Business logic (handled by registered handlers)

---

### Agents

**Location:** `OpenAgentsCore/Agents/`

**Responsibility:** Agent provider system for registering and managing agents (Codex, Claude, OpenAgents).

**Public Interface:**
```swift
// Registry
actor AgentRegistry {
  func register(provider: AgentProvider)
  func unregister(providerId: String)
  func lookup(mode: ACPSessionModeId) -> AgentProvider?
  func available() -> [AgentProvider]
}

// Protocol
protocol AgentProvider {
  var id: String { get }
  var name: String { get }
  var capabilities: AgentCapabilities { get }
  func isAvailable() async -> Bool
  func start(config: SessionConfig) async throws -> SessionHandle
  func cancel(handle: SessionHandle) async throws
}

// Implementations
class CodexAgentProvider: AgentProvider
class ClaudeCodeAgentProvider: AgentProvider
class OpenAgentsLocalProvider: AgentProvider
```

**Dependencies:** ACP (for session types), Bridge (for session updates via SessionUpdateHub)

**Dependents:** Orchestration (for agent execution), BridgeManager (for agent discovery)

**Key Principles:**
- ✅ **Plugin Architecture:** Easy to add new agents
- ✅ **Protocol-Oriented:** AgentProvider protocol for all agents
- ✅ **Actor-Safe:** Registry is an actor
- ✅ **Async:** All operations are async

**Boundaries:**
- ✅ **IN SCOPE:** Agent lifecycle (start, cancel, status), capability detection
- ❌ **OUT OF SCOPE:** Agent implementation details (handled by providers)

---

### Orchestration

**Location:** `OpenAgentsCore/Orchestration/`

**Responsibility:** Overnight scheduling, task queue, and decision-making for long-running agent workflows.

**Public Interface:**
```swift
// Coordinator
actor AgentCoordinator {
  func start(config: OrchestrationConfig) async throws
  func stop() async
  func runCycle() async throws -> CycleResult
}

// Decision Engine
actor DecisionEngine {
  func decide(insights: SessionInsights, config: OrchestrationConfig) async throws -> DecisionOutput
}

// Task Queue
actor TaskQueue {
  func enqueue(_ task: OvernightTask) async throws
  func dequeue() async throws -> OvernightTask?
  func updateStatus(taskId: String, status: TaskStatus) async throws
}

// Scheduler
actor SchedulerService {
  func start(coordinator: AgentCoordinator, config: OrchestrationConfig) async throws
  func stop() async
}
```

**Dependencies:** Agents (for execution), Tinyvex (for task persistence), ACP (for session types)

**Dependents:** BridgeManager (via RPC), OrchestrationViewModel (for UI)

**Key Principles:**
- ✅ **Actor-Safe:** All components are actors
- ✅ **Persistent:** Tasks survive app restarts (stored in Tinyvex)
- ✅ **LLM-First:** Decision-making uses Foundation Models
- ✅ **Constraint-Aware:** Respects power, network, time window constraints

**Boundaries:**
- ✅ **IN SCOPE:** Scheduling, task queue, decision-making, cycle execution
- ❌ **OUT OF SCOPE:** Agent implementation (delegated to AgentRegistry)

---

### Tinyvex

**Location:** `OpenAgentsCore/Tinyvex/`

**Responsibility:** SQLite persistence for sessions, tasks, workspace memory, embeddings.

**Public Interface:**
```swift
// Client
class TinyvexClient {
  func open(path: String) throws
  func close()
  func execute(sql: String, params: [Any]) throws
  func query(sql: String, params: [Any]) throws -> [[String: Any]]
}

// Server (for migrations)
class TinyvexServer {
  func migrate(from: Int, to: Int) throws
}

// Schema
protocol TinyvexSchema {
  static var tableName: String { get }
  static var createTableSQL: String { get }
}
```

**Dependencies:** None (foundation module)

**Dependents:** All modules that need persistence (Orchestration, History, WorkspaceMemory, Payments)

**Key Principles:**
- ✅ **Schema Versioning:** Migrations for schema changes
- ✅ **Type-Safe:** Use Swift types, not raw SQL strings (via query builders)
- ✅ **Transactional:** Support transactions for multi-step operations
- ✅ **Performant:** Indexes on frequently queried columns

**Boundaries:**
- ✅ **IN SCOPE:** Database operations, schema migrations, query building
- ❌ **OUT OF SCOPE:** Business logic (handled by modules using Tinyvex)

---

### Embeddings

**Location:** `OpenAgentsCore/Embeddings/`

**Responsibility:** Semantic search via MLX-Swift embeddings.

**Public Interface:**
```swift
// Service
actor EmbeddingService {
  func embed(text: String) async throws -> [Float]
  func search(query: String, topK: Int) async throws -> [SearchResult]
  func index(id: String, text: String) async throws
}

// Provider protocol
protocol EmbeddingProvider {
  func embed(text: String) async throws -> [Float]
}

// Implementations
class MLXEmbeddingProvider: EmbeddingProvider

// Vector store
class VectorStore {
  func add(id: String, embedding: [Float])
  func search(query: [Float], topK: Int) -> [SearchResult]
}
```

**Dependencies:** None (foundation module, but uses MLX-Swift external dependency)

**Dependents:** History (for semantic search), WorkspaceMemory (for entity similarity)

**Key Principles:**
- ✅ **On-Device:** Runs entirely on device (privacy-preserving)
- ✅ **Cached:** Cache embeddings in Tinyvex to avoid recomputation
- ✅ **Incremental:** Only embed new content
- ✅ **Pluggable:** Can swap embedding models via EmbeddingProvider protocol

**Boundaries:**
- ✅ **IN SCOPE:** Embedding generation, vector storage, cosine similarity search
- ❌ **OUT OF SCOPE:** Content indexing strategy (handled by modules using embeddings)

---

### Summarization

**Location:** `OpenAgentsCore/Summarization/`

**Responsibility:** FM-powered conversation summaries and titles.

**Public Interface:**
```swift
// Summarizer
class ConversationSummarizer {
  func generateTitle(messages: [Message]) async throws -> String
  func generateSummary(messages: [Message], maxTokens: Int) async throws -> String
}

// FM wrapper
class FoundationModelSummarizer {
  func generate(prompt: String, instructions: String) async throws -> String
}
```

**Dependencies:** None (foundation module, but uses Apple Foundation Models)

**Dependents:** BridgeManager (for title generation), History (for session summaries)

**Key Principles:**
- ✅ **LLM-First:** Use Foundation Models, no deterministic heuristics
- ✅ **Fallback:** Provide deterministic fallback when FM unavailable
- ✅ **Cached:** Cache titles/summaries to avoid recomputation
- ✅ **Incremental:** Update summaries as conversation grows

**Boundaries:**
- ✅ **IN SCOPE:** Title/summary generation, FM interaction
- ❌ **OUT OF SCOPE:** When to summarize (handled by callers)

---

## New Modules (To Be Implemented)

### Plugins

**Location:** `OpenAgentsCore/Plugins/` (new)

**Responsibility:** Dynamic plugin loading, MCP integration, sandboxing, permissions.

**Public Interface:**
```swift
// Registry
actor PluginRegistry {
  func register(_ plugin: Plugin) async throws
  func unregister(pluginId: String) async throws
  func lookup(pluginId: String) -> Plugin?
  func all() -> [Plugin]
}

// Manifest
struct PluginManifest: Codable {
  let id: String
  let name: String
  let version: String
  let capabilities: [PluginCapability]
  let permissions: [Permission]
  let dependencies: [String: String]  // Package ID -> Version
}

// Loader
class PluginLoader {
  func load(path: URL) throws -> Plugin
  func unload(plugin: Plugin) throws
}

// Sandbox
class PluginSandbox {
  func execute<T>(_ plugin: Plugin, operation: () throws -> T) throws -> T
}

// MCP Client
class MCPClient {
  func connect(server: URL) async throws
  func listCapabilities() async throws -> [MCPCapability]
  func call(capability: String, params: JSON) async throws -> JSON
}
```

**Dependencies:** Agents (for AgentPlugin type), Tinyvex (for plugin metadata)

**Dependents:** Orchestration (for tool plugins), Marketplace (for plugin installation)

**Key Principles:**
- ✅ **Sandboxed:** Plugins run in isolated processes
- ✅ **Permission-Based:** User grants permissions (file, network, etc.)
- ✅ **Signed:** Plugins must be code-signed
- ✅ **Versioned:** Support multiple versions, rollback

**Boundaries:**
- ✅ **IN SCOPE:** Plugin loading/unloading, sandboxing, MCP client, permission enforcement
- ❌ **OUT OF SCOPE:** Plugin UI (handled by app layer), plugin business logic (handled by plugins)

**Security:**
- Code signing verification before loading
- Separate process with entitlements for sandboxing
- XPC or local socket for IPC
- User prompt for permissions (file access, network, etc.)

---

### Marketplace

**Location:** `OpenAgentsCore/Marketplace/` (new)

**Responsibility:** Discover, install, and manage third-party agents/plugins from marketplace.

**Public Interface:**
```swift
// Client
actor MarketplaceClient {
  func search(query: String, filters: Filters) async throws -> [PackageSummary]
  func detail(packageId: String) async throws -> PackageDetail
  func reviews(packageId: String) async throws -> [Review]
  func download(packageId: String) async throws -> URL
}

// Package
struct AgentPackage {
  let manifest: PluginManifest
  let checksum: String
  let signatureURL: URL
}

// Installer
class PackageInstaller {
  func install(package: AgentPackage) async throws -> Plugin
  func uninstall(pluginId: String) async throws
  func update(pluginId: String) async throws -> Plugin
}

// Version Manager
class VersionManager {
  func resolve(dependencies: [String: String]) throws -> [String: String]
  func rollback(pluginId: String, toVersion: String) async throws
}

// Reputation
class ReputationStore {
  func getReviews(packageId: String) async throws -> [Review]
  func addReview(packageId: String, review: Review) async throws
}
```

**Dependencies:** Plugins (for installation), Tinyvex (for package cache)

**Dependents:** App layer (MarketplaceManager, MarketplaceBrowserView)

**Key Principles:**
- ✅ **Secure:** Verify checksums and signatures before installing
- ✅ **Versioned:** Support semantic versioning, dependency resolution
- ✅ **Cached:** Cache package metadata locally
- ✅ **Offline-Friendly:** Browse cached packages offline

**Boundaries:**
- ✅ **IN SCOPE:** Package discovery, installation, version management, reputation
- ❌ **OUT OF SCOPE:** Package creation (handled by plugin authors)

**API Endpoints (Future):**
- `GET /api/v1/packages?q=<query>&category=<cat>` - Search
- `GET /api/v1/packages/<id>` - Detail
- `GET /api/v1/packages/<id>/reviews` - Reviews
- `GET /api/v1/packages/<id>/download` - Download URL

---

### Payments

**Location:** `OpenAgentsCore/Payments/` (new)

**Responsibility:** Billing, usage tracking, revenue sharing, subscription management.

**Public Interface:**
```swift
// Billing Service
actor BillingService {
  func balance() async -> Double  // Credits
  func charge(amount: Double, description: String) async throws
  func credit(amount: Double, description: String) async throws
  func usage(startDate: Date, endDate: Date) async throws -> [UsageEvent]
}

// Usage Tracker
actor UsageTracker {
  func track(event: UsageEvent) async throws
  func computeCost(event: UsageEvent) -> Double
}

// Revenue Splitter
class RevenueSplitter {
  func split(amount: Double, pluginId: String) -> (author: Double, platform: Double)
  func payout(authorId: String, amount: Double) async throws
}

// Payment Provider Protocol
protocol PaymentProvider {
  func addPaymentMethod(_ method: PaymentMethod) async throws
  func charge(amount: Double, method: PaymentMethod) async throws -> TransactionId
  func refund(transactionId: TransactionId) async throws
}

// Implementations
class StripeProvider: PaymentProvider
class AppleIAPProvider: PaymentProvider
class CryptoProvider: PaymentProvider

// Subscription Manager
class SubscriptionManager {
  func subscribe(tier: Tier) async throws -> Subscription
  func cancel(subscriptionId: String) async throws
  func changeTier(subscriptionId: String, newTier: Tier) async throws
}
```

**Dependencies:** Tinyvex (for transaction log), Plugins (for plugin author info)

**Dependents:** App layer (BillingManager, SubscriptionSheet), Orchestration (for cost constraints)

**Key Principles:**
- ✅ **Secure:** No credit card info stored locally (use Stripe tokens)
- ✅ **Auditable:** All transactions logged in Tinyvex
- ✅ **Transparent:** User sees costs before incurring
- ✅ **Fair:** 70% to author, 30% to platform

**Boundaries:**
- ✅ **IN SCOPE:** Usage tracking, billing, payment processing, revenue splits
- ❌ **OUT OF SCOPE:** Pricing strategy (defined by config), fraud detection (handled by payment providers)

**Usage Event Types:**
- `agent_run`: Running an agent (local: free, remote: paid)
- `plugin_call`: Calling a plugin tool (author sets price)
- `remote_execution`: Remote compute (per compute-hour)

---

### WorkspaceMemory

**Location:** `OpenAgentsCore/WorkspaceMemory/` (new)

**Responsibility:** Long-term workspace context via knowledge graphs, conventions, goals.

**Public Interface:**
```swift
// Knowledge Graph
actor WorkspaceKnowledgeGraph {
  func addEntity(_ entity: Entity) async throws
  func addRelation(from: EntityId, to: EntityId, type: RelationType) async throws
  func query(_ query: String) async throws -> [Entity]
  func neighbors(entityId: EntityId) async throws -> [Entity]
}

// Profile
struct WorkspaceProfile {
  let workspaceId: String
  let languages: [String]
  let frameworks: [String]
  let teamSize: Int?
  let conventions: Conventions
}

// Conventions
struct Conventions {
  let indentation: IndentationType
  let naming: NamingConvention
  let architecture: ArchitecturePattern?
}

// Goals
struct Goal {
  let id: String
  let description: String
  let progress: Double  // 0.0 to 1.0
  let createdAt: Date
}

// Summarizer
class WorkspaceSummarizer {
  func analyze(workspace: URL) async throws -> WorkspaceProfile
  func detectConventions(files: [URL]) async throws -> Conventions
}
```

**Dependencies:** Tinyvex (for graph persistence), Embeddings (for entity similarity), Summarization (for FM analysis)

**Dependents:** Orchestration (for context), History (for insights)

**Key Principles:**
- ✅ **LLM-First:** Use FM for analysis, no deterministic heuristics
- ✅ **Incremental:** Update graph as sessions run
- ✅ **Pruned:** Remove old/irrelevant memories periodically
- ✅ **Queryable:** Support natural language queries

**Boundaries:**
- ✅ **IN SCOPE:** Entity extraction, relation building, convention detection, goal tracking
- ❌ **OUT OF SCOPE:** Workspace scanning (handled by WorkspaceScanner)

**Entity Types:**
- `file`, `function`, `class`, `module`, `person`, `pr`, `issue`, `convention`

**Relation Types:**
- `refactored_by`, `depends_on`, `similar_to`, `used_in`, `authored_by`

---

### History (Enhanced)

**Location:** `OpenAgentsCore/History/` (enhanced)

**Responsibility:** Smart history recall with semantic search, NL queries, cross-workspace insights.

**Public Interface (Additions):**
```swift
// Smart Query
class SmartHistoryQuery {
  func query(_ naturalLanguageQuery: String) async throws -> [Session]
}

// Temporal Parser
class TemporalQueryParser {
  func parse(_ query: String) -> DateRange?
}

// Semantic Search
class SemanticHistorySearch {
  func search(query: String, topK: Int) async throws -> [Session]
}

// Cross-Workspace
class CrossWorkspaceHistory {
  func query(_ query: String) async throws -> [Session]
}

// Hybrid Search
class HybridSearch {
  func search(query: String, filters: Filters) async throws -> [Session]
}
```

**Dependencies:** Tinyvex (for session storage), Embeddings (for semantic search), Summarization (for query understanding)

**Dependents:** App layer (HistoryListView), Orchestration (for session insights)

**Key Principles:**
- ✅ **Hybrid:** Combine full-text search + semantic search
- ✅ **Fast:** Index sessions incrementally
- ✅ **Relevant:** Rank results by relevance (BM25 + cosine similarity)
- ✅ **Natural:** Support NL queries ("show me sessions about auth")

**Boundaries:**
- ✅ **IN SCOPE:** Query parsing, search indexing, result ranking
- ❌ **OUT OF SCOPE:** Session storage (handled by Tinyvex), embedding generation (handled by Embeddings)

**Query Examples:**
- Temporal: "what did I work on yesterday?" → SQL `WHERE date >= yesterday`
- Semantic: "show me sessions about authentication" → Embedding search
- Hybrid: "show me recent sessions about error handling" → FTS + temporal filter
- Cross-workspace: "show me all PRs across all workspaces" → Join workspaces

---

### PullRequests

**Location:** `OpenAgentsCore/PullRequests/` (new)

**Responsibility:** Automated PR creation, monitoring, and management.

**Public Interface:**
```swift
// PR Service
actor PRService {
  func create(params: PRParams) async throws -> PR
  func monitor(prId: String) async throws -> PRStatus
  func list() async throws -> [PR]
}

// Git Operations
class GitOperations {
  func createBranch(name: String) throws
  func commit(message: String, files: [URL]) throws
  func push(branch: String, remote: String) throws
}

// Provider Protocol
protocol GitProvider {
  func createPR(params: PRParams) async throws -> PR
  func getPRStatus(prId: String) async throws -> PRStatus
  func mergePR(prId: String) async throws
}

// Implementations
class GitHubClient: GitProvider
class GitLabClient: GitProvider

// Template
struct PRTemplate {
  let title: String
  let body: String
  let labels: [String]
}

// Monitor
class PRMonitor {
  func startMonitoring(prId: String, handler: (PRStatus) -> Void) async
  func stopMonitoring(prId: String)
}
```

**Dependencies:** Tinyvex (for PR cache), Summarization (for title/body generation)

**Dependents:** Orchestration (for PR automation after overnight runs)

**Key Principles:**
- ✅ **Automated:** Create PRs without user intervention
- ✅ **FM-Powered:** Generate titles/bodies using FM
- ✅ **Monitored:** Track check status, review comments
- ✅ **Extensible:** Support GitHub, GitLab, Bitbucket

**Boundaries:**
- ✅ **IN SCOPE:** PR creation, git operations, monitoring, provider abstractions
- ❌ **OUT OF SCOPE:** Git client implementation (shell out to `git` command)

**PR Creation Flow:**
1. Create branch: `git checkout -b feature/task-123`
2. Commit changes: `git commit -m "Add tests"`
3. Push to remote: `git push -u origin feature/task-123`
4. Create PR via API: `POST /repos/:owner/:repo/pulls`
5. Monitor: Poll for check status, reviews

---

## Module Interaction Patterns

### Event-Driven Updates

**Pattern:** Modules publish events, subscribers react.

**Example:** Agent execution updates
```
AgentProvider → SessionUpdateHub → Bridge (notify iOS) + Tinyvex (persist)
```

**Implementation:**
- Use `AsyncStream` or Combine `Publisher` for event broadcasting
- Subscribers filter events by session ID

**Benefits:**
- ✅ Decouples producer from consumers
- ✅ Multiple subscribers without coordination
- ✅ Async-friendly

---

### Dependency Injection

**Pattern:** Inject dependencies via initializers, not global singletons.

**Example:** AgentCoordinator dependencies
```swift
let coordinator = AgentCoordinator(
  taskQueue: taskQueue,
  decisionEngine: decisionEngine,
  agentRegistry: agentRegistry,
  updateHub: updateHub
)
```

**Benefits:**
- ✅ Testable (can inject mocks)
- ✅ Explicit dependencies
- ✅ No hidden global state

---

### Protocol-Oriented Abstractions

**Pattern:** Define protocols for extensibility, not concrete types.

**Example:** Payment providers
```swift
protocol PaymentProvider {
  func charge(amount: Double, method: PaymentMethod) async throws -> TransactionId
}

class StripeProvider: PaymentProvider { ... }
class CryptoProvider: PaymentProvider { ... }
```

**Benefits:**
- ✅ Easy to add new providers
- ✅ Testable (can mock protocol)
- ✅ Swifty

---

### Actor Isolation

**Pattern:** Use actors for shared mutable state.

**Example:** PluginRegistry
```swift
actor PluginRegistry {
  private var plugins: [String: Plugin] = [:]

  func register(_ plugin: Plugin) {
    plugins[plugin.id] = plugin
  }
}
```

**Benefits:**
- ✅ Compile-time thread safety
- ✅ No manual locking
- ✅ Async/await integration

---

## Module Ownership & Maintenance

### Core Team Ownership

| Module | Primary Owner | Secondary Owner |
|--------|--------------|-----------------|
| ACP | Architecture | Bridge |
| Bridge | Networking | Architecture |
| Agents | Orchestration | Core |
| Orchestration | Core | Agents |
| Tinyvex | Infrastructure | All |
| Embeddings | ML/AI | History |
| Summarization | ML/AI | History |

### New Module Ownership (Future)

| Module | Primary Owner | Secondary Owner |
|--------|--------------|-----------------|
| Plugins | Ecosystem | Security |
| Marketplace | Ecosystem | Payments |
| Payments | Business | Security |
| WorkspaceMemory | ML/AI | Orchestration |
| History (enhanced) | ML/AI | Infrastructure |
| PullRequests | Orchestration | Ecosystem |

### Ownership Responsibilities

**Primary Owner:**
- ✅ Design decisions (APIs, architecture)
- ✅ Code reviews (approve all changes)
- ✅ Bug triage (prioritize fixes)
- ✅ Documentation (maintain module docs)

**Secondary Owner:**
- ✅ Code reviews (backup for primary)
- ✅ Feature contributions
- ✅ Bug fixes

---

## Testing Boundaries

### Unit Tests

**Scope:** Test individual components in isolation.

**Example:** `TaskQueueTests` - Test enqueue/dequeue without database
```swift
func testEnqueueDequeue() async throws {
  let queue = TaskQueue(db: MockDatabase())
  let task = OvernightTask(...)
  try await queue.enqueue(task)
  let dequeued = try await queue.dequeue()
  XCTAssertEqual(dequeued?.id, task.id)
}
```

**Coverage Goal:** 80%+ for core logic

---

### Integration Tests

**Scope:** Test interactions between modules.

**Example:** `BridgeServerClientTests` - Test WebSocket bridge E2E
```swift
func testSessionUpdate() async throws {
  let server = DesktopWebSocketServer()
  let client = MobileWebSocketClient()
  try await server.start(port: 9999)
  try await client.connect(host: "localhost", port: 9999)

  let update = ACP.Client.SessionUpdate(...)
  server.broadcast(update)

  let received = try await client.awaitNotification("session/update")
  XCTAssertEqual(received.sessionId, update.sessionId)
}
```

**Coverage Goal:** 70%+ for critical paths

---

### UI Tests

**Scope:** Test user flows end-to-end.

**Example:** `MarketplaceInstallFlowTests` - Test plugin installation
```swift
func testInstallPlugin() throws {
  let app = XCUIApplication()
  app.launch()

  // Navigate to marketplace
  app.buttons["Marketplace"].tap()

  // Search for plugin
  app.searchFields["Search"].typeText("Slack")

  // Install plugin
  app.buttons["Install"].tap()

  // Verify installed
  XCTAssertTrue(app.staticTexts["Slack Integration"].exists)
}
```

**Coverage Goal:** 50%+ for critical flows

---

## Module Versioning & Evolution

### Semantic Versioning

**Format:** `MAJOR.MINOR.PATCH`

**Rules:**
- **MAJOR:** Breaking changes (API incompatibility)
- **MINOR:** New features (backward compatible)
- **PATCH:** Bug fixes (backward compatible)

**Example:**
- `1.0.0` → `1.1.0`: Added `PluginRegistry.list()` method
- `1.1.0` → `2.0.0`: Removed `PluginRegistry.register(name:)`, replaced with `register(plugin:)`

---

### Deprecation Policy

**Process:**
1. Mark API as deprecated with `@available` annotation
2. Document replacement in doc comment
3. Wait 2 minor versions before removing
4. Remove in next major version

**Example:**
```swift
// Version 1.0.0
func register(name: String, plugin: Plugin) { ... }

// Version 1.1.0 - Deprecate old API
@available(*, deprecated, message: "Use register(_:) instead")
func register(name: String, plugin: Plugin) { ... }

func register(_ plugin: Plugin) { ... }

// Version 2.0.0 - Remove old API
func register(_ plugin: Plugin) { ... }
```

---

## Security Boundaries

### Trust Levels

| Component | Trust Level | Rationale |
|-----------|------------|-----------|
| App Layer | Trusted | Signed by OpenAgents, runs with full privileges |
| OpenAgentsCore | Trusted | Part of main app, code-signed |
| First-Party Plugins | Trusted | Signed by OpenAgents |
| Third-Party Plugins | Untrusted | Sandboxed, limited permissions |
| MCP Servers | Untrusted | External services, network-isolated |

### Permission Model

**Plugins Request Permissions:**
```json
{
  "permissions": [
    "fileRead:/path/to/workspace",
    "fileWrite:/path/to/workspace/output",
    "network:https://api.slack.com",
    "agent:codex"
  ]
}
```

**User Grants Permissions:**
- First-time use: Prompt user for permission
- Subsequent uses: Use cached permission
- Revocation: User can revoke anytime in Settings

**Enforcement:**
- File access: Sandbox enforced via entitlements
- Network: Firewall rules enforced via entitlements
- Agent execution: Checked before delegating to AgentRegistry

---

## Performance Boundaries

### Latency Budgets

| Operation | Target Latency | Max Latency |
|-----------|----------------|-------------|
| Bridge RPC call | < 10ms | 100ms |
| History query | < 50ms | 500ms |
| Semantic search | < 200ms | 1s |
| Plugin load | < 100ms | 1s |
| FM inference | < 500ms | 5s |

### Memory Limits

| Component | Memory Budget | Rationale |
|-----------|---------------|-----------|
| VectorStore (in-memory) | 100 MB | ~1M embeddings (100 dims each) |
| Tinyvex cache | 50 MB | Hot sessions + recent history |
| Plugin sandbox | 256 MB per plugin | Prevent runaway plugins |

### Concurrency Limits

| Resource | Max Concurrent | Rationale |
|----------|----------------|-----------|
| Agent sessions | 10 | Prevent resource exhaustion |
| Plugin loads | 5 | Prevent startup thrashing |
| Bridge connections | 10 | Support multiple iOS devices |

---

## Documentation Requirements

### Module README

**Each module must have:**
- **Purpose:** What does this module do?
- **Public API:** What can other modules use?
- **Dependencies:** What does this module depend on?
- **Examples:** How to use the module?

**Location:** `OpenAgentsCore/<Module>/README.md`

---

### API Documentation

**All public APIs must have:**
- **Summary:** One-line description
- **Parameters:** Type and description
- **Returns:** Type and description
- **Throws:** Error types and when they're thrown
- **Example:** Code snippet

**Format:** Swift doc comments (`///`)

**Example:**
```swift
/// Registers a new plugin with the registry.
///
/// - Parameter plugin: The plugin to register.
/// - Throws: `PluginError.alreadyRegistered` if plugin ID already exists.
///
/// - Example:
/// ```swift
/// let plugin = MyPlugin()
/// try await registry.register(plugin)
/// ```
public func register(_ plugin: Plugin) async throws {
  // Implementation
}
```

---

## Conclusion

This document establishes **clear module boundaries** for the OpenAgents codebase. Key principles:

1. ✅ **Single Responsibility** - Each module does one thing well
2. ✅ **Explicit Dependencies** - No hidden dependencies or global state
3. ✅ **Protocol-Oriented** - Extensibility via protocols, not inheritance
4. ✅ **Actor-Safe** - Thread safety via Swift actors
5. ✅ **Testable** - Dependency injection enables mocking
6. ✅ **Documented** - Every module has README and API docs
7. ✅ **Secure** - Trust boundaries enforced via sandboxing and permissions

**Next Steps:**
- See `architectural-audit.md` for current state analysis
- See `future-state-architecture.md` for target structure
- See `refactoring-roadmap.md` for migration plan
- See `code-smell-inventory.md` for specific issues to fix

---

**Document Version:** 1.0
**Last Updated:** November 11, 2025
