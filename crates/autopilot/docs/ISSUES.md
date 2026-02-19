# Issues in Autopilot

This document describes how the autopilot UI integrates with the issue tracking system. For the core issue library API, see [`crates/issues/README.md`](../../issues/README.md).

## Overview

Autopilot uses issues as its primary work queue. During startup, it analyzes the codebase and suggests issues to work on. After completing a task, it verifies the work and either marks the issue complete or retries.

**Key components:**
- **Inline Issue Selector** - Boot-time UI for picking an issue
- **Boot Flow** - Streaming analysis and suggestion generation
- **Post-Completion Verification** - DSPy-based verification after task completion
- **State Management** - SQLite snapshot loading and status tracking

## Inline Issue Selector

The inline issue selector is a card displayed in the chat during boot that lets users pick an issue to work on.

**Source:** `src/app/chat/state.rs:81-115` (`InlineIssueSelector`)

### Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `1-9` | Select issue by index |
| `S` | Skip selection (dismiss selector) |
| `Escape` | Dismiss selector |

When a key is pressed, the handler in `src/app_entry/commands.rs:1000-1055` processes it:

```rust
// Simplified flow
if state.input_focus == InputFocus::IssueSelector {
    match key {
        Escape => dismiss_selector(),
        Character("s") => skip_selection(),
        Character("1".."9") => select_issue(index),
    }
}
```

### Structure

```rust
pub struct InlineIssueSelector {
    /// Issue suggestions to display (max 9)
    pub suggestions: Vec<IssueSuggestionDisplay>,
    /// Number of issues filtered out (stale/blocked)
    pub filtered_count: usize,
    /// Confidence score from the LLM (0.0 - 1.0)
    pub confidence: f32,
    /// Whether awaiting user selection
    pub await_selection: bool,
    /// Index of currently hovered suggestion (for UI highlighting)
    pub hovered_index: Option<usize>,
    /// Computed bounds for each suggestion button (for click detection)
    pub suggestion_bounds: Vec<Bounds>,
    /// Computed bounds for the skip button
    pub skip_button_bounds: Option<Bounds>,
}
```

### Selection Flow

1. User presses a number key (1-9)
2. Handler looks up the corresponding suggestion
3. Creates a `PendingValidation` with issue details
4. Transitions to `ModalState::ValidatingIssue`
5. Dismisses the selector and returns focus to chat input

## Boot Flow

During startup, autopilot analyzes the codebase and suggests issues through a streaming process.

**Source:** `src/app/chat/state.rs:45-73` (`BootSections`)

### Boot Sections

```rust
pub struct BootSections {
    /// Initialize section (Hardware, Compute, Network, Identity, Workspace, Summary)
    pub initialize: BootSection,
    /// Issue suggestion section
    pub suggest_issues: BootSection,
    /// Bounds of the boot card (for click detection)
    pub card_bounds: Option<Bounds>,
    /// Streaming text for issue analysis (rendered in suggest_issues card)
    pub streaming_text: String,
    /// Scroll offset for streaming text viewport
    pub streaming_scroll_offset: f32,
}
```

### Flow

1. **Initialization** - Hardware, compute, network, identity, workspace detection
2. **Issue Suggestion** - Streaming analysis of codebase
   - LLM analyzes workspace and existing issues
   - Filters out stale/blocked issues
   - Ranks remaining issues by relevance
   - Displays streaming tokens in the boot card
3. **Selector Display** - When suggestions are ready:
   - Boot section marked complete (`SectionStatus::Success`)
   - `InlineIssueSelector` created with suggestions
   - `InputFocus` set to `IssueSelector` for keyboard capture

**Trigger:** `src/app_entry/coder_actions.rs:1636-1646`

## Post-Completion Verification

After a task is completed, autopilot runs DSPy-based verification to check if the work satisfies the issue requirements.

**Source:** `src/app/autopilot/post_completion.rs`

### PostCompletionHook

```rust
pub struct PostCompletionHook {
    pipeline: VerificationPipeline,
}
```

### Verification Flow

