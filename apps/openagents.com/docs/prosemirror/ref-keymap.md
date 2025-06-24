# ProseMirror Keymap Module Reference

## Overview

The prosemirror-keymap module provides functionality for defining and managing keyboard shortcuts in ProseMirror. It allows you to bind key combinations to commands and manage key event handling.

## Installation

```bash
npm install prosemirror-keymap
```

## Core Functions

### keymap(bindings)

Creates a plugin that handles key events according to a map of key names to command functions.

```javascript
import {keymap} from 'prosemirror-keymap'

const myKeymap = keymap({
  "Ctrl-b": toggleBold,
  "Ctrl-i": toggleItalic,
  "Enter": splitBlock,
  "Shift-Enter": insertHardBreak
})
```

Parameters:
- `bindings`: Object mapping key names to commands
- Returns: Plugin

### keydownHandler(bindings)

Create a keydown handler function for a set of bindings.

```javascript
const handleKeydown = keydownHandler({
  "Escape": clearSelection,
  "ArrowUp": moveUp
})

// Use in view props
new EditorView(place, {
  handleKeyDown: handleKeydown
})
```

## Key Names

Key names are strings that describe key combinations:

### Basic Keys
- Letter keys: `"a"`, `"b"`, `"c"`, etc.
- Number keys: `"1"`, `"2"`, `"3"`, etc.
- Special keys: `"Enter"`, `"Backspace"`, `"Delete"`, `"Tab"`, `"Escape"`, `"Space"`
- Arrow keys: `"ArrowUp"`, `"ArrowDown"`, `"ArrowLeft"`, `"ArrowRight"`
- Function keys: `"F1"`, `"F2"`, etc.

### Modifiers
- `"Ctrl-"`: Control key (Windows/Linux)
- `"Cmd-"`: Command key (Mac)
- `"Mod-"`: Platform-appropriate modifier (Cmd on Mac, Ctrl elsewhere)
- `"Alt-"`: Alt/Option key
- `"Shift-"`: Shift key
- `"Meta-"`: Meta/Windows key

### Combinations
- Multiple modifiers: `"Ctrl-Shift-z"`, `"Cmd-Alt-i"`
- Platform-aware: `"Mod-b"` (Cmd-b on Mac, Ctrl-b elsewhere)

## Usage Patterns

### Basic Keymap Setup

```javascript
import {keymap} from 'prosemirror-keymap'
import {baseKeymap} from 'prosemirror-commands'

// Create custom keymap
const customKeymap = keymap({
  "Mod-b": toggleBold,
  "Mod-i": toggleItalic,
  "Mod-u": toggleUnderline,
  "Mod-`": toggleCode,
  "Ctrl-Shift-1": makeHeading1,
  "Ctrl-Shift-2": makeHeading2,
  "Ctrl-Shift-3": makeHeading3,
  "Ctrl-Shift-0": makeParagraph,
  "Mod-z": undo,
  "Mod-y": redo,
  "Mod-Shift-z": redo, // Alternative redo
  "Tab": indent,
  "Shift-Tab": outdent
})

// Combine with base keymap
const plugins = [
  keymap(baseKeymap),
  customKeymap
]
```

### Conditional Key Bindings

```javascript
const contextualKeymap = keymap({
  "Enter": (state, dispatch, view) => {
    // Different behavior based on context
    if (state.selection.$head.parent.type.name === 'code_block') {
      return newlineInCode(state, dispatch, view)
    } else if (state.selection.$head.parent.type.name === 'list_item') {
      return splitListItem(state, dispatch, view)
    }
    return splitBlock(state, dispatch, view)
  },
  
  "Tab": (state, dispatch, view) => {
    // Tab behavior depends on context
    if (state.selection.$head.parent.type.name === 'list_item') {
      return sinkListItem(state, dispatch, view)
    } else if (state.selection.$head.parent.type.name === 'code_block') {
      if (dispatch) {
        dispatch(state.tr.insertText('\t'))
      }
      return true
    }
    return false
  }
})
```

### Key Sequences

```javascript
// Handle key sequences (not built-in, example implementation)
let keySequence = []
let sequenceTimeout

const sequenceKeymap = keymap({
  "g": (state, dispatch, view) => {
    keySequence.push('g')
    clearTimeout(sequenceTimeout)
    sequenceTimeout = setTimeout(() => {
      keySequence = []
    }, 1000)
    
    if (keySequence.length === 2 && keySequence[0] === 'g' && keySequence[1] === 'g') {
      // gg - go to start
      keySequence = []
      return goToStart(state, dispatch, view)
    }
    
    return true // Prevent default
  }
})
```

### Platform-Specific Bindings

```javascript
const isMac = navigator.platform.includes('Mac')

