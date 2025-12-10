# Wire Up TestGen UI for Interactive Test Generation

## Goal

Make the Gym's TestGen tab functional: select any TerminalBench task, click "Generate Tests", see real-time progress, and view/export results.

## Current State

**UI exists but is disconnected:**
- Beautiful 3-column layout (CategoryProgress | TestList | TestDetail)
- Shows hardcoded sample data for "regex-log" task
- No task selector, no generate button, no backend connection

**Backend is complete:**
- `TestGenerator::generate_iteratively()` - async test generation
- `TestGenEmitter` trait - streaming progress callbacks
- `TestGenStore` - SQLite persistence

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                        TestGenVisualizer                                │
├─────────────────────────────────────────────────────────────────────────┤
│  ┌──────────────────────────────────────────────────────────────────┐   │
│  │ HEADER: Task Selector + Generate Button + Status                 │   │
│  │ [Task: regex-log ▼]  [🚀 Generate Tests]  Status: Generating 3/5 │   │
│  └──────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  ┌─────────────┐  ┌────────────────────────┐  ┌────────────────────┐   │
│  │ Categories  │  │     Test List          │  │   Test Detail      │   │
│  │ ────────── │  │ ──────────────────     │  │ ────────────────   │   │
│  │ AntiCheat  │  │ ✓ test_basic_date      │  │ Name: test_basic   │   │
│  │ ████░ 4/5  │  │ ✓ test_no_match        │  │ Category: Correct. │   │
│  │            │  │ ◦ test_edge_case       │  │ Confidence: 95%    │   │
│  │ Existence  │  │ ◦ test_multi_date      │  │                    │   │
│  │ ████ 3/3   │  │                        │  │ def test_basic():  │   │
│  │            │  │                        │  │   assert ...       │   │
│  │ ...        │  │                        │  │                    │   │
│  └─────────────┘  └────────────────────────┘  └────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                     TestGenService (Background)                         │
├─────────────────────────────────────────────────────────────────────────┤
│  TestGenerator::generate_iteratively()                                  │
│    → Calls FM via fm-bridge                                             │
│    → Emits progress via TestGenEmitter                                  │
│    → Saves results to TestGenStore                                      │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## Implementation Plan

### Phase 1: Add Task Selector Header (No Backend Yet)

**Files to modify:**
- `crates/gym/src/testgen/visualizer.rs`

**Changes:**
1. Add header section with task dropdown
2. Load task list from TBCC task loader
3. Add "Generate Tests" button (disabled initially)
4. Add status indicator (Idle/Generating/Completed)
5. Wire up task selection to update UI state

**New state fields in TestGenVisualizer:**
```rust
struct TestGenVisualizer {
    // Existing
    session: Option<TestGenSession>,
    category_progress: Entity<CategoryProgress>,
    test_list: Entity<TestList>,
    test_detail: Entity<TestDetail>,
    selected_test_id: Option<String>,

    // NEW
    available_tasks: Vec<TBTask>,           // From TBCC
    selected_task_id: Option<String>,       // Currently selected task
    generation_status: GenerationStatus,    // Idle/Generating/Complete/Failed
    generation_progress: GenerationProgress, // Current iteration, etc.
}

enum GenerationStatus {
    Idle,
    Generating { iteration: u32, max_iterations: u32 },
    Complete { total_tests: u32, duration_ms: u64 },
    Failed { error: String },
}
```

---

### Phase 2: Background Generation Service

**Files to create:**
- `crates/gym/src/testgen/service.rs` - TestGen background service

**Service design:**
```rust
pub struct TestGenService {
    generator: TestGenerator,
    store: TestGenStore,
}

impl TestGenService {
    pub async fn generate_for_task(
        &self,
        task: &TBTask,
        emitter: impl TestGenEmitter,
    ) -> Result<GenerationResult>;
}
```

**Event bridge pattern:**
- Create `GymTestGenEmitter` that implements `TestGenEmitter`
- Emitter sends events to UI channel
- UI polls/receives events and updates state

---

### Phase 3: Wire Events to UI Updates

**Files to modify:**
- `crates/gym/src/testgen/visualizer.rs`
- `crates/gym/src/actions.rs`

**New actions:**
```rust
actions!(
    gym,
    [
        // ... existing actions ...
        TestGenSelectTask,       // User picks task from dropdown
        TestGenStartGeneration,  // User clicks "Generate" button
        TestGenCancelGeneration, // User cancels in-progress generation
        TestGenSelectTest,       // User clicks test in list
        TestGenExportTests,      // Export to pytest file
    ]
);
```

**Event handling:**
1. Button click → spawn async generation task
2. Emitter callbacks → update visualizer state
3. State change → `cx.notify()` → re-render

---

### Phase 4: Test Selection & Detail View

**Files to modify:**
- `crates/gym/src/testgen/test_list.rs`
- `crates/gym/src/testgen/test_detail.rs`

**Changes:**
1. Add click handlers to test rows
2. Update detail view when test selected
3. Add category click to filter test list

---

### Phase 5: Persistence & History

**Files to modify:**
- `crates/gym/src/testgen/visualizer.rs`

**Features:**
1. Load previous generation from TestGenStore
2. Show generation history
3. Compare generations over time

---

## Critical Files

| File | Action | Purpose |
|------|--------|---------|
| `crates/gym/src/testgen/visualizer.rs` | Modify | Add header, state, event handling |
| `crates/gym/src/testgen/service.rs` | Create | Background generation service |
| `crates/gym/src/testgen/test_list.rs` | Modify | Add click handlers |
| `crates/gym/src/testgen/test_detail.rs` | Modify | Wire to selection |
| `crates/gym/src/actions.rs` | Modify | Add TestGen actions |
| `crates/gym/src/types.rs` | Modify | Add GenerationStatus enum |

---

## API Integration Points

**From testgen crate:**
```rust
// Generator
use testgen::{TestGenerator, IterationConfig, TestGenEmitter};
use testgen::{GeneratedTest, TestCategory, GenerationResult};
use testgen::EnvironmentInfo;

// Store
use testgen::TestGenStore;
```

**From TBCC (task loading):**
```rust
use crate::tbcc::task_loader::TaskLoader;
use crate::tbcc::types::TBTask;
```

---

## Event Flow Diagram

```
User clicks "Generate"
        │
        ▼
TestGenStartGeneration action
        │
        ▼
visualizer.start_generation(cx)
        │
        ├─► Set status = Generating
        │
        └─► Spawn async task:
            │
            ▼
        TestGenService::generate_for_task()
            │
            ├─► on_progress() ──► Send to UI channel
            ├─► on_test()     ──► Send to UI channel
            ├─► on_complete() ──► Send to UI channel
            │
            ▼
        UI receives events (poll/stream)
            │
            ├─► Update category_progress
            ├─► Update test_list
            ├─► Update status
            └─► cx.notify() → re-render
```

---

## Success Criteria

- [ ] Task dropdown shows all TB2 tasks
- [ ] "Generate Tests" button starts generation
- [ ] Progress shows iteration/total in real-time
- [ ] Tests appear in list as they're generated
- [ ] Clicking test shows detail with code
- [ ] Generation persists to TestGenStore
- [ ] Can load previous generations
- [ ] Category filtering works
- [ ] Export to pytest file works

---

## Implementation Order

1. **Phase 1** - Header UI with task selector (static)
2. **Phase 2** - Background service with emitter bridge
3. **Phase 3** - Wire generate button to service
4. **Phase 4** - Test selection and detail view
5. **Phase 5** - Persistence and history
