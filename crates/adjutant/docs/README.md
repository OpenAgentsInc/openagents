# Adjutant

The agent that DOES THE WORK. Named after StarCraft's command & control AI.

## Overview

Adjutant is the core execution engine for autonomous coding tasks. It:
- Analyzes tasks to determine complexity and relevant files
- Uses tools directly (Read, Edit, Bash, Glob, Grep)
- Employs tiered inference for cost-effective execution
- Delegates to Claude Code for highly complex tasks

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      AUTOPILOT CLI                          │
│  (user-facing: `autopilot run`, `autopilot issue claim`)    │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                   OANIX (background)                        │
│  (discovers environment, reads .openagents/)                │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                       ADJUTANT                              │
│  The actual agent that DOES THE WORK                        │
│  - Prioritizes Claude (Pro/Max) via claude-agent-sdk        │
│  - Falls back to Cerebras TieredExecutor                    │
│  - Uses tools directly (Read, Edit, Bash, Glob, Grep)       │
│  - Uses RLM for large context analysis                      │
│  - Delegates to Claude Code for very complex work           │
└─────────────────────────────────────────────────────────────┘
```

## Quick Start

```bash
# Set Cerebras API key for AI-powered execution
export CEREBRAS_API_KEY="csk-your-key-here"

# Run a task (fast boot skips network/compute discovery)
cargo autopilot run "Add error handling to auth.rs"

# Full environment scan (slower)
cargo autopilot run "Add error handling to auth.rs" --full-boot

# Claim and work on a GitHub issue
cargo autopilot issue claim 123
```

### CLI Streaming Output

`autopilot run` streams the same ACP event flow used by the desktop app, formatted as concise CLI lines:

```
[DSPY] planning: complexity=Medium confidence=82%
[DSPY] files: src/auth.rs
[TOOL] start bash: command=rg "auth" src
[TOOL] done bash ok
[AI] Updated error handling and added tests.
```

Verbose logs (stderr) are opt-in via `RUST_LOG`:

```bash
RUST_LOG=adjutant=info cargo autopilot run "Summarize README.md"
```

## Execution Flow

1. **Planning**: Analyzes task, finds relevant files, estimates complexity
2. **DSPy Complexity Override**: If LM available and confidence > 0.7, uses DSPy classification
3. **DSPy Routing Decisions**:
   - `determine_use_rlm()` - Should RLM be used? (DSPy-first with fallback)
   - `determine_delegation()` - Should task be delegated? Where? (DSPy-first with fallback)
4. **Execution**: Runs the task using chosen strategy (priority order):
   - **Claude Pro/Max** (if `claude` CLI is installed) - Best quality, uses subscription
   - **Cerebras TieredExecutor** (if CEREBRAS_API_KEY set) - Cost-effective tiered inference
   - **Analysis-only** - Returns file analysis without making changes
5. **Synthesis**: Summarizes results, optionally commits changes
6. **Training Collection**: Records high-confidence DSPy decisions for optimization

```
┌─────────────────────────────────────────────────────────────┐
│                    Adjutant.execute() Flow                  │
├─────────────────────────────────────────────────────────────┤
│  1. plan_task()                                             │
│     └── Rule-based file discovery, initial complexity       │
│                                                              │
│  2. determine_complexity_dspy() [DSPy-first]                │
│     └── Override complexity if confidence > 0.7             │
│                                                              │
│  3. determine_use_rlm() [DSPy-first]                        │
│     └── RLM for large context analysis?                     │
│                                                              │
│  4. LM Provider Selection                                   │
│     ├── LlamaCpp → execute_with_local_lm()                  │
│     └── ClaudeSdk → ClaudeExecutor (with optional RLM)      │
│                                                              │
│  5. determine_delegation() [DSPy-first]                     │
│     ├── claude_code → delegate_to_claude_code()             │
│     ├── rlm → execute_with_rlm_delegate()                   │
│     └── local_tools → execute_with_tools()                  │
│                                                              │
│  6. Legacy Fallback Rules (if DSPy confidence low)          │
│     ├── complexity >= High → delegate                       │
│     └── tokens > 100k → RLM                                 │
└─────────────────────────────────────────────────────────────┘
```

## Execution Priority

```
┌─────────────────────────────────────────────────────────────┐
│                    Execution Priority                        │
├─────────────────────────────────────────────────────────────┤
│  1. Claude CLI detected?  ──YES──►  ClaudeExecutor          │
│           │                         (Pro/Max subscription)   │
│           NO                                                 │
│           ▼                                                  │
│  2. CEREBRAS_API_KEY set? ──YES──►  TieredExecutor          │
│           │                         (GLM 4.7 + Qwen-3-32B)   │
│           NO                                                 │
│           ▼                                                  │
│  3. Fall back to analysis-only mode                         │
└─────────────────────────────────────────────────────────────┘
```

## Components

### Task

```rust
pub struct Task {
    pub id: String,          // e.g., "#123" for issues
    pub title: String,
    pub description: String,
    pub files: Vec<PathBuf>, // Optional hints
    pub acceptance_criteria: Vec<String>,
}
```

### Complexity Levels

| Level | Characteristics | Strategy |
|-------|----------------|----------|
| `Low` | Single file, simple edit | TieredExecutor |
| `Medium` | Multi-file, moderate scope | TieredExecutor |
| `High` | Complex refactoring, many files | TieredExecutor or delegate |
| `VeryHigh` | Architectural changes, 30+ files | Claude Code delegation |

### Tools

Adjutant uses these core tools:

| Tool | Description |
|------|-------------|
| `Read` | Read file contents |
| `Edit` | Replace old_string with new_string |
| `Write` | Create new files |
| `Bash` | Execute shell commands |
| `Glob` | Find files by pattern |
| `Grep` | Search file contents (uses ripgrep if available) |

## Tiered Inference

Adjutant uses a two-tier model architecture for cost-effective execution:

```
┌────────────────────────────────────────────────────────────┐
│                    PLANNING LAYER                          │
│              Cerebras GLM 4.7 (smart, $2.25/M)             │
│  "Break this task into subtasks, decide what to do"        │
└─────────────────────────┬──────────────────────────────────┘
                          │
          ┌───────────────┼───────────────┐
          ▼               ▼               ▼
