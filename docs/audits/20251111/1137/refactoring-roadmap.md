# OpenAgents Refactoring Roadmap
**Date:** November 11, 2025
**Purpose:** Step-by-step migration plan for delivering all 10 features from Episode 195

## Executive Summary

This roadmap provides a **pragmatic, incremental migration strategy** to evolve the OpenAgents codebase from its current state to the future-state architecture that delivers all 10 features:

1. ✅ ChatGPT-style desktop app (DONE)
2. ✅ Mobile sync (DONE)
3. ✅ Overnight scheduling (DONE, polish needed)
4. ✅ Sub-agent delegation (DONE, polish needed)
5. 🎯 Smart history/memory (semantic layer + workspace memory)
6. 🎯 Plugin ecosystem (full system)
7. ✅ Open source (DONE)
8. ✅ Local + cloud inference (DONE, routing UI needed)
9. 🎯 Compute marketplace (full infrastructure)
10. 🎯 Revenue sharing (payment infrastructure)

**Timeline:** 11-17 weeks (2.5-4 months)
**Approach:** Incremental, maintain working code throughout
**Risk Mitigation:** Feature flags, extensive testing, backwards compatibility

## Guiding Principles

### 1. Never Break Main
- All changes must maintain working build
- Feature flags for incomplete features
- Comprehensive tests before merging

### 2. Incremental Over Big-Bang
- Small, focused PRs (< 500 LOC)
- Ship polish alongside new features
- Parallel workstreams where possible

### 3. LLM-First Policy
- Use Foundation Models for all interpretation
- No deterministic heuristics unless explicitly requested
- Validate model effectiveness with telemetry (privacy-preserving)

### 4. Backwards Compatibility
- Old Tinyvex schemas must migrate cleanly
- Support loading legacy sessions
- Deprecate, don't delete (2-version grace period)

### 5. Security First
- Plugin sandboxing from day one
- Code signing required for third-party plugins
- Permission prompts for sensitive operations

## Phase 0: Preparation (Week 1)

**Goal:** Set up infrastructure for safe, incremental development.

### Tasks

#### 1. Create Feature Flags System
**File:** `OpenAgentsCore/Utils/FeatureFlags.swift`
```swift
enum FeatureFlag: String, CaseIterable {
  case plugins = "feature.plugins"
  case marketplace = "feature.marketplace"
  case payments = "feature.payments"
  case workspaceMemory = "feature.workspaceMemory"
  case smartHistory = "feature.smartHistory"
  case prAutomation = "feature.prAutomation"

  var isEnabled: Bool {
    #if DEBUG
    return UserDefaults.standard.bool(forKey: rawValue)
    #else
    // Production: check remote config
    return false
    #endif
  }
}
```

**Why:** Allow shipping incomplete features behind flags.

#### 2. Add Telemetry System (Privacy-Preserving)
**File:** `OpenAgentsCore/Utils/Telemetry.swift`
```swift
actor Telemetry {
  func track(event: String, properties: [String: String] = [:]) async {
    // Local-only telemetry (no network calls)
    // Store in Tinyvex for debugging
  }
}
```

**Why:** Measure effectiveness of new features without compromising privacy.

#### 3. Set Up Parallel Workstreams
**Branches:**
- `feature/plugins` - Plugin system
- `feature/marketplace` - Marketplace
- `feature/payments` - Payment infrastructure
- `feature/workspace-memory` - Workspace memory
- `feature/smart-history` - Smart history

**Why:** Enable parallel development without conflicts.

#### 4. Enhance CI/CD
- Add SwiftLint for code quality
- Add danger-swift for PR automation
- Set up automated TestFlight builds

**Estimated Time:** 3-5 days

---

## Phase 1: Code Quality Improvements (Week 2)

**Goal:** Clean up existing code to make room for new features.

### Tasks

#### 1. Extract ChatAreaView Components
**Problem:** `ChatAreaView.swift` at 672 LOC is growing too large.

