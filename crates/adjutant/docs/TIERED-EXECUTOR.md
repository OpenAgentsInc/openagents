# Tiered Executor

The TieredExecutor uses a two-model architecture for cost-effective AI-powered task execution.

> **Note:** TieredExecutor is now the **fallback** when Codex CLI is not available.
> If the `codex` CLI is installed (Pro/Max subscription), Adjutant uses `CodexExecutor` instead.
> See [README.md](./README.md) for the full execution priority.

## Execution Modes

TieredExecutor supports two execution modes:

| Mode | Description | Use Case |
|------|-------------|----------|
| `Gateway` | Original hardcoded prompts via CerebrasGateway | Default, stable |
| `Dsrs` | DSPy-powered with typed signatures and training | Optimization, A/B testing |

```rust
// Default: Gateway mode (hardcoded prompts)
let executor = TieredExecutor::new()?;

// DSPy mode (optimizable signatures)
let executor = TieredExecutor::with_mode(ExecutionMode::Dsrs)?;
```

See [DSPY-INTEGRATION.md](./DSPY-INTEGRATION.md) for full DSPy documentation.

## Architecture

```
┌────────────────────────────────────────────────────────────┐
│                    PHASE 1: PLANNING                       │
│              Cerebras GLM 4.7 (smart model)                │
│  - Analyzes task and context                               │
│  - Breaks task into atomic subtasks                        │
│  - Determines action types and targets                     │
└─────────────────────────┬──────────────────────────────────┘
                          │
                          ▼
┌────────────────────────────────────────────────────────────┐
│                    PHASE 2: EXECUTION                      │
│              Cerebras Qwen-3-32B (cheaper model)           │
│  - Executes each subtask sequentially                      │
│  - Generates edit instructions                             │
│  - Applies changes via tool registry                       │
└─────────────────────────┬──────────────────────────────────┘
                          │
                          ▼
┌────────────────────────────────────────────────────────────┐
│                    PHASE 3: SYNTHESIS                      │
│              Cerebras GLM 4.7 (smart model)                │
│  - Reviews subtask results                                 │
│  - Determines overall success/failure                      │
│  - Generates summary                                       │
└────────────────────────────────────────────────────────────┘
```

## Models

| Model | Role | Pricing | Context | Why |
|-------|------|---------|---------|-----|
| `zai-glm-4.7` | Planning & Synthesis | $2.25/M in, $2.75/M out | 131k | Smart reasoning for orchestration |
| `qwen-3-32b` | Subtask Execution | $0.15/M in, $0.30/M out | 128k | Cheaper for routine work |

## Subtask Structure

```rust
pub struct Subtask {
    pub id: String,        // e.g., "1", "2", "3"
    pub action: String,    // "read", "edit", "bash"
    pub target: String,    // File path or empty
    pub instruction: String, // What to do
}
```

### Actions

| Action | Description | Tool Used |
|--------|-------------|-----------|
| `read` | Read and analyze a file | `ToolRegistry::read()` |
| `edit` | Modify a file (old_string → new_string) | `ToolRegistry::edit()` |
| `bash` | Execute a shell command | `ToolRegistry::bash()` |

## Execution Flow

### Phase 1: Planning

The planner receives the task and context (relevant file contents) and outputs a JSON plan:

```json
{
  "subtasks": [
    {"id": "1", "action": "read", "target": "src/auth.rs", "instruction": "Understand current error handling"},
    {"id": "2", "action": "edit", "target": "src/auth.rs", "instruction": "Add Result type to login function"},
    {"id": "3", "action": "bash", "target": "", "instruction": "Run cargo test to verify changes"}
  ]
}
```

**Planner System Prompt:**
```
You are a task planner. Break the given task into concrete, atomic subtasks.
- Keep subtasks atomic and focused
- Order subtasks logically (read before edit, edit before test)
- Use specific file paths from the context
- Maximum 5 subtasks per task
```

### Phase 2: Execution

Each subtask is executed sequentially. For edit actions, the executor:

1. Reads current file content
2. Sends instruction to Qwen-3-32B
3. Parses response for old_string/new_string
4. Applies edit via ToolRegistry

**Executor Output Format (for edits):**
```json
{
  "old_string": "fn login(user: &str)",
  "new_string": "fn login(user: &str) -> Result<Token, AuthError>"
}
```

Alternative format (markers):
```
<<<OLD>>>
fn login(user: &str)
<<<NEW>>>
fn login(user: &str) -> Result<Token, AuthError>
<<<END>>>
```

### Phase 3: Synthesis

The synthesizer reviews all subtask results and generates the final TaskResult:

```json
{
  "success": true,
  "summary": "Added error handling to auth module. Modified login() and logout() functions.",
  "modified_files": ["src/auth.rs"]
}
```

## Cost Estimate

For a typical task with 5 subtasks:

