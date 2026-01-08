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
│  └── /compute, /containers, /claude         │
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
| Claude CLI | File exists at `~/.claude/local/claude` | Subprocess |
| Cerebras | `CEREBRAS_API_KEY` env var | `https://api.cerebras.ai/v1` |
| Ollama | HTTP check :11434 | `http://localhost:11434` |
| Apple FM | macOS + Metal check | In-process |
| llama.cpp | HTTP check :8080 | `http://localhost:8080` |

**Execution Priority:**

When Adjutant executes tasks, it uses backends in this order:
1. **Claude CLI** (Pro/Max subscription) - Best quality, uses existing subscription
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
  [OK] Claude CLI (Pro/Max) - PRIORITY
  [OK] Ollama (localhost:11434) - 3 models
       ├── llama3.2:latest
       ├── qwen2.5:7b
       └── deepseek-coder:6.7b
  [--] Apple FM: Not available
  [--] llama.cpp: Not available

  Execution: Claude Pro/Max (priority)

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
│   ├── lib.rs          # Public exports
│   ├── boot.rs         # Boot sequence orchestration
│   ├── manifest.rs     # Manifest types
│   ├── situation.rs    # Situation assessment
│   ├── display.rs      # Formatted output
│   └── discovery/
│       ├── mod.rs
│       ├── hardware.rs # CPU/RAM/GPU detection
│       ├── compute.rs  # Backend detection
│       ├── network.rs  # Connectivity checks
│       ├── identity.rs # Key/wallet status
│       └── workspace.rs # .openagents/ parsing
└── docs/
    └── README.md       # This file
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

- [../../docs/OANIX.md](../../docs/OANIX.md) - Full OANIX vision document
- [../adjutant/docs/README.md](../adjutant/docs/README.md) - Adjutant (uses OANIX)
- [../pylon/docs/QUICKSTART.md](../pylon/docs/QUICKSTART.md) - Pylon setup