1. **Extract Requirements** - Parse issue title and description for requirements
   - Title becomes primary requirement
   - Bullet points and numbered items from description become additional requirements
2. **Get Code Changes** - Run `git diff HEAD` to capture changes
3. **Run Verification** - DSPy pipeline evaluates:
   - Requirements vs solution summary
   - Code changes vs expected behavior
4. **Process Verdict**:
   - `Pass` → Mark issue complete, get next issue (if continuous mode)
   - `Fail/Retry` → Retry if attempts < 1, else unclaim and move to next

### PostCompletionEvent

```rust
pub struct PostCompletionEvent {
    pub issue_id: String,           // Issue UUID
    pub issue_number: i32,          // Display number
    pub issue_title: String,        // For retry prompts
    pub issue_description: Option<String>,
    pub workspace_root: PathBuf,
    pub task_summary: String,       // From assistant response
    pub retry_count: u8,            // Current attempt (max 1 retry)
    pub autopilot_continuous: bool, // Auto-start next issue?
}
```

### PostCompletionResult

```rust
pub enum PostCompletionResult {
    /// Verification passed, issue marked complete
    Success { issue_number: i32, next_issue: Option<NextIssueInfo> },
    /// Verification failed, should retry
    RetryNeeded { issue_number: i32, issue_title: String, reason: String },
    /// Retry exhausted, moving to next issue
    MovingToNext { failed_issue_number: i32, reason: String, next_issue: Option<NextIssueInfo> },
    /// No more issues available
    NoMoreIssues,
    /// Error during processing
    Error(String),
}
```

### Database Operations

The hook interacts directly with the SQLite database:

```rust
// Mark issue complete
fn complete_issue_in_db(&self, issue_id: &str, workspace_root: &PathBuf) -> Result<()> {
    let db_path = workspace_root.join(".openagents").join("autopilot.db");
    let conn = Connection::open(&db_path)?;
    issues::issue::complete_issue(&conn, issue_id)?;
    Ok(())
}

// Get next ready issue
fn get_next_issue(&self, workspace_root: &PathBuf) -> Result<Option<NextIssueInfo>> {
    let db_path = workspace_root.join(".openagents").join("autopilot.db");
    let conn = Connection::open(&db_path)?;
    issues::issue::get_next_ready_issue(&conn, Some("codex"))
}
```

## State Management

Autopilot maintains a snapshot of issues loaded from the SQLite database.

**Source:** `src/app/autopilot_issues.rs`

### AutopilotIssuesState

```rust
pub struct AutopilotIssuesState {
    pub status: AutopilotIssuesStatus,
    pub snapshot: AutopilotIssuesSnapshot,
    pub last_refresh: Option<u64>,
}
```

### Status Values

| Status | Meaning |
|--------|---------|
| `Idle` | Ready, snapshot loaded |
| `Refreshing` | Loading snapshot |
| `NoWorkspace` | No workspace root detected |
| `MissingDatabase` | `.openagents/autopilot.db` not found |
| `Error(msg)` | Database or query error |

### Snapshot Loading

The `load_snapshot()` function reads all issues from SQLite:

```rust
fn load_snapshot(workspace_root: Option<&Path>) -> (Status, Snapshot) {
    let db_path = root.join(".openagents").join("autopilot.db");
    let conn = Connection::open_with_flags(&db_path, SQLITE_OPEN_READ_ONLY)?;
    let issues = issues::list_issues(&conn, None)?;
    // Returns all issues for display in UI
}
```

### Issue Sorting

Issues are sorted for display by:
1. State rank: Blocked → InProgress → Open → Done
2. Priority rank: Urgent → High → Medium → Low
3. Issue number (ascending)

## Related Documentation

- [`crates/issues/README.md`](../../issues/README.md) - Core issue library API
- [`crates/issues/docs/schema.md`](../../issues/docs/schema.md) - Database schema
- [`docs/adr/ADR-0019-wgpui-hud-component-contract.md`](../../../docs/adr/ADR-0019-wgpui-hud-component-contract.md) - UI component contract
