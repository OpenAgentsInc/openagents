# Overnight Agents Tech Demo

**Vision**: Demonstrate autonomous agent orchestration working around the clock, with on-device Apple Intelligence making smart decisions about what work to delegate to which agents.

## Demo Concept

Agents (Claude Code, OpenAI Codex) run overnight on macOS, orchestrated by Foundation Models that periodically (every 10-60 minutes) decide:
- What tasks need attention based on repo state and session history
- Which agent is best suited for each task
- What priority and constraints to apply

By morning, you wake up to 5-10 pull requests with quality refactoring work and feature implementations, all created autonomously while you slept.

**Key Proof Points**:
1. ✅ **Agent Control**: Full command of Claude Code and OpenAI Codex via ACP
2. ✅ **Smart Orchestration**: On-device FM makes context-aware decisions
3. ✅ **Periodic Check-ins**: Cron-like scheduler with constraint enforcement
4. ✅ **Deterministic Logic**: All orchestration logic in JSON "upgrade" manifests
5. ✅ **Future-Ready**: Manifest format designed for Nostr marketplace + Bitcoin payments

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                        macOS Desktop                             │
├─────────────────────────────────────────────────────────────────┤
│                                                                   │
│  ┌─────────────────┐         ┌──────────────────┐               │
│  │ SchedulerService│────────▶│UpgradeExecutor   │               │
│  │ (Timer: 30min)  │         │ (JSON manifest)   │               │
│  └─────────────────┘         └──────────────────┘               │
│          │                            │                          │
│          ▼                            ▼                          │
│  ┌─────────────────────────────────────────────┐                │
│  │     DecisionOrchestrator                    │                │
│  │  ┌──────────────────────────────────────┐   │                │
│  │  │  Foundation Models (macOS 26+)       │   │                │
│  │  │  "What should we work on next?"      │   │                │
│  │  └──────────────────────────────────────┘   │                │
│  │                                              │                │
│  │  Input: Session history, repo state         │                │
│  │  Output: Task + Agent + Priority            │                │
│  └─────────────────────────────────────────────┘                │
│          │                                                       │
│          ▼                                                       │
│  ┌─────────────────┐                                            │
│  │   TaskQueue     │ (SQLite/Tinyvex)                           │
│  │  - pending      │                                            │
│  │  - in_progress  │                                            │
│  │  - completed    │                                            │
│  └─────────────────┘                                            │
│          │                                                       │
│          ▼                                                       │
│  ┌─────────────────────────────────────────────┐                │
│  │      AgentCoordinator                       │                │
│  │  ┌──────────────┐    ┌──────────────┐      │                │
│  │  │ Claude Code  │    │ OpenAI Codex │      │                │
│  │  │ Provider     │    │ Provider     │      │                │
│  │  └──────────────┘    └──────────────┘      │                │
│  └─────────────────────────────────────────────┘                │
│          │                                                       │
│          ▼ (ACP SessionUpdate stream)                           │
│  ┌─────────────────────────────────────────────┐                │
│  │    PRAutomationService                      │                │
│  │    - Create branches                        │                │
│  │    - Generate commits from tool calls       │                │
│  │    - Push to GitHub                         │                │
│  │    - Create PRs via gh CLI                  │                │
│  └─────────────────────────────────────────────┘                │
│          │                                                       │
└──────────┼───────────────────────────────────────────────────────┘
           │
           │ (WebSocket Bridge)
           ▼
┌─────────────────────────────────────────────────────────────────┐
│                          iOS Device                              │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────────────────────────┐                │
│  │   Overnight Monitoring UI                   │                │
│  │  - Real-time task queue                     │                │
│  │  - Agent session cards                      │                │
│  │  - FM decision rationale                    │                │
│  │  - PR preview & approval                    │                │
│  └─────────────────────────────────────────────┘                │
└─────────────────────────────────────────────────────────────────┘
```

## Demo Flow (Overnight Run)

### Phase 1: Setup (6:00 PM)
```json
// Load upgrade manifest: nightly-refactor.json
{
  "id": "nightly-refactor-v1",
  "schedule": {
    "type": "cron",
    "expression": "*/30 1-5 * * *",  // Every 30 min, 1am-5am
    "constraints": {
      "plugged_in": true,
      "wifi_only": true,
      "cpu_max_percentage": 80,
      "respect_dnd": false
    }
  },
  "pipeline": [
    {
      "op": "orchestrate.decide",
      "backend": "foundation_models",
      "context": ["session.history", "repo.status"]
    },
    {
      "op": "agent.execute",
      "agent": "{orchestrate.decide.agent}",
      "task": "{orchestrate.decide.task}",
      "max_duration": "30m"
    },
    {
      "op": "pr.create",
      "branch": "agent/{session_id}",
      "auto_push": true
    }
  ]
}
```

User runs:
```bash
# Start scheduler with upgrade manifest
openagents scheduler start \
  --upgrade examples/nightly-refactor.json \
  --dry-run=false \
  --log-level=debug
