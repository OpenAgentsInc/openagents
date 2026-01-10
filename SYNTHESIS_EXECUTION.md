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

### What DSPy Enables (Plain Language)

DSPy fundamentally changes how AI agents make decisions. Instead of hand-crafting prompts and hoping they work, you declare *what* you want the AI to do using typed signatures, and let the system automatically discover the best way to achieve it.

**The Problem It Solves:** Traditional AI development involves endless prompt tweaking. You write a prompt, test it, notice edge cases, add more instructions, break something else, and repeat forever. This doesn't scale — especially when you need to support multiple models (Claude, GPT-4, local LLMs) that each respond differently to the same prompt.

**The DSPy Solution:** Instead of prompts, you write signatures:

```rust
#[Signature]
struct TaskPlanner {
    /// Detailed task description
    #[input] task_description: String,
    /// Files in the repository
    #[input] file_count: u32,

    /// Complexity classification: Low, Medium, High, VeryHigh
    #[output] complexity: String,
    /// Reasoning for the classification
    #[output] reasoning: String,
    /// Confidence score (0.0-1.0)
    #[output] confidence: f32,
}
```

This signature declares: "Given a task description and file count, produce a complexity classification with reasoning and confidence." The field names and doc comments act as mini-prompts. DSPy's optimizers (MIPROv2, GEPA) then automatically discover the best prompt phrasing, examples, and reasoning structure to reliably produce these outputs.

### How DSPy Connects to Coder/Autopilot

The Coder uses **Adjutant** as its execution engine. Adjutant now has DSPy integration at two levels:

**Level 1: Task Execution (TieredExecutor)**
The TieredExecutor uses three signatures for task execution:
- `SubtaskPlanningSignature` — Breaks a task into atomic subtasks
- `SubtaskExecutionSignature` — Executes individual subtasks (read, edit, bash)
- `ResultSynthesisSignature` — Synthesizes subtask results into final outcome

**Level 2: Decision Routing (Decision Pipelines)**
Before executing, Adjutant uses DSPy to make intelligent routing decisions:
- `ComplexityPipeline` — Classifies task complexity (Low/Medium/High/VeryHigh)
- `DelegationPipeline` — Decides whether to delegate (to Claude Code, RLM, or local tools)
- `RlmTriggerPipeline` — Decides whether RLM (Recursive Language Model) should be used

**The Execution Flow:**
```
1. Coder receives task
2. Adjutant.plan_task() discovers relevant files
3. ComplexityPipeline classifies task complexity
4. RlmTriggerPipeline decides if RLM is needed
5. DelegationPipeline decides execution strategy
6. Execute via chosen path (local tools, Claude Code, or RLM)
7. Training data collected for optimization
```

**Autonomous Autopilot Loop:**

When Coder is in Autopilot mode, it doesn't just execute once — it runs Adjutant in an **autonomous loop** until the task is truly complete:

```
┌─────────────────────────────────────────────────────────────┐
│                    Coder Autopilot Loop                      │
├─────────────────────────────────────────────────────────────┤
│  User: "Fix the auth bug"                                    │
│        ↓                                                     │
│  --- Iteration 1/10 ---                                      │
│  [Adjutant analyzes, identifies issue in login.rs]          │
│        ↓                                                     │
│  --- Iteration 2/10 ---                                      │
│  [Adjutant applies fix]                                      │
│  🔍 Verifying...                                             │
│    cargo check... OK                                         │
│    cargo test... FAILED                                      │
│  ⚠ Verification failed, continuing...                       │
│        ↓                                                     │
│  --- Iteration 3/10 ---                                      │
│  [Adjutant fixes failing test]                               │
│  🔍 Verifying...                                             │
│    cargo check... OK                                         │
│    cargo test... OK                                          │
│  ✓ Verification passed                                       │
│  ✓ Task completed                                            │
└─────────────────────────────────────────────────────────────┘
```

The loop terminates when:
- **Success**: LLM reports success AND verification passes (cargo check + cargo test)
- **Definitive failure**: Unrecoverable error detected (permission denied, file not found, etc.)
- **Max iterations**: 10 iterations reached (can continue with another message)
- **User interrupt**: Press Escape to stop cleanly

Each iteration builds on the previous one, passing context about what was attempted and what failed. This transforms Autopilot from "single-shot execution" to "true autonomous agent."

Implementation: `crates/coder/src/autopilot_loop.rs`

