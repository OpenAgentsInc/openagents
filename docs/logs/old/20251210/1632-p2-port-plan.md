# P2: Integration & Wiring Plan

> **Goal:** Wire up all the completed Rust crates to work end-to-end
> **Status:** P1 crates (LLM, TBCC, Sandbox) are ~90% complete. Now we integrate.

---

## Current State

| Crate | Lines | Status |
|-------|-------|--------|
| `crates/llm/` | 3,851 | OpenAI, Anthropic, retry, models |
| `crates/sandbox/` | 1,393 | Docker, macOS, credentials |
| `crates/gym/src/tbcc/` | 2,293 | Dashboard, Tasks, Runs, Settings |
| `crates/gym/src/testgen/` | 2,040 | Visualizer, CategoryProgress, TestList |
| `crates/orchestrator/` | ? | Exists but needs integration |
| `crates/hillclimber/` | ? | Exists but needs integration |

---

## P2 Tasks

### 1. Wire LLM to Orchestrator

**Files to modify:**
- `crates/orchestrator/src/executor.rs` - Add multi-provider LLM support
- `crates/hillclimber/src/orchestrator.rs` - Use `crates/llm` instead of FM-only

**What to do:**
- Replace hardcoded FM calls with provider-agnostic `LlmClient`
- Add `--model` flag to select provider (fm, claude, gpt-4o)
- Integrate retry logic from `crates/llm/src/retry.rs`
- Add cost tracking using `crates/llm/src/models.rs`

### 2. Wire Sandbox to Orchestrator

**Files to modify:**
- `crates/orchestrator/src/executor.rs` - Add sandbox execution path
- `crates/hillclimber/src/orchestrator.rs` - Run in Docker/macOS container

**What to do:**
- Use `crates/sandbox/src/detect.rs` to auto-detect backend
- Run tool calls inside container when sandbox enabled
- Extract Claude Code credentials via `crates/sandbox/src/credentials.rs`
- Add `--sandbox` flag to enable container isolation

### 3. Wire TBCC to Real Data

**Files to modify:**
- `crates/gym/src/tbcc/dashboard.rs` - Connect to RunStore
- `crates/gym/src/tbcc/run_browser.rs` - Connect to TrajectoryStore
- `crates/gym/src/tbcc/task_browser.rs` - Load from TB2 task list
- `crates/gym/src/gym_screen.rs` - Pass stores to TBCC

**What to do:**
- Pass `Arc<RwLock<RunStore>>` to Dashboard for real stats
- Pass `Arc<Mutex<TrajectoryStore>>` to RunBrowser
- Load tasks from `tasks/terminal-bench-2.json`
- Add WebSocket/channel subscription for real-time updates

### 4. Wire TestGen to Service

**Files to modify:**
- `crates/gym/src/testgen/visualizer.rs` - Connect to TestGenService
- `crates/gym/src/testgen/service.rs` - Complete implementation

**What to do:**
- Connect visualizer to real TestGen output
- Stream test generation progress to UI
- Store generated tests in TestGenStore

### 5. End-to-End Testing

**Tests to add:**
- Run HillClimber with `--model gpt-4o` (OpenAI provider works)
- Run HillClimber with `--sandbox` (Docker isolation works)
- View results in TBCC Dashboard (real stats from SQLite)
- Export trajectory to HuggingFace

---

## Execution Order

1. **LLM Integration** - Make `--model` flag work
2. **Sandbox Integration** - Make `--sandbox` flag work
3. **TBCC Real Data** - Dashboard shows real stats
4. **TestGen Service** - Visualizer shows real tests
5. **E2E Tests** - Validate everything works together

---

## Success Criteria

- [ ] `cargo run -p hillclimber -- --model gpt-4o --tasks regex-log` works
- [ ] `cargo run -p hillclimber -- --sandbox --tasks regex-log` works
- [ ] TBCC Dashboard shows real run statistics
- [ ] TBCC Run Browser shows real trajectories
- [ ] TestGen Visualizer streams real test generation