```

### Phase 2: Overnight Execution (1:00 AM - 5:00 AM)

**1:00 AM - First Check-in**
- SchedulerService wakes up (30-min interval)
- Checks constraints: ✓ plugged in, ✓ WiFi, ✓ CPU idle
- Triggers DecisionOrchestrator

DecisionOrchestrator:
```swift
// Uses Foundation Models with session history tools
let decision = await fmOrchestrator.decideNextTask(
    context: [
        "Recent sessions: 15 Claude Code, 8 Codex",
        "Most touched files: BridgeManager.swift, AgentProvider.swift",
        "Common user intents: refactor, add tests, improve error handling"
    ]
)
// Returns: {
//   task: "Refactor BridgeManager error handling with proper Swift Result types",
//   agent: "claude-code",
//   priority: "high",
//   estimated_duration: "20-30 minutes",
//   rationale: "User frequently requests error handling improvements in bridge code"
// }
```

TaskQueue enqueues task → AgentCoordinator delegates to Claude Code

**1:25 AM - Claude Code completes work**
- ACP stream shows 47 tool calls, 230 lines changed
- PRAutomationService:
  - Creates branch `agent/claude-code-session-abc123`
  - Generates commits from tool call history
  - Pushes to GitHub
  - Creates PR #42: "Refactor BridgeManager error handling"

**1:30 AM - Second Check-in**
- SchedulerService wakes up again
- DecisionOrchestrator analyzes new context:
  - Recent completion: error handling refactor
  - Repo state: 12 files without tests
  - Session history: user mentioned "need more tests"

Decision: "Generate tests for WebSocketServer" → Codex

**2:00 AM - Codex finishes**
- PR #43: "Add comprehensive tests for DesktopWebSocketServer"

**2:30 AM - Third Check-in**
- Decision: "Refactor SessionUpdateHub to use Swift 6 concurrency" → Claude Code

**3:00 AM - PR #44 created**

... continues every 30 minutes until 5:00 AM

### Phase 3: Morning Review (8:00 AM)

User wakes up, opens GitHub:
- **8 PRs created overnight** (5 refactoring, 3 testing)
- **~1,200 lines of quality code** across all PRs
- **All tests passing** (agents ran tests before creating PRs)
- **Clear commit messages** generated from ACP tool call context

iOS app shows overnight summary:
- 9 orchestration decisions made by FM
- 8 tasks completed successfully
- 1 task skipped (time budget exceeded)
- Total autonomous work time: 3.5 hours
- Agent selection: 5× Claude Code, 3× Codex

## Key Components

### 1. SchedulerService
**File**: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/SchedulerService.swift`

Timer-based orchestration with constraint checking:
```swift
actor SchedulerService {
    func start(upgrade: UpgradeManifest) async throws
    func stop() async
    func checkConstraints() async -> Bool
    func nextWakeTime() -> Date
}
```

### 2. DecisionOrchestrator
**File**: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/DecisionOrchestrator.swift`

Foundation Models-powered decision engine:
```swift
actor DecisionOrchestrator {
    func decideNextTask(context: OrchestrationContext) async throws -> TaskDecision
    func analyzeSessionHistory() async throws -> [SessionInsight]
    func prioritizeTasks(_ candidates: [TaskCandidate]) async throws -> [TaskDecision]
}

struct TaskDecision {
    let task: String
    let agent: AgentType  // .claude_code or .codex
    let priority: Priority
    let estimatedDuration: TimeInterval
    let rationale: String  // FM explanation
}
```

### 3. TaskQueue
**File**: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/TaskQueue.swift`