┌─────────────┐   ┌─────────────┐   ┌─────────────┐
│  SUBTASK 1  │   │  SUBTASK 2  │   │  SUBTASK 3  │
│  qwen-3-32b │   │  qwen-3-32b │   │  qwen-3-32b │
│  ($0.15/M)  │   │  ($0.15/M)  │   │  ($0.15/M)  │
└─────────────┘   └─────────────┘   └─────────────┘
```

- **GLM 4.7**: Smart model for planning/orchestration and synthesis
- **Qwen-3-32B**: Cheaper model for subtask execution

See [TIERED-EXECUTOR.md](./TIERED-EXECUTOR.md) for detailed documentation.

## DSPy Integration

Adjutant integrates with [dsrs](../../dsrs/) (Rust DSPy implementation) for optimizable prompt engineering:

```
┌─────────────────────────────────────────────────────────────┐
│                      TieredExecutor                          │
│                                                              │
│  ExecutionMode::Gateway ──► Original hardcoded prompts       │
│  ExecutionMode::Dsrs    ──► Optimizable DSPy signatures      │
└─────────────────────────────────────────────────────────────┘
```

### Key Features

- **Typed Signatures**: Replace string prompts with `#[Signature]` structs
- **Automatic Optimization**: MIPROv2 improves prompts from training data
- **Training Collection**: Records successful executions and decisions for optimization
- **Evaluation Metrics**: Quantitative assessment of output quality
- **Decision Pipelines**: DSPy-first routing for complexity, delegation, and RLM decisions
- **LM Caching**: Lazy initialization and caching of decision LM for efficiency

### Quick Start

```rust
use adjutant::{Task, TieredExecutor, ExecutionMode, ToolRegistry};

// Create DSPy-powered executor
let mut executor = TieredExecutor::with_mode(ExecutionMode::Dsrs)?;

// Execute with training collection
let result = executor.execute_dsrs(&task, &context, &mut tools).await?;
```

### Signatures

**Task Execution Signatures:**

| Signature | Replaces | Purpose |
|-----------|----------|---------|
| `SubtaskPlanningSignature` | `PLANNER_SYSTEM_PROMPT` | Break tasks into subtasks |
| `SubtaskExecutionSignature` | `EXECUTOR_SYSTEM_PROMPT` | Execute individual subtasks |
| `ResultSynthesisSignature` | `SYNTHESIZER_SYSTEM_PROMPT` | Synthesize final results |

**Decision Pipeline Signatures:**

| Signature | Purpose | Fallback |
|-----------|---------|----------|
| `ComplexityClassificationSignature` | Classify task complexity (Low/Medium/High/VeryHigh) | Rule-based heuristics |
| `DelegationDecisionSignature` | Decide if/where to delegate (claude_code/rlm/local_tools) | Complexity thresholds |
| `RlmTriggerSignature` | Decide if RLM should be used | Token count + keywords |

Decision pipelines use a 0.7 confidence threshold - below that, they fall back to legacy rule-based logic.

See [DSPY-INTEGRATION.md](./DSPY-INTEGRATION.md) for detailed documentation.

## Configuration

### Execution Backends

**Priority 1: Claude Pro/Max** (Recommended)

Install the Claude CLI to use your existing Claude subscription:
```bash
# Install Claude CLI (see https://claude.ai/claude-code)
# Then authenticate:
claude auth login
```

