# Onyx Development Log

## 2026-01-05: Milestone 1.1 - LiveEditor Component Shell

### Completed

**LiveEditor component (`crates/wgpui/src/components/live_editor/`)**

Created a full-featured multi-line text editor component:

- `mod.rs` - Main component with ~650 lines of code
- `cursor.rs` - Cursor and Selection types

**Features implemented:**

1. **Text Storage**
   - Simple `Vec<String>` line-based storage
   - Full content get/set with proper line handling

2. **Cursor**
   - Line/column tracking
   - Preferred column for vertical movement
   - Movement: left, right, up, down, home, end

3. **Selection**
   - Anchor + head model
   - Shift+arrow to extend selection
   - Ctrl+A to select all
   - Selection rendering with highlight

4. **Text Editing**
   - Character insertion
   - Backspace (with line merging)
   - Delete forward
   - Enter for newlines
   - Tab inserts 4 spaces
   - Selection deletion before insert

5. **Clipboard**
   - Ctrl+C copy
   - Ctrl+X cut
   - Ctrl+V paste
   - Multi-line paste support

6. **Scrolling**
   - Mouse wheel scrolling
   - Cursor kept in view after movement
   - Only visible lines rendered

7. **Rendering**
   - Line numbers gutter (left side)
   - Monospace text rendering
   - Cursor as 2px vertical line
   - Selection highlight per line
   - Configurable colors and sizing via `LiveEditorStyle`

**Onyx test app (`apps/onyx/`)**

Created minimal desktop app to test the editor:

- `src/main.rs` - Entry point with winit event loop
- `src/app.rs` - Application handler with wgpu setup
- Keyboard input conversion from winit to wgpui
- Mouse wheel scrolling
- Sample markdown content loaded by default

### Files Changed

```
crates/wgpui/src/components/
├── mod.rs                    # Added live_editor module export
└── live_editor/
    ├── mod.rs                # Main LiveEditor component (NEW)
    └── cursor.rs             # Cursor/Selection types (NEW)

apps/onyx/
├── Cargo.toml                # Updated dependencies
├── src/
│   ├── main.rs               # Updated with winit loop
│   └── app.rs                # New application handler
└── docs/
    └── DEVLOG.md             # This file (NEW)

Cargo.toml                    # Added onyx to workspace
```

### Testing

Run Onyx:
```bash
cargo run -p onyx
```

Expected behavior:
- Window opens with sample markdown content
- Arrow keys navigate cursor
- Typing inserts text
- Backspace/Delete work
- Shift+arrows select text
- Ctrl+C/X/V for clipboard
- Mouse wheel scrolls
- Ctrl+S prints "Save requested!" to console

### Bugfixes

**Cursor alignment fix** - Cursor was getting ahead of text because:
1. Text was rendering with proportional (sans) font but cursor position used fixed-width estimate
2. Fixed by switching all text to mono font (`layout_styled_mono`) and caching the actual measured char width via `measure_styled_mono("M", ...)`
3. Now cursor, selection, and mouse click positioning all use consistent mono char width

### Known Limitations (to address in future milestones)

- ~~No mouse click to position cursor~~ (now works!)
- No cursor blinking animation
- No line wrapping
- No undo/redo (editor crate has this, not yet integrated)
- No markdown formatting (Milestone 2)

### Next Steps (Milestone 1.2)

1. Wire mouse click to position cursor
2. Add cursor blinking
3. Integrate editor crate's undo/redo
4. Add line wrapping option
