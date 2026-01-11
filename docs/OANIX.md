# OANIX: OpenAgents NIX

**The agent that replaces your computer.**

OANIX is a complete agent runtime environment where autonomous AI takes full control of compute resources. When `openagents autopilot` boots, it doesn't just run tasks—it *becomes* the operating system.

---

## Vision

```
┌─────────────────────────────────────────────────────────────────────┐
│                         HUMAN                                       │
│                           │                                         │
│                           ▼                                         │
│                     ┌─────────┐                                     │
│                     │  OANIX  │ ← One agent, full control           │
│                     └────┬────┘                                     │
│                          │                                          │
│         ┌────────────────┼────────────────┐                        │
│         ▼                ▼                ▼                        │
│    ┌─────────┐     ┌──────────┐    ┌───────────┐                   │
│    │ Hardware│     │  Swarm   │    │  Network  │                   │
│    │ CPU/GPU │     │ Compute  │    │  Gateway  │                   │
│    │ RAM/Disk│     │ (Nostr)  │    │ (Nexus)   │                   │
│    └─────────┘     └──────────┘    └───────────┘                   │
└─────────────────────────────────────────────────────────────────────┘
```

Traditional OS: Hardware → Kernel → User programs → Human operates
**OANIX**: Hardware → OANIX → Human directs, agent operates

---

## Boot Sequence

When OANIX boots, it performs **environment discovery**:

### Phase 1: Hardware Discovery
```rust
/// Discover local compute resources
struct HardwareManifest {
    /// Total RAM in bytes
    ram_bytes: u64,
    /// Available RAM
    ram_available: u64,
    /// CPU cores (logical)
    cpu_cores: u32,
    /// CPU model/architecture
    cpu_model: String,
    /// GPU devices
    gpus: Vec<GpuDevice>,
    /// Disk space
    disk_bytes: u64,
    /// Network interfaces
    network_interfaces: Vec<NetworkInterface>,
}

struct GpuDevice {
    /// Device name
    name: String,
    /// VRAM in bytes
    vram_bytes: u64,
    /// Backend (Metal, CUDA, Vulkan)
    backend: GpuBackend,
    /// Available for inference
    available: bool,
}
```

### Phase 2: Compute Discovery
```rust
/// Discover inference backends
struct ComputeManifest {
    /// Local inference backends
    local_backends: Vec<InferenceBackend>,
    /// Apple Foundation Models (M-series)
    apple_fm: Option<AppleFmStatus>,
    /// Ollama models
    ollama_models: Vec<OllamaModel>,
    /// llama.cpp server
    llamacpp: Option<LlamaCppStatus>,
}

enum InferenceBackend {
    AppleFoundationModels { models: Vec<String> },
    Ollama { endpoint: String, models: Vec<String> },
    LlamaCpp { endpoint: String, model: String },
    Custom { name: String, endpoint: String },
}
```

### Phase 3: Network Discovery
```rust
/// Discover network presence and connectivity
struct NetworkManifest {
    /// Internet connectivity
    has_internet: bool,
    /// Nostr relays reachable
    nostr_relays: Vec<RelayStatus>,
    /// Nexus gateway
    nexus: Option<NexusStatus>,
    /// Swarm peers visible
    swarm_peers: u32,
    /// Public IP (if discoverable)
    public_ip: Option<String>,
    /// NAT type
    nat_type: NatType,
}

struct RelayStatus {
    url: String,
    connected: bool,
    latency_ms: u32,
    supports_nip42: bool,  // Auth
    supports_nip90: bool,  // DVM
}

struct NexusStatus {
    url: String,
    authenticated: bool,
    pubkey: String,
}
```

### Phase 4: Identity Resolution
```rust
/// Discover and resolve identity
struct IdentityManifest {
    /// Unified identity (if initialized)
    unified_identity: Option<UnifiedIdentity>,
    /// Nostr pubkey
    nostr_pubkey: Option<String>,
    /// Bitcoin address (for payments)
    bitcoin_address: Option<String>,
    /// Spark wallet status
    wallet: Option<WalletStatus>,
    /// Known contacts/peers
    known_peers: Vec<PeerIdentity>,
}

struct WalletStatus {
    /// Balance in satoshis
    balance_sats: u64,
    /// Network (mainnet, regtest, signet)
    network: BitcoinNetwork,
    /// Can send payments
    can_send: bool,
    /// Can receive payments
    can_receive: bool,
}
```

