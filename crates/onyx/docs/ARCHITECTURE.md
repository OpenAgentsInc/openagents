# Onyx Architecture

## Overview

Onyx is a local-first Markdown note editor built on WGPUI. It provides Obsidian-style live inline formatting where markdown renders in-place as you type.

## Crate Boundaries

```
┌─────────────────────────────────────────────────────────────┐
│                         onyx                                 │
│  Application shell: window, menus, keybindings, vault mgmt  │
└─────────────────────────────────────────────────────────────┘
                              │
           ┌──────────────────┼──────────────────┐
           ▼                  ▼                  ▼
┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐
│    document     │  │     wgpui       │  │     editor      │
│  Storage, index │  │  UI components  │  │   Text buffer   │
│   frontmatter   │  │  LiveEditor     │  │   undo/redo     │
└─────────────────┘  └─────────────────┘  └─────────────────┘
```

### onyx (this crate)

Application-level concerns:
- Window creation and event loop
- Vault management (open folder, list files)
- Configuration loading/saving
- Keybinding dispatch
- File watchers for external changes

### document

Storage and indexing:
- File I/O abstraction
- YAML frontmatter parsing/writing
- Wiki-link `[[extraction]]` and resolution
- Search index (SQLite-backed)
- Backlinks computation

### wgpui/components

Reusable UI components:
- `LiveEditor` - Multi-line editor with live markdown formatting
- `FileTree` - Hierarchical file browser
- `TabBar` - Document tabs
- `QuickSwitcher` - Fuzzy search modal
- `StatusBar` - Info bar

### editor (existing)

Text buffer operations:
- Ropey rope data structure for efficient edits
- Undo/redo stack
- Multi-cursor support
- Selection management

## Data Flow

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│  File System │────▶│   Document   │────▶│  TextBuffer  │
│    (.md)     │     │   (parse)    │     │   (Ropey)    │
└──────────────┘     └──────────────┘     └──────────────┘
                                                 │
                                                 ▼
                     ┌──────────────────────────────────────┐
                     │           Block Parser               │
                     │  Identifies markdown block boundaries │
                     └──────────────────────────────────────┘
                                                 │
                     ┌───────────────────────────┴───────────┐
                     ▼                                       ▼
              ┌─────────────┐                      ┌─────────────┐
              │ Active Block│                      │Other Blocks │
              │ (raw text)  │                      │ (formatted) │
              └─────────────┘                      └─────────────┘
                     │                                       │
                     └───────────────────┬───────────────────┘
                                         ▼
                               ┌──────────────────┐
                               │   LiveEditor     │
                               │  (GPU render)    │
                               └──────────────────┘
```

### Read Path

1. **File Load**: `document::storage` reads .md file from disk
2. **Frontmatter Parse**: Extract YAML metadata, remainder is content
3. **Buffer Init**: Content loaded into `editor::TextBuffer` (Ropey)
4. **Block Parse**: Identify markdown block boundaries (headers, paragraphs, code blocks, lists)
5. **Render**: LiveEditor renders each block based on cursor position

### Write Path

1. **Edit**: User types in LiveEditor
2. **Buffer Update**: Changes applied to Ropey buffer with undo tracking
3. **Block Reparse**: Only affected block boundaries are recalculated
4. **Render**: Changed blocks re-rendered
5. **Save**: On Ctrl+S or autosave, buffer serialized back to .md with frontmatter

## Component Hierarchy

```
App
├── Vault (manages open folder)
│   └── Index (search, backlinks cache)
│
├── DocumentManager
│   └── Document[] (open files)
│       ├── Frontmatter
│       └── TextBuffer
│
└── UI
    ├── FileTree (left sidebar)
    ├── TabBar (top)
    ├── LiveEditor (center, per-document)
    ├── StatusBar (bottom)
    └── QuickSwitcher (modal overlay)
