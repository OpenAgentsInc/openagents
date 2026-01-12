# Onyx Roadmap

Milestone-based development plan for the Onyx Markdown editor.

## Milestone 1: Core Text Editor

Build a functional multi-line text editor without markdown formatting.

### Deliverables

#### 1.1 LiveEditor Component Shell

Create the basic component structure in wgpui.

**Files:**
- `crates/wgpui/src/components/live_editor/mod.rs`
- `crates/wgpui/src/components/live_editor/cursor.rs`

**Tasks:**
- [ ] Create `LiveEditor` struct with `TextBuffer` from editor crate
- [ ] Implement `Component` trait (paint, event)
- [ ] Basic monospace text rendering
- [ ] Scroll offset tracking

#### 1.2 Cursor Rendering

Implement cursor display and movement.

**Tasks:**
- [ ] Render blinking caret at cursor position
- [ ] Calculate cursor X position from column (handle variable-width chars)
- [ ] Cursor movement: arrow keys, Home/End
- [ ] Mouse click to position cursor
- [ ] Preserve preferred column on vertical movement

#### 1.3 Text Editing

Basic text input and manipulation.

**Tasks:**
- [ ] Character insertion at cursor
- [ ] Backspace and Delete
- [ ] Enter for newlines
- [ ] Tab handling (insert spaces or tab char)

#### 1.4 Selection

Text selection with mouse and keyboard.

**Tasks:**
- [ ] Shift+arrow to extend selection
- [ ] Click and drag to select
- [ ] Double-click to select word
- [ ] Triple-click to select line
- [ ] Ctrl+A to select all
- [ ] Selection highlight rendering

#### 1.5 Clipboard

Copy, cut, paste operations.

**Tasks:**
- [ ] Ctrl+C to copy selection
- [ ] Ctrl+X to cut selection
- [ ] Ctrl+V to paste
- [ ] Handle multi-line paste

#### 1.6 Scrolling

Scroll for documents larger than viewport.

**Tasks:**
- [ ] Vertical scrollbar
- [ ] Mouse wheel scrolling
- [ ] Keep cursor visible when typing (scroll into view)
- [ ] Page Up/Page Down

### Success Criteria

- Can open a text file and edit it
- All standard text editing operations work
- Scrolling works for large files
- No visible lag when typing

---

## Milestone 2: Live Markdown Formatting

Add the signature Onyx feature: live inline formatting.

### Deliverables

#### 2.1 Block Parser

Parse document into markdown blocks.

**Files:**
- `crates/wgpui/src/components/live_editor/block.rs`

**Tasks:**
- [ ] Identify block boundaries (paragraph, header, code, list, quote)
- [ ] Track which block contains the cursor
- [ ] Incremental re-parsing on edit
- [ ] Cache block heights for scrolling

#### 2.2 Formatted Block Rendering

Render non-active blocks as formatted markdown.

**Files:**
- `crates/wgpui/src/components/live_editor/format.rs`

**Tasks:**
- [ ] Header rendering with scaled fonts (H1-H6)
- [ ] Bold/italic/strikethrough detection and styling
- [ ] Code span rendering with background
- [ ] Link rendering (clickable appearance)
- [ ] Blockquote with left border
- [ ] List rendering with bullets/numbers

#### 2.3 Code Block Rendering

Special handling for fenced code blocks.