**Solution:** Extract message row components.

**New Structure:**
```
Views/macOS/Chat/
├── ChatAreaView.swift                 # Main container (< 200 LOC)
├── MessageRowView.swift               # Single message row
├── ToolCallRow.swift                  # Tool call renderer
├── ThinkingBlockRow.swift             # Thinking block renderer
└── AssistantMessageRow.swift          # Assistant message renderer
```

**Steps:**
1. Create `Views/macOS/Chat/` directory
2. Extract `MessageRowView` with switch over message types
3. Extract `ToolCallRow` for tool call rendering
4. Extract `ThinkingBlockRow` for thinking block rendering
5. Extract `AssistantMessageRow` for assistant messages
6. Update `ChatAreaView` to use new components
7. Add preview providers for each component
8. Update tests

**Estimated Time:** 2 days

#### 2. Decouple BridgeManager
**Problem:** BridgeManager knows about too many dependencies.

**Solution:** Introduce `BridgeCoordinator` protocol and message bus.

**New Structure:**
```swift
protocol BridgeCoordinator {
  func handlePrompt(_ prompt: String) async throws
  func handleSessionUpdate(_ update: ACP.Client.SessionUpdate) async
}

class BridgeManager: BridgeCoordinator {
  private let messageBus: MessageBus  // Combine publisher
  private let promptDispatcher: PromptDispatcher

  // Reduced surface area
}
```

**Steps:**
1. Create `BridgeCoordinator` protocol
2. Extract `MessageBus` for event broadcasting
3. Refactor `BridgeManager` to use message bus
4. Update dependents to subscribe to message bus
5. Add tests

**Estimated Time:** 2 days

#### 3. Add Settings Test Coverage
**Gap:** Settings views have no tests.

**Solution:** Add focused tests for each settings tab.

**Tests:**
- `ConnectionTabTests` - Connection settings persistence
- `WorkspaceTabTests` - Workspace configuration
- `AgentsTabTests` - Agent enable/disable
- `OrchestrationTabTests` - Orchestration config

**Estimated Time:** 1 day

**Total Phase 1 Time:** 5 days

---

## Phase 2: Plugin System (Weeks 3-6)

**Goal:** Enable dynamic plugin loading, MCP integration, sandboxing.

### Milestone 1: Plugin Registry & Manifest (Week 3)

#### 1. Define Plugin Manifest Schema
**File:** `OpenAgentsCore/Plugins/PluginManifest.swift`
```swift
struct PluginManifest: Codable {
  let id: String  // Reverse DNS: com.example.plugin
  let name: String
  let version: String  // Semantic versioning
  let author: String
  let capabilities: [PluginCapability]
  let permissions: [Permission]
  let dependencies: [String: String]  // Package ID -> Version constraint
}

enum PluginCapability: String, Codable {
  case tool, agent, integration, theme
}

enum Permission: String, Codable {
  case fileRead, fileWrite, network, agentExecution
}
```

#### 2. Implement Plugin Registry
**File:** `OpenAgentsCore/Plugins/PluginRegistry.swift`
```swift
actor PluginRegistry {
  private var plugins: [String: Plugin] = [:]

  func register(_ plugin: Plugin) async throws {
    // Verify manifest
    // Check dependencies
    // Load plugin
    plugins[plugin.id] = plugin
  }

  func unregister(pluginId: String) async throws {
    // Unload plugin
    plugins.removeValue(forKey: pluginId)
  }

  func lookup(pluginId: String) -> Plugin? {
    plugins[pluginId]
  }
}
```

#### 3. Add Plugin Storage to Tinyvex
**Migration:** `20251118_add_plugins_table.sql`
```sql
CREATE TABLE plugins (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  version TEXT NOT NULL,
  author TEXT NOT NULL,
  manifest JSON NOT NULL,
  installed_at INTEGER NOT NULL,
  enabled BOOLEAN DEFAULT 1
);
```