**Priority 2: Cerebras API**

Set the API key for tiered inference:
```bash
export CEREBRAS_API_KEY="csk-your-key-here"
```

### Environment Variables

| Variable | Required | Description |
|----------|----------|-------------|
| `CEREBRAS_API_KEY` | No | Cerebras API key for tiered inference |
| `PYLON_MNEMONIC` | No | BIP-39 mnemonic for Pylon Swarm inference |
| `ADJUTANT_ENABLE_RLM` | No | Enable RLM tools in Claude sessions (1/true) |
| `RLM_BACKEND` | No | RLM backend selection (claude) |

### LM Provider Priority

Adjutant auto-detects available LM providers in this priority order:

| Priority | Provider | Requirements | Use Case |
|----------|----------|--------------|----------|
| 1 | **LlamaCpp** | `llama-server` on :8080 | Local execution with GPT-OSS |
| 2 | **Claude SDK** | `claude` CLI installed | Pro/Max subscription |
| 3 | **Pylon Swarm** | `PYLON_MNEMONIC` env var | Distributed NIP-90 inference |
| 4 | **Cerebras** | `CEREBRAS_API_KEY` env var | Fast cloud inference |
| 5 | **Pylon Local** | Ollama on :11434 | Local Ollama fallback |

*Without any provider, Adjutant falls back to analysis-only mode.

### .env.local

```bash
# Store in .env.local (auto-loaded)
CEREBRAS_API_KEY="csk-your-key-here"
```

Get your Cerebras API key at: https://cloud.cerebras.ai

## Fallback Behavior

If Cerebras is not configured:
1. Adjutant still analyzes the task and finds relevant files
2. Returns summary with file count and estimated tokens
3. Suggests setting CEREBRAS_API_KEY for AI-powered execution

## Delegation

For very complex tasks, Adjutant delegates to Claude Code:

```rust
// When complexity >= VeryHigh or files > 20
if plan.complexity >= Complexity::High || plan.files.len() > 20 {
    return self.delegate_to_claude_code(task).await;
}

// When context is too large
if plan.estimated_tokens > 100_000 {
    return self.execute_with_rlm(task, &plan).await;
}
```

## Module Structure

```
crates/adjutant/
├── src/
│   ├── lib.rs           # Main Adjutant struct, LM caching, decision routing
│   ├── planner.rs       # Task analysis and planning
│   ├── executor.rs      # Task execution coordination
│   ├── claude_executor.rs # Claude Pro/Max execution via SDK
│   ├── rlm_agent.rs     # RLM custom agent definition
│   ├── tiered.rs        # TieredExecutor (Gateway + DSPy modes)
│   ├── delegate.rs      # Claude Code and RLM delegation
│   ├── tools.rs         # Tool registry (Read, Edit, Bash, etc.)
│   ├── auth.rs          # Claude CLI detection
│   ├── autopilot_loop.rs # Autonomous loop with session tracking
│   ├── cli/             # CLI commands
│   │   ├── mod.rs            # Command routing
│   │   ├── run.rs            # autopilot run
│   │   ├── status.rs         # autopilot status
│   │   ├── issue.rs          # autopilot issue
│   │   └── dspy.rs           # autopilot dspy (sessions, performance, auto-optimize)
│   ├── dspy/            # DSPy integration + self-improvement
│   │   ├── mod.rs            # Module exports
│   │   ├── lm_config.rs      # Multi-provider LM configuration
│   │   ├── module.rs         # AdjutantModule + task execution signatures
│   │   ├── decision_pipelines.rs # Decision routing signatures
│   │   ├── metrics.rs        # Evaluation metrics for MIPROv2
│   │   ├── training.rs       # Training data collection
│   │   ├── sessions.rs       # Session tracking (AutopilotSession, DecisionRecord)
│   │   ├── outcome_feedback.rs # Links outcomes to decision correctness
│   │   ├── performance.rs    # Rolling accuracy tracking per signature
│   │   └── auto_optimizer.rs # Auto-triggers MIPROv2 optimization
│   └── bin/main.rs      # Autopilot binary entry point
└── docs/
    ├── README.md            # This file
    ├── TIERED-EXECUTOR.md   # Tiered inference details
    └── DSPY-INTEGRATION.md  # DSPy integration guide
```

## RLM Integration

Adjutant integrates with the RLM (Recursive Language Model) crate for complex analysis tasks:

### When RLM Mode is Used

RLM mode is automatically enabled when:
- Task complexity is `High` or higher
- Estimated tokens exceed 50,000
- Task description contains keywords: analyze, recursive, investigate, security, audit, review, deep dive, comprehensive

### RLM Custom Agent

Adjutant defines an `rlm-analyzer` custom agent for Claude:

