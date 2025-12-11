# Plan: MechaCoder Screen (FM + CC HillClimber)

## Goal
Create a new **MechaCoder screen** in Gym with both FM and CC backends for solving Terminal-Bench tasks. Harvest useful code from Regex Crusade but build fresh.

---

## New Screen: MechaCoder

### Layout
```
┌─────────────────────────────────────────────────────────────────┐
│  MechaCoder                [● FM ○ CC]  [▶ Run] [■ Stop]        │
├──────────────┬────────────────────────┬─────────────────────────┤
│ TASK         │ ITERATION HISTORY      │ LIVE LOG                │
│              │                        │                         │
│ regex-log    │ Turn 1: 12/20 (60%)    │ 10:32:15 Starting...    │
│ ████████ 85% │ Turn 2: 15/20 (75%)    │ 10:32:16 Reading task   │
│ 17/20 passed │ Turn 3: 17/20 (85%) ←  │ 10:32:18 Trying regex   │
│              │                        │ 10:32:20 Running tests  │
│ Best:        │ [Expand to see regex]  │ 10:32:22 12/20 passed   │
│ \d{4}-\d{2}  │                        │ ...                     │
│              │                        │                         │
│ [Gen Tests]  │                        │                         │
└──────────────┴────────────────────────┴─────────────────────────┘
```

### Harvest from Regex Crusade
- `TaskPanel` layout (pass rate, best solution display)
- `IterationLog` component (streaming log entries)
- `CrusadeSession` state management pattern
- Log entry styling (color-coded by type)

### HillClimber FM (`crates/hillclimber/`)
- `MAPOrchestrator` with FM client abstraction
- `HillClimberEmitter` trait for progress callbacks
- SQLite persistence via `HillClimberStore`
- CLI binary working, UI monitor uses sample data

### Claude Agent SDK (`crates/claude_agent_sdk/`)
- Full SDK with query(), session API
- Supports subagents, skills, hooks, structured output
- `setting_sources` for loading skills

---

## Implementation Plan

### Phase 1: Create CC HillClimber Service

**New file: `crates/hillclimber/src/cc_runner.rs`**

```rust
/// Claude Code SDK-based HillClimber runner
pub struct CCHillClimberRunner {
    store: Arc<HillClimberStore>,
    sdk_options: CCRunnerOptions,
}

pub struct CCRunnerOptions {
    pub model: String,           // "claude-sonnet-4-5-20250929"
    pub max_turns: u32,
    pub max_budget_usd: f64,
    pub use_skills: bool,        // Load .claude/skills/
    pub parallel_samples: u32,   // Number of parallel candidates
}

impl CCHillClimberRunner {
    pub async fn run(
        &self,
        task: &TerminalBenchTask,
        emitter: impl HillClimberEmitter,
    ) -> Result<HillClimberRun>;
}
```

**Implementation approach:**
1. Use `claude_agent_sdk::query()` with skills loaded
2. Define subagents for TestGen, Decomposer (in SDK options)
3. Use structured output for iteration results
4. Emit progress via existing `HillClimberEmitter` trait
5. Store results in same SQLite schema

### Phase 2: Add Backend Selection Types

**Modify: `crates/hillclimber/src/types.rs`**

```rust
/// HillClimber backend selection
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HillClimberBackend {
    /// Apple Foundation Model (local)
    FM,
    /// Claude Code SDK (cloud)
    CC,
}
```

**Modify: `crates/gym/src/regex_crusade/types.rs`**

```rust
pub struct CrusadeSession {
    pub backend: HillClimberBackend,  // NEW
    pub status: CrusadeStatus,
    pub best_regex: Option<String>,
    // ... existing fields
}
```

### Phase 3: Create Unified HillClimber Service

**New file: `crates/gym/src/services/hillclimber_service.rs`**