**Estimated Time:** 5 days

### Milestone 2: Plugin Loader & Sandboxing (Week 4)

#### 1. Implement Plugin Loader
**File:** `OpenAgentsCore/Plugins/PluginLoader.swift`
```swift
class PluginLoader {
  func load(path: URL) throws -> Plugin {
    // Read manifest
    // Verify code signature
    // Load dynamic library / run subprocess
    // Return plugin handle
  }

  func unload(plugin: Plugin) throws {
    // Cleanup resources
    // Terminate subprocess if needed
  }
}
```

#### 2. Implement Plugin Sandbox
**File:** `OpenAgentsCore/Plugins/PluginSandbox.swift`
```swift
class PluginSandbox {
  func execute<T>(_ plugin: Plugin, operation: () throws -> T) throws -> T {
    // Run in separate process with entitlements
    // Enforce file system, network restrictions
  }
}
```

**Approach:**
- **Option A:** XPC service with entitlements (macOS)
- **Option B:** Subprocess with restricted environment

**Recommendation:** Start with subprocess, migrate to XPC for production.

#### 3. Add Permission System
**File:** `OpenAgentsCore/Plugins/PluginPermissions.swift`
```swift
class PluginPermissions {
  func request(plugin: Plugin, permission: Permission) async -> Bool {
    // Check if already granted
    // Prompt user if not
    // Store decision
  }

  func revoke(pluginId: String, permission: Permission) async {
    // Remove grant
  }
}
```

**Estimated Time:** 5 days

### Milestone 3: MCP Integration (Week 5)

#### 1. Implement MCP Client
**File:** `OpenAgentsCore/Plugins/MCP/MCPClient.swift`
```swift
class MCPClient {
  func connect(server: URL) async throws {
    // WebSocket or HTTP connection
  }

  func listCapabilities() async throws -> [MCPCapability] {
    // Query server capabilities
  }

  func call(capability: String, params: JSON) async throws -> JSON {
    // Invoke capability
  }
}
```

#### 2. Implement MCP Server Discovery
**File:** `OpenAgentsCore/Plugins/MCP/MCPServerDiscovery.swift`
```swift
class MCPServerDiscovery {
  func discover() async -> [MCPServer] {
    // Bonjour/mDNS discovery
    // Hardcoded server list
  }
}
```

#### 3. Add MCP Tool Integration
**Goal:** Allow MCP servers to provide tools to orchestration.

**Integration Point:** `FMOrchestrator` → `MCPClient` → Tool execution

**Estimated Time:** 5 days

### Milestone 4: Plugin UI & First-Party Plugins (Week 6)

#### 1. Build Plugin Settings UI
**File:** `ios/OpenAgents/Plugins/PluginSettingsView.swift`
```swift
struct PluginSettingsView: View {
  @ObservedObject var manager: PluginManager

  var body: some View {
    List(manager.plugins) { plugin in
      PluginRow(plugin: plugin)
    }
  }
}
```

#### 2. Add Plugin Installation Flow
**File:** `ios/OpenAgents/Plugins/PluginInstallSheet.swift`
- Select plugin from file picker
- Show manifest preview
- Request permissions
- Install and enable

#### 3. Create First-Party Plugins
**Examples:**
- `SlackPlugin` - Send messages to Slack
- `GitHubPlugin` - Create issues, PRs
- `LinearPlugin` - Create/update issues

**Structure:**
```
Plugins/FirstParty/
├── SlackPlugin/
│   ├── manifest.json
│   ├── SlackPlugin.swift
│   └── SlackClient.swift
```

**Estimated Time:** 5 days

**Total Phase 2 Time:** 4 weeks

---

## Phase 3: Marketplace (Weeks 7-9)

**Goal:** Enable discovery, installation, and management of third-party agents/plugins.

### Milestone 1: Marketplace Client (Week 7)