Persistent work queue with SQLite/Tinyvex:
```swift
actor TaskQueue {
    func enqueue(_ task: OvernightTask) async throws -> TaskID
    func dequeue() async throws -> OvernightTask?
    func updateStatus(_ taskId: TaskID, status: TaskStatus) async throws
    func all() async throws -> [OvernightTask]
}

struct OvernightTask {
    let id: TaskID
    var status: TaskStatus  // pending, in_progress, completed, failed
    let decision: TaskDecision
    let sessionId: String?
    let createdAt: Date
    var completedAt: Date?
    let metadata: [String: String]
}
```

### 4. AgentCoordinator
**File**: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/AgentCoordinator.swift`

Multi-agent session manager:
```swift
actor AgentCoordinator {
    func delegate(_ task: OvernightTask) async throws -> AgentSessionResult
    func monitorSession(_ sessionId: String) -> AsyncStream<ACPSessionUpdate>
    func cancelSession(_ sessionId: String) async throws
}
```

### 5. PRAutomationService
**File**: `ios/OpenAgentsCore/Sources/OpenAgentsCore/GitHubIntegration/PRAutomationService.swift`

GitHub integration via `gh` CLI:
```swift
actor PRAutomationService {
    func createBranch(from baseBranch: String, name: String) async throws
    func commitFromToolCalls(_ toolCalls: [ACPToolCallWire]) async throws
    func push(branch: String, remote: String) async throws
    func createPR(title: String, body: String, branch: String) async throws -> PRNumber
}
```

### 6. UpgradeExecutor
**File**: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Upgrades/UpgradeExecutor.swift`

JSON manifest runtime:
```swift
actor UpgradeExecutor {
    func load(_ manifestPath: String) async throws -> UpgradeManifest
    func validate(_ manifest: UpgradeManifest) async throws
    func execute(_ pipeline: [UpgradeOperation]) async throws -> ExecutionResult
}
```

