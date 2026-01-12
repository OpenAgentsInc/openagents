# OANIX Crate

OpenAgents NIX - Environment discovery and boot sequence for the agent runtime.

## Overview

OANIX is the OS layer that wraps the OpenAgents runtime. It discovers the local environment at boot time: hardware, compute backends, network connectivity, identity, and workspace context.

```
┌─────────────────────────────────────────────┐
│  OANIX = Operating System                   │
│  "What am I? What should I do?"             │
│  ├── Boot sequence                          │
│  ├── Hardware discovery                     │
│  ├── Situation assessment                   │
│  └── Autonomous decision loop               │
├─────────────────────────────────────────────┤
│  Runtime = Execution Engine                 │
│  "How do agents run?"                       │
│  ├── Tick model                             │
│  ├── Filesystem abstraction                 │
│  └── /compute, /containers, /codex         │
└─────────────────────────────────────────────┘
```

## Quick Start

```rust
use oanix::boot;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let manifest = boot().await?;

    println!("Hardware: {} CPU cores, {} GB RAM",
        manifest.hardware.cpu_cores,
        manifest.hardware.ram_bytes / 1_000_000_000);

    println!("Compute: {} backends, {} models",
        manifest.compute.backends.len(),
        manifest.compute.total_models);

    if manifest.identity.initialized {
        println!("Identity: {}", manifest.identity.npub.unwrap_or_default());
    }

    Ok(())
}
```

## Boot Sequence

The `boot()` function runs five discovery phases:

### Phase 1: Hardware Discovery

Detects CPU, RAM, and GPU resources.

```rust
pub struct HardwareManifest {
    pub cpu_cores: u32,       // Logical CPU cores
    pub cpu_model: String,    // e.g., "Apple M3 Pro"
    pub ram_bytes: u64,       // Total RAM
    pub ram_available: u64,   // Available RAM
    pub gpus: Vec<GpuDevice>, // Detected GPUs
}

pub struct GpuDevice {
    pub name: String,     // e.g., "Apple M3 Pro GPU"
    pub backend: String,  // "Metal", "CUDA", "Vulkan"
    pub available: bool,  // Ready for compute
}
```

### Phase 2: Compute Discovery

Detects available inference backends.

```rust
pub struct ComputeManifest {
    pub backends: Vec<InferenceBackend>,
    pub total_models: usize,
}

pub struct InferenceBackend {
    pub id: String,               // "ollama", "applefm", "llamacpp"
    pub name: String,             // Human-readable name
    pub endpoint: Option<String>, // API endpoint
    pub models: Vec<String>,      // Available models
    pub ready: bool,              // Backend is operational
}
```

**Detected Backends:**

| Backend | Detection Method | Endpoint |
|---------|------------------|----------|
| Codex CLI | File exists at `~/.codex/local/codex` | Subprocess |
| Cerebras | `CEREBRAS_API_KEY` env var | `https://api.cerebras.ai/v1` |
| Ollama | HTTP check :11434 | `http://localhost:11434` |
| Apple FM | macOS + Metal check | In-process |
| llama.cpp | HTTP check :8080 | `http://localhost:8080` |

**Execution Priority:**

When Adjutant executes tasks, it uses backends in this order:
1. **Codex CLI** (Pro/Max subscription) - Best quality, uses existing subscription
2. **Cerebras** (tiered inference) - Cost-effective GLM 4.7 + Qwen-3-32B
3. **Analysis-only** - Returns file analysis without AI execution

### Phase 3: Network Discovery

Checks internet connectivity, Nostr relays, and swarm presence.

```rust
pub struct NetworkManifest {
    pub has_internet: bool,
    pub relays: Vec<RelayStatus>,
    pub total_providers: u32,   // NIP-89 DVM providers
    pub pylon_count: u32,       // OpenAgents Pylon nodes
    pub pylons_online: u32,     // Currently active Pylons
    pub pylon_pubkeys: Vec<String>,
}

pub struct RelayStatus {
    pub url: String,
    pub connected: bool,
    pub latency_ms: Option<u32>,
}
```

### Phase 4: Identity Discovery

Checks for initialized Pylon identity and wallet.

```rust
pub struct IdentityManifest {
    pub initialized: bool,
    pub npub: Option<String>,
    pub wallet_balance_sats: Option<u64>,
    pub network: Option<String>,  // mainnet, regtest, signet
}
```