#### 1. Define Marketplace API
**Endpoints:**
- `GET /api/v1/packages?q=<query>` - Search
- `GET /api/v1/packages/<id>` - Detail
- `GET /api/v1/packages/<id>/reviews` - Reviews
- `GET /api/v1/packages/<id>/download` - Download URL

#### 2. Implement Marketplace Client
**File:** `OpenAgentsCore/Marketplace/MarketplaceClient.swift`
```swift
actor MarketplaceClient {
  func search(query: String, filters: Filters) async throws -> [PackageSummary]
  func detail(packageId: String) async throws -> PackageDetail
  func reviews(packageId: String) async throws -> [Review]
  func download(packageId: String) async throws -> URL
}
```

#### 3. Add Local Package Cache
**File:** `OpenAgentsCore/Marketplace/PackageCache.swift`
- Cache package metadata for offline browsing
- Store in `~/Library/Caches/OpenAgents/Marketplace/`

**Estimated Time:** 5 days

### Milestone 2: Package Installer & Version Manager (Week 8)

#### 1. Implement Package Installer
**File:** `OpenAgentsCore/Marketplace/PackageInstaller.swift`
```swift
class PackageInstaller {
  func install(package: AgentPackage) async throws -> Plugin {
    // Download package
    // Verify checksum
    // Verify signature
    // Extract to ~/Library/Application Support/OpenAgents/Plugins/
    // Register with PluginRegistry
  }

  func uninstall(pluginId: String) async throws {
    // Unregister from registry
    // Delete files
    // Clean up data
  }
}
```

#### 2. Implement Version Manager
**File:** `OpenAgentsCore/Marketplace/VersionManager.swift`
```swift
class VersionManager {
  func resolve(dependencies: [String: String]) throws -> [String: String] {
    // Semantic version resolution
    // Conflict detection
  }

  func rollback(pluginId: String, toVersion: String) async throws {
    // Uninstall current version
    // Install old version
  }
}
```

**Estimated Time:** 5 days

### Milestone 3: Marketplace UI & Reputation (Week 9)

#### 1. Build Marketplace Browser
**File:** `ios/OpenAgents/Marketplace/MarketplaceBrowserView.swift`
- Search bar
- Category filters
- Package list (with ratings, author, downloads)
- Detail view (README, reviews, install button)

#### 2. Implement Reputation System
**File:** `OpenAgentsCore/Marketplace/ReputationStore.swift`
```swift
class ReputationStore {
  func getReviews(packageId: String) async throws -> [Review]
  func addReview(packageId: String, review: Review) async throws
  func reportSpam(reviewId: String) async throws
}
```

**Spam Detection:** Use Foundation Models to detect spam reviews.

**Estimated Time:** 5 days

**Total Phase 3 Time:** 3 weeks

---

## Phase 4: Payments (Weeks 10-12)

**Goal:** Enable billing, usage tracking, and revenue sharing.

### Milestone 1: Billing Service & Usage Tracking (Week 10)

#### 1. Implement Billing Service
**File:** `OpenAgentsCore/Payments/BillingService.swift`
```swift
actor BillingService {
  func balance() async -> Double  // Credits
  func charge(amount: Double, description: String) async throws
  func credit(amount: Double, description: String) async throws
  func usage(startDate: Date, endDate: Date) async throws -> [UsageEvent]
}
```

#### 2. Implement Usage Tracker
**File:** `OpenAgentsCore/Payments/UsageTracker.swift`
```swift
actor UsageTracker {
  func track(event: UsageEvent) async throws {
    // Store in Tinyvex
    // Update BillingService
  }

  func computeCost(event: UsageEvent) -> Double {
    // Compute cost based on event type
    // agent_run: local = free, remote = paid
    // plugin_call: author-defined price
  }
}
```

