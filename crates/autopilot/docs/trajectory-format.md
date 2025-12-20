# Trajectory Format Documentation

Autopilot records complete execution traces in two formats:
1. **JSON** - Structured data format for programmatic analysis
2. **rlog** - Human-readable text format for viewing and debugging

## JSON Format

### Trajectory Structure

```json
{
  "session_id": "unique-session-identifier",
  "prompt": "The initial user prompt",
  "model": "claude-sonnet-4",
  "cwd": "/path/to/working/directory",
  "repo_sha": "git-commit-hash",
  "branch": "main",
  "started_at": "2025-12-20T10:38:00Z",
  "ended_at": "2025-12-20T10:42:00Z",
  "steps": [...],
  "result": {...},
  "usage": {...}
}
```

### Top-Level Fields

- **session_id** (string): Unique identifier for this execution
- **prompt** (string): The initial user prompt that started the task
- **model** (string): AI model used (e.g., "claude-sonnet-4")
- **cwd** (string): Working directory path
- **repo_sha** (string): Git commit SHA of the repository state
- **branch** (string, optional): Git branch name
- **started_at** (ISO 8601 datetime): When the execution started
- **ended_at** (ISO 8601 datetime, optional): When the execution completed
- **steps** (array): Ordered list of execution steps
- **result** (object, optional): Final execution result
- **usage** (object): Token usage statistics

### Step Types

Each step in the `steps` array has this structure:

```json
{
  "step_id": 1,
  "timestamp": "2025-12-20T10:38:05Z",
  "type": "step_type_here",
  "tokens_in": 100,
  "tokens_out": 50,
  "tokens_cached": 20
}
```

#### Step Type: `user`

User message or input:

```json
{
  "type": "user",
  "content": "Please fix the bug in main.rs"
}
```

#### Step Type: `assistant`

Assistant text response:

```json
{
  "type": "assistant",
  "content": "I'll help you fix that bug."
}
```

#### Step Type: `thinking`

Extended thinking/reasoning block:

```json
{
  "type": "thinking",
  "content": "Let me analyze the error...",
  "signature": "optional-signature-string"
}
```

#### Step Type: `tool_call`

Tool invocation:

```json
{
  "type": "tool_call",
  "tool": "Read",
  "tool_id": "toolu_abc123",
  "input": {
    "file_path": "/path/to/file.rs"
  }
}
```

#### Step Type: `tool_result`

Tool execution result:

```json
{
  "type": "tool_result",
  "tool_id": "toolu_abc123",
  "success": true,
  "output": "File contents here..."
}
```

#### Step Type: `system_init`

System initialization:

```json
{
  "type": "system_init",
  "model": "claude-sonnet-4"
}
```

#### Step Type: `system_status`

System status update:

```json
{
  "type": "system_status",
  "status": "Processing complete"
}
```

### Token Usage

```json
{
  "input_tokens": 1500,
  "output_tokens": 800,
  "cache_read_tokens": 200,
  "cache_creation_tokens": 100,
  "cost_usd": 0.025
}
```

### Trajectory Result

```json
{
  "success": true,
  "duration_ms": 45000,
  "num_turns": 8,
  "result_text": "Task completed successfully",
  "errors": [],
  "issues_completed": 2
}
```

- **success** (boolean): Whether the task completed successfully
- **duration_ms** (integer): Total execution time in milliseconds
- **num_turns** (integer): Number of conversation turns
- **result_text** (string, optional): Final result message
- **errors** (array): List of error messages if any
- **issues_completed** (integer): Number of issues completed (autopilot mode)

## rlog Format

The rlog (run log) format is a human-readable text format with the following structure:

### Header Section

```
---
format: rlog/1
id: session-id
repo_sha: abc123def
branch: main
model: claude-sonnet-4
cwd: /path/to/dir
agent: autopilot
version: 0.1.0
tokens_total_in: 1500
tokens_total_out: 800
tokens_cached: 200
---

>>> [abc123de] 2025-12-20 10:38:00 UTC
```

### Body Section

Each step is prefixed with a marker:

- `u:` - User message
- `a:` - Assistant message
- `t:` - Thinking block
- `tc:` - Tool call
- `tr:` - Tool result
- `si:` - System init
- `ss:` - System status

Example:

