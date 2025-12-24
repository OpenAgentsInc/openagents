# ACP Component Parity Plan

## Objective
Create ACP (Agent Client Protocol) versions of ALL UI components needed to render Claude Code conversations, matching Zed's structural approach while using our current UI style (Maud/HTMX/Tailwind with sharp corners, semantic tokens).

## Decisions
- **Scope**: Both reusable components in `ui` crate AND stories in storybook
- **Rendering**: Static HTML with CSS highlighting (no xterm.js/Monaco JS deps)
- **Phasing**: All 35 components at once for complete structural parity

---

## Current State vs Target

| Category | Current | Target |
|----------|---------|--------|
| ACP Atoms | 0 | 12 |
| ACP Molecules | 0 | 10 |
| ACP Organisms | 0 | 9 |
| ACP Sections | 0 | 4 |
| **Total** | **0** | **35** |

---

## Architecture

### Dual-Crate Structure
Following the existing pattern (recorder components), each ACP component lives in TWO places:

1. **UI Crate** (`crates/ui/src/acp/`) - Reusable components
   - Atoms: Function-based, return `Markup`
   - Molecules/Organisms: Struct + builder pattern, `.build()` method
   - Exports via `crates/ui/src/lib.rs`

2. **Storybook** (`crates/storybook/src/stories/acp/`) - Stories/demos
   - Import from `ui` crate
   - Demonstrate variants, states, usage examples
   - Register routes in `main.rs`

### Component Patterns

**Atoms (function-based):**
```rust
// crates/ui/src/acp/atoms/tool_icon.rs
pub fn tool_icon(kind: ToolKind) -> Markup {
    html! { span class=(kind.class()) { (kind.icon_char()) } }
}
```

**Molecules/Organisms (struct + builder):**
```rust
// crates/ui/src/acp/molecules/tool_header.rs
pub struct ToolHeader {
    kind: ToolKind,
    label: String,
    status: ToolStatus,
}

impl ToolHeader {
    pub fn new(kind: ToolKind, label: &str) -> Self { ... }
    pub fn status(mut self, status: ToolStatus) -> Self { self.status = status; self }
    pub fn build(self) -> Markup { ... }
}
```

### Shared Styling
Create `crates/ui/src/acp/styles.rs`:
```rust
pub const ACP_CARD_CLASS: &str = "bg-card border border-border";
pub const ACP_HEADER_CLASS: &str = "px-3 py-2 border-b border-border flex items-center gap-2";
pub const ACP_CONTENT_CLASS: &str = "px-3 py-3 text-sm";
pub const ACP_PERMISSION_BAR_CLASS: &str = "px-3 py-2 border-t border-border flex gap-2";
```

---

## Phase 1: ACP Atoms (Foundation)

**UI Location**: `crates/ui/src/acp/atoms/`
**Story Location**: `crates/storybook/src/stories/acp/atoms/`

### 1.1 Tool Icons (`tool_icon.rs`)
Map each `acp::ToolKind` to an icon:
- `Read` → Search icon
- `Edit` → Pencil/file icon
- `Delete` → Trash icon
- `Execute` → Terminal icon
- `Search` → Magnifying glass
- `Think` → Brain icon
- `Fetch` → Globe icon
- `SwitchMode` → Arrow swap icon
- `Other` → Hammer icon

### 1.2 Tool Status Badge (`tool_status_badge.rs`)
States matching `ToolCallStatus`:
- `Running` → spinning indicator + "Running..."
- `Success` → green checkmark
- `Error` → red X with message
- `WaitingForConfirmation` → amber clock

### 1.3 Permission Button (`permission_button.rs`)
Four variants from `acp::PermissionOptionKind`:
- `AllowOnce` → "Allow" (primary)
- `AllowAlways` → "Always Allow" (secondary)
- `RejectOnce` → "Reject" (ghost)
- `RejectAlways` → "Always Reject" (destructive)

### 1.4 Mode Badge (`mode_badge.rs`)
Display current agent mode:
- `plan` → Plan mode indicator
- `code` → Code mode indicator
- `ask` → Ask mode indicator
- Custom modes supported