#### 3. Add Tinyvex Tables
**Migration:** `20251201_add_payments_tables.sql`
```sql
CREATE TABLE usage_events (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL,
  event_type TEXT NOT NULL,
  duration_ms INTEGER,
  cost_credits REAL,
  created_at INTEGER NOT NULL
);

CREATE TABLE transactions (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL,
  type TEXT NOT NULL,  -- 'charge', 'credit', 'payout'
  amount_credits REAL,
  status TEXT NOT NULL,
  created_at INTEGER NOT NULL
);
```

**Estimated Time:** 5 days

### Milestone 2: Payment Providers (Week 11)

#### 1. Define Payment Provider Protocol
**File:** `OpenAgentsCore/Payments/PaymentProvider.swift`
```swift
protocol PaymentProvider {
  func addPaymentMethod(_ method: PaymentMethod) async throws
  func charge(amount: Double, method: PaymentMethod) async throws -> TransactionId
  func refund(transactionId: TransactionId) async throws
}
```

#### 2. Implement Stripe Provider
**File:** `OpenAgentsCore/Payments/StripeProvider.swift`
- Use Stripe SDK
- Store tokens, not card numbers
- Handle webhooks for async events

#### 3. Implement Apple IAP Provider (Optional)
**File:** `OpenAgentsCore/Payments/AppleIAPProvider.swift`
- Use StoreKit 2
- Subscription management
- Receipt validation

**Note:** Start with Stripe, add Apple IAP later if needed.

**Estimated Time:** 5 days

### Milestone 3: Subscription Management & UI (Week 12)

#### 1. Implement Subscription Manager
**File:** `OpenAgentsCore/Payments/SubscriptionManager.swift`
```swift
class SubscriptionManager {
  func subscribe(tier: Tier) async throws -> Subscription
  func cancel(subscriptionId: String) async throws
  func changeTier(subscriptionId: String, newTier: Tier) async throws
}
```

**Tiers:**
- **Free:** 100 credits/month, local agents only
- **Pro:** $20/month, 10,000 credits, remote execution
- **Enterprise:** Custom pricing, unlimited credits

#### 2. Build Billing UI
**Files:**
- `ios/OpenAgents/Billing/BillingTab.swift` - Settings → Billing tab
- `ios/OpenAgents/Billing/SubscriptionSheet.swift` - Tier selection
- `ios/OpenAgents/Billing/PaymentMethodSheet.swift` - Add payment method

#### 3. Implement Revenue Splitter
**File:** `OpenAgentsCore/Payments/RevenueSplitter.swift`
```swift
class RevenueSplitter {
  func split(amount: Double, pluginId: String) -> (author: Double, platform: Double) {
    // 70% to author, 30% to platform
  }

  func payout(authorId: String, amount: Double) async throws {
    // Stripe Connect payout
  }
}
```

**Estimated Time:** 5 days

**Total Phase 4 Time:** 3 weeks

---

## Phase 5: Workspace Memory (Weeks 13-15)

**Goal:** Enable long-term workspace context via knowledge graphs.

### Milestone 1: Knowledge Graph Core (Week 13)

#### 1. Define Entity & Relation Types
**File:** `OpenAgentsCore/WorkspaceMemory/WorkspaceTypes.swift`
```swift
enum EntityType: String, Codable {
  case file, function, class, module, person, pr, issue, convention
}

enum RelationType: String, Codable {
  case refactoredBy, dependsOn, similarTo, usedIn, authoredBy
}

struct Entity: Codable {
  let id: String
  let type: EntityType
  let name: String
  let metadata: JSON
}

struct Relation: Codable {
  let id: String
  let from: EntityId
  let to: EntityId
  let type: RelationType
  let confidence: Double  // 0.0 to 1.0
}
```

#### 2. Implement Knowledge Graph
**File:** `OpenAgentsCore/WorkspaceMemory/WorkspaceKnowledgeGraph.swift`
```swift
actor WorkspaceKnowledgeGraph {
  func addEntity(_ entity: Entity) async throws
  func addRelation(_ relation: Relation) async throws
  func query(_ query: String) async throws -> [Entity]
  func neighbors(entityId: EntityId) async throws -> [Entity]
}
```

