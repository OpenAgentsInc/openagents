# ProseMirror History Module Reference

## Overview

The prosemirror-history module implements undo/redo history tracking for ProseMirror editors. It tracks document changes and allows users to step backward and forward through their editing history.

## Installation

```bash
npm install prosemirror-history
```

## Core Functions

### history(config)

Creates a history plugin with configurable options.

```javascript
import {history} from 'prosemirror-history'

const historyPlugin = history({
  depth: 100,              // Max number of history entries
  newGroupDelay: 500       // Delay before starting new history group (ms)
})
```

Configuration options:
- `depth`: Maximum number of history steps to track (default: 100)
- `newGroupDelay`: Time delay before starting a new history group (default: 500ms)

### undo(state, dispatch)

Command to undo the last change.

```javascript
import {undo} from 'prosemirror-history'

// Check if undo is available
if (undo(editorState)) {
  // Can undo
}

// Execute undo
undo(editorState, editorView.dispatch)
```

### redo(state, dispatch)

Command to redo the last undone change.

```javascript
import {redo} from 'prosemirror-history'

// Check if redo is available
if (redo(editorState)) {
  // Can redo
}

// Execute redo
redo(editorState, editorView.dispatch)
```

### undoDepth(state)

Get the number of undo steps available.

```javascript
import {undoDepth} from 'prosemirror-history'

const stepsBack = undoDepth(editorState)
console.log(`Can undo ${stepsBack} times`)
```

### redoDepth(state)

Get the number of redo steps available.

```javascript
import {redoDepth} from 'prosemirror-history'

const stepsForward = redoDepth(editorState)
console.log(`Can redo ${stepsForward} times`)
```

### closeHistory(tr)

Close the current history group, starting a new one for subsequent changes.

```javascript
import {closeHistory} from 'prosemirror-history'

// Force a new history entry
let tr = state.tr.insertText("Some text")
closeHistory(tr)
view.dispatch(tr)
```

## Plugin Key

The history plugin exports its key for accessing plugin state:

```javascript
import {historyKey} from 'prosemirror-history'

// Get history state
const histState = historyKey.getState(editorState)
```

## Usage Examples

### Basic Setup

```javascript
import {EditorState} from 'prosemirror-state'
import {history, undo, redo} from 'prosemirror-history'
import {keymap} from 'prosemirror-keymap'

// Create editor with history
const state = EditorState.create({
  schema,
  plugins: [
    history(),
    keymap({
      "Mod-z": undo,
      "Mod-y": redo,
      "Mod-Shift-z": redo
    })
  ]
})
```

### Custom History Configuration

```javascript
// History with custom settings
const customHistory = history({
  depth: 50,           // Smaller history
  newGroupDelay: 1000  // Longer delay for grouping
})

// History that preserves more steps
const deepHistory = history({
  depth: 1000,
  newGroupDelay: 250
})
```

### History UI Integration

```javascript
// Undo/redo buttons with state
class HistoryButtons {
  constructor(view) {
    this.view = view
    this.undoBtn = document.querySelector('#undo')
    this.redoBtn = document.querySelector('#redo')
    
    this.undoBtn.addEventListener('click', () => {
      undo(this.view.state, this.view.dispatch)
      this.view.focus()
    })
    
    this.redoBtn.addEventListener('click', () => {
      redo(this.view.state, this.view.dispatch)
      this.view.focus()
    })
    
    this.update()
  }
  
  update() {
    this.undoBtn.disabled = !undo(this.view.state)
    this.redoBtn.disabled = !redo(this.view.state)
    
    // Show depth in tooltip
    this.undoBtn.title = `Undo (${undoDepth(this.view.state)} steps)`
    this.redoBtn.title = `Redo (${redoDepth(this.view.state)} steps)`
  }
}

// Update buttons when state changes
const view = new EditorView(place, {
  state,
  dispatchTransaction(tr) {
    const newState = view.state.apply(tr)
    view.updateState(newState)
    historyButtons.update()
  }
})

const historyButtons = new HistoryButtons(view)
```

### Manual History Management

```javascript
// Force new history group
function insertAndGroup(view, text) {
  const tr = view.state.tr.insertText(text)
  closeHistory(tr)
  view.dispatch(tr)
}

// Batch changes in single history step
function batchChanges(view, changes) {
  let tr = view.state.tr
  
  for (const change of changes) {
    tr = change(tr)
  }
  
  // All changes will be in one history step
  view.dispatch(tr)
}

// Prevent history tracking
function changeWithoutHistory(view, change) {
  const tr = view.state.tr
  change(tr)
  tr.setMeta('addToHistory', false)
  view.dispatch(tr)
}
```

### History-Aware Commands

