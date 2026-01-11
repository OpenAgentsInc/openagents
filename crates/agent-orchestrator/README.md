# agent-orchestrator

Native Rust agent orchestration framework for multi-model agent management, lifecycle hooks, background task orchestration, and deep integration with OpenAgents infrastructure.

## Overview

The agent-orchestrator crate provides the control plane for **sovereign AI agents** — agents that own their identity, manage their own budgets, route work across multiple backends, and operate with graduated autonomy.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                        agent-orchestrator                                     │
├──────────────────────────────────────────────────────────────────────────────┤
│                                                                               │
│  ┌─────────────────────────────────────────────────────────────────────────┐ │
│  │                           AgentRegistry                                  │ │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐      │ │
│  │  │ Sisyphus │ │  Oracle  │ │ Librarian│ │ Explore  │ │ Frontend │ ...  │ │
│  │  │(primary) │ │ (GPT-5.2)│ │ (Sonnet) │ │  (Grok)  │ │ (Gemini) │      │ │
│  │  └──────────┘ └──────────┘ └──────────┘ └──────────┘ └──────────┘      │ │
│  └─────────────────────────────────────────────────────────────────────────┘ │
│                                                                               │
│  ┌─────────────────────────────────────────────────────────────────────────┐ │
│  │                          HookManager                                     │ │
│  │  session.*  │  tool.*  │  context.*  │  todo.*  │  cost.*               │ │
│  └─────────────────────────────────────────────────────────────────────────┘ │
│                                                                               │
│  ┌─────────────────────────────────────────────────────────────────────────┐ │
│  │                     BackgroundTaskManager                                │ │
│  │  spawn() │ get_output() │ cancel() │ cancel_all() │ list()              │ │
│  └─────────────────────────────────────────────────────────────────────────┘ │
│                                                                               │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐        │
│  │  Directives  │ │   Autopilot  │ │  Marketplace │ │   Advanced   │        │
│  │  Integration │ │  Integration │ │  Integration │ │   Features   │        │
│  └──────────────┘ └──────────────┘ └──────────────┘ └──────────────┘        │
│                                                                               │
└──────────────────────────────────────────────────────────────────────────────┘
```

## Features

### Core Framework
- **AgentConfig** — Configuration for model, prompt, temperature, permissions
- **AgentRegistry** — Register, lookup, enable/disable agents
- **HookManager** — Lifecycle hooks with priority ordering

### 7 Specialized Agents
- **Sisyphus** — Primary orchestrator, delegates to specialists
- **Oracle** — Architecture decisions, debugging after failures
- **Librarian** — External docs, GitHub search, OSS reference
- **Explore** — Fast codebase exploration, pattern search
- **Frontend** — UI/UX development, visual changes
- **DocWriter** — Technical documentation
- **Multimodal** — PDF/image analysis

### Background Tasks
- Spawn parallel agent sessions
- Track completion with notifications
- Cancel individual or all tasks
- Session-scoped task management

### Lifecycle Hooks
- **Session hooks** — created, idle, error, recovery
- **Tool hooks** — before/after execution, blocking, truncation
- **Context hooks** — inject AGENTS.md, README, directives
- **Todo hooks** — enforce task completion, context monitoring

### OpenAgents Integrations
- **Directives** — Load active directives into agent context
- **Autopilot** — Issue claim/complete workflow hooks
- **Trajectory** — APM metrics and action logging
- **Marketplace** — Skill licensing and usage tracking

### Advanced Features (Phase 6)
- **FROSTR** — Threshold-protected agent identity
- **NIP-SA** — Sovereign agent protocol with autonomy levels
- **Multi-backend** — Route to Codex, OpenAI, Codex, GPT-OSS, local
- **Cost tracking** — Budget enforcement with daily/session limits

### DSPy Signatures (Wave 9)
- **DelegationSignature** — Sisyphus decides which subagent handles task
- **ArchitectureSignature** — CoT for Oracle architecture decisions
- **LibraryLookupSignature** — Librarian external docs lookup
- **CodeExplorationSignature** — Explore codebase navigation
- **UIDesignSignature** — Frontend UI/UX design
- **DocumentationSignature** — DocWriter technical docs
- **MediaAnalysisSignature** — Multimodal PDF/image analysis

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
agent-orchestrator = { path = "../agent-orchestrator" }
```

## Quick Start

