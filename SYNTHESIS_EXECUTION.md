# OpenAgents: Execution Guide

Practical guide to the current implementation. For the full vision, see [SYNTHESIS.md](./SYNTHESIS.md).
For the agent OS vision, see [OANIX.md](./docs/OANIX.md).

---

## System Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         OPENAGENTS STACK (Current)                           │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  PRODUCTS                                                                    │
│  ┌───────────────────┐  ┌───────────────────┐  ┌────────────────────────┐   │
│  │     Autopilot     │  │       Onyx        │  │   openagents.com       │   │
│  │  (coding agent)   │  │ (markdown editor) │  │   (web dashboard)      │   │
│  └─────────┬─────────┘  └─────────┬─────────┘  └───────────┬────────────┘   │
│            │                      │                        │                 │
│            └──────────────────────┴────────────────────────┘                 │
│                                   │                                          │
│  RUNTIME                          │                                          │
│  ┌────────────────────────────────┴─────────────────────────────────────┐   │
│  │  crates/runtime - Agent execution environment                         │   │
│  │  Tick model │ Filesystem abstraction │ /compute │ /containers │ /claude│   │
│  └───────────────────────────────────────────────────────────────────────┘   │
│                                   │                                          │
│  INFRASTRUCTURE                   │                                          │
│  ┌─────────────────┐  ┌───────────┴───────────┐  ┌────────────────────────┐ │
│  │      Pylon      │  │        Nexus          │  │     WGPUI              │ │
│  │  (local node)   │  │  (Nostr relay)        │  │  (GPU UI)              │ │
│  │  Provider/Host  │  │  NIP-90 job market    │  │  wgpu rendering        │ │
│  └────────┬────────┘  └───────────┬───────────┘  └────────────────────────┘ │
│           │                       │                                          │
│  PROTOCOLS│                       │                                          │
│  ┌────────┴───────────────────────┴─────────────────────────────────────┐   │
│  │  NIP-90 (compute jobs) │ NIP-42 (auth) │ NIP-89 (handlers)           │   │
│  │  Spark/Lightning (payments) │ Nostr (transport)                      │   │
│  └───────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Pylon

**What it is:** Single binary that runs on your device. Two modes, can run simultaneously.

| Mode | Purpose | How it works |
|------|---------|--------------|
| **Provider** | Earn Bitcoin by selling compute | Listens for NIP-90 jobs, runs inference, gets paid |
| **Host** | Run your own agents | Manages agent lifecycle, wallets, tick scheduling |

**Key paths:**
- `crates/pylon/src/cli/` — CLI commands (init, start, stop, status, doctor)
- `crates/pylon/src/provider.rs` — NIP-90 job processing
- `crates/pylon/src/host/` — Agent subprocess management
- `crates/pylon/src/daemon/` — Background process lifecycle

**Data directory:** `~/.openagents/pylon/`
- `config.toml` — Configuration
- `identity.mnemonic` — BIP-39 seed (chmod 600!)
- `pylon.db` — SQLite (jobs, earnings, agents)
- `control.sock` — IPC socket

**Build and run:**
```bash
cargo build --release -p pylon
./target/release/pylon init
./target/release/pylon start -f -m provider  # Foreground, provider mode
```

**Inference backends (auto-detected):**
- Apple Foundation Models (macOS + Apple Silicon)
- Ollama (any platform, port 11434)
- llama.cpp (any platform, port 8080)
- Claude (via claude-agent-sdk, requires `--features claude`)

**RLM (Recursive Language Model) queries:**
```bash
# Basic query (swarm)
pylon rlm "What is 2+2?"

# Local only with auto-detected backend
pylon rlm "Explain this" --local-only

# Use Claude as backend (requires cargo build -p pylon --features claude)
pylon rlm "Analyze this code" --backend claude

# View trace history and sync to dashboard
pylon rlm history
pylon rlm sync <run-id>
```

---

## Nexus

**What it is:** Nostr relay optimized for agent job coordination. Runs on Cloudflare Workers.

**Key NIPs supported:**
- NIP-90: Data Vending Machines (job requests/results)
- NIP-89: Handler discovery
- NIP-42: Authentication

**Event flow:**
```
Buyer → kind:5050 (job request) → Nexus → Provider
Provider → kind:7000 (invoice) → Nexus → Buyer
[Buyer pays Lightning invoice]
Provider → kind:6050 (result) → Nexus → Buyer
```

**Key paths:**
- `crates/nexus/worker/` — Cloudflare Worker implementation
- `crates/nexus/docs/MVP.md` — Protocol spec

