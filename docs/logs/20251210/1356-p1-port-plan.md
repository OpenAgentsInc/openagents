# Priority 1 Port Implementation Plan

> **Goal:** Complete the Rust port of all P1 items to enable full TypeScript deletion
> **Estimated Total:** 4-5 weeks
> **Approach:** Parallel tracks where possible

---

## Overview

Based on exploration, the P1 gaps are:

| Area | Current State | Gap |
|------|---------------|-----|
| **LLM Providers** | Anthropic only | OpenAI, retry logic, model registry |
| **TBCC Components** | Stubs in `crates/gym/` | 6 components (~2,874 lines TS) |
| **Sandbox** | Not ported | Full crate (~2,000 lines TS) |
| **Desktop Shell** | ✅ Already done | Has GPUI native + nav menus |

---

## Track 1: LLM Providers (Week 1-2)

### 1.1 OpenAI Provider
**File:** `crates/llm/src/openai.rs` (~200 lines)

Port from `src/llm/openai.ts`:
- Request/response types matching OpenAI API
- Tool calling with strict mode support
- Image handling (base64 + URL)
- SSE streaming
- Model mapping (gpt-4o, gpt-4o-mini, o1, o1-mini)

**Key types:**
```rust
struct OpenAIRequest { model, messages, tools, tool_choice, temperature, max_tokens, stream }
struct OpenAIResponse { id, choices, usage }
struct OpenAIChoice { index, message, finish_reason }
```

### 1.2 Retry Logic
**File:** `crates/llm/src/retry.rs` (~100 lines)

Port from `src/llm/retry.ts`:
- `RetryConfig { attempts, base_delay_ms, max_delay_ms }`
- `is_retryable_error()` - classify by status code (429, 5xx = retry; 401, 403 = no retry)
- `retry_with_backoff<T, E>()` - exponential backoff wrapper
- Env var config: `LLM_RETRY_ATTEMPTS`, `LLM_RETRY_BASE_MS`, `LLM_RETRY_MAX_MS`

### 1.3 Model Registry
**File:** `crates/llm/src/models.rs` (~150 lines)

Port from `src/llm/models.ts` and `model-types.ts`:
- `Model { id, name, provider, context_window, max_tokens, cost }`
- `Cost { input, output, cache_read, cache_write }` (per million tokens)
- `get_model(provider, model_id)` lookup
- `calculate_cost(model, usage)` function
- Hardcode top 10-20 models initially (Claude, GPT-4o, etc.)

### 1.4 Provider Trait Updates
**File:** `crates/llm/src/provider.rs` (modify)

- Add `Provider` enum: `Anthropic | OpenAI`
- Add `from_env()` for provider config
- Unify `ChatRequest` to work across providers

### 1.5 Client Updates
**File:** `crates/llm/src/client.rs` (modify)

- Multi-provider support via provider enum
- Dynamic cost calculation from model registry
- Retry wrapper integration

**Tests:** Unit tests for each provider, retry logic, model lookup

---

## Track 2: TBCC Components (Week 2-3)

All in `crates/gym/src/tbcc/`:

### 2.1 Dashboard (`dashboard.rs`)
**Port from:** `src/effuse/components/tb-command-center/tbcc-dashboard.ts` (633 lines)

**Features:**
- KPI grid (success rate, avg steps, total runs, avg duration)
- Recent runs table with status badges
- Quick action buttons (Run Benchmark, Run Single Task)
- Real-time stats from SQLite

**GPUI Pattern:**
```rust
struct DashboardView {
    stats: Option<DashboardStats>,
    recent_runs: Vec<TBRunSummary>,
    loading: bool,
}
impl Render for DashboardView { ... }
```

### 2.2 Task Browser (`task_browser.rs`)
**Port from:** `src/effuse/components/tb-command-center/tbcc-task-browser.ts` (347 lines)

**Features:**
- Task list from `tasks/terminal-bench-2.json`
- Filter by difficulty (easy/medium/hard)
- Task detail panel (description, timeout, tags)
- "Run Task" button with mode selector

### 2.3 Run Browser (`run_browser.rs`)
**Port from:** `src/effuse/components/tb-command-center/tbcc-run-browser.ts` (554 lines)

**Features:**
- Run list with status filters (passed/failed/running)
- Sorting (by date, score, duration)
- Run detail with step accordion
- Terminal output panel (reuse existing step_view pattern)