### Phase 5: Workspace Discovery

Reads `.openagents/` folder for project context.

```rust
pub struct WorkspaceManifest {
    pub root: PathBuf,
    pub project_name: Option<String>,
    pub has_openagents: bool,
    pub directives: Vec<DirectiveSummary>,
    pub issues: Vec<IssueSummary>,
    pub open_issues: u32,
    pub pending_issues: u32,
    pub active_directive: Option<String>,
}
```

## Situation Assessment

After discovery, OANIX assesses the situation:

```rust
pub struct SituationAssessment {
    pub environment: Environment,
    pub compute_power: ComputePower,
    pub connectivity: Connectivity,
    pub recommended_action: RecommendedAction,
}
```

## DSPy Signatures (Wave 8)

OANIX includes learnable DSPy signatures that can replace rule-based decision making:

### SituationAssessmentSignature

Analyzes system state and determines what the agent should prioritize:

```rust
use oanix::{SituationAssessmentSignature, PriorityAction, Urgency};

let sig = SituationAssessmentSignature::new();

// Inputs:
// - system_state: Current hardware/compute state as JSON
// - pending_events: Events in queue
// - recent_history: Recent decisions and outcomes

// Outputs:
// - priority_action: AWAIT_USER, WORK_ISSUE, ACCEPT_JOB, etc.
// - urgency: IMMEDIATE, NORMAL, DEFERRED
// - reasoning: Why this action
// - confidence: 0.0-1.0
```

**Priority Actions:**

| Action | Description |
|--------|-------------|
| `AWAIT_USER` | Wait for user direction (default) |
| `WORK_ISSUE` | Work on a repository issue |
| `ACCEPT_JOB` | Accept a NIP-90 swarm job |
| `START_PROVIDER` | Enter provider mode to earn sats |
| `INITIALIZE_IDENTITY` | Set up Nostr identity first |
| `CONNECT_NETWORK` | Establish network connectivity |
| `HOUSEKEEPING` | Cleanup, sync, refresh |
| `IDLE` | Low-priority background mode |

### IssueSelectionSignature

Chooses the best issue to work on from available options:

```rust
use oanix::{IssueSelectionSignature, Complexity};

let sig = IssueSelectionSignature::new();

// Inputs:
// - available_issues: JSON array of issues with metadata
// - agent_capabilities: Available backends, tools, compute
// - current_context: Branch, recent commits, files changed

// Outputs:
// - selected_issue: Issue number
// - rationale: Why this issue
// - estimated_complexity: LOW, MEDIUM, HIGH
// - confidence: 0.0-1.0
```

### WorkPrioritizationSignature

Orders tasks by importance and dependencies:

```rust
use oanix::WorkPrioritizationSignature;

let sig = WorkPrioritizationSignature::new();

// Inputs:
// - task_list: JSON array of tasks
// - dependencies: Task dependency graph
// - deadlines: Time constraints

// Outputs:
// - ordered_tasks: Tasks in priority order
// - blocking_tasks: Tasks blocking others
// - parallel_groups: Tasks that can run together
```

### LifecycleDecisionSignature (CoT)

Determines agent state transitions with chain-of-thought reasoning:

```rust
use oanix::{LifecycleDecisionSignature, LifecycleState};

let sig = LifecycleDecisionSignature::new();

// Inputs:
// - current_state: IDLE, WORKING, BLOCKED, PROVIDER, TERMINATING
// - recent_events: Task completion, errors, user input
// - resource_status: Memory, CPU, network, wallet

// Outputs (with reasoning):
// - reasoning: Chain-of-thought about transition
// - next_state: Target state
// - transition_reason: Summary
// - cleanup_needed: Actions before transition
```

**Lifecycle States:**

```
IDLE ──────▶ WORKING (start task)
  │              │
  │              ▼
  │          BLOCKED (waiting for input)
  │              │
  ▼              ▼
PROVIDER ◀──────┘ (input received)
  │
  ▼
TERMINATING (shutdown)
```

### Using Signatures with dsrs

These signatures implement `MetaSignature` and can be used with dsrs predictors:

```rust
use dsrs::predictors::Predict;
use dsrs::data::example::Example;
use oanix::IssueSelectionSignature;

let sig = IssueSelectionSignature::new();
let predictor = Predict::new(sig);

let example = Example::from([
    ("available_issues", serde_json::to_string(&issues)?),
    ("agent_capabilities", format_capabilities(&manifest)),
    ("current_context", format_context(&workspace)),
]);

let result = predictor.forward(&example, &lm).await?;
let selected = result.get("selected_issue", None);
```