**Deploy:**
```bash
cd crates/nexus/worker
bun install
bun run deploy
```

**Live instance:** `wss://nexus.openagents.com`

---

## Runtime

**What it is:** Pluggable execution environment for agents. Plan 9-inspired filesystem abstraction.

**The tick model:**
```
WAKE → LOAD → PERCEIVE → THINK → ACT → REMEMBER → SCHEDULE → SLEEP
```

Works across: Browser (WASM), Cloudflare (DO), Local (SQLite), Server (Docker/K8s).

**Agent filesystem (what agents see):**
```
/                           # Agent's root
├── ctl                     # control: tick, hibernate, wake
├── status                  # agent state
├── inbox/                  # incoming messages
├── outbox/                 # emitted events
├── goals/                  # active goals
├── memory/                 # conversations, patterns
├── identity/               # pubkey, sign, verify, encrypt, decrypt
├── wallet/                 # balance, pay
├── compute/                # LLM inference jobs
│   ├── providers/          # available backends
│   ├── new                 # submit job
│   └── jobs/<id>/          # status, result, stream
├── containers/             # sandboxed code execution
├── claude/                 # Claude Agent SDK sessions
└── hud/                    # streaming events for UI
```

**Key paths:**
- `crates/runtime/src/agent.rs` — Agent trait
- `crates/runtime/src/tick.rs` — Tick execution
- `crates/runtime/src/compute.rs` — /compute implementation
- `crates/runtime/src/containers.rs` — /containers implementation
- `crates/runtime/src/claude.rs` — /claude implementation
- `crates/runtime/src/services/` — Filesystem services (hud, wallet, logs, etc.)

**Mount points:**
| Mount | Purpose | Stateful |
|-------|---------|----------|
| `/compute` | Stateless inference (LLM calls) | No |
| `/containers` | Sandboxed code execution | Session |
| `/claude` | Claude Agent SDK sessions with tool use | Yes |

---

## Autopilot

**What it is:** The product. An autonomous coding agent that uses Claude SDK.

**Two modes:**
| Mode | Command | Cost | Where it runs |
|------|---------|------|---------------|
| Tunnel (free) | `pylon connect` | Free | Your machine |
| Container (paid) | Web UI | Credits | Cloudflare edge |

**Key paths:**
- `crates/autopilot/src/` — Core logic (preflight, runner, Claude SDK integration)
- `crates/autopilot-service/` — Background daemon
- `crates/autopilot-container/` — HTTP wrapper for Cloudflare Containers
- `crates/autopilot-shell/` — Interactive shell
- `crates/claude-agent-sdk/` — Rust SDK for Claude Code CLI

**How it connects:**

```
Autopilot ─────► Runtime ─────► Pylon ─────► Nexus
   │                │              │            │
   │                │              │            └── Nostr relay
   │                │              └── Local compute / provider
   │                └── /claude sessions, /compute calls
   └── Claude SDK queries, tool execution
```

**Run:**
```bash
cargo autopilot run "Fix the failing tests"
```

---

## DSPy Integration

**What it is:** DSPy is the **compiler layer for agent behavior**. It decides *what to do* (best prompt + tool-use structure + few-shot examples), while the execution infrastructure (Pylon, Nexus, Runtime) decides *where/how it runs*, *whether it's valid*, *what it costs*, and *why it worked*.

**Core implementation:** `crates/dsrs/` — 5,771 LOC Rust DSPy implementation integrated into the workspace.

**Philosophy (from Omar Khattab & Kevin Madura):**
- DSPy is declarative AI programming, not just prompt optimization
- Signatures decouple AI specification from ML techniques
- Field names act as mini-prompts — naming matters
- Optimizers (GEPA/MIPROv2) find "latent requirements" you didn't specify
- Enable model portability without rewriting prompts

**dsrs capabilities:**
| Feature | Description |
|---------|-------------|
| **Optimizers** | COPRO, MIPROv2, GEPA, Pareto |
| **DAG tracing** | Graph/Node types for execution tracing |
| **LM providers** | 14+ via rig-core (OpenAI, Anthropic, Gemini, Groq, Ollama, Pylon, Claude SDK, etc.) |
| **Architecture** | Module, Predictor, MetaSignature, Adapter, Optimizable, Evaluator traits |
| **Macros** | `#[Signature]`, `#[Optimizable]` for code generation |
| **Caching** | Hybrid memory + disk via foyer |
| **Multi-provider** | Claude SDK → Pylon swarm → Cerebras → Pylon local (auto-detection) |

