# Plan: Implement Plan Mode (TodoWrite) in MechaCoder

## Overview
Add support for Claude's TodoWrite tool to display real-time task progress in MechaCoder UI.

TodoWrite arrives as `tool_use` blocks with `name: "TodoWrite"` and `input.todos` array. Each todo has `{content, activeForm, status}`. Complete replacement semantics (each call sends ALL todos).

## Files to Modify

### 1. `crates/mechacoder/src/sdk_thread.rs` - Data + Detection

**Add new types** (after line 80):
```rust
/// Todo item status
#[derive(Clone, Debug, Default, PartialEq)]
pub enum TodoStatus {
    #[default]
    Pending,
    InProgress,
    Completed,
}

/// Single todo item from TodoWrite
#[derive(Clone, Debug)]
pub struct TodoItem {
    pub content: String,
    pub active_form: String,
    pub status: TodoStatus,
}

/// Current todo list state
#[derive(Clone, Debug, Default)]
pub struct TodoState {
    pub items: Vec<TodoItem>,
}
```

**Add to `SdkThreadEvent`** (line 16):
```rust
TodosUpdated,
```

**Add to `SdkThread` struct** (line 129):
```rust
todo_state: TodoState,
```

**Add parsing + accessors**:
```rust
impl TodoItem {
    pub fn parse_from_json(input: &str) -> Option<Vec<Self>> {
        let parsed: serde_json::Value = serde_json::from_str(input).ok()?;
        parsed.get("todos")?.as_array()?.iter().filter_map(|item| {
            Some(TodoItem {
                content: item.get("content")?.as_str()?.to_string(),
                active_form: item.get("activeForm")?.as_str()?.to_string(),
                status: match item.get("status")?.as_str()? {
                    "in_progress" => TodoStatus::InProgress,
                    "completed" => TodoStatus::Completed,
                    _ => TodoStatus::Pending,
                },
            })
        }).collect::<Vec<_>>().into()
    }
}

impl SdkThread {
    pub fn todo_state(&self) -> &TodoState { &self.todo_state }
    pub fn has_todos(&self) -> bool { !self.todo_state.items.is_empty() }
}
```

**Detect in ToolResult handler** (where tool_use.output is set):
```rust
if tool_use.tool_name == "TodoWrite" {
    if let Some(items) = TodoItem::parse_from_json(&tool_use.input) {
        this.todo_state = TodoState { items };
        cx.emit(SdkThreadEvent::TodosUpdated);
    }
}
```

### 2. `crates/mechacoder/src/ui/todo_panel_view.rs` - NEW FILE

```rust
//! Todo panel for plan mode progress display.

use crate::sdk_thread::TodoState;
use gpui::{div, prelude::*, px, IntoElement, ParentElement, Styled};
use theme_oa::{bg, border, status, text};

pub fn render_todo_panel(state: &TodoState) -> impl IntoElement {
    if state.items.is_empty() {
        return div().into_any_element();
    }

    let completed = state.items.iter().filter(|t| t.status == TodoStatus::Completed).count();
    let total = state.items.len();
    let progress = completed as f32 / total as f32;

    div()
        .w_full()
        .bg(bg::CARD)
        .border_b_1()
        .border_color(border::DEFAULT)
        .px(px(16.0))
        .py(px(12.0))
        // Header
        .child(
            div().flex().items_center().gap(px(8.0))
                .child(div().text_sm().font_medium().child("Plan"))
                .child(div().text_xs().text_color(text::SECONDARY)
                    .child(format!("{}/{}", completed, total)))
        )
        // Progress bar
        .child(
            div().w_full().h(px(4.0)).mt(px(8.0)).bg(bg::SURFACE).rounded(px(2.0))
                .child(div().h_full().w(relative(progress)).bg(status::SUCCESS).rounded(px(2.0)))
        )
        // Items
        .child(
            div().mt(px(8.0)).flex().flex_col().gap(px(4.0))
                .children(state.items.iter().map(render_todo_item))
        )
        .into_any_element()
}

fn render_todo_item(item: &TodoItem) -> impl IntoElement {
    let (color, icon) = match item.status {
        TodoStatus::Pending => (status::PENDING, "○"),
        TodoStatus::InProgress => (status::RUNNING, "◐"),
        TodoStatus::Completed => (status::SUCCESS, "●"),
    };
    let text = if item.status == TodoStatus::InProgress { &item.active_form } else { &item.content };

    div().flex().items_center().gap(px(8.0))
        .child(div().text_color(color).child(icon))
        .child(div().text_sm()
            .text_color(if item.status == TodoStatus::Completed { text::SECONDARY } else { text::PRIMARY })
            .child(text.clone()))
}
```

### 3. `crates/mechacoder/src/ui/mod.rs`

Add:
```rust
pub mod todo_panel_view;
```

### 4. `crates/mechacoder/src/ui/thread_view.rs`

**Add import**:
```rust
use super::todo_panel_view::render_todo_panel;
```

**Handle new event** (in `handle_thread_event`):
```rust
SdkThreadEvent::TodosUpdated => {
    cx.notify();
}
```

**Add to render** (above message list):
```rust
// In Render impl, before the list
.child(render_todo_panel(self.thread.read(cx).todo_state()))
```

## Implementation Order

1. Add data types to `sdk_thread.rs` (TodoStatus, TodoItem, TodoState)
2. Add `TodosUpdated` event variant
3. Add `todo_state` field and accessors to SdkThread
4. Add TodoItem::parse_from_json
5. Add detection in ToolResult handler
6. Create `todo_panel_view.rs`
7. Update `ui/mod.rs`
8. Update `thread_view.rs` to render panel

## Testing

Run MechaCoder and give it a multi-step task. It should use TodoWrite and display progress in the header panel above messages.