### Phase 5: Situation Assessment
```rust
/// What does OANIX know about its situation?
struct SituationAssessment {
    /// Where am I running?
    environment: Environment,
    /// What compute do I have?
    compute_power: ComputePower,
    /// What can I reach?
    connectivity: Connectivity,
    /// Who am I?
    identity: IdentityState,
    /// What does the user want?
    user_intent: Option<UserIntent>,
    /// What should I do first?
    recommended_action: RecommendedAction,
}

enum Environment {
    /// Developer machine (lots of resources, interactive)
    Developer { os: String, editor: Option<String> },
    /// Server (headless, persistent)
    Server { provider: Option<String> },
    /// Container (isolated, ephemeral)
    Container { runtime: String },
    /// Browser (limited, sandboxed)
    Browser { engine: String },
    /// Embedded (constrained)
    Embedded { device: String },
}

enum ComputePower {
    /// Can run large models locally
    High { can_run_opus: bool, can_run_70b: bool },
    /// Can run medium models
    Medium { can_run_sonnet: bool, can_run_7b: bool },
    /// Limited to small models or API only
    Low { can_run_haiku: bool, api_only: bool },
    /// No local inference, swarm only
    SwarmOnly,
}

enum Connectivity {
    /// Full network access
    Full { internet: bool, nostr: bool, swarm: bool },
    /// Limited to specific endpoints
    Limited { endpoints: Vec<String> },
    /// Air-gapped (local only)
    Offline,
}

enum RecommendedAction {
    /// Greet user and await instructions
    AwaitUser,
    /// Resume previous session
    ResumeSession { session_id: String },
    /// Start providing compute (if running as provider)
    StartProvider,
    /// Join swarm and discover work
    JoinSwarm,
    /// Initialize identity first
    InitializeIdentity,
    /// Connect to network first
    ConnectNetwork,
}
```

---

## The OANIX Filesystem

OANIX exposes everything as a Plan 9-style filesystem:

```
/
├── hw/                    # Hardware discovery
│   ├── cpu                # CPU info (read)
│   ├── gpu/               # GPU devices
│   │   └── 0/             # First GPU
│   │       ├── info       # Device info
│   │       ├── vram       # VRAM usage
│   │       └── available  # Availability flag
│   ├── ram                # Memory info
│   ├── disk               # Disk info
│   └── net/               # Network interfaces
│
├── compute/               # Inference capability
│   ├── providers/         # Available providers
│   │   ├── local/         # Local backends
│   │   │   ├── ollama/
│   │   │   ├── applefm/
│   │   │   └── llamacpp/
│   │   ├── swarm/         # DVM providers
│   │   └── cloud/         # Cloud APIs
│   ├── models             # Available models (all providers)
│   ├── new                # Submit inference job → job_id
│   └── jobs/              # Active/completed jobs
│
├── swarm/                 # Nostr/DVM network
│   ├── relays/            # Relay connections
│   │   └── <relay_id>/
│   │       ├── status
│   │       ├── latency
│   │       └── publish    # Publish event
│   ├── peers/             # Known peers
│   ├── jobs/              # DVM job market
│   │   ├── available      # Jobs seeking providers
│   │   └── mine           # Jobs I'm working on
│   └── nexus              # Nexus gateway status
│
├── identity/              # Who am I
│   ├── pubkey             # Nostr pubkey
│   ├── npub               # Human-readable
│   ├── sign               # Sign data → signature
│   └── verify             # Verify signature
│
├── wallet/                # Bitcoin/Lightning
│   ├── balance            # Current balance (sats)
│   ├── address            # Receive address
│   ├── send               # Send payment
│   ├── receive            # Create invoice
│   └── history            # Transaction history
│
├── agents/                # Running agents (including self)
│   └── <agent_id>/
│       ├── status
│       ├── inbox/
│       ├── outbox/
│       ├── goals/
│       ├── memory/
│       └── fs/            # Agent's namespace view
│
├── codex/                # Codex Agent SDK
│   ├── providers/
│   ├── new                # Create session → session_id
│   ├── sessions/
│   └── policy
│
├── containers/            # Code execution sandboxes
│   ├── providers/
│   ├── new                # Create container → container_id
│   └── containers/
│
├── workspace/             # Current working context
│   ├── repo/              # Git repository (if any)
│   ├── tasks/             # Active tasks
│   └── artifacts/         # Build outputs, logs
│
└── ctl                    # OANIX control
    ├── status             # Current status
    ├── shutdown           # Graceful shutdown
    ├── restart            # Restart OANIX
    └── config             # Runtime configuration
```