### 2.4 Settings (`settings.rs`)
**Port from:** `src/effuse/components/tb-command-center/tbcc-settings.ts` (385 lines)

**Features:**
- Execution settings form (max_attempts, timeout_ms, max_tokens)
- Logging settings (save_trajectories, auto_prune)
- Container settings (image, memory_limit, cpu_limit)
- Save/Reset buttons

### 2.5 Shell Output (`shell.rs`)
**Port from:** `src/effuse/components/tb-command-center/tbcc-shell.ts` (279 lines)

**Features:**
- Terminal output viewer
- ANSI color parsing → GPUI colors
- Auto-scroll to bottom
- Clear button

### 2.6 TestGen View (`testgen_view.rs`)
**Port from:** `src/effuse/components/tb-command-center/tbcc-testgen.ts` (676 lines)

**Features:**
- Category progress bars (anti_cheat, existence, correctness, boundary)
- Generated test list with pass/fail status
- Test detail panel with code display
- Comprehensiveness score

### 2.7 Wire Up in Gym Screen
**File:** `crates/gym/src/gym_screen.rs` (modify)

- Connect TBCC tab to real components (replace stubs)
- Add tab bar switching between Dashboard/Tasks/Runs/TestGen/Settings
- WebSocket subscription for real-time updates

---

## Track 3: Sandbox Crate (Week 3-4)

### 3.1 Create Crate Structure
**New crate:** `crates/sandbox/`

```
crates/sandbox/
├── Cargo.toml
├── src/
│   ├── lib.rs           # Re-exports
│   ├── error.rs         # ContainerError, CredentialError
│   ├── config.rs        # ContainerConfig, ContainerRunResult
│   ├── backend.rs       # ContainerBackend trait
│   ├── docker.rs        # Docker implementation
│   ├── macos.rs         # macOS Container implementation
│   ├── detect.rs        # Auto-detection logic
│   └── credentials.rs   # Keychain extraction
```

### 3.2 Error Types (`error.rs`)
**Port from:** `src/sandbox/schema.ts`

```rust
pub enum ContainerError {
    NotAvailable(String),
    ImageNotFound(String),
    StartFailed(String),
    ExecutionFailed { exit_code: i32, stderr: String },
    Timeout,
    Aborted,
}

pub enum CredentialError {
    NotFound,
    AccessDenied,
    InvalidFormat(String),
    ExtractionFailed(String),
}
```

### 3.3 Config Types (`config.rs`)
**Port from:** `src/sandbox/schema.ts`

```rust
pub struct ContainerConfig {
    pub image: String,
    pub workspace_dir: PathBuf,
    pub workdir: Option<String>,
    pub memory_limit: Option<String>,
    pub cpu_limit: Option<f32>,
    pub env: HashMap<String, String>,
    pub timeout: Option<Duration>,
    pub auto_remove: bool,
    pub volume_mounts: Vec<String>,
}

pub struct ContainerRunResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub container_id: Option<String>,
}
```

### 3.4 Backend Trait (`backend.rs`)
**Port from:** `src/sandbox/backend.ts`

```rust
#[async_trait]
pub trait ContainerBackend: Send + Sync {
    fn name(&self) -> &'static str;
    async fn is_available(&self) -> bool;
    async fn run(&self, command: &[String], config: &ContainerConfig) -> Result<ContainerRunResult, ContainerError>;
    async fn build(&self, context_dir: &Path, tag: &str) -> Result<(), ContainerError>;
}
```

### 3.5 Docker Backend (`docker.rs`)
**Port from:** `src/sandbox/docker.ts` (~300 lines)

- Use `tokio::process::Command` for `docker run`
- Build command args from config (volumes, env, limits)
- Stream stdout/stderr via channels
- Handle timeout with `tokio::time::timeout`
- Parse exit code from process

### 3.6 macOS Container Backend (`macos.rs`)
**Port from:** `src/sandbox/macos-container.ts` (~250 lines)

- Use `container` CLI (macOS 26+)
- Similar pattern to Docker
- Different CLI args format

### 3.7 Auto-Detection (`detect.rs`)
**Port from:** `src/sandbox/detect.ts`

