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

# Run a task
cargo autopilot run "Add error handling to auth.rs"

# Claim and work on a GitHub issue
cargo autopilot issue claim 123
```

## Execution Flow

1. **Planning**: Analyzes task, finds relevant files, estimates complexity
2. **Routing**: Decides execution strategy based on complexity:
   - Low/Medium: Use local executor
   - High: May still use local executor or delegate
   - VeryHigh: Delegate to Claude Code
   - Large context: Use RLM for distributed processing
3. **Execution**: Runs the task using chosen strategy (priority order):
   - **Claude Pro/Max** (if `claude` CLI is installed) - Best quality, uses subscription
   - **Cerebras TieredExecutor** (if CEREBRAS_API_KEY set) - Cost-effective tiered inference
   - **Analysis-only** - Returns file analysis without making changes
4. **Synthesis**: Summarizes results, optionally commits changes

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
| `CEREBRAS_API_KEY` | No* | Cerebras API key for tiered inference |

*Without Claude CLI or CEREBRAS_API_KEY, Adjutant falls back to analysis-only mode.

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
│   ├── lib.rs           # Main Adjutant struct, public exports
│   ├── planner.rs       # Task analysis and planning
│   ├── executor.rs      # Task execution coordination
│   ├── claude_executor.rs # Claude Pro/Max execution via SDK
│   ├── rlm_agent.rs     # RLM custom agent definition
│   ├── tiered.rs        # TieredExecutor (GLM 4.7 + Qwen-3-32B)
│   ├── delegate.rs      # Claude Code and RLM delegation
│   ├── tools.rs         # Tool registry (Read, Edit, Bash, etc.)
│   ├── cli.rs           # CLI argument parsing
│   └── bin/main.rs      # Autopilot binary entry point
└── docs/
    ├── README.md        # This file
    └── TIERED-EXECUTOR.md
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

## See Also

- [TIERED-EXECUTOR.md](./TIERED-EXECUTOR.md) - Detailed tiered inference documentation
- [../../gateway/docs/README.md](../../gateway/docs/README.md) - Gateway crate (Cerebras integration)
- [../../oanix/README.md](../../oanix/README.md) - OANIX environment discovery