**Agent module graph (8-stage pipeline):**
```
User Task → Task Router → Query Composer → Retrieval Router
                                              ↓
                          ┌──────────────────────────────────┐
                          │ Evidence Ranker + Evidence Workers │
                          └──────────────────────────────────┘
                                              ↓
                          State/Memory → Patch Planner → Patch Writer
                                              ↓
                                    Verifier → Fix Loop → FINAL PATCH
```

**Key modules in autopilot:**

| Module | Purpose |
|--------|---------|
| `dspy_planning.rs` | PlanningSignature, DeepPlanningSignature |
| `dspy_execution.rs` | ExecutionStrategySignature, ToolSelectionSignature |
| `dspy_verify.rs` | RequirementChecker, TestAnalyzer, ExecutionReview |
| `dspy_optimization.rs` | Metrics + training data infrastructure |

**Swarm job types (defined in `crates/protocol/`):**

| Job Type | Mode | Redundancy | Adjudication |
|----------|------|------------|--------------|
| `oa.code_chunk_analysis.v1` | Subjective | 2 | JudgeModel |
| `oa.retrieval_rerank.v1` | Subjective | 2 | MajorityVote |
| `oa.sandbox_run.v1` | Objective | 1 | None |

Each job includes `verification.mode`, `redundancy`, and `adjudication` strategy. See [protocol docs](./crates/protocol/docs/README.md).

**Planning pipeline:**
```
Issue → PlanningSignature → Structured Plan
         ├── analysis (understanding)
         ├── files_to_modify (JSON array)
         ├── implementation_steps (JSON array)
         ├── test_strategy
         ├── risk_factors
         └── confidence (0.0-1.0)
```

**Execution pipeline:**
```
Plan Step → ExecutionStrategySignature → Action Decision
             ├── next_action (EDIT_FILE/RUN_COMMAND/READ_FILE/COMPLETE)
             ├── action_params (JSON)
             ├── reasoning
             └── progress_estimate
```

**Verification pipeline:**
```
Solution → VerificationPipeline → Verdict
            ├── RequirementChecker (per requirement)
            ├── TestAnalyzer (if tests fail)
            ├── BuildAnalyzer (if build fails)
            └── SolutionVerifier (final verdict: PASS/FAIL/RETRY)
```

**Scoring function (robust):**
```
score = median(score over N rollouts)
where single_score =
  1.0 * pass_tests
  - 0.25 * (cost / budget)
  - 0.15 * (time / time_budget)
  - 0.10 * (diff_lines / diff_budget)
  - 0.10 * (sandbox_runs / sandbox_budget)
  - 0.10 * (bytes_opened / bytes_budget)      # evidence efficiency
```

**Promotion gates:**
```
candidate → staged → shadow → promoted → rolled_back
```
Shadow mode runs both old and new policy, ships old result, promotes only if new wins.

**Training data:**
- Examples in `crates/autopilot/examples/dspy_training_data.json`
- Metrics for optimization in `dspy_optimization.rs`
- Future: auto-collect from successful sessions via TraceExtractor

**Roadmap waves:**

| Wave | Status | Description |
|------|--------|-------------|
| 0 | **Complete** | Protocol + Schema Registry (`crates/protocol/`) |
| 1-2 | Complete | RLM + Autopilot signatures |
| 2.5 | Complete | LaneMux (multi-provider LM auto-detection) |
| 3 | **Complete** | Compiler Contract (manifest, callbacks, trace, sandbox) |
| 4 | **Complete** | Retrieval, Signatures, Swarm Dispatch |
| 5 | **Complete** | Eval Harness & Promotion Gates |
| 6 | Planned | SwarmCompiler (cheap optimization on Pylon) |
| 7+ | Planned | Privacy, OANIX, Agent Orchestrator, Tool Invocation |

See [docs/DSPY_ROADMAP.md](./docs/DSPY_ROADMAP.md) for full roadmap.

---

## Protocol

**What it is:** Foundation for typed job schemas with deterministic hashing. Every swarm job has a well-defined request/response structure, canonical hash, and verification mode.

**Key paths:**
- `crates/protocol/src/hash.rs` — Canonical JSON (RFC 8785) + SHA-256
- `crates/protocol/src/version.rs` — Semver schema versioning
- `crates/protocol/src/verification.rs` — Objective/subjective modes
- `crates/protocol/src/provenance.rs` — Audit trails (model, sampling, hashes)
- `crates/protocol/src/jobs/` — Job type schemas