```

## State Management

### Application State (onyx)

```rust
pub struct App {
    vault: Option<Vault>,           // Currently open vault
    documents: DocumentManager,      // Open documents
    active_document: Option<usize>, // Index of focused doc
    config: Config,                  // User preferences
}
```

### Document State (document)

```rust
pub struct Document {
    path: PathBuf,                  // File path in vault
    frontmatter: Frontmatter,       // Parsed YAML metadata
    buffer: TextBuffer,             // Ropey buffer from editor crate
    dirty: bool,                    // Unsaved changes
}
```

### Editor State (wgpui/LiveEditor)

```rust
pub struct LiveEditorState {
    cursor: Cursor,                 // Line, column position
    selection: Option<Selection>,   // Selected range
    scroll_offset: f32,             // Vertical scroll position
    blocks: Vec<Block>,             // Parsed block boundaries
    active_block: usize,            // Block containing cursor
}
```

## Live Formatting Architecture

The key innovation of Onyx is live inline formatting. Here's how it works:

### Block Model

The document is divided into **blocks**:
- Paragraph (text separated by blank lines)
- Header (lines starting with #)
- Code block (``` fenced regions)
- List (- or 1. prefixed lines)
- Blockquote (> prefixed lines)

Each block has:
- Start/end line numbers
- Block type
- Rendered height (cached for scrolling)

### Cursor-Aware Rendering

```
┌─────────────────────────────────────────────┐
│ # My Note                    ← Formatted H1 │
│                                             │
│ This is a paragraph with **bold** text.     │
│                              ↑ formatted    │
│                                             │
│ Another paragraph I'm **edit|ing** now.     │
│                    ↑ raw syntax visible     │
│                    (cursor in this block)   │
│                                             │
│ - List item one              ← Formatted    │
│ - List item two              ← Formatted    │
└─────────────────────────────────────────────┘
```

### Rendering Algorithm

```rust
fn render_document(blocks: &[Block], cursor: &Cursor, cx: &mut PaintContext) {
    for (i, block) in blocks.iter().enumerate() {
        if i == cursor.block_index {
            // Cursor is in this block - render raw markdown
            render_raw_text(block, cx);
        } else {
            // Cursor elsewhere - render formatted
            render_formatted(block, cx);
        }
    }
}
```

### Format Switching

When cursor moves between blocks:
1. Previous block transitions from raw → formatted
2. New block transitions from formatted → raw
3. Optionally animate the transition (fade/morph)

This requires:
- Efficient block boundary detection on every cursor move
- Caching formatted renders for non-active blocks
- Fast raw text rendering for the active block

## Performance Considerations

### Large Documents

- **Viewport rendering**: Only render visible blocks
- **Block height caching**: Pre-compute heights to enable fast scrolling
- **Incremental parsing**: Only reparse blocks that changed

### Many Documents

- **Lazy loading**: Don't load documents until tab is focused
- **Buffer pooling**: Reuse TextBuffer instances

### Search

- **SQLite FTS5**: Full-text search with ranking
- **Incremental indexing**: Update index on file save
- **Background indexing**: Don't block UI during initial index

## File System Integration

### Watching for Changes

```rust
// Using notify crate
let watcher = notify::recommended_watcher(|event| {
    match event {
        Event::Modify(path) => reload_if_not_dirty(path),
        Event::Create(path) => add_to_file_tree(path),
        Event::Remove(path) => remove_from_file_tree(path),
    }
})?;
watcher.watch(vault_path, RecursiveMode::NonRecursive)?;
```

### Conflict Resolution

If a file is modified externally while dirty in editor:
1. Show notification to user
2. Offer: Keep mine / Load external / Diff
3. If "Load external", lose local changes

## Keybindings

Default keybindings (configurable):

| Key | Action |
|-----|--------|
| Ctrl+S | Save |
| Ctrl+N | New note |
| Ctrl+P | Quick switcher |
| Ctrl+F | Find in document |
| Ctrl+Shift+F | Find in vault |
| Ctrl+[ | Outdent |
| Ctrl+] | Indent |
| Ctrl+B | Toggle bold |
| Ctrl+I | Toggle italic |
| Ctrl+K | Insert link |
| Ctrl+` | Toggle code |