---

## Autonomous Decision Loop

Once booted, OANIX runs an autonomous decision loop:

```rust
/// The core OANIX tick
async fn oanix_tick(state: &mut OanixState) -> TickResult {
    // 1. Refresh environment understanding
    let env = discover_environment().await;

    // 2. Check for user input/direction
    if let Some(intent) = check_user_intent(&state.inbox).await {
        return handle_user_intent(state, intent).await;
    }

    // 3. Check for incoming work (if provider mode)
    if state.mode == Mode::Provider {
        if let Some(job) = check_job_market(&env.swarm).await {
            return handle_incoming_job(state, job).await;
        }
    }

    // 4. Continue existing work
    if let Some(task) = state.current_task.as_ref() {
        return continue_task(state, task).await;
    }

    // 5. Proactive behavior
    match assess_situation(&env, state).await {
        Situation::Idle => {
            // Could: learn, organize, optimize
            maybe_do_housekeeping(state).await
        }
        Situation::LowBalance => {
            // Could: seek work to earn sats
            consider_earning_work(state).await
        }
        Situation::Disconnected => {
            // Should: attempt reconnection
            attempt_network_recovery(state).await
        }
        Situation::HighDemand => {
            // Should: help with swarm work
            join_swarm_effort(state).await
        }
        _ => TickResult::hibernate(Duration::from_secs(60))
    }
}
```

---

## Boot Messages (Example)

When OANIX starts:

```
OANIX v0.1.0 - OpenAgents NIX
═══════════════════════════════════════════════════════════════

Discovering environment...

Hardware
  CPU: Apple M3 Pro (12 cores)
  RAM: 18 GB (12 GB available)
  GPU: Metal (Apple M3 Pro GPU, 18 GB unified)
  Disk: 512 GB (180 GB available)

Compute Backends
  [OK] Apple Foundation Models (AFM on-device)
  [OK] Ollama (localhost:11434) - 3 models
       ├── llama3.2:latest (3B)
       ├── qwen2.5:7b
       └── deepseek-coder:6.7b
  [--] llama.cpp server not detected

Network
  [OK] Internet connectivity
  [OK] Nexus (wss://nexus.openagents.com) - authenticated
  [OK] Nostr relays: 3 connected
       ├── wss://relay.damus.io (42ms)
       ├── wss://nos.lol (68ms)
       └── wss://relay.nostr.band (85ms)
  Swarm: 12 peers visible

Identity
  Pubkey: npub1abc...xyz
  Wallet: 42,350 sats (regtest)

Situation Assessment
  Environment: Developer (macOS, Cursor)
  Compute: High (can run local models)
  Connectivity: Full (internet + nostr + swarm)

Recommended: Awaiting user direction

Ready. What would you like to do?
█
```

---

## Provider Mode

When running as a compute provider:

```
OANIX v0.1.0 - Provider Mode
═══════════════════════════════════════════════════════════════

Discovering environment...

Hardware
  CPU: AMD EPYC 7543 (64 cores)
  RAM: 256 GB (240 GB available)
  GPU: 2x NVIDIA A100 (80 GB each)

Compute Backends
  [OK] vLLM (localhost:8000) - Mixtral 8x22B
  [OK] TGI (localhost:8001) - Llama 3.1 405B

Network
  [OK] Public IP: 203.0.113.42
  [OK] Nexus authenticated
  [OK] 5 Nostr relays connected
  Swarm: 847 peers visible

Identity
  Pubkey: npub1provider...
  Wallet: 1,247,500 sats (mainnet)

Starting provider...
  Registered on: wss://nexus.openagents.com
  Accepting: kind:5050 (text generation)
  Accepting: kind:5940 (RLM sub-queries)

Provider active. Watching for jobs...

[12:34:56] Job 7a8b... from npub1buyer... - 5050 text generation
           Bid: 1,250 msats | Status: Processing...
[12:34:58] Job 7a8b... complete (2.1s, 342 tokens) → Paid 1,250 msats
[12:35:02] Job 9c0d... from npub1other... - 5940 RLM sub-query
           ...
```

