# Harbor Integration: TB2 Submission Ready

## Goal

Get fully ready for Terminal-Bench v2 official submission by:
1. Adding streaming support to `tbench` for real-time UI
2. GYM screen uses `tbench` (not raw DockerRunner)
3. ATIF trajectories saved to git-committed `results/trajectories/`
4. CLI convenience script for manual testing
5. Keep DockerRunner as backup fallback

## Current State

| Component | Uses | Streaming | ATIF Output |
|-----------|------|-----------|-------------|
| GYM Screen | DockerRunner | ✅ mpsc events | ❌ SQLite only |
| tbench | Claude --json | ❌ Final only | ✅ trajectory.json |
| Manual CLI | Raw claude | ✅ stream-json | ❌ None |

## Architecture After

```
┌─────────────────────────────────────────────────────────────┐
│                        GYM Screen                           │
│                                                             │
│  ┌─────────────┐    ┌──────────────────────────────────┐   │
│  │ Task Panel  │───▶│           tbench                 │   │
│  └─────────────┘    │  --stream (stdout JSON events)   │   │
│                     │  --output-dir (ATIF files)       │   │
│  ┌─────────────┐    └──────────────────────────────────┘   │
│  │ Log Panel   │◀── stdout: streaming DockerEvent JSON     │
│  └─────────────┘                                           │
│                     Output files:                          │
│                     ├── trajectory.json (ATIF v1.4)        │
│                     ├── events.jsonl (event log)           │
│                     └── metrics.json (summary)             │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
              results/trajectories/{task-id}/{session-id}/
```

---

## Implementation Steps

### Step 1: Add Streaming to tbench

**File:** `crates/harbor/src/bin/tbench.rs`

Changes:
1. Add `--stream` CLI flag
2. Switch from `--output-format json` to `--output-format stream-json`
3. Parse each line and emit to stdout as JSON event (when --stream)
4. Still write events.jsonl, trajectory.json, metrics.json at end

```rust
// New arg
#[arg(long)]
stream: bool,  // Emit JSON events to stdout for UI consumption

// Change Claude invocation
let output_format = if args.stream { "stream-json" } else { "json" };
```

Event format (stdout when --stream):
```json
{"type":"assistant","turn":1,"text":"I'll analyze..."}
{"type":"tool_use","tool":"Bash","id":"call_123"}
{"type":"tool_result","id":"call_123","output":"..."}
{"type":"complete","success":true,"turns":5,"cost":0.05}
```

### Step 2: Add ATIF File Writer to tbench

**File:** `crates/harbor/src/bin/tbench.rs`

Currently tbench uses TrajectoryBuilder but doesn't capture all steps. Enhance to:
1. Parse each stream-json line into ATIF Step
2. Include tool_calls and observations
3. Write complete trajectory at end

### Step 3: GYM Spawns tbench Instead of DockerRunner

**File:** `crates/gym/src/mechacoder/mod.rs`

Replace DockerRunner spawn with tbench spawn:

```rust
// Before (lines 793-1072)
let runner = DockerRunner::new();
runner.run_claude(config, event_tx, abort_rx).await

// After
let output_dir = format!("results/trajectories/{}/{}", task.id, session_id);
let child = Command::new("tbench")
    .args([
        "--instruction", &task.instruction,
        "--output-dir", &output_dir,
        "--stream",
        "--timeout", &timeout.to_string(),
        "--cwd", workspace_dir,
    ])
    .stdout(Stdio::piped())
    .spawn();

// Read stdout line by line, parse JSON, emit DockerEvent
```

Keep DockerRunner code but gate behind `use_legacy_runner` flag for fallback.

### Step 4: Trajectory Storage Location

**New directory:** `results/trajectories/`

Structure:
```
results/trajectories/
├── .gitignore              # Ignore large files, keep structure
├── regex-log/
│   ├── 20251211-153500-abc123/
│   │   ├── trajectory.json
│   │   ├── events.jsonl
│   │   └── metrics.json
│   └── 20251211-160000-def456/
│       └── ...
└── another-task/
    └── ...
```

Add to git:
- `results/trajectories/.gitignore` (ignore *.jsonl over 1MB)
- `results/trajectories/README.md` (format docs)

### Step 5: CLI Convenience Script

**New file:** `scripts/tb2-run.sh`

```bash
#!/bin/bash
# Run a TB2 task with tbench and save ATIF trajectories
#
# Usage: ./scripts/tb2-run.sh regex-log [--model claude-opus-4-5-20251101]

TASK_ID="${1:-regex-log}"
MODEL="${2:-}"
TIMESTAMP=$(date +%Y%m%d-%H%M%S)
SESSION_ID="${TIMESTAMP}-$(openssl rand -hex 4)"
OUTPUT_DIR="results/trajectories/${TASK_ID}/${SESSION_ID}"

# Load task from TB2
TB2_ROOT="${TB2_ROOT:-$HOME/code/terminal-bench-2}"
INSTRUCTION=$(cat "${TB2_ROOT}/${TASK_ID}/instruction.md")

# Create workspace
WORKSPACE=$(mktemp -d)
mkdir -p "${OUTPUT_DIR}"

# Run tbench
tbench \
  --instruction "${INSTRUCTION}" \
  --output-dir "${OUTPUT_DIR}" \
  --cwd "${WORKSPACE}" \
  --stream \
  --timeout 900

# Run TB2 verification
docker run --rm \
  -v "${WORKSPACE}:/app" \
  -v "${OUTPUT_DIR}:/logs" \
  -v "${TB2_ROOT}/${TASK_ID}/tests:/tests:ro" \
  "$(grep docker_image ${TB2_ROOT}/${TASK_ID}/task.toml | cut -d'"' -f2)" \
  bash /tests/test.sh

# Show result
cat "${OUTPUT_DIR}/../verifier/reward.txt"
echo "Trajectories saved to: ${OUTPUT_DIR}"
```

### Step 6: Keep DockerRunner as Fallback

**File:** `crates/gym/src/mechacoder/mod.rs`

Add config option:
```rust
pub struct MechaConfig {
    /// Use legacy DockerRunner instead of tbench
    pub use_legacy_runner: bool,
}
```

GYM UI toggle in TaskPanel to switch between tbench and legacy.

---

## Files to Modify

| File | Change |
|------|--------|
| `crates/harbor/src/bin/tbench.rs` | Add --stream, use stream-json, emit events to stdout |
| `crates/harbor/src/lib.rs` | Add streaming event types if needed |
| `crates/gym/src/mechacoder/mod.rs` | Spawn tbench instead of DockerRunner |
| `crates/gym/src/mechacoder/types.rs` | Add MechaConfig with use_legacy_runner |

## Files to Create

| File | Purpose |
|------|---------|
| `scripts/tb2-run.sh` | CLI convenience for manual TB2 runs |
| `results/trajectories/README.md` | Document ATIF storage format |
| `results/trajectories/.gitignore` | Ignore large event logs |

---

## Success Criteria

1. `tbench --stream` emits real-time JSON events to stdout
2. GYM Log Panel shows streaming output from tbench
3. Every TB2 run produces trajectory.json in `results/trajectories/`
4. `./scripts/tb2-run.sh regex-log` works end-to-end
5. Legacy DockerRunner still available as fallback toggle

---

## Testing Plan

1. Build tbench: `cargo build -p harbor --release`
2. Test streaming: `tbench --instruction "echo hello" --output-dir /tmp/test --stream`
3. Verify ATIF: `cat /tmp/test/trajectory.json | jq .schema_version`
4. Run from GYM: Start task, verify streaming in Log Panel
5. Check git storage: `ls results/trajectories/regex-log/`


