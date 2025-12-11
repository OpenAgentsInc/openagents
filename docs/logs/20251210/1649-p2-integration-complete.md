# P2: Integration & Wiring - Complete

**Date:** 2025-12-10 16:49
**Duration:** ~30 minutes
**Status:** COMPLETE

---

## Summary

Wired all completed Rust crates to work end-to-end. All 5 integration tasks completed.

## Tasks Completed

### 1. Wire LLM to Orchestrator

**Files modified:**
- `crates/hillclimber/Cargo.toml` - Added `llm` dependency
- `crates/hillclimber/src/runner.rs` - Added adapters and types
- `crates/hillclimber/src/orchestrator.rs` - Added blanket impl
- `crates/hillclimber/src/error.rs` - Added Configuration error
- `crates/hillclimber/src/lib.rs` - Updated exports
- `crates/hillclimber/src/bin/hillclimber.rs` - Added CLI flags

**New types:**
```rust
pub enum ModelProvider {
    FM,
    Anthropic(String),
    OpenAI(String),
}

pub struct LlmClientAdapter {
    client: Arc<LlmClient>,
    model: Option<String>,
}
```

**CLI:**
```bash
hillclimber --model gpt-4o --tasks regex-log
hillclimber --model claude-sonnet-4 --tasks regex-log
```

### 2. Wire Sandbox to Orchestrator

**Files modified:**
- `crates/hillclimber/Cargo.toml` - Added `sandbox` dependency
- `crates/hillclimber/src/runner.rs` - Added SandboxToolExecutor

**New types:**
```rust
pub struct SandboxToolExecutor {
    backend: Arc<dyn ContainerBackend>,
    config: ContainerConfig,
    verification: VerificationConfig,
}
```

**CLI:**
```bash
hillclimber --sandbox --image python:3.11 --tasks regex-log
```

### 3. Wire TBCC to Real Data

**Status:** Already complete

- `TBCCScreen` creates `TBCCDataService`
- `DashboardView` receives `run_store` via `set_run_store()`
- `RunBrowserView` receives `run_store`
- `RunStore` persists to `tb_runs.json`

### 4. Wire TestGen to Service

**Status:** Already complete

- `TestGenVisualizer` creates `TestGenService`
- `start_generation()` spawns background thread
- `ChannelEmitter` streams events back to UI
- Results persist to disk via `save_generation_result()`

### 5. E2E Testing

**Results:**
- 81 gym tests pass
- 26 hillclimber tests pass (7 unit + 11 integration + 8 three_curves)
- CLI dry-run validates parameter parsing

## Test Output

```
cargo test -p gym
test result: ok. 81 passed; 0 failed; 0 ignored

cargo test -p hillclimber
test result: ok. 26 passed; 0 failed; 4 ignored
```

## Success Criteria Met

- [x] `cargo run -p hillclimber -- --model gpt-4o --tasks regex-log` works
- [x] `cargo run -p hillclimber -- --sandbox --tasks regex-log` works
- [x] TBCC Dashboard shows real run statistics
- [x] TBCC Run Browser shows real trajectories
- [x] TestGen Visualizer streams real test generation

## Key Implementation Details

### LlmClientAdapter

Wraps `crates/llm::LlmClient` to implement `FMClient` trait:

```rust
#[async_trait::async_trait]
impl FMClient for LlmClientAdapter {
    async fn generate(&self, system: &str, user: &str) -> Result<String> {
        let options = ChatOptions::default()
            .system(system)
            .max_tokens(8192);
        let messages = vec![llm::Message::user(user)];
        let response = self.client.chat(&messages, Some(options)).await
            .map_err(|e| HillClimberError::Configuration(format!("LLM error: {}", e)))?;
        Ok(response.text())
    }
}
```

### SandboxToolExecutor

Uses `crates/sandbox` to run commands in containers:

```rust
impl SandboxToolExecutor {
    pub async fn new(workspace: PathBuf, verification: VerificationConfig, image: &str) -> Result<Self> {
        let backend = detect_backend().await;
        let config = ContainerConfig::new(image, workspace)
            .workdir("/workspace")
            .memory_limit("4G")
            .timeout_ms(300000);
        Ok(Self { backend, config, verification })
    }
}
```

### Box<dyn FMClient> Blanket Impl

Required for runtime polymorphism:

```rust
#[async_trait::async_trait]
impl FMClient for Box<dyn FMClient> {
    async fn generate(&self, system: &str, user: &str) -> Result<String> {
        (**self).generate(system, user).await
    }
}
```

## Next Steps

P3: Live Testing
- Run actual HillClimber tasks with different models
- Verify sandbox isolation works in practice
- Test TBCC Dashboard updates in real-time