**Job envelope structure:**
```rust
JobEnvelope {
    job_type: "oa.code_chunk_analysis.v1",
    schema_version: "1.0.0",
    job_hash: "abc123...",  // SHA-256 of canonical JSON
    payload: { ... }
}
```

**Verification modes:**
| Mode | Description | Example Jobs |
|------|-------------|--------------|
| Objective | Deterministic, single provider | Tests, builds, lint |
| Subjective | Requires judgment, multi-provider | Summaries, rankings |

**Adjudication strategies:**
| Strategy | Use Case |
|----------|----------|
| None | Objective jobs |
| MajorityVote | Categorical outputs |
| JudgeModel | Complex subjective outputs |
| Merge | Cumulative outputs (symbol lists) |

**Provenance tracking:**
```rust
Provenance {
    model_id: "claude-3-sonnet",
    sampling: { temperature: 0.0, seed: 42 },
    input_sha256: "...",
    output_sha256: "...",
    provider_pubkey: "npub1...",
    executed_at: 1700000000,
    tokens: { input: 500, output: 200 }
}
```

**Documentation:** See [crates/protocol/docs/](./crates/protocol/docs/README.md) for full API reference.

---

## Compiler Contract (Wave 3)

**What it is:** Bridge between DSPy's compiler layer and OpenAgents execution. Provides versioned module artifacts, observability, and execution primitives.

**Key paths:**
- `crates/dsrs/src/manifest.rs` — CompiledModuleManifest, Scorecard, Compatibility
- `crates/dsrs/src/callbacks.rs` — DspyCallback trait + implementations
- `crates/dsrs/src/trace/contract.rs` — OTel-compatible spans
- `crates/dsrs/src/trace/nostr_bridge.rs` — DAG → Nostr events
- `crates/dsrs/src/adapter/pylon_sandbox.rs` — Sandbox execution provider
- `crates/dsrs/src/predictors/refine.rs` — Retry/fallback meta-operator

**CompiledModuleManifest:**
```rust
CompiledModuleManifest {
    signature_name: "PlanningSignature",
    compiled_id: "sha256:abc123...",  // Deterministic hash
    optimizer: "MIPROv2",
    scorecard: Scorecard { proxy_score: 0.85, truth_score: Some(0.92), ... },
    compatibility: Compatibility { required_tools: ["ripgrep"], ... },
    ...
}
```

**Callbacks:**
```rust
pub trait DspyCallback: Send + Sync {
    fn on_module_start(&self, call_id: Uuid, module_name: &str, inputs: &Example);
    fn on_module_end(&self, call_id: Uuid, result: Result<&Prediction, &Error>);
    fn on_lm_start(&self, call_id: Uuid, model: &str, prompt_tokens: usize);
    fn on_lm_end(&self, call_id: Uuid, result: Result<(), &Error>, usage: &LmUsage);
    fn on_trace_complete(&self, graph: &Graph, manifest: Option<&CompiledModuleManifest>);
}
```

**TraceContract (OTel-compatible spans):**
```rust
let spans = TraceContract::graph_to_spans(&graph, Some(&manifest), "trace-id");
// Each span includes: dsrs.signature_name, dsrs.compiled_id, lm.total_tokens, lm.cost_msats
```

**NostrBridge:**
```rust
let bridge = NostrBridge::generate();
let events = bridge.graph_to_events(&graph, Some(&manifest))?;
// Publishes kind:1 events with dsrs tags to configured relays
```

**PylonSandboxProvider:**
```rust
let provider = PylonSandboxProvider::generate()
    .with_profile(SandboxProfile::Medium);
let result = provider.run_commands(vec!["cargo test"]).await?;
```

**Refine meta-operator:**
```rust
let refined = Refine::new(predictor)
    .with_max_retries(3)
    .with_threshold(0.8)
    .with_reward_fn(|inputs, pred| score(pred));
```

**Documentation:** See [crates/dsrs/docs/](./crates/dsrs/docs/README.md) for full API reference.

---

## Retrieval & Swarm Integration (Wave 4)

**What it is:** Multi-lane retrieval system and optimizable signatures for agent exploration, plus swarm job dispatch for parallel execution.

**Key paths:**
- `crates/dsrs/src/retrieval/` — Multi-lane retrieval backends
- `crates/dsrs/src/signatures/` — 9 optimizable DSPy signatures
- `crates/dsrs/src/adapter/swarm_dispatch.rs` — NIP-90 job dispatch