**Tasks:**
- [ ] Detect ``` fence with language
- [ ] Syntax highlighting (reuse wgpui/markdown/highlighter)
- [ ] Code block background
- [ ] Copy button on hover

#### 2.4 Active Block Transition

Smooth transition when cursor moves between blocks.

**Tasks:**
- [ ] Detect block change on cursor move
- [ ] Switch previous block to formatted
- [ ] Switch new block to raw
- [ ] Optional: animate transition

### Success Criteria

- Headers appear large and bold when not editing
- **Bold** and *italic* render correctly when not editing
- Code blocks have syntax highlighting
- Editing a line shows raw markdown syntax
- Moving cursor immediately updates formatting

---

## Milestone 3: File Management

Full document lifecycle with vault support.

### Deliverables

#### 3.1 Document Crate

Create the document management crate.

**Files:**
- `crates/document/Cargo.toml`
- `crates/document/src/lib.rs`
- `crates/document/src/storage.rs`
- `crates/document/src/frontmatter.rs`

**Tasks:**
- [ ] Create crate with serde, serde_yaml, rusqlite deps
- [ ] File read/write with proper encoding
- [ ] Frontmatter parsing and serialization
- [ ] Document struct with content + metadata

#### 3.2 Vault Management

Open and manage a folder of notes.

**Files:**
- `crates/onyx/src/vault.rs`

**Tasks:**
- [ ] Open folder dialog
- [ ] List .md files in folder
- [ ] Watch for external file changes (notify crate)
- [ ] Handle file conflicts

#### 3.3 FileTree Component

File browser sidebar.

**Files:**
- `crates/wgpui/src/components/file_tree.rs`

**Tasks:**
- [ ] Flat list of files (no directories)
- [ ] Click to open file
- [ ] Right-click context menu (rename, delete)
- [ ] New file button
- [ ] Search/filter input

#### 3.4 TabBar Component

Document tabs for multiple open files.

**Files:**
- `crates/wgpui/src/components/tab_bar.rs`

**Tasks:**
- [ ] Horizontal tab strip
- [ ] Active tab highlighting
- [ ] Close button on each tab
- [ ] Dirty indicator (dot or *)
- [ ] Drag to reorder (optional)

#### 3.5 Save Operations

Save documents to disk.

**Tasks:**
- [ ] Ctrl+S to save current document
- [ ] Update frontmatter modified timestamp
- [ ] Prompt unsaved changes on close
- [ ] Autosave option

### Success Criteria

- Can open a folder as vault
- File tree shows all .md files
- Can open multiple files in tabs
- Save works and preserves frontmatter
- External file changes are detected

---

## Milestone 4: Note Features

Wiki-links, search, and navigation.

### Deliverables

#### 4.1 Wiki-Link Support

Parse and render [[wiki-links]].

**Files:**
- `crates/document/src/links.rs`

**Tasks:**
- [ ] Extract [[links]] from content
- [ ] Render links in formatted mode (styled, clickable)
- [ ] Show raw [[syntax]] when editing
- [ ] Ctrl+click to follow link
- [ ] Create new note if link target doesn't exist

#### 4.2 Search Index

Full-text search across vault.

**Files:**
- `crates/document/src/index.rs`

**Tasks:**
- [ ] SQLite database with FTS5
- [ ] Index notes on vault open
- [ ] Incremental index on save
- [ ] Search API returning ranked results

#### 4.3 QuickSwitcher Component

Fuzzy file finder modal.

**Files:**
- `crates/wgpui/src/components/quick_switcher.rs`

**Tasks:**
- [ ] Ctrl+P to open modal
- [ ] Fuzzy search by filename
- [ ] Show search results
- [ ] Keyboard navigation (up/down, enter)
- [ ] Create new note from search

#### 4.4 Backlinks

Show notes linking to current note.

**Tasks:**
- [ ] Query links table for backlinks
- [ ] Display in sidebar or panel
- [ ] Click to navigate to source

#### 4.5 Tags

Tag support in frontmatter.

**Tasks:**
- [ ] Parse tags from frontmatter
- [ ] Tag index in database
- [ ] Filter by tag in file tree
- [ ] Tag autocomplete in frontmatter

### Success Criteria

- [[Links]] work and navigate
- Ctrl+P opens quick switcher
- Search finds content across vault
- Backlinks show in sidebar
- Tags are indexed and filterable

---

## Milestone 5: Polish

Quality of life improvements and advanced features.

### Deliverables

#### 5.1 Vim Keybindings (Optional Mode)

Modal editing for vim users.

**Tasks:**
- [ ] Normal/Insert/Visual mode tracking
- [ ] Basic motions: h, j, k, l, w, b, e
- [ ] Operators: d, c, y
- [ ] Visual selection
- [ ] : command line

#### 5.2 Find and Replace

In-document search.

**Tasks:**
- [ ] Ctrl+F to open find bar
- [ ] Highlight all matches
- [ ] Next/previous navigation
- [ ] Replace and replace all

#### 5.3 StatusBar Component

Information bar at bottom of window.

**Files:**
- `crates/wgpui/src/components/status_bar.rs`

**Tasks:**
- [ ] Current file name
- [ ] Line:column position
- [ ] Word count
- [ ] Dirty indicator
- [ ] Vim mode indicator (if enabled)

#### 5.4 Undo/Redo UI

Visual feedback for undo/redo.

**Tasks:**
- [ ] Ctrl+Z to undo
- [ ] Ctrl+Shift+Z to redo
- [ ] Toast notification on undo/redo
- [ ] Undo history panel (optional)

#### 5.5 Keyboard Shortcut Help

Discoverable keybindings.

**Tasks:**
- [ ] ? or F1 to show cheat sheet
- [ ] Modal with all keybindings
- [ ] Searchable

### Success Criteria

- Vim mode works for common operations
- Find and replace works
- Status bar shows useful info
- Undo/redo feel responsive
- Users can discover keybindings

---

## Implementation Order

```
Milestone 1 ─────────────────────────────────────────────────┐
  1.1 LiveEditor shell                                       │
  1.2 Cursor ─────────────────────────────────────────────►  │
  1.3 Text editing ──────────────────────────────────────►   │
  1.4 Selection ─────────────────────────────────────────►   │
  1.5 Clipboard ─────────────────────────────────────────►   │
  1.6 Scrolling ─────────────────────────────────────────►   │
                                                             ▼