### 7. PolicyEnforcer
**File**: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/PolicyEnforcer.swift`

Safety and resource constraints:
```swift
actor PolicyEnforcer {
    func checkAUP(_ prompt: String) async throws -> PolicyResult
    func checkResourceLimits() async throws -> Bool
    func enforceTimeBudget(_ task: OvernightTask, elapsed: TimeInterval) -> Bool
}
```

## Upgrade Manifest Format

Full JSON schema with all supported operations:

```json
{
  "$schema": "https://openagents.com/schemas/upgrade-manifest-v1.json",
  "id": "nightly-refactor-v1",
  "version": "1.0.0",
  "title": "Nightly Code Refactoring Agent",
  "description": "Autonomous overnight refactoring with FM-driven decisions",
  "author": {
    "name": "Christopher David",
    "npub": "npub1...",
    "url": "https://github.com/openclay"
  },
  "license": "MIT",
  "categories": ["development", "code-quality"],
  "tags": ["refactoring", "testing", "automation"],

  "capabilities": {
    "platforms": ["macos"],
    "min_macos_version": "26.0",
    "backends": ["foundation_models"],
    "required_tools": ["git", "gh"]
  },

  "permissions": {
    "filesystem": {
      "read": ["$WORKSPACE/**"],
      "write": ["$WORKSPACE/**"],
      "exclude": [".env", "*.key", "credentials.json"]
    },
    "network": {
      "allowed_domains": ["github.com", "api.github.com"],
      "ports": []
    },
    "tools": {
      "allowed": ["git", "gh", "xcodebuild", "swift"],
      "denied": ["rm", "curl", "wget"]
    }
  },

  "schedule": {
    "type": "cron",
    "expression": "*/30 1-5 * * *",
    "timezone": "America/Los_Angeles",
    "window": {
      "start": "01:00",
      "end": "05:00"
    },
    "constraints": {
      "plugged_in": true,
      "wifi_only": true,
      "cpu_max_percentage": 80,
      "respect_dnd": false,
      "suspend_if_active": true
    },
    "jitter": 300,
    "on_missed": "run_once_at_next_opportunity"
  },

  "triggers": [
    {
      "type": "file_change",
      "glob": "**/*.swift",
      "debounce": 600
    }
  ],

  "pipeline": [
    {
      "op": "session.analyze",
      "params": {
        "providers": ["claude-code", "codex"],
        "topK": 20,
        "since": "7d"
      }
    },
    {
      "op": "orchestrate.decide",
      "backend": "foundation_models",
      "model": "default",
      "context": [
        "{session.analyze.insights}",
        "{repo.status}",
        "{repo.recent_commits}"
      ],
      "prompt": "Based on recent development patterns, suggest the highest-impact refactoring or testing task. Consider files touched most often, user intents, and code quality metrics.",
      "output_format": "json",
      "max_tokens": 1000
    },
    {
      "op": "agent.execute",
      "agent": "{orchestrate.decide.agent}",
      "task": "{orchestrate.decide.task}",
      "working_dir": "$WORKSPACE",
      "max_duration": "30m",
      "resume_on_error": false,
      "stream_updates": true
    },
    {
      "op": "pr.create",
      "branch_prefix": "agent/",
      "base_branch": "main",
      "title": "{orchestrate.decide.task}",
      "body_template": "## Autonomous Agent Work\n\n**Task**: {orchestrate.decide.task}\n\n**Agent**: {agent.execute.agent}\n\n**Rationale**: {orchestrate.decide.rationale}\n\n**Session**: {agent.execute.session_id}\n\n**Duration**: {agent.execute.duration}\n\n---\n\nGenerated with [OpenAgents Overnight Orchestration](https://github.com/OpenAgentsInc/openagents)",
      "auto_push": true,
      "draft": false
    }
  ],

  "pricing": {
    "model": "pay_per_run",
    "amount_sats": 1000,
    "revenue_split": [
      {"npub": "npub1...", "basis_points": 10000}
    ]
  },

  "policy": {
    "aup_compliance": true,
    "data_retention": "30d",
    "telemetry_level": "aggregate"
  },

  "signing": {
    "manifest_sha256": "abc123...",
    "nostr_event_id": "def456...",
    "sig_author": "sig123..."
  }
}
```

## Testing Strategy

### Unit Tests
- SchedulerService constraint checking
- DecisionOrchestrator FM integration
- TaskQueue persistence and lifecycle
- UpgradeManifest parsing and validation

### Integration Tests
- End-to-end overnight run (compressed: 5 min instead of 4 hours)
- Multi-agent coordination (Claude + Codex in parallel)
- PR creation pipeline with mock GitHub API
- FM decision quality (validate reasonable task selection)

### Manual Testing
1. **Dry Run**: `--dry-run=true` mode shows what would happen without executing
2. **Compressed Timeline**: Test with 1-min intervals instead of 30-min
3. **Single Task**: Run one orchestration cycle manually
4. **PR Preview**: Generate PR content without pushing

## Future Extensions

### Phase 2: Nostr Marketplace
- Publish upgrade manifests as Nostr events (kind 30051)
- Discover upgrades from relays
- Payment coordination via Spark SDK
- Reputation system (kind 30054 events)

### Phase 3: Multi-Device Coordination
- iOS initiates overnight jobs, macOS executes
- Real-time monitoring from iPhone while laptop works
- Remote cancel/pause controls

### Phase 4: Federated Agent Mesh
- P2P task delegation across user's devices
- Contribute compute to shared pools
- Earn Bitcoin for background availability

## Files in This Directory

- **README.md** (this file): Overview and architecture
- **architecture.md**: Detailed technical design with sequence diagrams
- **testing-plan.md**: Comprehensive testing strategy
- **demo-script.md**: Step-by-step guide for recording demo video
- **issues/**: GitHub issue templates (001-scheduler.md through 012-documentation.md)
- **examples/**: Sample upgrade manifests
  - `nightly-refactor.json`: Code quality improvements
  - `feature-worker.json`: Implement features from backlog
  - `test-generator.json`: Generate tests for uncovered code

## Getting Started

1. **Read**: `architecture.md` for detailed technical design
2. **Review**: GitHub issues in `issues/` directory (ordered by dependency)
3. **Implement**: Start with Issue #001 (SchedulerService)
4. **Test**: Follow `testing-plan.md` for validation
5. **Demo**: Use `demo-script.md` when recording video

## Questions?

See individual issue files for implementation details, acceptance criteria, and testing requirements.