**DSPy-First with Fallback:** Each decision pipeline uses a 0.7 confidence threshold. If the DSPy model returns a prediction with confidence above 0.7, it's used. Below that, the system falls back to legacy rule-based logic. This ensures reliability while collecting training data to improve the DSPy decisions over time.

### Automatic Training Collection

Every successful DSPy decision is recorded:
- High-confidence complexity classifications
- Delegation decisions and their outcomes
- RLM trigger decisions

Training data is saved to `~/.openagents/adjutant/training/dataset.json` and can be used to optimize the decision pipelines with MIPROv2. Over time, the system learns from its own successes.

### LM Caching for Efficiency

DSPy decisions require an LM (Language Model) to run. Adjutant uses lazy initialization — the LM is created on first decision and cached for all subsequent calls:

```rust
async fn get_or_create_decision_lm(&mut self) -> Option<Arc<LM>> {
    if self.decision_lm.is_none() {
        self.decision_lm = get_planning_lm().await.ok();
    }
    self.decision_lm.clone()
}
```

This means the first decision might take a moment to initialize the LM, but all subsequent decisions reuse the same connection, making routing decisions fast.

### What This Means in Practice

**For Users:** The Coder is now smarter about when to use expensive resources. Instead of blindly delegating complex tasks to Claude Code (expensive) or running simple tasks through RLM (overkill), it makes intelligent decisions based on the actual task content.

**For Optimization:** As the system collects training data, it can be optimized to make better decisions. A task that's initially misclassified as "High" complexity might, after optimization, be correctly classified as "Low" — saving resources.

**For Model Portability:** Because decisions are expressed as signatures rather than prompts, switching from Claude to GPT-4 to a local LLM requires no prompt rewriting. The optimizer re-discovers the best approach for each model automatically.

**For Observability:** Every decision includes confidence scores and reasoning. When something goes wrong, you can see exactly why the system made the choices it did.

---

### How to Actually Use the Training Data (The Full Loop)

DSPy training isn't magic — it's a concrete workflow. Here's exactly how it works:

#### Step 1: Training Data Gets Collected Automatically

Every time Adjutant makes a DSPy decision with confidence > 0.7, it records an example:

```
~/.openagents/adjutant/training/dataset.json
```

This file grows as you use the Coder. Each entry looks like:

```json
{
  "complexity_examples": [
    {
      "task_description": "Add error handling to auth.rs",
      "file_count": 3,
      "estimated_tokens": 5000,
      "keywords": ["error"],
      "expected_complexity": "Medium",
      "confidence": 0.85
    }
  ],
  "delegation_examples": [...],
  "rlm_trigger_examples": [...],
  "planning_examples": [...],
  "execution_examples": [...],
  "synthesis_examples": [...]
}
```

You can inspect this file anytime:
```bash
cat ~/.openagents/adjutant/training/dataset.json | jq .
```

#### Step 2: Run Optimization When You Have Enough Data

Once you have 20-50+ examples per signature, you can optimize. Currently this is done programmatically:

```rust
use adjutant::dspy::{AdjutantModule, AdjutantTrainingDataset};
use dsrs::{MIPROv2, Optimizer};

// Load the training data you've collected
let dataset = AdjutantTrainingDataset::load()?;
println!("Loaded {} examples", dataset.len());

// Convert to DSPy examples
let planning_examples = dataset.planning_as_examples();
let complexity_examples = dataset.complexity_as_examples();

// Create module and optimizer
let mut module = AdjutantModule::new();
let optimizer = MIPROv2::builder()
    .num_candidates(10)      // Try 10 different prompt variations
    .num_trials(20)          // Run 20 optimization trials
    .build();

// Run optimization - this takes a few minutes
// The optimizer tries different prompts and measures which work best
optimizer.compile(&mut module, planning_examples).await?;

// The module now has optimized instructions
println!("Before: Break the given task into concrete, atomic subtasks.");
println!("After:  {}", module.planner.get_signature().instruction());
```

**What MIPROv2 Actually Does:**
1. Looks at your training examples (inputs → expected outputs)
2. Generates candidate prompt variations
3. Runs each candidate on the training data
4. Scores results using the metrics in `metrics.rs`
5. Keeps the best-performing prompt

