# Zed Crates Import Log

**Date:** 2025-12-10
**Goal:** Import Zed's markdown rendering infrastructure for MechaCoder

## Problem

Claude Code returns markdown in responses, but MechaCoder was displaying it as plain text. We needed proper markdown rendering with syntax highlighting.

## Solution: Import Zed Crates

Zed has a sophisticated `markdown` crate that handles:
- Markdown parsing with pulldown-cmark
- Syntax highlighting via tree-sitter
- Rich text rendering in GPUI

### The Challenge

Zed's `markdown` crate has deep dependencies:
- `language` (for syntax highlighting)
- `theme` (for colors)
- `ui` (for components)
- And ~200 other crates

These crates expect Zed's specific `theme` and `ui` APIs, which conflict with our existing OpenAgents theme/ui crates.

## What We Did

### 1. Copied All Zed Crates (~200 crates)

```bash
cp -rL /Users/christopherdavid/code/zed/crates/* ./crates/
```

This brought in the full Zed infrastructure including:
- `markdown` - the main target
- `language` - syntax highlighting
- `theme` - Zed's theme system
- `ui` - Zed's UI components
- Many supporting crates

### 2. Resolved Dependency Conflicts

**SQLite version conflict:**
- Our crates used `rusqlite 0.31`
- Zed uses `libsqlite3-sys 0.30`
- Fixed by upgrading our crates to `rusqlite 0.32`

**Polyfills needed:**
- `str::floor_char_boundary` and `ceil_char_boundary` - added polyfills to `rope` crate since edition 2024 doesn't have them stabilized

**Stub implementations:**
- `denoise` crate - replaced ONNX-dependent engine with pass-through stub (no model files needed)
- Created empty `assets/` folder structure for `assets` crate

### 3. Separated Theme and UI Crates

The core insight: **keep Zed's crates as-is, rename ours**.

#### Theme Separation

| Crate | Purpose |
|-------|---------|
| `theme` | Zed's theme system (used by Zed crates) |
| `theme_oa` | Our original OpenAgents colors |

#### UI Separation

| Crate | Purpose |
|-------|---------|
| `ui` | Zed's UI components (used by Zed crates) |
| `ui_oa` | Our original shadcn-style components |

### 4. Updated Our Crates

All OpenAgents crates were updated to use the `_oa` versions:

**Cargo.toml changes:**
```toml
# Before
theme = { path = "../theme" }
ui = { path = "../ui" }

# After
theme_oa = { path = "../theme_oa" }
ui_oa = { path = "../ui_oa" }
```

**Source file changes:**
```rust
// Before
use theme::{bg, text, border};
use ui::Button;

// After
use theme_oa::{bg, text, border};
use ui_oa::Button;
```

Affected crates:
- `mechacoder`
- `commander`
- `hud`
- `vibe`
- `gym`
- `marketplace`
- `storybook`
- `ui_oa` (uses `theme_oa`)

### 5. Fixed API Changes

**async-tungstenite 0.31:**
- `WebSocketStream::split()` now returns `WebSocketSender/WebSocketReceiver` instead of `SplitSink/SplitStream`
- `close()` now takes `Option<CloseFrame>` parameter
- Updated `nostr-relay` crate accordingly

**tungstenite Message API:**
- `WsMessage::Text(String)` now requires `WsMessage::Text(string.into())` for `Utf8Bytes`

## Current State

### Compiling Crates

- `mechacoder` ✅
- `commander` ✅
- `hud` ✅
- Most Zed crates ✅

### Known Issues

Some Zed crates have unresolved issues:
- `client` - telemetry macro issues, TLS trait bounds
- These don't block our main crates

## File Structure After Changes

```
crates/
├── theme/          # Zed's theme (for Zed crates)
├── theme_oa/       # Our theme (for OpenAgents crates)
├── ui/             # Zed's UI (for Zed crates)
├── ui_oa/          # Our UI (for OpenAgents crates)
├── markdown/       # Zed's markdown renderer
├── language/       # Zed's syntax highlighting
├── mechacoder/     # Uses theme_oa, ui_oa, can use markdown
└── ... (~200 more crates)
```

