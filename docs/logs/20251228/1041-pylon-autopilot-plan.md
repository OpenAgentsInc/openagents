# Plan: Pylon Integration into Autopilot Startup

## Goal

Integrate Pylon into autopilot's startup flow so it:
1. Checks for local pylon, auto-starts if not running
2. Detects local inference backends (Ollama, Apple FM, Llama.cpp)
3. Queries NIP-89 relays for remote swarm providers
4. Displays "compute mix" in UI/logs showing all available options
5. Provides CLI command for same info

---

## Phase 1: Data Structures

**File:** `crates/autopilot/src/preflight.rs`

Add new structs after `LocalBackend`:

```rust
/// Local Pylon daemon status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PylonInfo {
    pub running: bool,
    pub pid: Option<u32>,
    pub uptime_secs: Option<u64>,
    pub jobs_completed: u64,
    pub models: Vec<String>,
}

/// Remote compute provider via NIP-89
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmProvider {
    pub pubkey: String,
    pub name: String,
    pub price_msats: Option<u64>,
    pub relay: String,
}

/// Complete compute availability summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeMix {
    pub pylon: Option<PylonInfo>,
    pub local_backends: Vec<LocalBackend>,
    pub cloud_providers: Vec<String>,
    pub swarm_providers: Vec<SwarmProvider>,
}
```

Extend `InferenceInfo`:
```rust
pub struct InferenceInfo {
    // ... existing fields ...
    pub pylon: Option<PylonInfo>,
    pub swarm_providers: Vec<SwarmProvider>,
}
```

---

## Phase 2: Pylon Integration Module

**New file:** `crates/autopilot/src/pylon_integration.rs`

```rust
//! Pylon daemon integration for autopilot

use pylon::daemon::process::is_daemon_running;
use compute::backends::BackendRegistry;

/// Check if pylon is running (reuse pylon crate's function)
pub fn check_pylon_running() -> bool {
    is_daemon_running()
}

/// Start pylon daemon via CLI command
pub fn start_pylon() -> anyhow::Result<()> {
    std::process::Command::new("pylon")
        .arg("start")
        .spawn()?;
    std::thread::sleep(std::time::Duration::from_millis(1000));
    Ok(())
}

/// Async: Detect local inference backends
pub async fn detect_local_backends() -> Vec<LocalBackend> {
    let registry = BackendRegistry::detect().await;
    // Convert to LocalBackend format
}

/// Async: Query NIP-89 relays for swarm providers
pub async fn discover_swarm_providers() -> Vec<SwarmProvider> {
    let relays = ["wss://relay.damus.io", "wss://nos.lol"];
    // Query kind 31990 events, parse into SwarmProvider
}
```

---

## Phase 3: New Startup Phases

**File:** `crates/autopilot/src/startup.rs`

### Add phases after `PreflightComplete`:

```rust
pub enum StartupPhase {
    // ... existing ...
    PreflightComplete,
    // NEW PHASES
    CheckingPylon,           // Check if pylon running
    StartingPylon,           // Auto-start if needed
    DetectingCompute,        // Detect local backends + query swarm
    ComputeMixReady,         // Display summary
    // Continue with existing...
    AnalyzingIssues,
}
```

### Add fields to `StartupState`:

```rust
pub struct StartupState {
    // ... existing ...
    pub compute_mix: Option<ComputeMix>,
    pylon_started: bool,
    compute_task: Option<tokio::task::JoinHandle<ComputeMix>>,
}
```

### Implement tick() handlers:

**CheckingPylon:**
```
Checking local pylon...
  Pylon daemon is running
  Uptime: 2h 15m
```
or
```
Checking local pylon...
  Pylon not running
```

**StartingPylon:**
```
Starting pylon daemon...
  Pylon started successfully
```

**DetectingCompute:**
```
Detecting compute backends...
  [OK] Ollama (localhost:11434) - llama3.2, mistral
  [OK] Apple FM (localhost:11435) - MLX-7B
  [--] Llama.cpp (not running)

Querying NIP-89 swarm providers...
  Found 3 remote providers
```

**ComputeMixReady:**
```
Compute mix:
  Local: Ollama (llama3.2), Apple FM (MLX-7B)
  Cloud: anthropic, openai
  Swarm: 3 providers via NIP-89
```

---

## Phase 4: CLI Command

**New file:** `crates/autopilot/src/cli/compute.rs`

```rust
#[derive(Args)]
pub struct ComputeArgs {
    #[arg(long)]
    pub json: bool,
}

pub async fn run(args: ComputeArgs) -> anyhow::Result<()> {
    // 1. Check pylon status
    // 2. Detect local backends
    // 3. Query swarm providers
    // 4. Check cloud providers (env vars)
    // 5. Output as JSON or formatted text
}
```

**Wire up in:** `crates/autopilot/src/cli/mod.rs`

Add to CLI dispatch:
```rust
Commands::Compute(args) => compute::run(args).await,
```

---

## Files to Modify

| File | Changes |
|------|---------|
| `crates/autopilot/src/preflight.rs` | Add PylonInfo, SwarmProvider, ComputeMix structs |
| `crates/autopilot/src/startup.rs` | Add 4 new phases, ComputeMix field, tick() handlers |
| `crates/autopilot/src/lib.rs` | Export new pylon_integration module |
| `crates/autopilot/Cargo.toml` | Add dep on `pylon`, `compute` crates |

## New Files

| File | Purpose |
|------|---------|
| `crates/autopilot/src/pylon_integration.rs` | Pylon detection, start, backend discovery |
| `crates/autopilot/src/cli/compute.rs` | CLI command for compute mix status |

---

## Implementation Order

1. Add data structures to `preflight.rs`
2. Create `pylon_integration.rs` with detection functions
3. Add new phases to `startup.rs` enum
4. Implement tick() handlers for each phase
5. Add CLI command
6. Test: run autopilot, verify logs show compute mix
7. Test: `openagents autopilot compute` CLI

---

## Expected UI Output

```
Checking local pylon...
  Pylon daemon is running
  Uptime: 2h 15m

Detecting compute backends...
  [OK] Ollama (localhost:11434)
    Models: llama3.2, mistral
  [OK] Apple FM (localhost:11435)
    Models: MLX-7B
  [--] Llama.cpp (not running)

Querying NIP-89 swarm providers...
  Found 2 remote providers

Compute mix:
  Local: Ollama (llama3.2, mistral), Apple FM (MLX-7B)
  Cloud: anthropic, openai
  Swarm: 2 providers via NIP-89
```

---

## Key Dependencies

- `pylon::daemon::process::is_daemon_running()` - Already exists, reuse
- `compute::backends::BackendRegistry::detect()` - Already exists, reuse
- NIP-89 discovery - Reference `agent-customer` implementation

---

## Success Criteria

1. Autopilot startup logs show pylon check + auto-start
2. Local backends (Ollama, etc.) detected and logged
3. NIP-89 swarm providers queried (with timeout)
4. "Compute mix" summary displayed
5. `openagents autopilot compute` CLI works
6. JSON output option available