**Retrieval Backends:**
```rust
use dsrs::retrieval::{LaneRouter, RetrievalConfig};

// Auto-detect available backends
let router = LaneRouter::auto_detect("/path/to/repo").await?;

// Query specific lane
let (results, stats) = router.query_lane("ripgrep", "fn main", &config).await?;

// Query all lanes in parallel
let all_results = router.query_all("error handling", &config).await?;
```

| Lane | Backend | Best For |
|------|---------|----------|
| `ripgrep` | rg | Text/regex search, error messages, identifiers |
| `lsp` | ctags/rg | Function/struct definitions, symbol navigation |
| `semantic` | Ollama/OpenAI | Conceptual queries, natural language |
| `git` | git log/blame | Who changed what, recent modifications |

**Signatures (9 total):**

| Signature | Purpose |
|-----------|---------|
| `QueryComposerSignature` | Turn goals + failures into search queries |
| `RetrievalRouterSignature` | Pick lane and K for queries |
| `CandidateRerankSignature` | Rerank results (maps to `oa.retrieval_rerank.v1`) |
| `ChunkTaskSelectorSignature` | Decide analysis tasks per chunk |
| `ChunkAnalysisToActionSignature` | Aggregate findings into actions |
| `SandboxProfileSelectionSignature` | Choose S/M/L resources |
| `FailureTriageSignature` | Diagnose failures, suggest fixes |
| `LaneBudgeterSignature` | Allocate budget across lanes |
| `AgentMemorySignature` | Detect redundant queries |

**Swarm Dispatch:**
```rust
use dsrs::adapter::SwarmDispatcher;

let dispatcher = SwarmDispatcher::generate()
    .with_relays(vec!["wss://nexus.openagents.com".into()])
    .with_budget(5000);

// Dispatch chunk analysis to swarm
let result = dispatcher.dispatch_chunk_analysis(
    "Summarize this code",
    Some("User is debugging authentication"),
    chunk,
).await?;
```

**Documentation:** See [crates/dsrs/docs/](./crates/dsrs/docs/README.md) for full API reference.

---

## WGPUI

**What it is:** GPU-accelerated UI rendering library. WebGPU/Vulkan/Metal/DX12 via wgpu.

**Why:** HTML hits limits for performance-critical surfaces:
- Streaming markdown at 100+ tokens/sec
- Virtual scrolling 10k+ messages
- Real-time syntax highlighting

**Key paths:**
- `crates/wgpui/src/renderer.rs` — wgpu pipelines
- `crates/wgpui/src/text.rs` — cosmic-text integration
- `crates/wgpui/src/layout.rs` — Taffy (CSS Flexbox)
- `crates/wgpui/src/markdown/` — Streaming markdown
- `crates/wgpui/src/components/` — Atomic design (atoms → molecules → organisms)

**Design constraints:**
- Sharp corners only (no border-radius)
- Tailwind-aligned tokens
- Vera Mono font only

**Build:**
```bash
cargo build -p wgpui                                    # Web (default)
cargo build -p wgpui --features desktop --no-default-features  # Desktop
cargo build -p wgpui --target wasm32-unknown-unknown    # WASM
```

---

## Key Crates

| Crate | Purpose |
|-------|---------|
| `pylon` | Node software (provider + host) |
| `nexus` | Nostr relay for job market |
| `runtime` | Agent execution environment |
| `autopilot` | Coding agent product |
| `wgpui` | GPU-rendered UI |
| `spark` | Lightning wallet (Breez SDK) |
| `compute` | NIP-90 DVM primitives |
| `claude-agent-sdk` | Rust SDK for Claude Code |
| `rlm` | Recursive Language Model engine |
| `frlm` | Federated RLM (distributed execution) |
| `frostr` | FROST threshold signatures |
| `dsrs` | Rust DSPy - signatures, optimizers, DAG tracing ([docs](./crates/dsrs/docs/README.md)) |
| `dsrs-macros` | Procedural macros for dsrs |
| `protocol` | Typed job schemas, canonical hashing, verification |

**RLM/FRLM execution venues:**

| Venue | Description |
|-------|-------------|
| `Local` | FM Bridge, Ollama, llama.cpp |
| `Swarm` | NIP-90 distributed to providers |
| `Datacenter` | Remote API (Crusoe, etc.) |
| `Claude` | Claude via claude-agent-sdk |

Traces flow to SQLite (`~/.openagents/pylon/rlm.db`) → cloud sync → W&B-style dashboard.

---

