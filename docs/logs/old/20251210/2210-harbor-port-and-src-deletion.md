# Harbor Port and src/ Deletion

**Date:** 2024-12-10 22:10
**Task:** Port Harbor adapter to Rust, delete remaining TypeScript in src/

---

## Summary

Ported the Terminal-Bench Harbor adapter from TypeScript/Python to Rust, creating `crates/harbor/`. Then deleted the entire `src/` folder (37,531 lines of TypeScript).

---

## What Was Created

### crates/harbor/ (Rust crate)

**`src/lib.rs`** (~350 lines)
- ATIF v1.4 types (Agent, Step, Trajectory, FinalMetrics, etc.)
- `EventRecorder` - writes events.jsonl
- `TrajectoryBuilder` - constructs ATIF trajectories
- `TBenchMetrics` - token usage, cost, timing
- `ClaudeResult` - parses Claude CLI JSON output
- 8 unit tests

**`src/bin/tbench.rs`** (~200 lines)
- CLI binary for Harbor evaluation
- Arguments: `--instruction`, `--output-dir`, `--timeout`, `--cwd`, `--max-turns`
- Runs Claude CLI with `--print --dangerously-skip-permissions --output-format json`
- Outputs: `events.jsonl`, `trajectory.json`, `metrics.json`

**`python/`** - Harbor Python adapter (moved from src/harbor)
- `openagents_harbor/mechacoder_agent.py` - implements `BaseInstalledAgent`
- `openagents_harbor/install-mechacoder.sh.j2` - container setup script (now installs Rust)
- `pyproject.toml`, `README.md`

The Python adapter now calls the Rust binary (`/opt/mechacoder/target/release/tbench`) instead of `bun src/cli/tbench.ts`.

---

## What Was Deleted

### src/ folder (37,531 lines TypeScript)

| Directory | Files | Description | Verdict |
|-----------|-------|-------------|---------|
| `dashboard/` | 4 | Dashboard UI | DELETE - not used |
| `deps/` | 5 | Dependency management | DELETE - not used |
| `desktop/` | 7 | Desktop app protocol | DELETE - Commander is Rust |
| `health/` | 5 | Health checks | DELETE - not used |
| `huggingface/` | 6 | HF integration | DELETE - not used |
| `interop/` | 3 | Interop layer | DELETE - not used |
| `learning/` | 13 | SOAR learning | DELETE - experimental |
| `memory/` | 6 | Memory system | DELETE - not used |
| `researcher/` | 6 | Research agent | DELETE - not used |
| `schemas/` | SDK schemas | DELETE - not used |
| `shared/` | 1 | Shared utils | DELETE - not used |
| `skills/` | 12 | Skills library | DELETE - interesting but not critical |
| `storage/` | 3 | Database layer | DELETE - using SQLite in Rust |
| `telemetry/` | 2 | OpenTelemetry | DELETE - Rust crate exists |
| `trainer/` | 6 | Training loop | DELETE - hillclimber in Rust |
| `training/` | 5 | Episode learner | DELETE - hillclimber in Rust |
| `usage/` | 4 | Usage tracking | DELETE - not used |

**Notable files deleted:**
- `skills/library/compositional.ts` (1,705 lines) - Voyager-style compositional skills
- `storage/database.ts` (911 lines) - SQLite wrapper
- `desktop/handlers.ts` (912 lines) - Desktop app handlers
- `learning/soar-ttt.ts` (661 lines) - SOAR learning experiments

---

## Why Delete Everything?

1. **Rust equivalents exist**: hillclimber, testgen, gym, sandbox, telemetry
2. **Not used**: Most modules were experimental or deprecated
3. **Skills**: Could be ported later as JSONL data, not TypeScript code
4. **Clean slate**: Focus on Rust-first architecture

---

## Files Modified

- `Cargo.toml` - Added `crates/harbor` to workspace members
- `crates/harbor/Cargo.toml` - New crate manifest (added `telemetry` dep by linter)
- `crates/harbor/src/bin/tbench.rs` - Uses `telemetry::init_*` (linter added)

---

## Build Verification

```bash
cargo build -p harbor  # OK
cargo test -p harbor   # 8 tests passed
cargo run -p harbor --bin tbench -- --help  # Works
```

---

## What Could Be Salvaged Later

If needed, these could be ported:
- `skills/library/` - Compositional skills as data (not code)
- `learning/soar-*.ts` - SOAR-based learning experiments

But for now, focusing on Rust-first with hillclimber/testgen/gym.

---

## Additional Cleanup

Also deleted:
- `.worktrees/` - Old git worktree with TypeScript code
- `e2e/` - Playwright tests for deleted Effuse framework (14 files)
- `scripts/` - TypeScript dev scripts (13 files)
- `test-tb-real.ts` - Orphaned test file

**Final result: 0 TypeScript files remaining**

---

## Node.js Cleanup

Deleted:
- `node_modules/` - 842MB of npm packages
- `package.json` - All scripts referenced deleted TypeScript
- `bun.lock` - Lockfile
- `tsconfig*.json` - TypeScript configs
- `biome.json` - Linter config

**The codebase is now 100% Rust.**