```rust
use agent_orchestrator::{
    AgentRegistry, HookManager, BackgroundTaskManager,
    hooks::{ContextInjectionHook, SessionRecoveryHook, TodoHook},
};

// Create registry with builtin agents
let registry = AgentRegistry::new();

// Get the primary agent (Sisyphus)
let sisyphus = registry.primary().expect("no primary agent");
println!("Primary agent: {} ({})", sisyphus.name, sisyphus.model);

// Set up hooks
let mut hooks = HookManager::new();
hooks.register(ContextInjectionHook::new("/path/to/workspace"));
hooks.register(SessionRecoveryHook::new(3)); // max 3 retries
hooks.register(TodoHook::new(true)); // enforce completion

// Create background task manager
let bg = BackgroundTaskManager::new();

// Spawn a background task
let task_id = bg.spawn(
    "session-123",
    "explore",
    "Find all authentication code",
    "Auth code search"
).await?;

// Get result (blocking)
let result = bg.get_output(&task_id, true).await?;
```

## Agent Configuration

Each agent is configured with:

```rust
pub struct AgentConfig {
    pub name: String,           // Agent identifier
    pub model: String,          // e.g., "codex-sonnet-4-5"
    pub prompt: String,         // System prompt
    pub temperature: f32,       // 0.0 - 2.0
    pub description: String,    // Short description
    pub mode: AgentMode,        // Primary, Subagent, or All
    pub tools: HashMap<String, bool>,  // Tool permissions
    pub permission: AgentPermission,   // Edit, bash, webfetch
}
```

## Hook System

Hooks intercept agent lifecycle events:

```rust
#[async_trait]
pub trait Hook: Send + Sync {
    fn name(&self) -> &str;
    fn priority(&self) -> i32 { 0 }

    async fn on_session(&self, event: &SessionEvent) -> HookResult {
        HookResult::Continue
    }

    async fn before_tool(&self, call: &mut ToolCall) -> HookResult {
        HookResult::Continue
    }

    async fn after_tool(&self, call: &ToolCall, output: &mut ToolOutput) -> HookResult {
        HookResult::Continue
    }

    async fn inject_context(&self, ctx: &mut ContextBuilder) -> HookResult {
        HookResult::Continue
    }
}
```

Hooks can:
- **Continue** — Let execution proceed
- **Block** — Stop execution with a message
- **Modify** — Change the call or output

## Cost Tracking

Track and enforce budgets across backends:

```rust
use agent_orchestrator::{
    CostTracker, BudgetConfig, MultiBackendRouter,
    BackendProvider, BackendConfig, CostTrackingHook,
};

// Configure backends
let router = MultiBackendRouter::new(BackendProvider::Codex)
    .add_backend(BackendConfig::codex("sonnet-4"))
    .add_backend(BackendConfig::openai("gpt-4"))
    .add_backend(BackendConfig::local("llama3", "http://localhost:11434"))
    .route_agent("oracle", BackendProvider::OpenAI);

// Configure budgets
let tracker = CostTracker::new()
    .with_default_budget(BudgetConfig {
        daily_limit_sats: 100_000,
        session_limit_sats: 10_000,
        warn_threshold_pct: 80,
    });

// Create cost tracking hook
let hook = CostTrackingHook::new(
    Arc::new(tracker),
    Arc::new(router),
);
```

## Autonomy Levels

Agents operate at different autonomy levels:

| Level | Behavior |
|-------|----------|
| `Supervised` | All actions require approval |
| `SemiAutonomous` | Low-cost actions auto-approved, high-cost require approval |
| `FullyAutonomous` | No approval required |

```rust
use agent_orchestrator::{AgentIdentity, AutonomyLevel, SolverAgentCoordinator};

let identity = AgentIdentity::new("pubkey", "MyAgent", "codex-sonnet-4")
    .with_autonomy(AutonomyLevel::SemiAutonomous)
    .with_operator("operator-pubkey");

let coordinator = SolverAgentCoordinator::new(identity);

// Check if action needs approval
if coordinator.requires_approval("purchase_skill", 5000) {
    let request_id = coordinator.request_approval(
        "purchase_skill",
        "Buy web-scraper skill for $5",
        5000
    );
    // Wait for operator approval...
}
```

## Documentation

- [Architecture](docs/architecture.md) — System design and component interaction
- [Agents](docs/agents.md) — Agent definitions and configuration
- [Hooks](docs/hooks.md) — Hook system and builtin hooks
- [Integrations](docs/integrations.md) — OpenAgents-specific integrations
- [Advanced](docs/advanced.md) — FROSTR, NIP-SA, multi-backend, cost tracking

## Testing

```bash
# Run all tests
cargo test -p agent-orchestrator

# Run with output
cargo test -p agent-orchestrator -- --nocapture

# Run specific test
cargo test -p agent-orchestrator test_cost_tracker
```

## Related Crates

| Crate | Purpose |
|-------|---------|
| `opencode-sdk` | Primary backend communication |
| `codex-agent-sdk` | Direct Codex access |
| `codex-agent-sdk` | Codex backend |
| `gpt-oss` | Local inference backend |
| `frostr` | Threshold signatures |
| `marketplace` | Skill licensing |
| `issues` | Issue tracking |
| `recorder` | Trajectory logging |

## License

MIT