## Next Steps

1. ~~Integrate `markdown` crate into MechaCoder's message view~~ ✅ DONE
2. Fix remaining Zed crate issues if needed
3. Consider slimming down unused Zed crates later

---

## Update: Markdown Integration Complete

**Date:** 2025-12-11

### What Was Done

Integrated Zed's markdown rendering into MechaCoder's message view.

### Changes Made

#### 1. Updated `crates/mechacoder/Cargo.toml`

Added dependencies for markdown rendering:
```toml
markdown = { path = "../markdown" }
language = { path = "../language" }
theme = { path = "../theme" }
ui = { path = "../ui" }
```

#### 2. Rewrote `crates/mechacoder/src/ui/message_view.rs`

**Before:** `MessageView` was a simple struct implementing `IntoElement` that rendered text content directly.

**After:** `MessageView` is now a GPUI entity that:
- Stores an `Entity<Markdown>` for rich markdown rendering
- Implements `Render` trait instead of `IntoElement`
- Uses `MarkdownElement::new()` from Zed's markdown crate
- Creates a `MarkdownStyle` with proper theming

Key changes:
```rust
// MessageView now returns Entity<Self>, not Self
pub fn assistant(content: &str, cx: &mut App) -> Entity<Self> {
    cx.new(|cx| {
        let markdown = cx.new(|cx| Markdown::new(content.into(), None, None, cx));
        Self { role: MessageRole::Assistant, markdown }
    })
}

// Render uses MarkdownElement
impl Render for MessageView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let style = self.markdown_style(window, cx);
        div()
            .child(MarkdownElement::new(self.markdown.clone(), style))
    }
}
```

Also added `SimpleMessageView` as a fallback for non-markdown rendering.

#### 3. Updated `crates/mechacoder/src/ui/thread_view.rs`

**Key changes:**
- Added `message_cache: HashMap<usize, Entity<MessageView>>` to cache markdown entities
- Added `streaming_message: Option<Entity<MessageView>>` for streaming content
- Pre-populates cache in render to ensure markdown entities exist before list render
- Uses streaming view pattern for live updates

#### 4. Fixed `crates/text/src/locator.rs`

The original code used `SmallVec::from_const_with_len_unchecked` which isn't available in the version we have. Changed to:
```rust
// Before (broken)
pub const fn min() -> Self {
    Self(unsafe { SmallVec::from_const_with_len_unchecked([u64::MIN; 4], 1) })
}

// After (working)
pub fn min() -> Self {
    let mut v = SmallVec::new();
    v.push(u64::MIN);
    Self(v)
}
```

Also changed `min_ref()`/`max_ref()` from const functions to use `LazyLock` for static references.

### Current State

- **mechacoder compiles** ✅
- **markdown rendering works** ✅
- All assistant messages now render with:
  - Proper headings (h1-h6)
  - Code blocks with syntax highlighting (if language detected)
  - Inline code styling
  - Block quotes
  - Links with underlines
  - Lists (ordered and unordered)
  - Tables

### Architecture

```
MessageView (Entity)
├── role: MessageRole
└── markdown: Entity<Markdown>
    └── Zed's markdown parsing/rendering

ThreadView
├── message_cache: HashMap<usize, Entity<MessageView>>  # Cached per entry
├── streaming_message: Option<Entity<MessageView>>       # Live updates
└── list_state: ListState                                # Virtual list
```

### Styling

The `MarkdownStyle` uses:
- `theme_oa` colors for text/borders/backgrounds
- `theme::ActiveTheme` for syntax highlighting and Zed theme colors
- Custom code block styling with `bg::SURFACE` background

## Lessons Learned

1. **Don't fight dependencies** - Renaming our crates was cleaner than modifying Zed's
2. **Edition 2024 has quirks** - Some "stable" features still need polyfills
3. **Batch commits help** - Git push with large file counts needs `http.postBuffer` increase
4. **Stub what you don't need** - The denoise stub saved us from ONNX model files
