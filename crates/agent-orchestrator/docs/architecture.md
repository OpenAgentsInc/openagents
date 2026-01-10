# Architecture

The agent-orchestrator crate provides a layered architecture for managing AI agents with different capabilities, routing work to appropriate backends, and enforcing policies through hooks.

## System Layers

```
┌─────────────────────────────────────────────────────────────────┐
│                      Application Layer                          │
│   autopilot │ desktop │ marketplace │ gitafter                  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                   Orchestration Layer                           │
│                                                                 │
│  ┌───────────────┐  ┌───────────────┐  ┌───────────────┐       │
│  │ AgentRegistry │  │  HookManager  │  │ BackgroundTask│       │
│  │               │  │               │  │   Manager     │       │
│  └───────────────┘  └───────────────┘  └───────────────┘       │
│                                                                 │
│  ┌───────────────────────────────────────────────────────────┐ │
│  │                    Integrations                            │ │
│  │  directives │ autopilot │ trajectory │ marketplace │ adv  │ │
│  └───────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                      Backend Layer                              │
│                                                                 │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐          │
│  │  Claude  │ │  OpenAI  │ │  Codex   │ │ GPT-OSS  │          │
│  │   API    │ │   API    │ │   API    │ │  Local   │          │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘          │
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │                   NIP-90 DVMs                             │  │
│  │  Nostr relays → Compute providers → Lightning payments    │  │
│  └──────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

## Core Components

### AgentRegistry

The registry maintains agent configurations and provides lookup:

```rust
pub struct AgentRegistry {
    agents: HashMap<String, AgentConfig>,
    disabled: HashSet<String>,
    overrides: HashMap<String, AgentOverride>,
}
```

Key operations:
- `new()` — Create with builtin agents (Sisyphus, Oracle, etc.)
- `get(name)` — Lookup agent by name
- `primary()` — Get the primary orchestrator agent
- `list_subagents()` — Get all non-primary agents
- `disable(name)` — Disable an agent
- `override_agent(name, config)` — Apply runtime override

### HookManager

Manages lifecycle hooks with priority ordering:

```rust
pub struct HookManager {
    hooks: Vec<(i32, Arc<dyn Hook>)>,  // (priority, hook)
    disabled: HashSet<String>,
}
```

Hook dispatch order:
1. Hooks sorted by priority (higher first)
2. Each hook called in order
3. First `Block` result stops dispatch
4. All `Modify` results applied

### BackgroundTaskManager

Manages parallel agent sessions:

```rust
pub struct BackgroundTaskManager {
    tasks: Arc<RwLock<HashMap<TaskId, BackgroundTask>>>,
    sessions: Arc<RwLock<HashMap<SessionId, Vec<TaskId>>>>,
}
```

Task lifecycle:
1. `spawn()` — Create task, return TaskId
2. Task runs asynchronously
3. `get_output(block=true)` — Wait for completion
4. `cancel()` — Stop a running task

### DSPy Integrations

When DSPy is configured, integrations use classifiers for:
- Directive status/priority parsing and semantic matching
- Issue selection from open queues

Fallbacks remain in place when DSPy is unavailable.

## Data Flow

### Tool Execution

```
User Request
     │
     ▼
┌─────────────┐
│ HookManager │ ──► before_tool() hooks
└─────────────┘
     │
     ▼ (if not blocked)
┌─────────────┐
│   Backend   │ ──► Execute tool
└─────────────┘
     │
     ▼
┌─────────────┐
│ HookManager │ ──► after_tool() hooks
└─────────────┘
     │
     ▼
Response
```

### Context Injection

```
Session Start
     │
     ▼
┌─────────────────┐
│ ContextBuilder  │
└─────────────────┘
     │
     ├── inject_context() hooks
     │   ├── AGENTS.md injection
     │   ├── README.md injection
     │   ├── Directive injection
     │   └── Rules injection
     │
     ▼
┌─────────────────┐
│ Agent Context   │
└─────────────────┘
```

### Cost Tracking

```
Tool Call
     │
     ▼
┌───────────────────┐
│ CostTrackingHook  │ ──► Check budget
└───────────────────┘
     │
     ├── Budget OK → Continue
     │
     └── Budget Exceeded → Block
     
After Execution
     │
     ▼
┌───────────────────┐
│ CostTrackingHook  │ ──► Record cost
└───────────────────┘
     │
     ▼
┌───────────────────┐
│   CostTracker     │ ──► Update totals
└───────────────────┘
```

## Module Structure

```
src/
├── lib.rs              # Public exports
├── config.rs           # AgentConfig, AgentMode, permissions
├── registry.rs         # AgentRegistry
├── background.rs       # BackgroundTaskManager
├── error.rs            # Error types
│
├── agents/
│   ├── mod.rs          # Agent definitions
│   └── *_prompt.md     # Agent system prompts
│
├── hooks/
│   ├── mod.rs          # Hook trait, HookManager
│   ├── session.rs      # Session lifecycle hooks
│   ├── tool.rs         # Tool execution hooks
│   ├── context.rs      # Context injection hooks
│   └── todo.rs         # Todo tracking hooks
│
└── integrations/
    ├── mod.rs          # Integration exports
    ├── directives.rs   # Directive loading
    ├── autopilot.rs    # Issue tracking
    ├── trajectory.rs   # APM metrics
    ├── marketplace.rs  # Skill licensing
    └── advanced.rs     # FROSTR, NIP-SA, multi-backend
```

## Thread Safety

All core types are thread-safe:

| Type | Safety Mechanism |
|------|------------------|
| `AgentRegistry` | Immutable after construction |
| `HookManager` | `Arc<dyn Hook>` with `Send + Sync` |
| `BackgroundTaskManager` | `Arc<RwLock<HashMap>>` |
| `CostTracker` | `RwLock<Vec<CostRecord>>` |
| `SolverAgentCoordinator` | `RwLock<Vec<PendingApproval>>` |

## Extension Points

### Custom Agents

```rust
let mut registry = AgentRegistry::new();
registry.register(AgentConfig {
    name: "custom-agent".to_string(),
    model: "my-model".to_string(),
    prompt: "You are a custom agent...".to_string(),
    // ...
});
```

### Custom Hooks

```rust
struct MyHook;

#[async_trait]
impl Hook for MyHook {
    fn name(&self) -> &str { "my-hook" }
    
    async fn before_tool(&self, call: &mut ToolCall) -> HookResult {
        // Custom logic
        HookResult::Continue
    }
}

hooks.register(MyHook);
```

### Custom Backends

```rust
let router = MultiBackendRouter::new(BackendProvider::Claude)
    .add_backend(BackendConfig {
        provider: BackendProvider::Local,
        model: "custom-model".to_string(),
        endpoint: Some("http://my-server:8080".to_string()),
        cost_per_1k_input: 0,
        cost_per_1k_output: 0,
        enabled: true,
    });
```

## Performance Considerations

### Hook Dispatch
- Hooks are called sequentially (not parallel)
- Keep hook logic fast to avoid blocking
- Use `priority` to order critical hooks first

### Background Tasks
- Tasks run in separate Tokio tasks
- Use `cancel_all()` before session cleanup
- Monitor task count to avoid resource exhaustion

### Cost Tracking
- Cost records accumulate in memory
- Implement periodic cleanup for long-running sessions
- Use session-scoped tracking for bounded memory