### 1.5 Model Badge (`model_badge.rs`)
Display model info:
- Model name (e.g., "claude-sonnet-4-20250514")
- Compact and full variants

### 1.6 Thinking Toggle (`thinking_toggle.rs`)
Expand/collapse control for thinking blocks:
- Collapsed: "Show thinking" with chevron
- Expanded: "Hide thinking" with chevron

### 1.7 Checkpoint Badge (`checkpoint_badge.rs`)
Indicates checkpoint availability:
- Visual indicator for restorable state
- Git SHA preview

### 1.8 Feedback Button (`feedback_button.rs`)
Thread rating:
- Thumbs up (positive)
- Thumbs down (negative)
- Active/inactive states

### 1.9 Content Type Icon (`content_type_icon.rs`)
Icons for `ToolCallContent` types:
- `ContentBlock` → Document icon
- `Diff` → Diff icon
- `Terminal` → Terminal icon

### 1.10 Entry Marker (`entry_marker.rs`)
Visual marker for entry types:
- User → User icon
- Assistant → AI icon
- Tool → Tool icon

### 1.11 Keybinding Hint (`keybinding_hint.rs`)
Show keyboard shortcuts:
- `y` → AllowOnce
- `Y` → AllowAlways
- `n` → RejectOnce
- `N` → RejectAlways

### 1.12 Streaming Indicator (`streaming_indicator.rs`)
Animated indicator for streaming content:
- Pulsing dot
- "Generating..." text

---

## Phase 2: ACP Molecules (Compositions)

Location: `crates/storybook/src/stories/acp/molecules/`

### 2.1 Tool Header (`tool_header.rs`)
Composition: `ToolIcon` + label + `ToolStatusBadge`
- Shows tool kind icon
- Displays tool label/name
- Status indicator on right

### 2.2 Permission Bar (`permission_bar.rs`)
Row of `PermissionButton` components:
- Groups: AllowOnce + AllowAlways | RejectOnce + RejectAlways
- Keybinding hints below buttons

### 2.3 Mode Selector (`mode_selector.rs`)
Dropdown for mode selection:
- Current mode display
- Available modes list
- Selection triggers `session/set_mode`

### 2.4 Model Selector (`model_selector.rs`)
Dropdown for model selection:
- Current model display
- Available models list
- Search/filter support

### 2.5 Message Header (`message_header.rs`)
Header for user/assistant messages:
- Entry marker + timestamp
- Edit button (for user messages)
- Copy button

### 2.6 Thinking Block (`thinking_block.rs`)
Collapsible thinking content:
- `ThinkingToggle` header
- Markdown content (collapsible)
- Subtle styling to differentiate from main content

### 2.7 Diff Header (`diff_header.rs`)
Header for diff content:
- File path
- Change stats (+X/-Y lines)
- File type icon

### 2.8 Terminal Header (`terminal_header.rs`)
Header for terminal output:
- Command preview
- Working directory
- Exit status

### 2.9 Checkpoint Restore (`checkpoint_restore.rs`)
Restore control:
- `CheckpointBadge`
- "Restore Checkpoint" button
- Confirmation state

### 2.10 Entry Actions (`entry_actions.rs`)
Action buttons for entries:
- Copy
- Regenerate (for user messages)
- Cancel editing

---

## Phase 3: ACP Organisms (Complex Components)

Location: `crates/storybook/src/stories/acp/organisms/`

### 3.1 User Message (`user_message.rs`)
Full user message component:
- `MessageHeader`
- Editable content area
- `CheckpointRestore` (if checkpoint exists)
- `EntryActions`

### 3.2 Assistant Message (`assistant_message.rs`)
Full assistant response:
- `MessageHeader`
- Markdown content chunks
- Embedded `ThinkingBlock` components
- `StreamingIndicator` (while streaming)

### 3.3 Tool Call Card (`tool_call_card.rs`)
Complete tool call display:
- `ToolHeader`
- Collapsible content area
- `PermissionBar` (if waiting for confirmation)
- Content (markdown/generic)