#### 3. Add Tinyvex Tables
**Migration:** `20251215_add_workspace_memory_tables.sql`
```sql
CREATE TABLE workspace_entities (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL,
  type TEXT NOT NULL,
  name TEXT NOT NULL,
  metadata JSON,
  created_at INTEGER NOT NULL
);

CREATE TABLE workspace_relations (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL,
  from_entity_id TEXT NOT NULL,
  to_entity_id TEXT NOT NULL,
  relation_type TEXT NOT NULL,
  confidence REAL,
  created_at INTEGER NOT NULL
);
```

**Estimated Time:** 5 days

### Milestone 2: Memory Extraction & Analysis (Week 14)

#### 1. Implement Entity Extractor
**File:** `OpenAgentsCore/WorkspaceMemory/EntityExtractor.swift`
```swift
class EntityExtractor {
  func extract(from session: Session) async throws -> [Entity] {
    // Use FM to extract entities from session
    // Files touched, functions modified, people mentioned
  }
}
```

#### 2. Implement Relation Builder
**File:** `OpenAgentsCore/WorkspaceMemory/RelationBuilder.swift`
```swift
class RelationBuilder {
  func build(entities: [Entity], context: SessionContext) async throws -> [Relation] {
    // Use FM to infer relations
    // "Alice refactored auth.swift"
    // "auth.swift depends on crypto.swift"
  }
}
```

#### 3. Implement Convention Detector
**File:** `OpenAgentsCore/WorkspaceMemory/ConventionDetector.swift`
```swift
class ConventionDetector {
  func detect(files: [URL]) async throws -> Conventions {
    // Use FM to detect conventions
    // Indentation (tabs vs spaces)
    // Naming (camelCase vs snake_case)
    // Architecture (MVVM, MVC, etc.)
  }
}
```

**Estimated Time:** 5 days

### Milestone 3: Workspace Profile & UI (Week 15)

#### 1. Implement Workspace Summarizer
**File:** `OpenAgentsCore/WorkspaceMemory/WorkspaceSummarizer.swift`
```swift
class WorkspaceSummarizer {
  func analyze(workspace: URL) async throws -> WorkspaceProfile {
    // Scan workspace
    // Detect languages, frameworks
    // Generate summary
  }
}
```

#### 2. Build Workspace Memory UI
**File:** `ios/OpenAgents/WorkspaceMemory/WorkspaceMemoryView.swift`
- Show workspace profile
- Show conventions
- Show knowledge graph (entity-relation visualization)
- Query interface ("Who usually works on auth?")

#### 3. Integrate with Orchestration
**Goal:** Use workspace memory in DecisionEngine for smarter task selection.

**Estimated Time:** 5 days

**Total Phase 5 Time:** 3 weeks

---

## Phase 6: Smart History (Weeks 16-17)

**Goal:** Enable semantic search, NL queries, and cross-workspace insights.

### Milestone 1: Semantic Search Integration (Week 16)

#### 1. Implement Embedding Cache
**File:** `OpenAgentsCore/Embeddings/EmbeddingCache.swift`
```swift
class EmbeddingCache {
  func store(contentHash: String, embedding: [Float]) async throws {
    // Store in Tinyvex
  }

  func retrieve(contentHash: String) async throws -> [Float]? {
    // Retrieve from Tinyvex
  }
}
```

#### 2. Add Tinyvex Table
**Migration:** `20251229_add_embeddings_table.sql`
```sql
CREATE TABLE embeddings (
  id TEXT PRIMARY KEY,
  content_hash TEXT NOT NULL UNIQUE,
  embedding BLOB NOT NULL,
  model_version TEXT NOT NULL,
  created_at INTEGER NOT NULL
);
```

