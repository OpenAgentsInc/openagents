# OpenAgents Future-State Architecture
**Date:** November 11, 2025
**Episode:** 195 - Designing 10x Better
**Vision:** Architecture for all 10 features fully implemented

## Executive Summary

This document describes the **target architecture** for OpenAgents once all 10 features from Episode 195 are fully implemented. The vision is a **mature agent IDE** with:

1. ✅ ChatGPT-style desktop app (DONE)
2. ✅ Mobile sync (DONE)
3. ✅ Overnight scheduling (DONE, needs polish)
4. ✅ Sub-agent delegation (DONE, needs polish)
5. 🎯 Smart history/memory (needs semantic layer + workspace memory)
6. 🎯 Plugin ecosystem (needs full system)
7. ✅ Open source (DONE)
8. ✅ Local + cloud inference (DONE, needs routing UI)
9. 🎯 Compute marketplace (needs full infrastructure)
10. 🎯 Revenue sharing (needs payment infrastructure)

**Key Architectural Additions:**
- **Plugins/** - Dynamic plugin loading, MCP integration, sandboxing
- **Marketplace/** - Agent discovery, installation, reputation
- **Payments/** - Billing, usage tracking, revenue splits
- **WorkspaceMemory/** - Long-term context, knowledge graphs
- **History/** - Semantic search, NL queries, cross-workspace insights
- **PullRequests/** - Git operations, GitHub/GitLab APIs, PR monitoring

## High-Level Architecture (Future State)

```
┌───────────────────────────────────────────────────────────────────┐
│                      iOS/macOS App Layer                           │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────────┐│
│  │ Chat UI      │  │ Marketplace  │  │ Settings & Billing       ││
│  │ - Desktop    │  │ UI           │  │ - Connection             ││
│  │ - Mobile     │  │ - Browse     │  │ - Workspace              ││
│  │ - Timeline   │  │ - Install    │  │ - Agents + Plugins       ││
│  │ - Composer   │  │ - Reviews    │  │ - Orchestration          ││
│  │              │  │              │  │ - Billing + Subscription ││
│  └──────────────┘  └──────────────┘  └──────────────────────────┘│
└───────────────────────────────────────────────────────────────────┘
                               ▼
┌───────────────────────────────────────────────────────────────────┐
│                       Manager Layer (App)                          │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────────┐│
│  │ Bridge       │  │ Tinyvex      │  │ Plugin Manager           ││
│  │ Manager      │  │ Manager      │  │ - Load/unload            ││
│  │              │  │              │  │ - Sandbox                ││
│  └──────────────┘  └──────────────┘  └──────────────────────────┘│
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────────┐│
│  │ Marketplace  │  │ Billing      │  │ Workspace Memory         ││
│  │ Manager      │  │ Manager      │  │ Manager                  ││
│  │              │  │              │  │                          ││
│  └──────────────┘  └──────────────┘  └──────────────────────────┘│
└───────────────────────────────────────────────────────────────────┘
                               ▼
┌───────────────────────────────────────────────────────────────────┐
│               OpenAgentsCore (Business Logic)                      │
│  ┌──────────────────────────────────────────────────────────────┐ │
│  │ Core Protocols & Types                                       │ │
│  │ - ACP (Agent Client Protocol)                                │ │
│  │ - Bridge (JSON-RPC, WebSocket)                               │ │
│  │ - Agents (Registry, Providers, Capabilities)                 │ │
│  └──────────────────────────────────────────────────────────────┘ │
│  ┌────────────┐ ┌────────────┐ ┌────────────┐ ┌───────────────┐ │
│  │Orchestrate │ │ Plugins    │ │Marketplace │ │ Payments      │ │
│  │- Coord     │ │- Registry  │ │- Client    │ │- Billing      │ │
│  │- Decision  │ │- Loader    │ │- Installer │ │- Providers    │ │
│  │- Queue     │ │- Sandbox   │ │- Version   │ │- Usage Track  │ │
│  │- Scheduler │ │- MCP       │ │- Reputation│ │- Revenue Split│ │
│  └────────────┘ └────────────┘ └────────────┘ └───────────────┘ │
│  ┌────────────┐ ┌────────────┐ ┌────────────┐ ┌───────────────┐ │
│  │Workspace   │ │ History    │ │PullRequests│ │ Intelligence  │ │
│  │Memory      │ │- Smart Qry │ │- Git Ops   │ │- FM Orch      │ │
│  │- Knowledge │ │- Semantic  │ │- GitHub API│ │- Summarizer   │ │
│  │- Profile   │ │- Temporal  │ │- PR Monitor│ │- Embeddings   │ │
│  │- Goals     │ │- X-Workspace│ │           │ │               │ │
│  └────────────┘ └────────────┘ └────────────┘ └───────────────┘ │
│  ┌──────────────────────────────────────────────────────────────┐ │
│  │ Persistence & Infrastructure                                 │ │
│  │ - Tinyvex (SQLite: sessions, tasks, workspace KB, embeddings)│ │
│  │ - File System (workspace scanner, file ops)                  │ │
│  │ - Network (bridge server/client, marketplace API, git remote)│ │
│  └──────────────────────────────────────────────────────────────┘ │
└───────────────────────────────────────────────────────────────────┘
```

## Detailed Future-State Folder Structure

### ios/OpenAgents/ (App Layer - Future)

```
OpenAgents/
├── Views/
│   ├── macOS/
│   │   ├── Chat/                       # REFACTORED: Split from ChatAreaView
│   │   │   ├── ChatMacOSView.swift     # Root (NavigationSplitView)
│   │   │   ├── SessionSidebarView.swift
│   │   │   ├── ChatAreaView.swift      # Main container (< 200 LOC)
│   │   │   ├── MessageRowView.swift    # Single message row
│   │   │   ├── ToolCallRow.swift       # Tool call renderer
│   │   │   ├── ThinkingBlockRow.swift  # Thinking block renderer
│   │   │   └── AssistantMessageRow.swift
│   │   ├── Composer/
│   │   │   └── ComposerMac.swift
│   │   ├── Settings/                   # EXPANDED: Add plugin/billing tabs
│   │   │   ├── SettingsView.swift      # 6 tabs total
│   │   │   ├── ConnectionTab.swift
│   │   │   ├── WorkspaceTab.swift
│   │   │   ├── AgentsTab.swift
│   │   │   ├── PluginsTab.swift        # NEW: Plugin management
│   │   │   ├── OrchestrationTab.swift
│   │   │   └── BillingTab.swift        # NEW: Credits, subscription
│   │   ├── Developer/
│   │   │   └── DeveloperView.swift
│   │   ├── Marketplace/                # NEW: Marketplace UI
│   │   │   ├── MarketplaceBrowserView.swift
│   │   │   ├── AgentDetailView.swift
│   │   │   ├── PluginDetailView.swift
│   │   │   ├── ReviewsView.swift
│   │   │   └── InstallProgressView.swift
│   │   ├── Orchestration/
│   │   │   ├── OrchestrationSidebarSection.swift
│   │   │   ├── OrchestrationConsoleView.swift
│   │   │   └── CycleHistoryView.swift  # NEW: Persisted cycle audit
│   │   └── InspectorPaneView.swift
│   ├── Components/
│   │   ├── HistoryListView.swift
│   │   ├── UpdatesListView.swift
│   │   ├── JSONInspectorView.swift
│   │   ├── DrawerMenuView.swift
│   │   ├── AgentToggleRow.swift
│   │   ├── PluginRow.swift             # NEW: Plugin list item
│   │   └── BillingStatusCard.swift     # NEW: Credit balance
│   ├── ChatHomeView.swift              # iOS root
│   ├── NewChatView.swift
│   └── ACPTimelineView.swift
├── ACP/                                # Keep as-is
├── Bridge/                             # REFACTORED: Decouple
│   ├── BridgeManager.swift             # Reduced responsibilities
│   ├── BridgeCoordinator.swift         # NEW: Protocol for coordination
│   ├── BridgeManager+Mac.swift
│   ├── BridgeManager+iOS.swift
│   ├── TimelineStore.swift
│   ├── PromptDispatcher.swift
│   ├── ConnectionManager.swift
│   ├── LocalJsonRpcClient.swift
│   └── ... (other bridge files)
├── Plugins/                            # NEW: Plugin UI integration
│   ├── PluginManager.swift             # App-level plugin coordinator
│   ├── PluginInstallSheet.swift        # Install wizard
│   └── PluginSettingsView.swift        # Per-plugin settings
├── Marketplace/                        # NEW: Marketplace UI
│   └── MarketplaceManager.swift        # App-level marketplace coordinator
├── Billing/                            # NEW: Billing UI
│   ├── BillingManager.swift            # App-level billing coordinator
│   ├── SubscriptionSheet.swift         # Tier selection
│   └── PaymentMethodSheet.swift        # Add payment method
├── WorkspaceMemory/                    # NEW: Workspace memory UI
│   └── WorkspaceMemoryManager.swift    # App-level WM coordinator
├── History/                            # Keep as-is
├── ViewModels/
│   ├── ACPTimelineViewModel.swift
│   ├── OrchestrationViewModel.swift
│   ├── MarketplaceViewModel.swift      # NEW
│   ├── BillingViewModel.swift          # NEW
│   └── WorkspaceMemoryViewModel.swift  # NEW
├── Theme.swift
├── Fonts.swift
├── TinyvexManager.swift
└── OpenAgentsApp.swift
```

### ios/OpenAgentsCore/ (Business Logic - Future)

```
OpenAgentsCore/Sources/OpenAgentsCore/
├── AgentClientProtocol/                # Keep as-is
├── ACP/                                # Keep as-is
├── Bridge/                             # Keep as-is
├── DesktopBridge/                      # Keep as-is
├── MobileBridge/                       # Keep as-is
├── Agents/                             # Keep as-is
├── Orchestration/                      # REFACTORED: Split large files
│   ├── Core/
│   │   ├── AgentCoordinator.swift      # Keep (522 LOC acceptable)
│   │   ├── DecisionEngine.swift        # Keep (425 LOC acceptable)
│   │   ├── TaskQueue.swift             # Keep (344 LOC acceptable)
│   │   ├── OrchestrationConfig.swift   # Keep (441 LOC acceptable)
│   │   └── OrchestrationTypes.swift    # Keep (478 LOC acceptable)
│   ├── Scheduler/                      # SPLIT from SchedulerService.swift
│   │   ├── SchedulerService.swift      # Core loop (< 500 LOC)
│   │   ├── CronParser.swift            # Cron parsing (< 300 LOC)
│   │   ├── ConstraintChecker.swift     # Power, network, time checks
│   │   ├── TimeWindowEvaluator.swift   # Time window logic
│   │   └── SchedulerTypes.swift        # Shared types
│   ├── SessionTools/                   # SPLIT from SessionTools.swift
│   │   ├── SessionListTool.swift       # session.list (< 1000 LOC)
│   │   ├── SessionSearchTool.swift     # session.search (< 1000 LOC)
│   │   ├── SessionReadTool.swift       # session.read (< 1000 LOC)
│   │   ├── SessionAnalyzeTool.swift    # session.analyze (< 1000 LOC)
│   │   └── SessionToolsRegistry.swift  # Registration (< 100 LOC)
│   ├── FMOrchestrator.swift            # Keep as-is
│   ├── ExploreOrchestrator.swift       # Keep as-is
│   ├── SetupOrchestrator.swift         # Keep as-is
│   ├── ContentSpanTool.swift           # Keep as-is
│   ├── GrepTool.swift                  # Keep as-is
│   ├── WorkspaceScanner.swift          # Keep as-is
│   └── ... (other orchestration files)
├── Plugins/                            # NEW: Plugin system
│   ├── Core/
│   │   ├── PluginRegistry.swift        # Central registry (actor)
│   │   ├── PluginManifest.swift        # Manifest schema
│   │   ├── PluginLoader.swift          # Dynamic loading
│   │   ├── PluginSandbox.swift         # Security boundaries
│   │   └── PluginTypes.swift           # Shared types
│   ├── MCP/                            # Model Context Protocol
│   │   ├── MCPServerDiscovery.swift    # Discover MCP servers
│   │   ├── MCPClient.swift             # Connect as client
│   │   ├── MCPCapabilities.swift       # Capability negotiation
│   │   └── MCPTypes.swift              # MCP-specific types
│   ├── Providers/
│   │   ├── ToolPlugin.swift            # Plugin type: tool
│   │   ├── AgentPlugin.swift           # Plugin type: agent
│   │   ├── IntegrationPlugin.swift     # Plugin type: integration
│   │   └── ThemePlugin.swift           # Plugin type: theme
│   └── Security/
│       ├── PluginPermissions.swift     # Permission system
│       ├── PluginCodeSigning.swift     # Code signing verification
│       └── PluginSandboxExecutor.swift # Isolated execution
├── Marketplace/                        # NEW: Compute marketplace
│   ├── Core/
│   │   ├── MarketplaceClient.swift     # API client (actor)
│   │   ├── AgentPackage.swift          # Package schema
│   │   ├── PackageInstaller.swift      # Install/update/uninstall
│   │   ├── VersionManager.swift        # Version resolution
│   │   └── MarketplaceTypes.swift      # Shared types
│   ├── Discovery/
│   │   ├── SearchIndex.swift           # Local search cache
│   │   ├── CategoryBrowser.swift       # Browse by category
│   │   └── RecommendationEngine.swift  # FM-powered recommendations
│   ├── Reputation/
│   │   ├── ReputationStore.swift       # Local cache of reviews
│   │   ├── RatingSystem.swift          # Rating aggregation
│   │   └── ReviewValidator.swift       # FM-powered spam detection
│   └── RemoteExecution/
│       ├── RemoteExecutionClient.swift # Delegate to cloud agents
│       ├── TaskDistributor.swift       # Distribute work to swarm
│       └── ComputePool.swift           # Local compute pooling
├── Payments/                           # NEW: Revenue sharing
│   ├── Core/
│   │   ├── BillingService.swift        # Track credits/usage/costs (actor)
│   │   ├── UsageTracker.swift          # Meter execution time
│   │   ├── RevenueSplitter.swift       # Split revenue with authors
│   │   └── PaymentTypes.swift          # Shared types
│   ├── Providers/
│   │   ├── PaymentProvider.swift       # Protocol
│   │   ├── StripeProvider.swift        # Stripe integration
│   │   ├── AppleIAPProvider.swift      # Apple In-App Purchase
│   │   └── CryptoProvider.swift        # Bitcoin/Lightning/Nostr zaps
│   ├── Subscription/
│   │   ├── SubscriptionManager.swift   # Manage tiers
│   │   ├── TierDefinitions.swift       # Free, Pro, Enterprise
│   │   └── FeatureGating.swift         # Tier-based feature access
│   └── Analytics/
│       ├── UsageAnalytics.swift        # Usage patterns (private)
│       └── CostEstimator.swift         # Estimate task costs
├── WorkspaceMemory/                    # NEW: Long-term context
│   ├── Core/
│   │   ├── WorkspaceKnowledgeGraph.swift # Entity-relation graph (actor)
│   │   ├── WorkspaceProfile.swift        # Workspace metadata
│   │   ├── WorkspaceConventions.swift    # Style, architecture
│   │   ├── WorkspaceGoals.swift          # Long-term goals
│   │   └── WorkspaceTypes.swift          # Shared types
│   ├── Analysis/
│   │   ├── WorkspaceSummarizer.swift     # FM-powered understanding
│   │   ├── ConventionDetector.swift      # Detect code style
│   │   ├── ArchitectureAnalyzer.swift    # Understand structure
│   │   └── GoalTracker.swift             # Track goal progress
│   ├── Memory/
│   │   ├── EntityExtractor.swift         # Extract entities from sessions
│   │   ├── RelationBuilder.swift         # Build relations between entities
│   │   └── MemoryConsolidator.swift      # Consolidate memories over time
│   └── Persistence/
│       ├── KnowledgeGraphStore.swift     # Persist graph to Tinyvex
│       └── ConventionsCache.swift        # Cache conventions
├── History/                            # ENHANCED: Smart recall
│   ├── Core/
│   │   ├── HistoryLoader.swift           # Keep as-is
│   │   └── ThreadSummary.swift           # Keep as-is
│   ├── Query/
│   │   ├── SmartHistoryQuery.swift       # NL → SQL/semantic
│   │   ├── TemporalQueryParser.swift     # "last Monday", "past 2 weeks"
│   │   ├── SemanticHistorySearch.swift   # Integrate embeddings
│   │   └── CrossWorkspaceHistory.swift   # Query across workspaces
│   ├── Search/
│   │   ├── HistorySearchIndex.swift      # Full-text search index
│   │   ├── SemanticIndex.swift           # Embedding-based index
│   │   └── HybridSearch.swift            # Combine FTS + semantic
│   └── Insights/
│       ├── SessionInsights.swift         # Extract patterns from sessions
│       ├── ProductivityAnalyzer.swift    # Analyze productivity trends
│       └── WorkPatternDetector.swift     # Detect work patterns
├── PullRequests/                       # NEW: PR automation
│   ├── Core/
│   │   ├── PRService.swift               # High-level PR operations (actor)
│   │   ├── GitOperations.swift           # Branch, commit, push
│   │   └── PRTypes.swift                 # Shared types
│   ├── Providers/
│   │   ├── GitProvider.swift             # Protocol
│   │   ├── GitHubClient.swift            # GitHub API
│   │   ├── GitLabClient.swift            # GitLab API
│   │   └── BitbucketClient.swift         # Bitbucket API
│   ├── Templates/
│   │   ├── PRTemplate.swift              # Customizable PR body
│   │   ├── PRTitleGenerator.swift        # FM-powered title
│   │   └── PRDescriptionGenerator.swift  # FM-powered description
│   └── Monitoring/
│       ├── PRMonitor.swift               # Track PR status
│       ├── CheckRunner.swift             # Monitor CI checks
│       └── ReviewTracker.swift           # Track reviews
├── Tinyvex/                            # ENHANCED: Add new tables
│   ├── TinyvexCore.swift
│   ├── Client/
│   │   └── TinyvexClient.swift
│   ├── Server/
│   │   └── TinyvexServer.swift
│   ├── Schema/                         # NEW: Schema definitions
│   │   ├── SessionSchema.swift
│   │   ├── TaskSchema.swift
│   │   ├── KnowledgeGraphSchema.swift  # NEW: Workspace memory
│   │   ├── EmbeddingSchema.swift       # NEW: Cached embeddings
│   │   ├── UsageSchema.swift           # NEW: Usage tracking
│   │   └── MigrationRunner.swift       # Schema migrations
│   └── Queries/                        # NEW: Query builders
│       ├── SessionQueries.swift
│       ├── HistoryQueries.swift
│       ├── WorkspaceQueries.swift
│       └── UsageQueries.swift
├── Providers/                          # Keep as-is
├── Translators/                        # Keep as-is
├── Summarization/                      # Keep as-is
├── Embeddings/                         # ENHANCED: Add caching
│   ├── EmbeddingService.swift          # Keep as-is
│   ├── EmbeddingProvider.swift         # Keep as-is
│   ├── MLXEmbeddingProvider.swift      # Keep as-is
│   ├── VectorStore.swift               # Keep as-is
│   ├── EmbeddingTypes.swift            # Keep as-is
│   ├── EmbeddingCache.swift            # NEW: Cache embeddings to Tinyvex
│   └── IncrementalEmbedder.swift       # NEW: Only embed new content
├── Nostr/                              # Keep as-is (low priority)
├── Utils/                              # Keep as-is
└── OpenAgentsLog.swift                 # Keep as-is
```

## New Module Descriptions

### Plugins/ Module

**Purpose:** Dynamic plugin system for extensibility.

**Key Components:**
- **PluginRegistry:** Central registry for installed plugins (actor)
- **PluginManifest:** Manifest schema (name, version, capabilities, dependencies, permissions)
- **PluginLoader:** Load/unload plugins dynamically
- **PluginSandbox:** Run plugins in isolated processes with limited permissions
- **MCPClient:** Connect to MCP servers as a client
- **MCPServerDiscovery:** Discover MCP servers via Bonjour or hardcoded URLs

**Plugin Types:**
- **ToolPlugin:** Adds new tools to orchestration (e.g., Slack integration)
- **AgentPlugin:** Adds new agent providers (e.g., Gemini integration)
- **IntegrationPlugin:** Adds new integrations (e.g., Jira, Linear)
- **ThemePlugin:** Adds custom themes/UI components

**Security:**
- **Code signing:** Verify plugin authenticity
- **Permissions:** File access, network, agent execution
- **Sandboxing:** Run in separate process with entitlements

**Example Manifest:**
```json
{
  "id": "com.example.slack-plugin",
  "name": "Slack Integration",
  "version": "1.0.0",
  "author": "Jane Doe",
  "capabilities": ["tool"],
  "permissions": ["network"],
  "dependencies": {
    "openagents": ">=0.4.0"
  },
  "tools": [
    {
      "name": "slack.send",
      "description": "Send a message to Slack",
      "parameters": { ... }
    }
  ]
}
```

### Marketplace/ Module

**Purpose:** Discover, install, and manage third-party agents/plugins.

**Key Components:**
- **MarketplaceClient:** API client for marketplace (actor)
- **AgentPackage:** Package schema (manifest, binaries, checksums)
- **PackageInstaller:** Install/update/uninstall agents
- **VersionManager:** Version resolution, rollback
- **ReputationStore:** Local cache of ratings/reviews
- **RemoteExecutionClient:** Delegate tasks to cloud agents
- **ComputePool:** Local compute pooling (share idle compute)

**User Flows:**
1. **Browse:** Search/filter agents by category, rating, price
2. **Install:** Download, verify checksum, install to `~/Library/OpenAgents/Plugins/`
3. **Review:** Rate and review installed plugins
4. **Update:** Check for updates, install new versions
5. **Uninstall:** Remove plugin and clean up data

**Discovery:**
- **Category browsing:** "Code Generation", "Testing", "Documentation"
- **Search:** Full-text search on name, description, tags
- **Recommendations:** FM-powered recommendations based on workspace context

**Remote Execution:**
- **Delegate to cloud:** For heavy tasks (e.g., "run full test suite on 100 cores")
- **Swarm compute:** Distribute work to other OpenAgents users (opt-in)
- **Pricing:** Pay per compute hour (credits)

### Payments/ Module

**Purpose:** Billing, usage tracking, and revenue sharing.

**Key Components:**
- **BillingService:** Track credits, usage, costs (actor)
- **UsageTracker:** Meter agent execution time, API calls, compute hours
- **RevenueSplitter:** Split revenue with plugin authors (e.g., 70% author, 30% OpenAgents)
- **PaymentProvider:** Protocol for payment methods
- **StripeProvider:** Stripe integration (credit card)
- **AppleIAPProvider:** Apple In-App Purchase (subscription)
- **CryptoProvider:** Bitcoin/Lightning/Nostr zaps (microtransactions)
- **SubscriptionManager:** Manage tiers (Free, Pro, Enterprise)

**Tiers:**
- **Free:** 100 credits/month, local agents only, community plugins
- **Pro:** $20/month, 10,000 credits, remote execution, premium plugins
- **Enterprise:** Custom pricing, unlimited credits, dedicated compute, priority support

**Usage Tracking:**
- **Local agents:** Free (runs on user's machine)
- **Remote execution:** Paid (runs on OpenAgents cloud or swarm)
- **Plugin usage:** Plugin authors set price (e.g., $0.01 per call)

**Revenue Sharing:**
- **Plugin sales:** Author gets 70%, OpenAgents gets 30%
- **Usage fees:** Author gets 70% of per-call fees, OpenAgents gets 30%
- **Payouts:** Monthly via Stripe, Lightning, or Nostr zaps

### WorkspaceMemory/ Module

**Purpose:** Long-term workspace context and knowledge graph.

**Key Components:**
- **WorkspaceKnowledgeGraph:** Entity-relation graph (actor)
  - Entities: Files, functions, classes, PRs, issues, people, conventions
  - Relations: "refactored by", "depends on", "similar to", "used in"
- **WorkspaceProfile:** Workspace-level metadata (languages, frameworks, team size)
- **WorkspaceConventions:** Detected code style (tabs vs spaces, naming, architecture)
- **WorkspaceGoals:** Long-term goals and progress tracking
- **WorkspaceSummarizer:** FM-powered workspace understanding

**Memory Lifecycle:**
1. **Extract:** Parse sessions, extract entities (files, functions, people)
2. **Relate:** Build relations between entities (e.g., "refactored by Alice")
3. **Consolidate:** Merge similar entities, prune old memories
4. **Query:** Answer questions like "Who usually works on auth code?"

**Example Queries:**
- "What files did I refactor last week?"
- "Who is the expert on the auth system?"
- "What's the coding convention for error handling?"
- "What's my progress on the 'migrate to SwiftUI' goal?"

**Persistence:**
- Store graph in Tinyvex (`workspace_entities`, `workspace_relations` tables)
- Cache conventions in memory for fast access
- Prune old/irrelevant memories periodically

### History/ Module (Enhanced)

**Purpose:** Smart history recall with semantic search and NL queries.

**Key Components:**
- **SmartHistoryQuery:** Natural language → SQL/semantic search
  - "show me all sessions where I refactored error handling"
  - "what did I work on last Monday?"
- **TemporalQueryParser:** Parse temporal queries ("last week", "past 2 weeks")
- **SemanticHistorySearch:** Integrate embeddings with history
- **CrossWorkspaceHistory:** Query across all workspaces
- **HistorySearchIndex:** Full-text search index (SQLite FTS5)
- **SemanticIndex:** Embedding-based index (VectorStore + Tinyvex)
- **HybridSearch:** Combine FTS + semantic for best results

**Query Examples:**
- **Temporal:** "what did I work on yesterday?" → SQL `WHERE date >= yesterday`
- **Semantic:** "show me sessions about authentication" → Embedding search
- **Hybrid:** "show me recent sessions about error handling" → FTS + temporal filter
- **Cross-workspace:** "show me all PRs across all workspaces" → Join workspaces

**Indexing:**
- Index session summaries, tool calls, file paths, commit messages
- Update index incrementally (only new sessions)
- Re-embed on major version changes

### PullRequests/ Module

**Purpose:** Automated PR creation, monitoring, and management.

**Key Components:**
- **PRService:** High-level PR operations (actor)
- **GitOperations:** Branch, commit, push (shell out to `git`)
- **GitHubClient:** GitHub API integration (REST + GraphQL)
- **GitLabClient:** GitLab API integration
- **PRTemplate:** Customizable PR body (user-defined or FM-generated)
- **PRMonitor:** Track PR status (checks, reviews, merge conflicts)

**PR Creation Flow:**
1. **Checkout branch:** `git checkout -b feature/overnight-task-123`
2. **Commit changes:** `git commit -m "Add tests for auth module"`
3. **Push to remote:** `git push -u origin feature/overnight-task-123`
4. **Create PR:** POST to GitHub API with title/body/labels
5. **Monitor:** Poll for check status, review comments

**PR Template (FM-Generated):**
```markdown
## Summary
Added comprehensive tests for the auth module, increasing coverage from 60% to 85%.

## Changes
- Added 15 new test cases for login/logout flows
- Refactored auth fixtures for better reusability
- Fixed flaky test in `testTokenRefresh`

## Test Plan
- ✅ All tests pass locally
- ✅ CI checks pass
- ⏳ Awaiting code review

🤖 Generated with OpenAgents overnight orchestration
```

**PR Monitoring:**
- **Check status:** Monitor CI checks (GitHub Actions, CircleCI, etc.)
- **Review comments:** Parse review comments, respond to simple requests
- **Merge conflicts:** Detect conflicts, notify user
- **Auto-merge:** Merge when checks pass + approved (if configured)

## Data Model Extensions

### Tinyvex Schema (Future)

**New Tables:**

```sql
-- Workspace knowledge graph
CREATE TABLE workspace_entities (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL,
  type TEXT NOT NULL,  -- 'file', 'function', 'class', 'person', 'convention'
  name TEXT NOT NULL,
  metadata JSON,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE TABLE workspace_relations (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL,
  from_entity_id TEXT NOT NULL,
  to_entity_id TEXT NOT NULL,
  relation_type TEXT NOT NULL,  -- 'refactored_by', 'depends_on', 'similar_to'
  confidence REAL,  -- 0.0 to 1.0
  created_at INTEGER NOT NULL,
  FOREIGN KEY (from_entity_id) REFERENCES workspace_entities(id),
  FOREIGN KEY (to_entity_id) REFERENCES workspace_entities(id)
);

-- Cached embeddings
CREATE TABLE embeddings (
  id TEXT PRIMARY KEY,
  content_hash TEXT NOT NULL UNIQUE,  -- SHA256 of content
  embedding BLOB NOT NULL,  -- Float array
  model_version TEXT NOT NULL,
  created_at INTEGER NOT NULL
);

-- Usage tracking
CREATE TABLE usage_events (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL,
  event_type TEXT NOT NULL,  -- 'agent_run', 'plugin_call', 'remote_execution'
  agent_id TEXT,
  plugin_id TEXT,
  duration_ms INTEGER,
  cost_credits REAL,
  metadata JSON,
  created_at INTEGER NOT NULL
);

-- Billing
CREATE TABLE transactions (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL,
  type TEXT NOT NULL,  -- 'charge', 'credit', 'payout'
  amount_credits REAL,
  amount_usd REAL,
  payment_method TEXT,
  status TEXT NOT NULL,  -- 'pending', 'completed', 'failed'
  metadata JSON,
  created_at INTEGER NOT NULL
);

-- Plugin registry
CREATE TABLE plugins (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  version TEXT NOT NULL,
  author TEXT NOT NULL,
  manifest JSON NOT NULL,
  installed_at INTEGER NOT NULL,
  enabled BOOLEAN DEFAULT 1
);

-- Orchestration cycle audit
CREATE TABLE orchestration_cycles (
  id TEXT PRIMARY KEY,
  config_id TEXT NOT NULL,
  cycle_number INTEGER NOT NULL,
  decision JSON,  -- DecisionOutput
  task JSON,  -- OvernightTask
  result JSON,  -- Success/failure
  started_at INTEGER NOT NULL,
  completed_at INTEGER,
  duration_ms INTEGER
);
```

## API Surface Changes

### New Bridge RPC Methods

```typescript
// Plugin management
"plugin/list": () => Plugin[]
"plugin/install": (packageUrl: string) => InstallProgress
"plugin/uninstall": (pluginId: string) => void
"plugin/enable": (pluginId: string) => void
"plugin/disable": (pluginId: string) => void
"plugin/configure": (pluginId: string, settings: JSON) => void

// Marketplace
"marketplace/search": (query: string, filters: Filters) => SearchResults
"marketplace/detail": (packageId: string) => PackageDetail
"marketplace/reviews": (packageId: string) => Review[]
"marketplace/install": (packageId: string) => InstallProgress

// Billing
"billing/balance": () => { credits: number, subscription: Tier }
"billing/usage": (startDate: Date, endDate: Date) => UsageEvent[]
"billing/addPaymentMethod": (method: PaymentMethod) => void
"billing/subscribe": (tier: Tier) => Subscription

// Workspace memory
"workspace/knowledge": () => KnowledgeGraph
"workspace/profile": () => WorkspaceProfile
"workspace/conventions": () => Conventions
"workspace/goals": () => Goal[]
"workspace/query": (query: string) => Entity[]

// History
"history/smartQuery": (query: string) => Session[]
"history/semanticSearch": (query: string) => Session[]
"history/crossWorkspace": (query: string) => Session[]

// Pull requests
"pr/create": (params: PRParams) => PR
"pr/monitor": (prId: string) => PRStatus
"pr/list": () => PR[]

// Orchestration audit
"orchestration/cycles": (configId: string) => Cycle[]
"orchestration/cycleDetail": (cycleId: string) => CycleDetail
```

## Configuration Extensions

### OrchestrationConfig (Enhanced)

```swift
struct OrchestrationConfig {
  // Existing fields...

  // NEW: Plugin preferences
  var pluginPreferences: PluginPreferences?

  // NEW: Billing constraints
  var billingConstraints: BillingConstraints?

  // NEW: Workspace memory settings
  var workspaceMemory: WorkspaceMemorySettings?

  // NEW: PR automation (full implementation)
  var prAutomation: PRAutomation  // Already exists, now fully implemented
}

struct PluginPreferences {
  var enabledPlugins: [String]
  var pluginSettings: [String: JSON]  // Plugin-specific settings
}

struct BillingConstraints {
  var maxCostPerCycle: Double?  // Max credits per cycle
  var maxCostPerDay: Double?    // Max credits per day
  var preferLocalCompute: Bool  // Prefer local over remote
}

struct WorkspaceMemorySettings {
  var enabled: Bool
  var autoConsolidate: Bool       // Consolidate memories periodically
  var memoryRetention: TimeInterval  // How long to keep memories
}
```

### User Settings (Enhanced)

```swift
struct UserSettings {
  // Existing fields...

  // NEW: Plugin settings
  var installedPlugins: [Plugin]
  var pluginPermissions: [String: [Permission]]

  // NEW: Billing settings
  var subscription: Tier
  var paymentMethods: [PaymentMethod]
  var creditBalance: Double

  // NEW: Marketplace settings
  var marketplaceUrl: URL
  var autoUpdate: Bool
}
```

## Security Model (Future)

### Plugin Permissions

```swift
enum Permission: String {
  case fileRead           // Read files in workspace
  case fileWrite          // Write files in workspace
  case network            // Make network requests
  case agentExecution     // Execute agents
  case embedding          // Generate embeddings
  case billing            // Charge credits
  case workspaceMemory    // Read/write workspace memory
}
```

### Plugin Sandboxing

- **Separate process:** Plugins run in separate process with entitlements
- **IPC:** Communicate with main app via XPC or local socket
- **File system:** Plugins can only access granted directories
- **Network:** Plugins can only access granted domains (e.g., slack.com)
- **Revocation:** User can revoke permissions at any time

### Code Signing

- **Developer ID:** Plugins must be signed with Apple Developer ID
- **Notarization:** Plugins must be notarized by Apple
- **Verification:** App verifies signature before loading
- **Revocation:** Revoke plugins with invalid signatures

## Performance Considerations

### Caching Strategy

- **Embeddings:** Cache in Tinyvex, only embed new content
- **Workspace memory:** Cache conventions in memory, persist to Tinyvex
- **History index:** Incremental updates, rebuild only when needed
- **Plugin manifests:** Cache in memory, reload on change

### Pagination

- **History:** Load sessions in batches (50 at a time)
- **Marketplace:** Load packages in pages (20 per page)
- **Usage events:** Load events in date ranges (last 30 days)

### Background Processing

- **Summarization:** Background thread, low priority
- **Embedding:** Background thread, batch processing
- **Memory consolidation:** Background thread, scheduled (e.g., 3 AM)
- **Plugin updates:** Background thread, check daily

## Migration Path

See `refactoring-roadmap.md` for detailed migration steps.

**High-Level Phases:**
1. **Phase 1: Code Quality Improvements** (1 week)
   - Extract ChatAreaView.swift (672 LOC → message row components)
   - Decouple BridgeManager (introduce coordinator protocol)

2. **Phase 2: Plugin System** (3-4 weeks)
   - PluginRegistry, PluginManifest, PluginLoader
   - MCP integration (MCPClient, MCPServerDiscovery)
   - Plugin sandboxing and permissions

3. **Phase 3: Marketplace** (2-3 weeks)
   - MarketplaceClient, AgentPackage, PackageInstaller
   - Discovery UI, installation flow
   - Reputation system

4. **Phase 4: Payments** (2-3 weeks)
   - BillingService, UsageTracker, RevenueSplitter
   - Stripe integration, Apple IAP
   - Subscription management

5. **Phase 5: Workspace Memory** (2-3 weeks)
   - WorkspaceKnowledgeGraph, WorkspaceProfile
   - Memory extraction, consolidation
   - Query interface

6. **Phase 6: Smart History** (1-2 weeks)
   - SmartHistoryQuery, SemanticHistorySearch
   - Hybrid search (FTS + embeddings)
   - Cross-workspace queries

7. **Phase 7: PR Automation** (1-2 weeks)
   - PRService, GitOperations, GitHubClient
   - PR creation flow, monitoring
   - Auto-merge logic

**Total Estimated Time:** 11-17 weeks (2.5-4 months) for full implementation

## Success Metrics

### Plugin Ecosystem
- **Goal:** 50+ plugins in marketplace within 6 months
- **Metric:** Plugin installs per month, plugin usage rate

### Marketplace
- **Goal:** 10,000 MAU (Monthly Active Users) within 1 year
- **Metric:** New installs, DAU/MAU ratio, retention rate

### Payments
- **Goal:** $100k MRR (Monthly Recurring Revenue) within 1 year
- **Metric:** Subscription conversions, ARPU (Average Revenue Per User), churn rate

### Workspace Memory
- **Goal:** 80% of queries answered without user intervention
- **Metric:** Query accuracy, user satisfaction, time saved

### Smart History
- **Goal:** 90% of history queries return relevant results
- **Metric:** Click-through rate, result relevance, query latency

### PR Automation
- **Goal:** 50% of overnight runs create PRs automatically
- **Metric:** PR creation rate, PR merge rate, CI pass rate

## Conclusion

This future-state architecture represents a **mature agent IDE** with a thriving plugin ecosystem, marketplace, and revenue model. The key architectural additions are:

1. **Plugins/** - Extensibility via dynamic plugin loading and MCP integration
2. **Marketplace/** - Discovery and installation of third-party agents
3. **Payments/** - Billing, usage tracking, and revenue sharing
4. **WorkspaceMemory/** - Long-term workspace context and knowledge graphs
5. **History/** - Smart recall with semantic search and NL queries
6. **PullRequests/** - Automated PR creation and monitoring

With these additions, OpenAgents will be a **10x improvement** over existing coding agents, delivering on all 10 features from Episode 195.

---

**Next Steps:** See `refactoring-roadmap.md` for detailed migration plan.