The output might change from:
```
"Break the given task into concrete, atomic subtasks."
```
to something like:
```
"Analyze the task description and repository context. Identify the minimal
set of atomic changes needed. For each subtask, specify exactly one file
and one action (read/edit/bash). Order subtasks by dependency."
```

This improved prompt produces better plans because it learned from your actual usage patterns.

#### Step 3: Deploy the Optimized Module

After optimization, you have two choices:

**Option A: Use in Code**
```rust
// The module variable now has optimized instructions
// Just use it directly
let plan = module.plan("Fix the bug", "Description...", "Context...").await?;
```

**Option B: Save and Load Later**
```rust
// Save the optimized module
module.save("~/.openagents/adjutant/optimized_module.json")?;

// Later, load it
let module = AdjutantModule::load("~/.openagents/adjutant/optimized_module.json")?;
```

#### Step 4: Measure Improvement

The optimization produces a scorecard:

```rust
let result = optimizer.compile(&mut module, examples).await?;
println!("Proxy score: {:.2}", result.scorecard.proxy_score);
println!("Truth score: {:?}", result.scorecard.truth_score);
```

- **Proxy score**: Quick evaluation during optimization (0.0-1.0)
- **Truth score**: Full evaluation with expensive metrics (optional)

A good optimization might take proxy score from 0.65 → 0.82.

#### The Optimization Loop (Long-Term)

```
┌─────────────────────────────────────────────────────────────────┐
│                    DSPy Optimization Loop                        │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  1. USE THE SYSTEM                                               │
│     └── Coder makes decisions with DSPy signatures              │
│     └── High-confidence decisions get recorded                   │
│                                                                  │
│  2. COLLECT TRAINING DATA                                        │
│     └── ~/.openagents/adjutant/training/dataset.json            │
│     └── Wait until you have 20-50+ examples                      │
│                                                                  │
│  3. RUN OPTIMIZATION                                             │
│     └── MIPROv2 tries prompt variations                          │
│     └── Finds what works best for YOUR usage patterns            │
│                                                                  │
│  4. DEPLOY OPTIMIZED MODULE                                      │
│     └── Better prompts = better decisions                        │
│     └── Higher confidence = less fallback to legacy rules        │
│                                                                  │
│  5. REPEAT                                                       │
│     └── Continue collecting data with new module                 │
│     └── Re-optimize periodically as patterns change              │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

#### What's Coming: CLI Integration

Future versions will have CLI commands:

```bash
# View training data stats
autopilot dspy stats

# Run optimization
autopilot dspy optimize --signature planning --trials 20

# Deploy optimized module
autopilot dspy deploy ./optimized_module.json

# A/B test old vs new
autopilot dspy test --shadow-mode
```

For now, use the programmatic API or write a simple script.

#### Why This Matters

Traditional prompt engineering:
- You tweak prompts by hand
- Test on a few examples
- Ship and hope it works
- Repeat when it breaks

DSPy workflow:
- Define what you want (signature)
- Let the system collect real usage data
- Optimizer finds what actually works
- Decisions get better automatically over time

The system learns from its own successes. The more you use it, the better it gets.

---

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
| 0 | Complete | Protocol + Schema Registry (`crates/protocol/`) |
| 1-2 | Complete | RLM + Autopilot signatures |
| 2.5 | Complete | LaneMux (multi-provider LM auto-detection) |
| 3 | Complete | Compiler Contract (manifest, callbacks, trace, sandbox) |
| 4 | Complete | Retrieval, Signatures, Swarm Dispatch |
| 5 | Complete | Eval Harness & Promotion Gates |
| 6 | Complete | SwarmCompiler (cheap bootstrap + premium validation) |
| 7 | Complete | Privacy Module (redaction, chunking, policy) |
| 8 | Complete | OANIX DSPy Signatures |
| 9 | Complete | Agent Orchestrator Signatures |
| 10 | Complete | Tool Invocation Signatures |
| 11 | Complete | Optimization Infrastructure (DspyHub, TrainingExtractor) |
| 12 | Complete | FRLM Integration |
| 13 | Complete | Pipeline Wiring (decision pipelines + LM caching + training collection) |
| 14 | **Complete** | Self-Improving Autopilot (sessions, outcome feedback, auto-optimize) |

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

## SwarmCompiler (Wave 6)

**What it is:** Cost-efficient DSPy optimization using cheap Pylon swarm for bootstrap and premium models for validation. Achieves ~96% cost reduction compared to premium-only approaches.

**Cost model:**
- Pylon swarm: ~10 msats/call (bootstrap phase)
- Premium (Claude/GPT-4): ~1000 msats/call (validation phase)
- Savings: Bootstrap 1800 calls on swarm ($0.18) vs Claude ($15) = 96.7% reduction

**Key paths:**
- `crates/dsrs/src/compiler/mod.rs` — Module exports
- `crates/dsrs/src/compiler/provider.rs` — LMProvider trait + implementations
- `crates/dsrs/src/compiler/budget.rs` — BudgetManager, cost tracking
- `crates/dsrs/src/compiler/trace_collector.rs` — Execution trace capture
- `crates/dsrs/src/compiler/swarm_compiler.rs` — SwarmCompiler orchestrator
- `crates/dsrs/src/compiler/result.rs` — CompileResult bundle

**Compilation phases:**

```
1. Bootstrap (cheap Pylon swarm)
   └─ Allocate bootstrap_budget_msats
   └─ Run MIPROv2 with bootstrap LM
   └─ Generate candidate prompts/demos
   └─ Quick proxy metric evaluation