```rust
/// Unified HillClimber service supporting FM and CC backends
pub struct HillClimberService {
    fm_runner: Arc<HillClimberRunner>,      // Existing FM runner
    cc_runner: Arc<CCHillClimberRunner>,    // New CC runner
    store: Arc<HillClimberStore>,
}

impl HillClimberService {
    /// Start a HillClimber run with specified backend
    pub fn start_run(
        &self,
        task: TerminalBenchTask,
        backend: HillClimberBackend,
    ) -> mpsc::UnboundedReceiver<HillClimberEvent>;

    /// Get recent runs for a task
    pub fn get_task_runs(&self, task_id: &str) -> Vec<HillClimberRun>;
}

pub enum HillClimberEvent {
    TurnStarted { turn: u32, max_turns: u32 },
    TurnCompleted { turn: u32, passed: u32, total: u32, regex: String },
    VerifyComplete { passed: u32, total: u32, failures: Vec<String> },
    RunComplete { run: HillClimberRun },
    Error { message: String },
}
```

### Phase 4: Create MechaCoder Screen

**New directory: `crates/gym/src/mechacoder/`**

```
crates/gym/src/mechacoder/
├── mod.rs              # Main MechaCoderScreen
├── types.rs            # MechaCoderSession, Backend enum
├── task_panel.rs       # Left panel (harvest from crusade)
├── iteration_panel.rs  # Center panel (iteration history)
└── log_panel.rs        # Right panel (live streaming log)
```

**Main screen struct:**
```rust
pub struct MechaCoderScreen {
    task_panel: Entity<TaskPanel>,
    iteration_panel: Entity<IterationPanel>,
    log_panel: Entity<LogPanel>,

    session: MechaCoderSession,
    selected_backend: HillClimberBackend,
    hillclimber_service: Arc<HillClimberService>,
    event_receiver: Option<mpsc::UnboundedReceiver<HillClimberEvent>>,
}
```

**Header with backend toggle:**
```
MechaCoder                [● FM ○ CC]  [▶ Run] [■ Stop]
```

- Radio buttons for FM vs CC
- Run button starts selected backend
- Stop button interrupts current run

### Phase 5: Wire Up Event Flow

**In `mechacoder/mod.rs`:**

```rust
fn start_run(&mut self, cx: &mut Context<Self>) {
    let receiver = self.hillclimber_service.start_run(
        self.get_current_task(),
        self.selected_backend,
    );
    self.event_receiver = Some(receiver);
    self.session.status = MechaCoderStatus::Running;
    self.add_log(LogKind::Info, format!("Starting {} run...", self.selected_backend));
    cx.notify();
}

fn poll_events(&mut self, cx: &mut Context<Self>) {
    if let Some(ref mut receiver) = self.event_receiver {
        while let Ok(event) = receiver.try_recv() {
            match event {
                HillClimberEvent::TurnCompleted { turn, passed, total, solution } => {
                    self.session.iterations.push(Iteration { turn, passed, total, solution });
                    self.session.best_passed = passed.max(self.session.best_passed);
                    self.add_log(LogKind::Progress, format!("Turn {}: {}/{}", turn, passed, total));
                }
                HillClimberEvent::RunComplete { run } => {
                    self.session.status = if run.passed {
                        MechaCoderStatus::Completed
                    } else {
                        MechaCoderStatus::Failed
                    };
                    self.event_receiver = None;
                    self.add_log(LogKind::Complete, format!("Run complete: {}", if run.passed { "SUCCESS" } else { "FAILED" }));
                }
                HillClimberEvent::Error { message } => {
                    self.add_log(LogKind::Error, message);
                }
                _ => {}
            }
            cx.notify();
        }
    }
}
```

### Phase 6: Add to Gym Tabs

**Modify: `crates/gym/src/types.rs`**

```rust
pub enum GymTab {
    // ... existing
    MechaCoder,  // NEW
}
```

**Modify: `crates/gym/src/gym_screen.rs`**

Add MechaCoder tab and screen instantiation.

---

## Files to Create/Modify