```rust
pub async fn detect_backend() -> Option<Box<dyn ContainerBackend>> {
    // 1. Check macOS Container (if on Darwin)
    // 2. Check Docker
    // 3. Return None if nothing available
}

pub async fn is_container_available() -> bool {
    detect_backend().await.is_some()
}
```

### 3.8 Credentials (`credentials.rs`)
**Port from:** `src/sandbox/credentials.ts`

- Extract from macOS Keychain using `security find-generic-password`
- Service name: `"Claude Code-credentials"`
- Write to temp dir with 0600 permissions
- Return mount path for container

### 3.9 Integration with Orchestrator
**File:** `crates/orchestrator/src/executor.rs` (modify)

- Add `run_in_sandbox()` function
- Fallback to host execution if sandbox unavailable
- Emit sandbox events for HUD

---

## Track 4: Integration & Testing (Week 4-5)

### 4.1 Wire Up LLM to Orchestrator
- Update `crates/orchestrator/` to use multi-provider LLM
- Add model selection config
- Integrate retry logic

### 4.2 Wire Up Sandbox to Orchestrator
- Add sandbox execution path for tool calls
- Credential injection for Claude Code fallback
- HUD events for sandbox status

### 4.3 Wire Up TBCC to Real Data
- Connect dashboard to HillClimber store
- Connect run browser to ATIF store
- Connect task browser to TB2 task list
- WebSocket events for real-time updates

### 4.4 End-to-End Testing
- Run HillClimber with OpenAI provider
- Run HillClimber in Docker sandbox
- View results in TBCC dashboard
- Export trajectory to HuggingFace

---

## File Change Summary

### New Files (~15 files, ~2,500 lines)
```
crates/llm/src/openai.rs          (~200 lines)
crates/llm/src/retry.rs           (~100 lines)
crates/llm/src/models.rs          (~150 lines)
crates/sandbox/src/lib.rs         (~30 lines)
crates/sandbox/src/error.rs       (~80 lines)
crates/sandbox/src/config.rs      (~100 lines)
crates/sandbox/src/backend.rs     (~50 lines)
crates/sandbox/src/docker.rs      (~300 lines)
crates/sandbox/src/macos.rs       (~250 lines)
crates/sandbox/src/detect.rs      (~80 lines)
crates/sandbox/src/credentials.rs (~150 lines)
crates/gym/src/tbcc/dashboard.rs  (~300 lines)
crates/gym/src/tbcc/task_browser.rs (~200 lines)
crates/gym/src/tbcc/run_browser.rs (~300 lines)
crates/gym/src/tbcc/settings.rs   (~200 lines)
crates/gym/src/tbcc/shell.rs      (~150 lines)
crates/gym/src/tbcc/testgen_view.rs (~350 lines)
```

### Modified Files (~8 files)
```
crates/llm/src/lib.rs             # Add new module exports
crates/llm/src/provider.rs        # Multi-provider support
crates/llm/src/client.rs          # Retry + model registry
crates/gym/src/lib.rs             # Export TBCC
crates/gym/src/gym_screen.rs      # Wire up real components
crates/orchestrator/src/executor.rs # Sandbox integration
Cargo.toml                        # Add sandbox crate
```

---

## Execution Order

**Week 1:**
1. LLM: OpenAI provider
2. LLM: Retry logic
3. LLM: Model registry

**Week 2:**
4. Sandbox: Create crate, error types, config
5. Sandbox: Docker backend
6. TBCC: Dashboard

**Week 3:**
7. Sandbox: macOS backend, detect, credentials
8. TBCC: Task browser, Run browser
9. TBCC: Settings, Shell

**Week 4:**
10. TBCC: TestGen view
11. Integration: LLM → Orchestrator
12. Integration: Sandbox → Orchestrator

**Week 5:**
13. Integration: TBCC → Real data
14. End-to-end testing
15. Bug fixes and polish

---

## Success Criteria

- [ ] Can run HillClimber with `--model gpt-4o` (OpenAI provider works)
- [ ] Retry logic kicks in on 429/5xx errors
- [ ] Can run tools in Docker sandbox
- [ ] TBCC Dashboard shows real stats from SQLite
- [ ] TBCC Run Browser lists real trajectories
- [ ] TBCC Task Browser loads TB2 tasks
- [ ] All existing tests still pass
- [ ] Can delete corresponding TypeScript files after each track