2. Validate (premium model)
   └─ Allocate validation_budget_msats
   └─ Run Scorer with validation LM
   └─ Full truth metric evaluation
   └─ Generate ScorecardResult

3. Promote (gates)
   └─ Feed scorecard to PromotionManager
   └─ Check promotion gates
   └─ Update manifest with eval_history
```

**Usage:**
```rust
use dsrs::compiler::{SwarmCompiler, SwarmCompileConfig, MockLM};
use std::sync::Arc;

// Create compiler with cheap + premium LMs
let compiler = SwarmCompiler::new(
    Arc::new(MockLM::cheap()),      // Bootstrap: Pylon swarm
    Arc::new(MockLM::expensive()),  // Validation: Claude/GPT-4
);

// Configure compilation
let config = SwarmCompileConfig::default()
    .bootstrap_budget(1000)   // ~100 calls at 10 msats
    .validation_budget(5000)  // ~5 calls at 1000 msats
    .rollouts(3, 5)           // 3 bootstrap, 5 validation
    .proxy_threshold(0.7);    // Skip validation if bootstrap < 0.7

// Run compilation
let result = compiler.compile(&module, trainset, &eval_tasks, config).await?;

// Check result
if result.is_promoted() {
    println!("Module promoted: {:?}", result.promotion_state());
} else {
    println!("Promotion failed: {}", result.promotion_result.reason);
}

// Inspect budget
let report = result.budget_report;
println!("Cost: {} msats (bootstrap: {}, validation: {})",
    report.spent,
    report.by_phase.get("bootstrap").unwrap_or(&0),
    report.by_phase.get("validate").unwrap_or(&0)
);
```

**LMProvider implementations:**
| Provider | Cost | Use Case |
|----------|------|----------|
| `MockLM` | Configurable | Testing |
| `PylonLM` | ~10 msats | Bootstrap (swarm/local) |
| `FallbackLM` | Variable | Hybrid (try cheap, fallback to premium) |

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
| `adjutant` | Execution engine with DSPy decision pipelines ([docs](./crates/adjutant/docs/README.md)) |
| `coder` | UI for autonomous coding agent |
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
| `agent-orchestrator` | Multi-agent delegation with DSPy ([docs](./crates/agent-orchestrator/docs/)) |

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
| Autopilot DSPy | Wave 11 | Planning, Execution, Verification + Hub + Router |
| Adjutant | **Wave 14** | Self-improving autopilot (sessions, outcome feedback, auto-optimize) |
| Coder Autopilot | **Complete** | Autonomous loop with verification (cargo check/test) |
| dsrs | **Wave 14** | SwarmCompiler, Pipelines, Privacy, Self-Improvement |
| WGPUI | Phase 16 | 377 tests, full component library |
| RLM | Working | Claude + Ollama backends, MCP tools |
| RLM DSPy | Wave 1 | DspyOrchestrator, provenance signatures |
| FRLM | Working | Claude venue, trace persistence, dashboard sync |
| Protocol | Complete | Job schemas, canonical hashing, verification modes |
| OANIX | Wave 8 | DSPy signatures complete |
| Agent Orchestrator | Wave 9+13 | DSPy signatures + pipeline wiring |
| Runtime Tools | Wave 10+13 | DSPy signatures + pipeline wiring |

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