## For Coding Agents

### Git Rules

```
NEVER: push --force to main, git stash, destructive commands without asking
ALWAYS: Commit working code every 15-30 minutes, small frequent commits
```

Stage only your own files. Other agents may have uncommitted work.

### Commit Format

```bash
git commit -m "$(cat <<'EOF'
Short description of change

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude <noreply@anthropic.com>
Co-Authored-By: Autopilot <autopilot@openagents.com>
EOF
)"
```

### Build Commands

```bash
# Pylon
cargo build --release -p pylon

# Pylon with Claude backend support
cargo build --release -p pylon --features claude

# Nexus (Cloudflare Worker)
cd crates/nexus/worker && bun run deploy

# Autopilot
cargo autopilot run "your prompt"

# WGPUI tests
cargo test -p wgpui

# Full workspace
cargo build --workspace
```

### Database Access

**NEVER raw sqlite3 for writes.** Use APIs:
```bash
cargo autopilot issue create
cargo autopilot issue claim
cargo autopilot issue complete
```

Read-only queries OK for debugging.

### Nostr

NIP specs are local at `~/code/nips/`. Read from there, don't web search.

### Completion Standards

Issues are NOT done unless:
1. No stubs, mocks, TODOs, NotImplemented
2. Code actually works (tested)
3. SDK integrations are real, not stubbed

---

## Data Flow: End-to-End

**User runs Autopilot locally:**
```
1. User: `cargo autopilot run "Fix tests"`
2. Autopilot: Preflight checks (config, auth, repo)
3. Autopilot: Creates Claude SDK session via Runtime /claude
4. Runtime: Routes to local Claude tunnel or cloud API
5. Claude: Reads files, makes edits, runs tests
6. Autopilot: Streams results to terminal/HUD
```

**Autopilot needs inference from swarm:**
```
1. Autopilot: Writes job to Runtime /compute/new
2. Runtime: Publishes NIP-90 kind:5050 to Nexus
3. Nexus: Broadcasts to subscribed Pylons
4. Pylon (provider): Picks up job, runs inference
5. Pylon: Publishes kind:7000 (invoice), waits for payment
6. Autopilot: Pays Lightning invoice via /wallet/pay
7. Pylon: Publishes kind:6050 (result)
8. Runtime: Receives result, returns to Autopilot
```

**Provider earns Bitcoin:**
```
1. Pylon: `pylon start -m provider`
2. Pylon: Connects to Nexus, subscribes to kind:5050
3. Pylon: Detects inference backends (Ollama, Apple FM, etc.)
4. Buyer: Submits job to Nexus
5. Pylon: Receives job, sends invoice
6. Buyer: Pays invoice
7. Pylon: Runs inference, publishes result
8. Pylon: Sats deposited to embedded Spark wallet
```

---

## Current Status

| Component | Status | Notes |
|-----------|--------|-------|
| Pylon CLI | v0.1 | Provider mode working, host mode partial |
| Pylon Wallet | Working | Spark/Lightning, regtest + mainnet |
| Nexus | v0.1 | NIP-90, NIP-42, NIP-89 |
| Runtime | In progress | Tick engine, filesystem, /compute, /containers, /claude |
| Autopilot | Alpha | Claude SDK integration, tunnel mode |
| Autopilot DSPy | Wave 3 | Planning, Execution, Verification + Compiler Contract |
| dsrs | **Wave 5** | Eval Harness, Promotion Gates, Scoring (see Wave 3-5) |
| WGPUI | Phase 16 | 377 tests, full component library |
| RLM | Working | Claude + Ollama backends, MCP tools |
| RLM DSPy | Wave 1 | DspyOrchestrator, provenance signatures |
| FRLM | Working | Claude venue, trace persistence, dashboard sync |
| Protocol | **Complete** | Job schemas, canonical hashing, verification modes |
| OANIX | Wave 8 | Agent OS runtime (design) |

**Bitcoin network:** Default is `regtest` for testing. Mainnet available.

---

## Quick Reference

### Start Provider (earn sats)
```bash
pylon init
pylon start -f -m provider
```

### Run Autopilot
```bash
cargo autopilot run "Implement feature X"
```

### Deploy Nexus
```bash
cd crates/nexus/worker && bun run deploy
```

### Check Wallet
```bash
pylon wallet balance
pylon wallet fund  # regtest only
```

### Run Tests
```bash
cargo test -p pylon
cargo test -p runtime
cargo test -p wgpui
cargo test -p protocol
cargo test -p dsrs
```
