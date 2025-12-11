# RegexCrusade: Laser-Focused Regex-Log Screen

## Goal
Create a new Gym tab called **RegexCrusade** - a single-purpose UI for solving the `regex-log` Terminal-Bench task. No generalization, no task browser - ONLY regex-log.

## The Problem
TestGen generates stub tests (`pass` statements) instead of real assertions. The FM never gets real failure feedback, so iteration doesn't work. This UI makes the problem visible and actionable.

## Architecture

**Three-panel layout:**
```
┌─────────────┬───────────────────────┬─────────────────┐
│ Task Panel  │     Test Panel        │  Iteration Log  │
│ (260px)     │     (flex-1)          │  (320px)        │
│             │                       │                 │
│ - Pass rate │ - Test list           │ - Sparkline     │
│ - Stub warn │ - STUB vs REAL flags  │ - Turn history  │
│ - Regex     │ - Category badges     │ - Diff preview  │
│ - Task desc │ - Pass/fail status    │                 │
└─────────────┴───────────────────────┴─────────────────┘
```

## Files to Create

### 1. `crates/gym/src/regex_crusade/mod.rs`
Main screen container with three-panel layout:
- Header with "Generate Tests" and "Validate" buttons
- Three child panels side-by-side
- Session state management

### 2. `crates/gym/src/regex_crusade/types.rs`
Domain types:
- `CrusadeStatus` - Idle, GeneratingTests, RunningIteration, Validating, Completed
- `TestQuality` - Stub, Real, Unknown (with `detect_stub()` function)
- `CrusadeTest` - Test with stub detection, category, pass/fail
- `CrusadeCategory` - AntiCheat, Existence, Correctness, Boundary, Integration
- `Iteration` - Turn number, regex tried, pass rate, change description
- `CrusadeSession` - Full session state

### 3. `crates/gym/src/regex_crusade/task_panel.rs`
Left panel showing:
- Task name: "Regex Log Parser"
- Large pass rate display (36px font, progress bar)
- Test quality summary (X real, Y stubs) with warning if stubs > real
- Current best regex (scrollable code block)
- Task description (scrollable)

### 4. `crates/gym/src/regex_crusade/test_panel.rs`
Center panel with scrollable test list:
- Each row: status icon, quality badge (R/S), category badge (AC/EX/CO/BO/IN), test ID, input preview, confidence
- Color-coded: green for Real, red for Stub
- Click to select (detail view later)
- Filter by quality/category

### 5. `crates/gym/src/regex_crusade/iteration_log.rs`
Right panel showing:
- Sparkline of pass rate over turns (last 20 iterations)
- Turn-by-turn history (most recent first)
- Each entry: turn #, pass rate, change description, regex preview

## Files to Modify

### 1. `crates/gym/src/types.rs`
Add `RegexCrusade` variant to `GymTab` enum:
```rust
pub enum GymTab {
    Trajectories,
    TBCC,
    HillClimber,
    TestGen,
    RegexCrusade,  // NEW
}
```

Update `label()` and `all()` methods.

### 2. `crates/gym/src/actions.rs`
Add `SwitchToRegexCrusade` action and `cmd-5` keybinding.

### 3. `crates/gym/src/gym_screen.rs`
- Import `RegexCrusadeScreen`
- Add `regex_crusade_view: Entity<RegexCrusadeScreen>` field
- Create entity in `with_store()`
- Add `switch_to_regex_crusade()` handler
- Update `render_active_tab_content()` match arm
- Register action handler in `render()`

### 4. `crates/gym/src/lib.rs`
Add `pub mod regex_crusade;` and re-export.

## Key Implementation Details

### Stub Detection (`detect_stub()`)
```rust
pub fn detect_stub(code: &str) -> TestQuality {
    let trimmed = code.trim();
    if trimmed.is_empty() || trimmed == "pass" {
        return TestQuality::Stub;
    }
    if !trimmed.contains("assert") {
        return TestQuality::Stub;
    }
    if trimmed.contains("==") || trimmed.contains(".match(") {
        return TestQuality::Real;
    }
    TestQuality::Unknown
}
```

### Session State Flow
```
RegexCrusadeScreen
    │
    ├── update_session(CrusadeSession) ──┬── TaskPanel.set_session()
    │                                    ├── IterationLog.set_iterations()
    │                                    └── TestPanel.set_tests()
    │
    └── add_iteration(Iteration) ────────┬── IterationLog.add_iteration()
                                         └── TaskPanel.update_pass_rate()
```

### GPUI Patterns (from existing code)
- `div().flex().flex_col().h_full().w_full().bg(bg::APP)`
- `cx.listener(move |this, _evt, _window, cx| { ... })`
- `cx.notify()` after state changes
- Entity pattern: `cx.new(|cx| Component::new(cx))`

## MVP Scope

**Phase 1 - Static UI with sample data:**
1. Create all 5 new files with hardcoded sample data
2. Wire into GymScreen with Cmd+5
3. Verify three-panel layout renders

**Phase 2 - Stub detection:**
1. Implement `detect_stub()`
2. Color-code tests red/green based on quality
3. Show warning when stub_count > real_count

**Future (not in this PR):**
- TestGen integration (Generate button)
- Docker validation (Validate button)
- Live iteration tracking
- Test detail drawer

## Execution Order

1. Create `regex_crusade/types.rs` with all domain types
2. Create `regex_crusade/task_panel.rs`
3. Create `regex_crusade/test_panel.rs`
4. Create `regex_crusade/iteration_log.rs`
5. Create `regex_crusade/mod.rs` assembling everything
6. Update `types.rs` - add RegexCrusade to GymTab
7. Update `actions.rs` - add action + keybinding
8. Update `gym_screen.rs` - wire everything together
9. Update `lib.rs` - add module export
10. Build and test