```
u: Fix the bug in main.rs

a: I'll help you fix that bug.

tc: Read file_path=/path/to/main.rs

tr: [SUCCESS] File contents...

a: I found the issue. Let me fix it.

tc: Edit file_path=/path/to/main.rs
```

### Footer Section

```
<<< [abc123de] 2025-12-20 10:42:00 UTC

=== Summary ===
Status: SUCCESS
Duration: 4m 0s
Turns: 8
Cost: $0.025
Input tokens: 1500
Output tokens: 800
Cached tokens: 200
Issues completed: 2
```

## Parsing Trajectories

### Reading JSON

```rust
use autopilot::trajectory::Trajectory;
use std::fs;

let json = fs::read_to_string("trajectory.json")?;
let traj: Trajectory = serde_json::from_str(&json)?;

println!("Session: {}", traj.session_id);
println!("Steps: {}", traj.steps.len());
```

### Reading rlog

```rust
use autopilot::extract_session_id_from_rlog;

let session_id = extract_session_id_from_rlog("trajectory.rlog")?;
```

## Example: Complete Trajectory

```json
{
  "session_id": "a1b2c3d4",
  "prompt": "Fix clippy warnings in main.rs",
  "model": "claude-sonnet-4",
  "cwd": "/home/user/project",
  "repo_sha": "abc123",
  "branch": "main",
  "started_at": "2025-12-20T10:00:00Z",
  "ended_at": "2025-12-20T10:05:00Z",
  "steps": [
    {
      "step_id": 1,
      "timestamp": "2025-12-20T10:00:00Z",
      "type": "system_init",
      "model": "claude-sonnet-4"
    },
    {
      "step_id": 2,
      "timestamp": "2025-12-20T10:00:01Z",
      "type": "user",
      "content": "Fix clippy warnings in main.rs"
    },
    {
      "step_id": 3,
      "timestamp": "2025-12-20T10:00:02Z",
      "type": "assistant",
      "content": "I'll help fix the clippy warnings.",
      "tokens_in": 50,
      "tokens_out": 20
    },
    {
      "step_id": 4,
      "timestamp": "2025-12-20T10:00:03Z",
      "type": "tool_call",
      "tool": "Read",
      "tool_id": "toolu_1",
      "input": {
        "file_path": "/home/user/project/src/main.rs"
      }
    },
    {
      "step_id": 5,
      "timestamp": "2025-12-20T10:00:04Z",
      "type": "tool_result",
      "tool_id": "toolu_1",
      "success": true,
      "output": "fn main() {...}"
    }
  ],
  "result": {
    "success": true,
    "duration_ms": 300000,
    "num_turns": 3,
    "result_text": "Fixed 5 clippy warnings",
    "errors": [],
    "issues_completed": 1
  },
  "usage": {
    "input_tokens": 500,
    "output_tokens": 250,
    "cache_read_tokens": 100,
    "cache_creation_tokens": 50,
    "cost_usd": 0.012
  }
}
```

## File Locations

Trajectories are saved by default to:

- **JSON**: `docs/logs/YYYYMMDD/HHMMSS-slug.json`
- **rlog**: `docs/logs/YYYYMMDD/HHMMSS-slug.rlog`

Where:
- `YYYYMMDD` is the date (e.g., `20251220`)
- `HHMMSS` is the time (e.g., `103800`)
- `slug` is generated from the prompt (e.g., `fix-clippy-warnings`)

## Use Cases

### Analyzing Performance

```rust
let traj: Trajectory = serde_json::from_str(&json)?;

println!("Total cost: ${:.3}", traj.usage.cost_usd);
println!("Duration: {}ms", traj.result.as_ref().unwrap().duration_ms);
println!("Tokens/dollar: {:.0}",
    (traj.usage.input_tokens + traj.usage.output_tokens) as f64
    / traj.usage.cost_usd
);
```

### Counting Tool Usage

```rust
let tool_calls: Vec<_> = traj.steps.iter()
    .filter_map(|s| match &s.step_type {
        StepType::ToolCall { tool, .. } => Some(tool.as_str()),
        _ => None
    })
    .collect();

println!("Tools used: {:?}", tool_calls);
```

### Extracting Errors

```rust
if let Some(result) = &traj.result {
    if !result.success {
        println!("Errors:");
        for error in &result.errors {
            println!("  - {}", error);
        }
    }
}
```