```javascript
// Command that groups related changes
function replaceAndFormat(state, dispatch) {
  if (!state.selection.empty) {
    if (dispatch) {
      let tr = state.tr
      // Replace selection
      tr.replaceSelectionWith(schema.text("replacement"))
      // Add formatting
      tr.addMark(
        state.selection.from,
        state.selection.from + 11,
        schema.marks.strong.create()
      )
      // Close history to group these changes
      closeHistory(tr)
      dispatch(tr)
    }
    return true
  }
  return false
}

// Smart undo that skips certain changes
function smartUndo(state, dispatch, view) {
  const histState = historyKey.getState(state)
  if (!histState || histState.done.eventCount === 0) return false
  
  // Check if last change was auto-formatting
  const lastTr = histState.done.transforms[histState.done.transforms.length - 1]
  if (lastTr && lastTr.getMeta('autoFormat')) {
    // Undo twice to skip auto-formatting
    if (dispatch) {
      undo(state, dispatch)
      undo(view.state, dispatch)
    }
    return true
  }
  
  return undo(state, dispatch)
}
```

### History Persistence

```javascript
// Save history to localStorage
function saveHistory(state) {
  const histState = historyKey.getState(state)
  if (histState) {
    localStorage.setItem('editor-history', JSON.stringify({
      done: histState.done.toJSON(),
      undone: histState.undone.toJSON()
    }))
  }
}

// Restore history from localStorage
function restoreHistory() {
  const saved = localStorage.getItem('editor-history')
  if (saved) {
    const data = JSON.parse(saved)
    // Note: Full restoration requires custom serialization
    // This is a simplified example
    return history({
      depth: 100,
      newGroupDelay: 500
    })
  }
  return history()
}
```

### History Visualization

```javascript
// Display history stack
class HistoryViewer {
  constructor(view) {
    this.view = view
    this.container = document.querySelector('#history-stack')
    this.update()
  }
  
  update() {
    const depth = undoDepth(this.view.state)
    const redoCount = redoDepth(this.view.state)
    
    this.container.innerHTML = `
      <div class="history-info">
        <div>Undo stack: ${depth} steps</div>
        <div>Redo stack: ${redoCount} steps</div>
      </div>
    `
  }
}

// Show history timeline
function renderHistoryTimeline(state) {
  const histState = historyKey.getState(state)
  if (!histState) return null
  
  const timeline = document.createElement('div')
  timeline.className = 'history-timeline'
  
  // Render done stack
  histState.done.transforms.forEach((tr, i) => {
    const step = document.createElement('div')
    step.className = 'history-step done'
    step.textContent = `Step ${i + 1}`
    timeline.appendChild(step)
  })
  
  // Current position marker
  const current = document.createElement('div')
  current.className = 'history-current'
  current.textContent = 'Current'
  timeline.appendChild(current)
  
  // Render undone stack
  histState.undone.transforms.forEach((tr, i) => {
    const step = document.createElement('div')
    step.className = 'history-step undone'
    step.textContent = `Redo ${i + 1}`
    timeline.appendChild(step)
  })
  
  return timeline
}
```

### Advanced History Control

```javascript
// Clear history
function clearHistory(view) {
  const newState = EditorState.create({
    doc: view.state.doc,
    selection: view.state.selection,
    schema: view.state.schema,
    plugins: view.state.plugins
  })
  view.updateState(newState)
}

// Limit history by time
class TimeLimitedHistory {
  constructor(maxAge = 3600000) { // 1 hour default
    this.maxAge = maxAge
  }
  
  plugin() {
    return new Plugin({
      appendTransaction(trs, oldState, newState) {
        const now = Date.now()
        const histState = historyKey.getState(newState)
        
        if (histState) {
          // Filter old entries
          const cutoff = now - this.maxAge
          // Note: Actual implementation would need to track timestamps
        }
        
        return null
      }
    })
  }
}

// Collaborative history
function createCollabHistory(authority) {
  return history({
    // Only track local changes
    trackTransform(tr) {
      return !tr.getMeta('remote')
    }
  })
}
```

## Best Practices

1. **Configure depth appropriately**: Balance memory usage with user needs
2. **Use newGroupDelay**: Group rapid changes into single history entries
3. **Handle edge cases**: Check if undo/redo available before calling
4. **Preserve focus**: Return focus to editor after UI interactions
5. **Consider memory**: Large documents with deep history can use significant memory
6. **Test with IME**: Ensure history works correctly with input methods
7. **Handle collaborative editing**: Filter remote changes from history if needed

## Integration with Other Modules

```javascript
import {EditorState} from 'prosemirror-state'
import {EditorView} from 'prosemirror-view'
import {history, undo, redo} from 'prosemirror-history'
import {keymap} from 'prosemirror-keymap'
import {baseKeymap} from 'prosemirror-commands'

// Complete setup
function createEditorWithHistory(place, schema) {
  const state = EditorState.create({
    schema,
    plugins: [
      history({
        depth: 100,
        newGroupDelay: 500
      }),
      keymap({
        "Mod-z": undo,
        "Mod-y": redo,
        "Mod-Shift-z": redo,
        "Ctrl-Alt-z": (state, dispatch) => {
          // Custom multi-undo
          for (let i = 0; i < 5 && undo(state); i++) {
            if (dispatch) undo(state, dispatch)
            state = view.state
          }
          return true
        }
      }),
      keymap(baseKeymap)
    ]
  })
  
  const view = new EditorView(place, {
    state,
    dispatchTransaction(tr) {
      const newState = view.state.apply(tr)
      view.updateState(newState)
      
      // Update UI based on history state
      updateHistoryUI(newState)
    }
  })
  
  return view
}
```

This module provides essential undo/redo functionality that users expect in any text editor.