### 3.4 Terminal Tool Call (`terminal_tool_call.rs`)
Tool call with embedded terminal:
- `ToolHeader` (Execute kind)
- `TerminalHeader`
- Terminal output area (styled like xterm)
- Exit status

### 3.5 Diff Tool Call (`diff_tool_call.rs`)
Tool call with embedded diff:
- `ToolHeader` (Edit kind)
- `DiffHeader`
- Diff content (green/red highlighting)
- Syntax highlighting

### 3.6 Search Tool Call (`search_tool_call.rs`)
Tool call for file search:
- `ToolHeader` (Search/Read kind)
- File list with paths
- Match previews

### 3.7 Thread Controls (`thread_controls.rs`)
Top-of-thread controls:
- `ModeSelector`
- `ModelSelector`
- Plan display (todo items)

### 3.8 Permission Dialog (`permission_dialog.rs`)
Full permission request:
- Tool info
- Request details
- `PermissionBar`
- Keybinding reference

### 3.9 Thread Entry (`thread_entry.rs`)
Unified entry renderer:
- Routes to UserMessage, AssistantMessage, or ToolCallCard
- Entry index tracking
- Scroll handling

---

## Phase 4: ACP Sections (Page Layouts)

Location: `crates/storybook/src/stories/acp/sections/`

### 4.1 Thread Header (`thread_header.rs`)
Session header:
- Session ID
- Model info
- Mode indicator
- Connection status

### 4.2 Thread Feedback (`thread_feedback.rs`)
Post-completion feedback:
- "How was this response?"
- `FeedbackButton` (thumbs up/down)
- Optional comment field

### 4.3 Message Editor (`message_editor.rs`)
User input component:
- Textarea with @ mention support
- Submit button
- Keyboard hints (Enter to send)
- Attachment indicators

### 4.4 Thread View (`thread_view.rs`)
Full conversation view:
- `ThreadHeader`
- List of `ThreadEntry` components
- `MessageEditor` at bottom
- `ThreadFeedback` (after completion)

---

## Phase 5: Storybook Registration

### 5.1 Route Structure
```
/stories/acp                    → ACP Index
/stories/acp/atoms              → All atoms overview
/stories/acp/atoms/{name}       → Individual atom stories
/stories/acp/molecules          → All molecules overview
/stories/acp/molecules/{name}   → Individual molecule stories
/stories/acp/organisms          → All organisms overview
/stories/acp/organisms/{name}   → Individual organism stories
/stories/acp/sections           → All sections overview
/stories/acp/sections/{name}    → Individual section stories
/stories/acp/demo               → Full thread demo
```

### 5.2 Sidebar Navigation Update
Add "ACP" section to sidebar:
- ACP Index
- Atoms (12 items)
- Molecules (10 items)
- Organisms (9 items)
- Sections (4 items)
- Demo

---

## Implementation Order

### Step 1: UI Crate Foundation
Create module structure in `crates/ui/src/`:
```
acp/
├── mod.rs              # pub mod atoms, molecules, organisms, sections;
├── styles.rs           # Shared CSS class constants
├── atoms/
│   └── mod.rs          # pub mod + pub use for all atoms
├── molecules/
│   └── mod.rs
├── organisms/
│   └── mod.rs
└── sections/
    └── mod.rs
```

### Step 2: ACP Atoms (12 files in ui crate)
```
crates/ui/src/acp/atoms/
├── tool_icon.rs
├── tool_status_badge.rs
├── permission_button.rs
├── mode_badge.rs
├── model_badge.rs
├── thinking_toggle.rs
├── checkpoint_badge.rs
├── feedback_button.rs
├── content_type_icon.rs
├── entry_marker.rs
├── keybinding_hint.rs
└── streaming_indicator.rs
```

### Step 3: ACP Molecules (10 files in ui crate)
```
crates/ui/src/acp/molecules/
├── tool_header.rs
├── permission_bar.rs
├── mode_selector.rs
├── model_selector.rs
├── message_header.rs
├── thinking_block.rs
├── diff_header.rs
├── terminal_header.rs
├── checkpoint_restore.rs
└── entry_actions.rs
```

