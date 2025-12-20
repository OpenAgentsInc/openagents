# Resume Autopilot Session

## Goal
Add ability to resume an autopilot session after a crash by pointing at an rlog/json file.

## Key Discovery
The SDK already has resume support built-in:
- `options.resume = Some(session_id)` → maps to `--resume <ID>`
- `options.continue_session = true` → maps to `--continue` (most recent session)
- `unstable_v2_resume_session(session_id, options)` helper function

## Implementation

### 1. Add Resume Command to CLI

**File:** `crates/autopilot/src/main.rs`

Add new command variant:
```rust
Commands {
    Run { ... },
    Replay { ... },
    Compare { ... },
    Resume {
        /// Path to .json or .rlog trajectory file
        #[arg(required_unless_present = "continue_last")]
        trajectory: Option<PathBuf>,

        /// Continue most recent session (no file needed)
        #[arg(long, short = 'c')]
        continue_last: bool,

        /// Working directory (default: from trajectory or current)
        #[arg(short, long)]
        cwd: Option<PathBuf>,

        /// Additional prompt to append on resume
        #[arg(short, long)]
        prompt: Option<String>,

        /// Remaining budget (auto-calculated if not provided)
        #[arg(long)]
        max_budget: Option<f64>,

        /// Enable issue tracking tools via MCP
        #[arg(long)]
        with_issues: bool,
    },
    Issue { ... },
}
```

### 2. Add Session ID Extraction

**File:** `crates/autopilot/src/lib.rs` (or new `resume.rs`)

```rust
/// Extract session_id from a trajectory JSON file
pub fn extract_session_id_from_json(path: &Path) -> Result<String> {
    let content = std::fs::read_to_string(path)?;
    let traj: Trajectory = serde_json::from_str(&content)?;
    if traj.session_id.is_empty() {
        anyhow::bail!("No session_id in trajectory file");
    }
    Ok(traj.session_id)
}

/// Extract session_id from rlog header (if present)
pub fn extract_session_id_from_rlog(path: &Path) -> Result<Option<String>> {
    // Parse YAML header between --- markers
    // Return id field if present and non-empty
}
```

### 3. Implement Resume Task Function

**File:** `crates/autopilot/src/main.rs`

```rust
async fn resume_task(
    trajectory: Option<PathBuf>,
    continue_last: bool,
    cwd: Option<PathBuf>,
    prompt: Option<String>,
    max_budget: Option<f64>,
    with_issues: bool,
) -> Result<()> {
    // 1. Get session_id
    let session_id = if continue_last {
        None // SDK will use --continue
    } else {
        let path = trajectory.expect("trajectory required");
        Some(if path.extension() == Some("json") {
            extract_session_id_from_json(&path)?
        } else {
            // Try rlog, fallback to continue
            extract_session_id_from_rlog(&path)?.ok_or_else(|| {
                anyhow::anyhow!("No session_id in rlog, use --continue-last")
            })?
        })
    };

    // 2. Build QueryOptions with resume
    let mut options = QueryOptions::new()
        .max_budget_usd(max_budget.unwrap_or(5.0))
        .cwd(&cwd.unwrap_or_else(|| std::env::current_dir().unwrap()))
        .dangerously_skip_permissions(true);

    if let Some(id) = session_id {
        options.resume = Some(id);
    } else {
        options.continue_session = true;
    }

    // 3. Create session and send optional prompt
    let mut session = unstable_v2_create_session(options).await?;

    if let Some(p) = prompt {
        session.send(&p).await?;
    }

    // 4. Process messages (similar to run_task)
    while let Some(msg) = session.receive().next().await {
        // ... process and collect
    }

    Ok(())
}
```

### 4. Update Imports

Add to `main.rs`:
```rust
use claude_agent_sdk::unstable_v2_create_session;
```

## Files to Modify

1. `crates/autopilot/src/main.rs` - Add Resume command and handler
2. `crates/autopilot/src/lib.rs` - Add session_id extraction helpers

## Usage Examples

```bash
# Resume from JSON file (has session_id)
cargo autopilot resume docs/logs/20251219/2138-start-working.json

# Continue most recent session (for crashes without JSON)
cargo autopilot resume --continue-last

# Resume with additional instructions
cargo autopilot resume 2138-start-working.json --prompt "Continue from where you left off"

# Resume with custom budget
cargo autopilot resume 2138-start-working.json --max-budget 10.0
```