Milestone 2 ─────────────────────────────────────────────────┐
  2.1 Block parser ──────────────────────────────────────►   │
  2.2 Formatted rendering ───────────────────────────────►   │
  2.3 Code blocks ───────────────────────────────────────►   │
  2.4 Block transitions ─────────────────────────────────►   │
                                                             ▼
Milestone 3 ─────────────────────────────────────────────────┐
  3.1 Document crate ────────────────────────────────────►   │
  3.2 Vault management ──────────────────────────────────►   │
  3.3 FileTree component ────────────────────────────────►   │
  3.4 TabBar component ──────────────────────────────────►   │
  3.5 Save operations ───────────────────────────────────►   │
                                                             ▼
Milestone 4 ─────────────────────────────────────────────────┐
  4.1 Wiki-links ────────────────────────────────────────►   │
  4.2 Search index ──────────────────────────────────────►   │
  4.3 QuickSwitcher ─────────────────────────────────────►   │
  4.4 Backlinks ─────────────────────────────────────────►   │
  4.5 Tags ──────────────────────────────────────────────►   │
                                                             ▼
Milestone 5 ─────────────────────────────────────────────────┐
  5.1 Vim mode ──────────────────────────────────────────►   │
  5.2 Find/replace ──────────────────────────────────────►   │
  5.3 StatusBar ─────────────────────────────────────────►   │
  5.4 Undo/redo UI ──────────────────────────────────────►   │
  5.5 Keyboard help ─────────────────────────────────────►   │
                                                             ▼
                                                        LAUNCH
```

## Dependencies Between Milestones

- **M1 → M2**: Live formatting requires working text editor
- **M2 → M3**: File management needs formatted rendering for preview
- **M3 → M4**: Wiki-links need vault context for resolution
- **M4 → M5**: Polish builds on complete feature set

## Parallel Work Opportunities

Within milestones, some tasks can proceed in parallel:

**Milestone 1:**
- 1.4 Selection and 1.5 Clipboard can be developed together
- 1.6 Scrolling is independent

**Milestone 3:**
- 3.1 Document crate and 3.3 FileTree component are independent
- 3.4 TabBar is independent of file operations

**Milestone 4:**
- 4.2 Search index and 4.1 Wiki-links are independent
- 4.3 QuickSwitcher can be built before search is complete

## Risk Areas

1. **Live formatting performance**: Reparsing on every cursor move could be slow
   - Mitigation: Incremental parsing, only reparse affected blocks

2. **Large file handling**: Ropey helps but rendering could lag
   - Mitigation: Virtual rendering (only visible lines)

3. **Cross-platform text input**: Different IME behaviors
   - Mitigation: Test early on macOS, Windows, Linux

4. **Syntax highlighting overhead**: syntect can be slow
   - Mitigation: Cache highlighted code blocks, only re-highlight on change