| Phase | Model | Tokens | Cost |
|-------|-------|--------|------|
| Planning | GLM 4.7 | ~3k in, ~1k out | ~$0.01 |
| Subtasks (5x) | Qwen-3-32B | ~10k each | ~$0.01 |
| Synthesis | GLM 4.7 | ~2k in, ~500 out | ~$0.005 |
| **Total** | | | **~$0.025** |

Comparison:
- Using GLM 4.7 for everything: ~$0.05
- Using Codex: ~$0.15

## Usage

### Programmatic

```rust
use adjutant::{Task, TieredExecutor, ToolRegistry};

// Create executor
let executor = TieredExecutor::new()?;

// Create task
let task = Task::new(
    "#123",
    "Add error handling",
    "Add Result types to auth module functions"
);

// Build context from relevant files
let context = "--- src/auth.rs ---\nfn login() { ... }";

// Execute
let mut tools = ToolRegistry::new(&workspace_root);
let result = executor.execute(&task, &context, &mut tools).await?;

println!("Success: {}", result.success);
println!("Summary: {}", result.summary);
```

### CLI

```bash
# Set API key
export CEREBRAS_API_KEY="csk-your-key-here"

# Run task
cargo run -p autopilot -- run "Add error handling to auth.rs"
```

## Error Handling

### Subtask Failures

If a subtask fails:
- The result is marked with `success: false`
- Error message is captured
- Execution continues to next subtask
- Synthesis phase reports partial completion

### Parse Failures

If the LLM output can't be parsed:
- For plans: Returns empty subtask list
- For edits: Returns subtask failure with parse error
- For bash: Falls back to first line of output as command

### Gateway Failures

If Cerebras is unavailable:
- `TieredExecutor::new()` returns error
- Caller (executor.rs) falls back to analysis-only mode
- Returns summary without making changes

## Configuration

### Environment

```bash
# Required
export CEREBRAS_API_KEY="csk-your-key-here"

# Optional (defaults shown)
export CEREBRAS_ENDPOINT="https://api.cerebras.ai/v1"
```

### Model Constants

```rust
// In tiered.rs
const PLANNING_MODEL: &str = "zai-glm-4.7";    // Smart, for planning
const EXECUTION_MODEL: &str = "qwen-3-32b";    // Cheaper, for subtasks
```

## Limitations

1. **Sequential Execution**: Subtasks run sequentially (parallel would need careful tool state handling)
2. **Context Size**: Large codebases may exceed context limits
3. **Edit Precision**: old_string must match exactly (including whitespace)
4. **Max Subtasks**: Planner is instructed to limit to 5 subtasks per task

## DSPy Execution Path

When using `ExecutionMode::Dsrs`, the TieredExecutor uses typed DSPy signatures instead of hardcoded prompts:

```
┌────────────────────────────────────────────────────────────┐
│                execute_dsrs() Flow                          │
├────────────────────────────────────────────────────────────┤
│  1. AdjutantModule.plan()                                   │
│     └── SubtaskPlanningSignature via dsrs Predict           │
│                                                              │
│  2. For each subtask:                                        │
│     └── AdjutantModule.execute_subtask()                     │
│         └── SubtaskExecutionSignature via dsrs Predict       │
│     └── apply_dsrs_action() → ToolRegistry                   │
│                                                              │
│  3. AdjutantModule.synthesize()                              │
│     └── ResultSynthesisSignature via dsrs Predict            │
│                                                              │
│  4. TrainingCollector records successful executions          │
│     └── Stored at ~/.openagents/adjutant/training/           │
└────────────────────────────────────────────────────────────┘
```

### Benefits of DSPy Mode

| Aspect | Gateway Mode | DSPy Mode |
|--------|--------------|-----------|
| Prompts | Hardcoded `const` strings | Typed `#[Signature]` structs |
| Optimization | Manual edit-test cycle | Automatic via MIPROv2 |
| Training | None | Records successes for optimization |
| Metrics | None | Built-in evaluation (0.0-1.0) |
| Validation | Runtime parsing | Compile-time type checking |

### Usage

```rust
use adjutant::{Task, TieredExecutor, ExecutionMode, ToolRegistry};

// Create DSPy-powered executor
let mut executor = TieredExecutor::with_mode(ExecutionMode::Dsrs)?;

// Execute (training data automatically collected)
let result = executor.execute_dsrs(&task, &context, &mut tools).await?;
```

## Future Improvements

- Parallel subtask execution for independent actions
- Streaming responses for real-time progress
- Retry logic for transient failures
- Model selection based on subtask complexity
- A/B testing between Gateway and DSPy modes
- Automatic MIPROv2 optimization pipeline

## See Also

- [README.md](./README.md) - Adjutant overview
- [DSPY-INTEGRATION.md](./DSPY-INTEGRATION.md) - Full DSPy integration guide
- [../../dsrs/README.md](../../dsrs/README.md) - dsrs (Rust DSPy) documentation
- [../../gateway/docs/PROVIDERS.md](../../gateway/docs/PROVIDERS.md) - Cerebras model details