### Step 4: ACP Organisms (9 files in ui crate)
```
crates/ui/src/acp/organisms/
├── user_message.rs
├── assistant_message.rs
├── tool_call_card.rs
├── terminal_tool_call.rs
├── diff_tool_call.rs
├── search_tool_call.rs
├── thread_controls.rs
├── permission_dialog.rs
└── thread_entry.rs
```

### Step 5: ACP Sections (4 files in ui crate)
```
crates/ui/src/acp/sections/
├── thread_header.rs
├── thread_feedback.rs
├── message_editor.rs
└── thread_view.rs
```

### Step 6: Storybook Stories (mirror structure)
```
crates/storybook/src/stories/acp/
├── mod.rs
├── index.rs            # Overview page
├── demo.rs             # Full thread demo
├── atoms/
│   ├── mod.rs
│   └── [12 story files, one per atom]
├── molecules/
│   ├── mod.rs
│   └── [10 story files]
├── organisms/
│   ├── mod.rs
│   └── [9 story files]
└── sections/
    ├── mod.rs
    └── [4 story files]
```

### Step 7: Integration
1. Update `crates/ui/src/lib.rs` - add `pub mod acp;` and exports
2. Update `crates/storybook/src/stories/mod.rs` - add `pub mod acp;`
3. Update `crates/storybook/src/main.rs` - add all 35+ routes
4. Update `sidebar_nav()` - add ACP section with all links
5. Add tests in `crates/ui/tests/acp_components.rs`

---

## Component to Zed Mapping Reference

| Our Component | Zed Equivalent | Location in Zed |
|---------------|----------------|-----------------|
| `ThreadView` | `AcpThreadView` | `agent_ui/src/acp/thread_view.rs` |
| `UserMessage` | `UserMessage` render | `thread_view.rs:1940-2098` |
| `AssistantMessage` | `AssistantMessage` render | `thread_view.rs:2099-2138` |
| `ToolCallCard` | `render_tool_call()` | `thread_view.rs:2139-2527` |
| `TerminalToolCall` | `render_terminal_tool_call()` | `thread_view.rs` |
| `DiffToolCall` | `render_diff_editor()` | `thread_view.rs` |
| `ModeSelector` | `ModeSelector` | `acp/mode_selector.rs` |
| `ModelSelector` | `ModelSelector` | `acp/model_selector.rs` |
| `MessageEditor` | `MessageEditor` | `acp/message_editor.rs` |
| `PermissionBar` | Permission buttons | `thread_view.rs:2777-2799` |

---

## Styling Notes

Our style (from existing components):
- **No border radius** - sharp corners everywhere
- **Semantic tokens** - `bg-background`, `text-foreground`, `border-border`, etc.
- **Monospace** - `font-mono` for code/technical content
- **Minimal** - clean, no excessive decoration
- **Flex-based** - composition via flexbox
- **Helper functions** - `section()`, `row()`, `item()`, `code_block()`

---

## File Count Summary

| Location | New Files | Modified Files |
|----------|-----------|----------------|
| `crates/ui/src/acp/` | 40 | 0 |
| `crates/storybook/src/stories/acp/` | 40 | 0 |
| `crates/ui/src/lib.rs` | 0 | 1 |
| `crates/ui/tests/` | 1 | 0 |
| `crates/storybook/src/stories/mod.rs` | 0 | 1 |
| `crates/storybook/src/main.rs` | 0 | 1 |
| **Total** | **81** | **3** |

### Files to Modify (existing)

1. `crates/ui/src/lib.rs` - Add `pub mod acp;` and re-exports
2. `crates/storybook/src/stories/mod.rs` - Add `pub mod acp;`
3. `crates/storybook/src/main.rs` - Add ~40 routes + sidebar ACP section

---

## Success Criteria

- [ ] All 35 ACP components implemented
- [ ] Each component has a storybook story
- [ ] All stories registered in main.rs routes
- [ ] Sidebar navigation updated
- [ ] Demo page shows full thread interaction
- [ ] Matches Zed's structural approach
- [ ] Uses our UI style consistently
