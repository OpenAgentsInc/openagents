# Plan: ATIF Components Storybook

Add stories to the storybook crate that showcase all permutations of the ATIF visualization components ported from Effuse.

## Components to Cover

### 1. Step View (`crates/commander/src/components/step_view.rs`)

**Source Badges** - 3 variants:
- User (blue)
- Agent (green)
- System (gray)

**Step Header** - collapsed view with:
- With/without tool count indicator
- With/without model name
- With/without timestamp

**Step Details** - expanded view sections:
- Message content (always)
- Reasoning content (agent-only, optional)
- Tool calls section (optional)
- Observation section (optional)
- Metrics section (optional)

**Tool Call Rendering**:
- Single tool call
- Multiple tool calls

**Observation Rendering**:
- With content only
- With subagent trajectory ref
- Multiple results

**Metrics Rendering**:
- All fields (prompt, completion, cached, cost)
- Partial fields

### 2. Thread Items (`crates/commander/src/components/thread_item.rs`)

**Category Badges** - 6 variants:
- AntiCheat (red)
- Existence (blue)
- Correctness (green)
- Boundary (yellow)
- Integration (purple)
- Other (gray)

**Thread Item Types** - 5 variants:
- Progress item
- Reflection item (with 3 action states: Refining, Assessing, Complete)
- Test item (collapsed + expanded)
- Complete item
- Error item

**Confidence Bar**:
- 0%, 50%, 100%

### 3. Trajectory Detail (`crates/commander/src/components/trajectory_detail.rs`)

**States**:
- Loading state
- Error state
- Empty state (no selection)
- Collapsed state
- Full view with metadata + steps + final metrics

### 4. Trajectory List (`crates/commander/src/components/trajectory_list.rs`)

**Item States**:
- Selected vs unselected

**Status Badges**:
- Completed (green)
- Failed (red)
- InProgress (yellow)

**List States**:
- Loading
- Error
- Empty (no results)
- With items

**Pagination**:
- First page (no prev)
- Middle page (prev + next)
- Last page (no next)

---

## Implementation

### New Story File
Create `crates/storybook/src/stories/atif_components.rs`

### Story Structure
```rust
pub struct AtifComponentsStory;

impl Render for AtifComponentsStory {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        Story::container()
            .child(Story::title("ATIF Components"))
            // Section 1: Source Badges
            // Section 2: Step Views (collapsed/expanded)
            // Section 3: Tool Calls
            // Section 4: Observations
            // Section 5: Metrics
            // Section 6: Category Badges
            // Section 7: Thread Items (all 5 types)
            // Section 8: Trajectory Detail States
            // Section 9: Trajectory List States
    }
}
```

### Dependencies to Add
In `crates/storybook/Cargo.toml`:
```toml
commander = { path = "../commander" }
atif = { path = "../atif" }
atif-store = { path = "../atif-store" }
chrono = "0.4"
```

### Wire Up Story
1. Add `mod atif_components;` to `stories/mod.rs`
2. Add `AtifComponents` variant to `ComponentStory` enum in `main.rs`
3. Export the story struct

---

## Prerequisite: Make Commander a Library

Commander is currently binary-only. To import its components into storybook, we need to add a lib.rs.

**Create `crates/commander/src/lib.rs`:**
```rust
//! Commander library - ATIF visualization components

pub mod components;
```

This exposes the components module for other crates to import.

---

## Files to Modify

| File | Action |
|------|--------|
| `crates/commander/src/lib.rs` | **CREATE** - Export components module |
| `crates/storybook/Cargo.toml` | Add dependencies |
| `crates/storybook/src/main.rs` | Add AtifComponents to enum |
| `crates/storybook/src/stories/mod.rs` | Add module export |
| `crates/storybook/src/stories/atif_components.rs` | **CREATE** - Main story file |

---

## Sample Data Strategy

Create mock data inline in the story file:
- Sample `Step` objects with various configurations
- Sample `ThreadItem` variants
- Sample `TrajectoryMetadata` for list items
- Sample `Trajectory` for detail view

Use `chrono::Utc::now()` for timestamps.