```rust
// Read-only analysis agent
rlm_agent_definition() -> AgentDefinition

// Agent with write access (use with caution)
rlm_agent_with_write_access() -> AgentDefinition
```

The RLM agent follows the DECOMPOSE → EXECUTE → VERIFY → ITERATE → SYNTHESIZE pattern:
1. Breaks problems into verifiable sub-questions
2. Executes Python code to gather evidence
3. Verifies hypotheses against execution results
4. Iterates until confident answer is reached

### RLM MCP Tools

When executing with RLM support, Claude has access to:

| Tool | Description |
|------|-------------|
| `rlm_query` | Deep recursive analysis using prompt-execute loop |
| `rlm_fanout` | Distribute query across multiple workers (local/swarm/datacenter) |

Enable via environment variable:
```bash
export ADJUTANT_ENABLE_RLM=1
```

### Configuration

```bash
# Enable RLM tools in Claude sessions
export ADJUTANT_ENABLE_RLM=1

# Control RLM backend (optional)
export RLM_BACKEND=claude  # Use Claude as RLM LlmClient
```

## Coder Integration

Adjutant is the execution engine behind Coder's **Autopilot mode**. When you switch to Autopilot in Coder, it runs Adjutant in an **autonomous loop** until the task is complete:

```
┌─────────────────────────────────────────────────────────────┐
│                    Coder Autopilot Loop                      │
├─────────────────────────────────────────────────────────────┤
│  User: "Fix the auth bug"                                    │
│        ↓                                                     │
│  --- Iteration 1/10 ---                                      │
│  [Adjutant analyzes, identifies issue]                       │
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

### Key Features

- **Autonomous Loop**: Keeps calling Adjutant until task succeeds or max iterations (10) reached
- **Verification**: After LLM reports success, runs `cargo check` + `cargo test` to verify
- **Interrupt**: Press Escape to stop the loop cleanly
- **Iteration Context**: Each iteration gets context from previous attempts

See `crates/adjutant/src/autopilot_loop.rs` for the implementation.

## Self-Improving Autopilot

The autopilot system now includes autonomous self-improvement capabilities. After each session completes, the system:

1. **Records Session Data** - Tracks all decisions made (complexity, delegation, RLM)
2. **Labels Decisions** - Links task outcomes to decision correctness
3. **Updates Performance** - Maintains rolling accuracy per signature type
4. **Triggers Optimization** - Auto-triggers MIPROv2 when accuracy drops or examples accumulate

```
Task Execution → Session Recorded → Decisions Labeled → Performance Updated
                                                              ↓
                           Triggers Met? → Auto-Optimize → Better Decisions
```

### CLI Commands

```bash
# View recent sessions
autopilot dspy sessions
autopilot dspy sessions --failed  # Only failed sessions
autopilot dspy sessions --limit 20

# View performance metrics
autopilot dspy performance

# Configure auto-optimization
autopilot dspy auto-optimize --enable
autopilot dspy auto-optimize --min-examples 20
autopilot dspy auto-optimize --accuracy-threshold 0.7
```

### Auto-Optimization Triggers

Optimization is automatically triggered when:
- **Example Threshold**: 20+ new labeled examples accumulate (default)
- **Accuracy Drop**: Rolling accuracy drops below 70% (default)
- **Time-Based**: At least 24 hours since last optimization (default)

### Storage Layout

```
~/.openagents/adjutant/
├── training/
│   ├── dataset.json           # Training examples
│   └── labeled/               # Labeled examples with outcomes
│       ├── complexity.json
│       ├── delegation.json
│       └── rlm_trigger.json
├── sessions/
│   ├── index.json             # Session index
│   └── <year>/<month>/<id>.json
├── metrics/
│   └── performance.json       # Rolling accuracy data
└── config/
    └── auto_optimizer.json    # Auto-optimization settings
```

### Decision Correctness Logic

| Decision | Correct When |
|----------|--------------|
| Complexity | Task succeeded within expected iterations for that level |
| Delegation | Task succeeded (delegated failures are less "wrong") |
| RLM Trigger | Task succeeded; or large context + RLM used |

See [DSPY-INTEGRATION.md](./DSPY-INTEGRATION.md) for detailed documentation.

## See Also

- [TIERED-EXECUTOR.md](./TIERED-EXECUTOR.md) - Detailed tiered inference documentation
- [DSPY-INTEGRATION.md](./DSPY-INTEGRATION.md) - DSPy integration and optimization guide
- [../../dsrs/README.md](../../dsrs/README.md) - dsrs (Rust DSPy) documentation
- [../../gateway/docs/README.md](../../gateway/docs/README.md) - Gateway crate (Cerebras integration)
- [../../oanix/README.md](../../oanix/README.md) - OANIX environment discovery
- [../../coder/docs/ROADMAP.md](../../coder/docs/ROADMAP.md) - Coder implementation roadmap