#### 3. Implement Semantic History Search
**File:** `OpenAgentsCore/History/SemanticHistorySearch.swift`
```swift
class SemanticHistorySearch {
  func search(query: String, topK: Int) async throws -> [Session] {
    // Generate embedding for query
    // Search vector store
    // Return top-K sessions
  }
}
```

**Estimated Time:** 5 days

### Milestone 2: Smart Query & Hybrid Search (Week 17)

#### 1. Implement Temporal Query Parser
**File:** `OpenAgentsCore/History/TemporalQueryParser.swift`
```swift
class TemporalQueryParser {
  func parse(_ query: String) -> DateRange? {
    // Use FM to parse temporal queries
    // "last Monday" → DateRange
    // "past 2 weeks" → DateRange
  }
}
```

#### 2. Implement Smart History Query
**File:** `OpenAgentsCore/History/SmartHistoryQuery.swift`
```swift
class SmartHistoryQuery {
  func query(_ naturalLanguageQuery: String) async throws -> [Session] {
    // Use FM to parse query
    // Determine if temporal, semantic, or hybrid
    // Execute appropriate search
  }
}
```

#### 3. Implement Hybrid Search
**File:** `OpenAgentsCore/History/HybridSearch.swift`
```swift
class HybridSearch {
  func search(query: String, filters: Filters) async throws -> [Session] {
    // Combine full-text search (SQLite FTS5)
    // With semantic search (embeddings)
    // Rank by BM25 + cosine similarity
  }
}
```

**Estimated Time:** 5 days

**Total Phase 6 Time:** 2 weeks

---

## Phase 7: PR Automation (Week 17-18)

**Goal:** Automate PR creation, monitoring, and management.

### Tasks

#### 1. Implement Git Operations
**File:** `OpenAgentsCore/PullRequests/GitOperations.swift`
```swift
class GitOperations {
  func createBranch(name: String) throws {
    // Shell out to `git checkout -b`
  }

  func commit(message: String, files: [URL]) throws {
    // Shell out to `git add` + `git commit`
  }

  func push(branch: String, remote: String) throws {
    // Shell out to `git push -u`
  }
}
```

#### 2. Implement GitHub Client
**File:** `OpenAgentsCore/PullRequests/GitHubClient.swift`
```swift
class GitHubClient: GitProvider {
  func createPR(params: PRParams) async throws -> PR {
    // POST /repos/:owner/:repo/pulls
  }

  func getPRStatus(prId: String) async throws -> PRStatus {
    // GET /repos/:owner/:repo/pulls/:number
    // Check CI status, reviews
  }
}
```

#### 3. Implement PR Service
**File:** `OpenAgentsCore/PullRequests/PRService.swift`
```swift
actor PRService {
  func create(params: PRParams) async throws -> PR {
    // Create branch
    // Commit changes
    // Push to remote
    // Create PR via API
    // Generate title/body with FM
  }

  func monitor(prId: String) async throws -> PRStatus {
    // Poll for status changes
  }
}
```

#### 4. Integrate with Orchestration
**Goal:** After overnight orchestration cycle, create PR automatically.

**Configuration:** `OrchestrationConfig.prAutomation`

**Estimated Time:** 5 days

**Total Phase 7 Time:** 1 week

---

## Phase 8: Polish & Testing (Week 18-19)

**Goal:** Polish all new features, comprehensive testing, documentation.

### Tasks

#### 1. Comprehensive Testing
- Unit tests for all new modules (80%+ coverage)
- Integration tests for end-to-end flows
- UI tests for critical paths
- Performance tests (latency, memory)

#### 2. Documentation
- Update CLAUDE.md with new modules
- Write READMEs for each new module
- API documentation for all public interfaces
- User-facing docs (how to install plugins, subscribe, etc.)

#### 3. Security Audit
- Review plugin sandboxing
- Review payment security
- Penetration testing for marketplace
- Code signing verification