const platformKeymap = keymap({
  [isMac ? "Cmd-z" : "Ctrl-z"]: undo,
  [isMac ? "Cmd-Shift-z" : "Ctrl-y"]: redo,
  [isMac ? "Cmd-a" : "Ctrl-a"]: selectAll,
  // Platform-aware modifier
  "Mod-s": saveDocument, // Cmd-s on Mac, Ctrl-s elsewhere
  "Mod-o": openDocument,
  "Mod-n": newDocument
})
```

### Advanced Key Handling

```javascript
// Custom key handler with side effects
const advancedKeymap = keymap({
  "Mod-k": (state, dispatch, view) => {
    // Show command palette
    showCommandPalette()
    return true // Handled
  },
  
  "/": (state, dispatch, view) => {
    // Trigger autocomplete on slash
    const {$from} = state.selection
    if ($from.parent.type.name === 'paragraph' && $from.parentOffset === 0) {
      showSlashCommands($from.pos)
      return true
    }
    return false
  },
  
  "Escape": (state, dispatch, view) => {
    // Multi-purpose escape
    if (isMenuOpen()) {
      closeMenu()
      return true
    } else if (hasSelection(state)) {
      clearSelection(state, dispatch)
      return true
    }
    return false
  }
})
```

### Keymap with State

```javascript
// Stateful keymap handler
class KeymapWithMemory {
  constructor() {
    this.lastAction = null
    this.repeatCount = 0
  }
  
  createKeymap() {
    return keymap({
      "Ctrl-d": (state, dispatch, view) => {
        const now = Date.now()
        const isRepeat = this.lastAction?.type === 'duplicate' && 
                        now - this.lastAction.time < 500
        
        if (isRepeat) {
          this.repeatCount++
        } else {
          this.repeatCount = 1
        }
        
        this.lastAction = {type: 'duplicate', time: now}
        
        // Duplicate line/selection
        return duplicateContent(state, dispatch, this.repeatCount)
      }
    })
  }
}
```

### Vim-like Bindings

```javascript
// Example of vim-like key bindings
const vimKeymap = keymap({
  "i": enterInsertMode,
  "Escape": enterNormalMode,
  "h": moveCursorLeft,
  "j": moveCursorDown,
  "k": moveCursorUp,
  "l": moveCursorRight,
  "w": moveWordForward,
  "b": moveWordBackward,
  "0": moveLineStart,
  "$": moveLineEnd,
  "g g": moveDocumentStart,
  "G": moveDocumentEnd,
  "d d": deleteLine,
  "y y": yankLine,
  "p": paste,
  "u": undo,
  "Ctrl-r": redo,
  "/": startSearch,
  "n": findNext,
  "N": findPrevious
})
```

### Accessibility Considerations

```javascript
// Accessible keymap with ARIA announcements
const accessibleKeymap = keymap({
  "Alt-/": (state, dispatch, view) => {
    // Show keyboard shortcuts help
    announceToScreenReader("Keyboard shortcuts: Press Alt+? for help")
    showKeyboardShortcuts()
    return true
  },
  
  "Mod-b": (state, dispatch, view) => {
    const result = toggleBold(state, dispatch, view)
    if (result && dispatch) {
      const isBold = state.selection.$from.marks().some(m => m.type.name === 'strong')
      announceToScreenReader(isBold ? "Bold removed" : "Bold applied")
    }
    return result
  }
})

function announceToScreenReader(message) {
  const announcement = document.createElement('div')
  announcement.setAttribute('role', 'status')
  announcement.setAttribute('aria-live', 'polite')
  announcement.className = 'sr-only'
  announcement.textContent = message
  document.body.appendChild(announcement)
  setTimeout(() => announcement.remove(), 1000)
}
```

### Debugging Keymaps

```javascript
// Debug keymap to log key events
const debugKeymap = keymap({
  "*": (state, dispatch, view, event) => {
    console.log('Key pressed:', {
      key: event.key,
      code: event.code,
      ctrlKey: event.ctrlKey,
      metaKey: event.metaKey,
      shiftKey: event.shiftKey,
      altKey: event.altKey
    })
    return false // Let other handlers process
  }
})

// Keymap with timing
const timedKeymap = keymap({
  "Mod-s": (state, dispatch, view) => {
    const start = performance.now()
    const result = saveDocument(state, dispatch, view)
    console.log(`Save took ${performance.now() - start}ms`)
    return result
  }
})
```

## Best Practices

1. **Use platform-aware modifiers**: Prefer `"Mod-"` over hardcoded `"Ctrl-"` or `"Cmd-"`
2. **Return boolean values**: Commands must return `true` if handled, `false` otherwise
3. **Check before dispatch**: Test applicability when `dispatch` is not provided
4. **Order matters**: Keymaps are checked in order, specific bindings before general
5. **Avoid conflicts**: Be aware of browser and OS shortcuts
6. **Document shortcuts**: Provide a help menu showing available shortcuts
7. **Test across platforms**: Ensure bindings work on different operating systems

## Integration Example

```javascript
import {EditorState} from 'prosemirror-state'
import {EditorView} from 'prosemirror-view'
import {Schema} from 'prosemirror-model'
import {keymap} from 'prosemirror-keymap'
import {baseKeymap} from 'prosemirror-commands'

// Create complete editor with keymaps
function createEditor(place, schema) {
  const state = EditorState.create({
    schema,
    plugins: [
      keymap({
        "Mod-b": toggleMark(schema.marks.strong),
        "Mod-i": toggleMark(schema.marks.em),
        "Mod-`": toggleMark(schema.marks.code),
        "Mod-z": undo,
        "Mod-y": redo,
        "Mod-Shift-z": redo
      }),
      keymap(baseKeymap)
    ]
  })
  
  return new EditorView(place, {
    state,
    dispatchTransaction(transaction) {
      const newState = view.state.apply(transaction)
      view.updateState(newState)
    }
  })
}
```

This module is essential for creating keyboard-driven editing experiences in ProseMirror.