### Environment Types

```rust
pub enum Environment {
    Developer { os: String, editor: Option<String> },
    Server { provider: Option<String> },
    Container { runtime: String },
    Unknown,
}
```

### Compute Power Levels

```rust
pub enum ComputePower {
    High,    // Can run large local models (70B+)
    Medium,  // Can run medium models (7B-32B)
    Low,     // Limited to small models or API
    None,    // Swarm only
}
```

### Connectivity Levels

```rust
pub enum Connectivity {
    Full,     // Internet + Nostr + Swarm
    Limited,  // Some services unavailable
    Offline,  // Local only
}
```

### Recommended Actions

```rust
pub enum RecommendedAction {
    AwaitUser,           // Ready for user input
    ResumeSession,       // Continue previous work
    InitializeIdentity,  // Need to run `pylon init`
    ConnectNetwork,      // No network connectivity
    StartProvider,       // Can offer compute
}
```

## Display Output

OANIX provides formatted output for the boot sequence:

```rust
use oanix::{boot, display};

let manifest = boot().await?;
display::print_manifest(&manifest);
```

Output:
```
OANIX Boot
══════════════════════════════════════════════════════

Hardware
  CPU: Apple M3 Pro (12 cores)
  RAM: 18 GB (12 GB available)
  GPU: Metal (Apple M3 Pro GPU)

Compute Backends
  [OK] Codex CLI (Pro/Max) - PRIORITY
  [OK] Ollama (localhost:11434) - 3 models
       ├── llama3.2:latest
       ├── qwen2.5:7b
       └── deepseek-coder:6.7b
  [--] Apple FM: Not available
  [--] llama.cpp: Not available

  Execution: Codex Pro/Max (priority)

Network
  [OK] Internet connectivity
  Relays: 2 connected
       ├── wss://nexus.openagents.com (45ms)
       └── wss://relay.damus.io (62ms)
  Swarm: 8 Pylons discovered (5 online)

Identity
  [OK] Initialized (npub1abc...xyz)
  Wallet: 42,350 sats (regtest)

Workspace
  Project: openagents
  Open issues: 3, Pending: 1
  Active: d-027 (Tiered inference)
```

## Integration with Adjutant

Adjutant uses OANIX at startup:

```rust
use oanix::boot;
use adjutant::Adjutant;

let manifest = boot().await?;
let adjutant = Adjutant::new(manifest)?;
let result = adjutant.execute(&task).await?;
```

## Module Structure

```
crates/oanix/
├── src/
│   ├── lib.rs              # Public exports
│   ├── boot.rs             # Boot sequence orchestration
│   ├── manifest.rs         # Manifest types
│   ├── situation.rs        # Rule-based situation assessment
│   ├── display.rs          # Formatted output
│   ├── state.rs            # OanixState, OanixMode
│   ├── tick.rs             # Autonomous tick loop
│   ├── dspy_situation.rs   # SituationAssessmentSignature (Wave 8)
│   ├── dspy_lifecycle.rs   # IssueSelection, WorkPrioritization, Lifecycle (Wave 8)
│   ├── bin/
│   │   └── main.rs         # CLI binary
│   └── discovery/
│       ├── mod.rs
│       ├── hardware.rs     # CPU/RAM/GPU detection
│       ├── compute.rs      # Backend detection
│       ├── network.rs      # Connectivity checks
│       ├── identity.rs     # Key/wallet status
│       └── workspace.rs    # .openagents/ parsing
└── docs/
    └── README.md           # This file
```

## CLI Usage

The `oanix` binary can be run standalone for diagnostics:

```bash
# Run boot sequence
cargo run -p oanix

# Or if installed
oanix
```

## See Also

- [../../SYNTHESIS.md](../../SYNTHESIS.md) - Current vision and product context
- [../../dsrs/docs/README.md](../../dsrs/docs/README.md) - DSPy Rust (signatures, predictors)
- [../../adjutant/docs/README.md](../../adjutant/docs/README.md) - Adjutant (uses OANIX)
- [../../pylon/docs/QUICKSTART.md](../../pylon/docs/QUICKSTART.md) - Pylon setup