### New Files
| File | Purpose |
|------|---------|
| `crates/hillclimber/src/cc_runner.rs` | CC HillClimber using Claude SDK |
| `crates/gym/src/services/hillclimber_service.rs` | Unified FM/CC service |
| `crates/gym/src/mechacoder/mod.rs` | Main MechaCoder screen |
| `crates/gym/src/mechacoder/types.rs` | Session, status types |
| `crates/gym/src/mechacoder/task_panel.rs` | Left panel |
| `crates/gym/src/mechacoder/iteration_panel.rs` | Center panel |
| `crates/gym/src/mechacoder/log_panel.rs` | Right panel |

### Modified Files
| File | Changes |
|------|---------|
| `crates/hillclimber/src/lib.rs` | Export `cc_runner` module |
| `crates/hillclimber/src/types.rs` | Add `HillClimberBackend` enum |
| `crates/gym/src/lib.rs` | Export `mechacoder` module |
| `crates/gym/src/types.rs` | Add `MechaCoder` to `GymTab` enum |
| `crates/gym/src/gym_screen.rs` | Add MechaCoder tab |
| `crates/gym/src/services/mod.rs` | Export `hillclimber_service` |

---

## CC Runner Implementation Details

### Using Claude Agent SDK

```rust
use claude_agent_sdk::{query, QueryOptions, SettingSource};

impl CCHillClimberRunner {
    async fn run_iteration(
        &self,
        task: &TerminalBenchTask,
        tests: &[GeneratedTest],
        current_best: Option<&str>,
    ) -> Result<IterationResult> {
        let prompt = format!(
            "Solve this regex task:\n{}\n\nTests to pass:\n{}\n\nCurrent best: {}",
            task.description,
            format_tests(tests),
            current_best.unwrap_or("none")
        );

        let options = QueryOptions::new()
            .model(&self.options.model)
            .cwd(&task.workspace)
            .setting_sources(vec![SettingSource::Project])
            .max_turns(self.options.max_turns)
            .max_budget_usd(self.options.max_budget_usd)
            .output_format(IterationResultSchema);

        let mut stream = query(&prompt, options).await?;

        // Collect result
        let result = collect_result(&mut stream).await?;
        Ok(result.structured_output)
    }
}
```

### Skills for HillClimber CC

Create `.claude/skills/hillclimber/` with:
- `testgen/SKILL.md` - Test generation procedures
- `decomposer/SKILL.md` - Task decomposition patterns
- `evaluator/SKILL.md` - Pytest execution patterns

---

## UI Mockup (removed - see layout at top)

---

## Implementation Order

1. **Add `HillClimberBackend` enum** to `hillclimber/types.rs`
2. **Create `CCHillClimberRunner`** in `hillclimber/cc_runner.rs`
3. **Create `HillClimberService`** in `gym/services/hillclimber_service.rs`
4. **Create MechaCoder screen directory** `gym/mechacoder/`
   - `types.rs` - Session, status, iteration types
   - `task_panel.rs` - Harvest from Crusade
   - `iteration_panel.rs` - New iteration history view
   - `log_panel.rs` - Harvest from Crusade
   - `mod.rs` - Main screen with backend toggle
5. **Add MechaCoder tab** to `gym_screen.rs` and `types.rs`
6. **Create skills** in `.claude/skills/hillclimber/` (optional, for CC)
7. **Test FM backend** through new screen
8. **Test CC backend** through new screen

---

## Dependencies

```toml
# crates/hillclimber/Cargo.toml
[dependencies]
claude_agent_sdk = { path = "../claude_agent_sdk" }
```

---

## Success Criteria

- [ ] New "MechaCoder" tab appears in Gym
- [ ] Can toggle between FM and CC backends
- [ ] FM backend runs using existing `MAPOrchestrator`
- [ ] CC backend runs using Claude Agent SDK
- [ ] Iteration history shows in center panel
- [ ] Live log streams in right panel
- [ ] Pass rate + best solution updates in left panel
- [ ] Results persist to SQLite store
