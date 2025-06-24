# The Editor State in ProseMirror

## Overview

The editor state in ProseMirror consists of three main components:
- `doc`: The document
- `selection`: The current selection
- `storedMarks`: Currently active marks

```javascript
import {EditorState} from "prosemirror-state"
import {schema} from "prosemirror-schema-basic"

let state = EditorState.create({schema})
console.log(state.doc.toString()) // An empty paragraph
console.log(state.selection.from) // 1, the start of the paragraph
```

## Selection

ProseMirror supports multiple selection types:
- Text selections (most common)
- Node selections

Selections are immutable and have key properties:
- `from`: Start position
- `to`: End position  
- `anchor`: Unmoveable side
- `head`: Moveable side
- `empty`: Boolean indicating if selection is collapsed

### Selection Examples

```javascript
// Creating different selection types
import {TextSelection, NodeSelection, AllSelection} from "prosemirror-state"

// Text selection from position 5 to 10
let textSel = TextSelection.create(state.doc, 5, 10)

// Select the node at position 5
let nodeSel = NodeSelection.create(state.doc, 5)

// Select entire document
let allSel = new AllSelection(state.doc)
```

## Transactions

State updates happen by applying a transaction to an existing state:

```javascript
let tr = state.tr
tr.insertText("hello") // Replaces selection with 'hello'
let newState = state.apply(tr)
```

Transactions are a subclass of Transform and include:
- Document modification methods
- Selection-related methods
- Ability to set stored marks
- Option to scroll into view

### Transaction Methods

```javascript
// Document changes
tr.insertText("text", from, to)
tr.delete(from, to)
tr.replace(from, to, slice)

// Selection changes
tr.setSelection(selection)
tr.replaceSelection(slice)

// Marks
tr.addStoredMark(mark)
tr.removeStoredMark(markType)
tr.ensureMarks(marks)

// Metadata
tr.setMeta(key, value)
tr.scrollIntoView()
```

## Plugins

Plugins extend editor behavior and can:
- Add props to the editor view
- Manage custom state
- Filter transactions
- Append transactions

### Simple Plugin Example

```javascript
let transactionCounter = new Plugin({
  state: {
    init() { return 0 },
    apply(tr, value) { return value + 1 }
  }
})

function getTransactionCount(state) {
  return transactionCounter.getState(state)
}
```

### Plugin with View Integration

```javascript
let selectionSizePlugin = new Plugin({
  view(editorView) {
    let dom = document.createElement("div")
    dom.textContent = getSelectionSize(editorView.state)
    
    return {
      update(view, prevState) {
        if (prevState.selection.eq(view.state.selection)) return
        dom.textContent = getSelectionSize(view.state)
      },
      
      destroy() {
        dom.remove()
      }
    }
  }
})

function getSelectionSize(state) {
  return state.selection.to - state.selection.from
}
```

### Plugin Metadata

Plugins can attach metadata to transactions:

```javascript
function markAsUncounted(tr) {
  tr.setMeta(transactionCounter, true)
}

// In plugin's apply method
apply(tr, value) {
  if (tr.getMeta(transactionCounter)) return value
  return value + 1
}
```

## State Fields

Plugins can define state fields to store and update custom state across transactions. The state must remain immutable, with the `apply` method returning a new value when changes occur.

Key principles:
- Plugin state is part of the immutable editor state
- Always return new values from `apply`, never mutate
- Access plugin state with `plugin.getState(editorState)`

### Advanced Plugin Example

```javascript
let wordCountPlugin = new Plugin({
  state: {
    init(config, state) {
      return countWords(state.doc)
    },
    
    apply(tr, value, oldState, newState) {
      if (!tr.docChanged) return value
      return countWords(newState.doc)
    }
  },
  
  props: {
    decorations(state) {
      // Return decorations based on plugin state
    }
  }
})

function countWords(doc) {
  let count = 0
  doc.descendants(node => {
    if (node.isText) {
      count += node.text.split(/\s+/).filter(w => w).length
    }
  })
  return count
}
```

## Transaction Filtering

Plugins can filter or modify transactions before they're applied:

```javascript
new Plugin({
  filterTransaction(tr, state) {
    // Return false to block transaction
    if (isForbidden(tr)) return false
    return true
  },
  
  appendTransaction(transactions, oldState, newState) {
    // Return a new transaction to append
    if (needsFixup(newState)) {
      return newState.tr.insertText("...")
    }
  }
})
```

## Common State Patterns

### Tracking Changes

```javascript
// Track whether document changed
if (tr.docChanged) {
  console.log("Document modified")
}

// Track selection changes
if (!oldState.selection.eq(newState.selection)) {
  console.log("Selection changed")
}
```

### Stored Marks

```javascript
// Check active marks at cursor
let marks = state.storedMarks || state.selection.$from.marks()

// Toggle a mark
let markType = schema.marks.strong
if (markType.isInSet(marks)) {
  tr.removeStoredMark(markType)
} else {
  tr.addStoredMark(markType.create())
}
```