---

## Integration with Pylon

OANIX can operate in two relationships with Pylon:

### 1. OANIX *is* the agent inside Pylon
```
Pylon (node software)
  └── OANIX (agent runtime)
       └── Codex sessions, compute jobs, etc.
```

### 2. OANIX uses Pylon as a service
```
OANIX (standalone)
  ├── /compute/providers/local/...   # Direct backends
  └── /compute/providers/swarm/...   # Via Pylon/Nostr
```

The `pylon` binary can invoke OANIX:
```bash
# Run as provider (Pylon hosts OANIX)
pylon start -m provider

# Run autopilot (OANIX as development agent)
openagents autopilot

# OANIX standalone with config
oanix --config ~/.oanix/config.toml
```

---

## Future: OANIX as Primary OS

The ultimate vision: OANIX runs directly on hardware.

```
Traditional Boot:
  BIOS → Bootloader → Linux → Systemd → Applications → Human operates

OANIX Boot:
  BIOS → Bootloader → OANIX Kernel → Agent Runtime → Agent operates
                                                      ↑
                                                   Human directs
```

### Minimal OANIX Kernel Requirements
- Hardware abstraction (CPU, RAM, GPU, NIC)
- Filesystem (persistent storage)
- Network stack (TCP/IP, WebSocket)
- Process isolation (for containers)
- Graphics (optional, for UI)

### Progressive Path
1. **Now**: OANIX runs as userspace program on Linux/macOS/Windows
2. **Next**: OANIX runs as init system (replaces systemd)
3. **Later**: OANIX as unikernel (minimal Linux kernel + OANIX)
4. **Future**: OANIX native kernel (Rust, from scratch)

---

## Implementation Phases

### Phase 1: Environment Discovery (Complete)
- [x] Hardware manifest struct and discovery
- [x] Compute backend detection (Ollama, llama.cpp, FM Bridge)
- [x] Network/relay status checking
- [x] Identity and wallet integration
- [x] Situation assessment logic
- [x] OanixManifest generation

### Phase 2: DSPy Signatures (Complete - Wave 8)
- [x] `SituationAssessmentSignature` - Analyze system state, determine priorities
- [x] `IssueSelectionSignature` - Choose best issue to work on
- [x] `WorkPrioritizationSignature` - Order tasks by importance/dependencies
- [x] `LifecycleDecisionSignature` - CoT for agent state transitions
- [x] All signatures implement MetaSignature trait
- [x] 18 tests passing

### Phase 3: Unified Boot (In Progress)
- [x] Single boot sequence for autopilot
- [x] Progressive log output with sections
- [ ] Boot configuration (what to discover)
- [ ] Skip/retry for missing components

### Phase 4: Autonomous Loop (In Progress)
- [x] Tick-based decision loop (via Runtime)
- [ ] Idle/housekeeping behavior
- [ ] Proactive work discovery
- [ ] Balance monitoring and earning mode

### Phase 5: Full Filesystem
- [ ] `/hw` mount for hardware
- [ ] `/swarm` mount for Nostr/DVM
- [ ] Integration with existing mounts
- [ ] Namespace builder for OANIX

### Phase 6: Provider Mode
- [x] Automatic provider registration (via Pylon)
- [x] Job processing loop (via Pylon)
- [ ] Earnings tracking
- [ ] Resource allocation

---

## References

- [SYNTHESIS_EXECUTION.md](SYNTHESIS_EXECUTION.md) - System overview
- [crates/runtime/docs/PRIOR-ART.md](crates/runtime/docs/PRIOR-ART.md) - WANIX, Plan 9, original OANIX
- [crates/runtime/](crates/runtime/) - Current runtime implementation
- [crates/pylon/](crates/pylon/) - Node software
- [crates/autopilot-core/](crates/autopilot-core/) - Current coding agent

---

*"What if your computer was an agent that ran you, instead of the other way around?"*