#### 4. Performance Optimization
- Profile hot paths
- Optimize database queries
- Cache aggressively
- Background processing for heavy tasks

#### 5. UX Polish
- Animations and transitions
- Error messages and recovery
- Loading states and progress bars
- Accessibility (VoiceOver, keyboard navigation)

**Estimated Time:** 2 weeks

**Total Phase 8 Time:** 2 weeks

---

## Rollout Strategy

### Beta Testing (Week 20-21)

**Goal:** Get feedback from early users before public launch.

**Approach:**
1. **Internal Alpha** (Week 20)
   - Core team dogfooding
   - Fix critical bugs
   - Refine UX

2. **External Beta** (Week 21)
   - Invite 50-100 beta testers
   - Collect feedback via in-app surveys
   - Monitor telemetry for issues
   - Iterate on UX

### Public Launch (Week 22)

**Pre-Launch Checklist:**
- [ ] All tests passing
- [ ] Security audit complete
- [ ] Documentation complete
- [ ] Marketing materials ready (blog post, video, screenshots)
- [ ] TestFlight build deployed
- [ ] Marketplace seeded with 5-10 first-party plugins

**Launch Day:**
- Publish blog post
- Tweet announcement
- Post on Hacker News, Reddit, Twitter
- Monitor for issues
- Respond to feedback

**Post-Launch:**
- Monitor crash reports
- Hot-fix critical bugs
- Iterate based on user feedback

---

## Risk Mitigation

### Technical Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Plugin sandboxing doesn't work on iOS | Medium | High | Start with macOS, add iOS later |
| Stripe integration delayed | Low | Medium | Use Apple IAP as backup |
| FM inference too slow | Low | Medium | Cache aggressively, use smaller models |
| Marketplace API unavailable | Low | High | Build static plugin catalog as fallback |

### Business Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Users don't want plugins | Medium | High | Seed with first-party plugins, demonstrate value |
| Revenue model too expensive | Medium | Medium | Start with generous free tier, iterate on pricing |
| App Store rejection | Low | High | Review guidelines carefully, have fallback (direct download) |

### Schedule Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Delays in plugin system | Medium | High | Prioritize, cut scope if needed |
| Payments integration takes longer | Medium | Medium | Start early, seek expert help |
| Testing takes longer than expected | High | Low | Budget extra time in Phase 8 |

---

## Success Metrics

### Development Metrics
- **Build health:** 100% passing tests on main
- **Code coverage:** 80%+ on new code
- **PR cycle time:** < 24 hours for review
- **Bug density:** < 1 critical bug per 1000 LOC

### Feature Adoption Metrics
- **Plugin installs:** 10+ installs per user within 30 days
- **Marketplace DAU:** 20% of total users browse marketplace weekly
- **Subscription conversion:** 10% of free users upgrade to Pro within 90 days
- **Revenue per user:** $5 ARPU within 180 days

### Quality Metrics
- **Crash rate:** < 0.1% of sessions
- **Latency:** 90th percentile RPC latency < 100ms
- **User satisfaction:** NPS score > 40

---

## Conclusion

This roadmap provides a **pragmatic, incremental path** to delivering all 10 features from Episode 195. Key principles:

1. ✅ **Incremental:** Small, focused changes that maintain working code
2. ✅ **Parallel:** Multiple workstreams to accelerate development
3. ✅ **LLM-First:** Foundation Models for all interpretation and decision-making
4. ✅ **Secure:** Plugin sandboxing and code signing from day one
5. ✅ **User-Centric:** Beta testing and iteration before public launch

**Timeline:** 11-17 weeks (2.5-4 months)
**Outcome:** OpenAgents as a 10x better agent IDE with thriving plugin ecosystem

---

**Next Steps:**
- Review roadmap with team
- Assign owners for each phase
- Create tracking issues for each milestone
- Begin Phase 0 preparation

**Last Updated:** November 11, 2025
