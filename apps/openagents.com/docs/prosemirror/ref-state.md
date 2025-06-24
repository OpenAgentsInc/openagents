# ProseMirror State Module Reference

## Overview

The prosemirror-state module manages the editor state, providing the data structure that represents the current state of a ProseMirror editor. It includes the document, selection, and any custom state that plugins might need.

## Installation

```bash
npm install prosemirror-state
```

## Key Classes

### EditorState

The state of a ProseMirror editor is represented by an object of this type. It is immutable - to update the state, you create a transaction and apply it.

Properties:
- `doc`: The current document
- `selection`: The current selection
- `storedMarks`: Marks to apply to next input (or null)
- `schema`: The schema of the document
- `plugins`: Active plugins
- `tr`: Get a transaction for this state

Methods:
- `apply(tr)`: Apply a transaction to create a new state
- `applyTransaction(rootTr)`: Apply a transaction with plugin processing
- `reconfigure(config)`: Create state with new configuration
- `toJSON(pluginFields)`: Serialize to JSON
- `static create(config)`: Create a new state
- `static fromJSON(config, json, pluginFields)`: Deserialize state

Configuration options:
```javascript
{
  schema: Schema,           // Required
  doc?: Node,              // Initial document
  selection?: Selection,   // Initial selection
  storedMarks?: Mark[],    // Initial stored marks
  plugins?: Plugin[]       // Plugins to use
}
```

### Transaction

A transaction is a subclass of Transform that also tracks selection and other editor-specific state. Transactions are the only way to modify editor state.

Inherits all Transform methods plus:

Selection methods:
- `setSelection(selection)`: Update selection
- `selection`: Current selection in transaction
- `selectionSet`: Whether selection was explicitly set
- `setStoredMarks(marks)`: Set stored marks
- `ensureMarks(marks)`: Ensure marks are in stored set
- `addStoredMark(mark)`: Add mark to stored set
- `removeStoredMark(mark)`: Remove mark from stored set
- `storedMarksSet`: Whether stored marks were set

Content methods:
- `replaceSelection(slice)`: Replace current selection
- `replaceSelectionWith(node, inheritMarks)`: Replace with single node
- `deleteSelection()`: Delete current selection
- `insertText(text, from, to)`: Insert text

Metadata methods:
- `setMeta(key, value)`: Set metadata for this transaction
- `getMeta(key)`: Get metadata value
- `isGeneric`: Whether this is a generic (non-special) transaction
- `scrollIntoView()`: Request scroll into view after applying
- `time`: Timestamp of transaction
- `docChanged`: Whether document was changed

### Selection

Superclass for all selection types. A selection is a range of the document with an anchor and a head position.

Properties:
- `anchor`: Fixed side of selection
- `head`: Moving side of selection
- `from`: Lower bound of selection
- `to`: Upper bound of selection
- `$anchor`: Resolved anchor position
- `$head`: Resolved head position
- `$from`: Resolved from position
- `$to`: Resolved to position
- `empty`: Whether selection is empty
- `ranges`: Array of selection ranges

Methods:
- `eq(other)`: Compare selections
- `map(doc, mapping)`: Map through changes
- `content()`: Get selected content
- `replace(tr, content)`: Replace selection in transaction
- `replaceWith(tr, node)`: Replace with single node
- `toJSON()`: Serialize to JSON
- `getBookmark()`: Get a bookmark for this selection

Static methods:
- `Selection.fromJSON(doc, json)`: Deserialize
- `Selection.atStart(doc)`: Selection at document start
- `Selection.atEnd(doc)`: Selection at document end
- `Selection.near($pos, bias)`: Selection near position
- `Selection.findFrom($pos, dir, textOnly)`: Find selection from position

### TextSelection

The most common selection type, representing a text cursor or range.

Constructor:
- `new TextSelection($anchor, $head)`: Create from resolved positions

Static methods:
- `TextSelection.create(doc, anchor, head)`: Create from numbers
- `TextSelection.between($anchor, $head, bias)`: Create between positions

### NodeSelection

Selection of a single node.

Constructor:
- `new NodeSelection($pos)`: Create from resolved position

Properties:
- `node`: The selected node

Static methods:
- `NodeSelection.create(doc, from)`: Create from position
- `NodeSelection.isSelectable(node)`: Check if node is selectable

### AllSelection

Selection of the entire document.

Constructor:
- `new AllSelection(doc)`: Create for document

### Plugin

Plugins extend editor functionality by adding state fields, props, or other behavior.

Constructor:
- `new Plugin(spec)`: Create from spec

Properties:
- `props`: Plugin props
- `spec`: Original spec
- `key`: Plugin key (if any)

Methods:
- `getState(state)`: Get plugin state from editor state

### PluginKey

Used to identify and access plugin state.

Constructor:
- `new PluginKey(name)`: Create with optional name

Methods:
- `get(state)`: Get plugin state
- `getState(state)`: Alias for get

### PluginSpec

Configuration object for plugins:

```javascript
{
  // Define plugin state
  state?: {
    init: (config, state) => T,
    apply: (tr, value, oldState, newState) => T
  },
  
  // Add editor props
  props?: EditorProps,
  
  // Identify plugin
  key?: PluginKey,
  
  // View layer integration
  view?: (view) => {
    update?: (view, prevState) => void,
    destroy?: () => void
  },
  
  // Filter transactions
  filterTransaction?: (tr, state) => boolean,
  
  // Append to transactions
  appendTransaction?: (trs, oldState, newState) => Transaction | null
}
```

### EditorProps

Properties that can be added by plugins:

```javascript
{
  // Handle DOM events
  handleDOMEvents?: {[event: string]: (view, event) => boolean},
  
  // Handle editor events
  handleKeyDown?: (view, event) => boolean,
  handleKeyPress?: (view, event) => boolean,
  handleTextInput?: (view, from, to, text) => boolean,
  handleClickOn?: (view, pos, node, nodePos, event, direct) => boolean,
  handleClick?: (view, pos, event) => boolean,
  handleDoubleClickOn?: (view, pos, node, nodePos, event, direct) => boolean,
  handleDoubleClick?: (view, pos, event) => boolean,
  handleTripleClickOn?: (view, pos, node, nodePos, event, direct) => boolean,
  handleTripleClick?: (view, pos, event) => boolean,
  handlePaste?: (view, event, slice) => boolean,
  handleDrop?: (view, event, slice, moved) => boolean,
  handleScrollToSelection?: (view) => boolean,
  
  // Focus handling
  handleFocus?: (view, event) => boolean,
  handleBlur?: (view, event) => boolean,
  
  // Create DOM
  nodeViews?: {[node: string]: NodeView},
  markViews?: {[mark: string]: MarkView},
  
  // Clipboard
  clipboardSerializer?: DOMSerializer,
  clipboardTextSerializer?: (slice) => string,
  clipboardParser?: DOMParser,
  clipboardTextParser?: (text, $context) => Slice,
  
  // Input rules
  transformPasted?: (slice, view) => Slice,
  transformPastedHTML?: (html, view) => string,
  transformPastedText?: (text, plain, view) => string,
  
  // Decoration
  decorations?: (state) => DecorationSet,
  
  // DOM attributes
  attributes?: {[attr: string]: string} | (state) => {[attr: string]: string},
  
  // Scroll
  scrollThreshold?: number | {top: number, right: number, bottom: number, left: number},
  scrollMargin?: number | {top: number, right: number, bottom: number, left: number}
}
```

## Usage Examples

### Creating Editor State

```javascript
import {EditorState} from 'prosemirror-state'
import {schema} from './schema'

// Basic state
const state = EditorState.create({
  schema,
  doc: schema.node('doc', null, [
    schema.node('paragraph', null, [
      schema.text('Hello world')
    ])
  ])
})

// With plugins
const stateWithPlugins = EditorState.create({
  schema,
  plugins: [keymap, history]
})
```

### Working with Transactions

```javascript
// Create and apply transaction
let tr = state.tr
tr.insertText('Hello', 1)
tr.setSelection(TextSelection.create(tr.doc, 1))
let newState = state.apply(tr)

// Replace selection
tr = state.tr
tr.replaceSelectionWith(schema.nodes.hard_break.create())
newState = state.apply(tr)

// With metadata
tr = state.tr.setMeta('origin', 'user-input')
tr.insertText('text')
newState = state.apply(tr)
```

### Working with Selections

```javascript
// Create text selection
const textSel = TextSelection.create(doc, 5, 10)

// Create node selection
const nodeSel = NodeSelection.create(doc, 5)

// Find selection near position
const $pos = doc.resolve(10)
const nearSel = Selection.near($pos)

// Selection at start/end
const startSel = Selection.atStart(doc)
const endSel = Selection.atEnd(doc)
```

### Creating Plugins

```javascript
import {Plugin, PluginKey} from 'prosemirror-state'

// Simple plugin
const myPlugin = new Plugin({
  props: {
    handleKeyDown(view, event) {
      if (event.key === 'Enter' && event.ctrlKey) {
        // Handle Ctrl+Enter
        return true
      }
      return false
    }
  }
})

// Plugin with state
const countKey = new PluginKey('count')
const countPlugin = new Plugin({
  key: countKey,
  state: {
    init() { return 0 },
    apply(tr, count) {
      if (tr.docChanged) return count + 1
      return count
    }
  },
  props: {
    decorations(state) {
      const count = countKey.getState(state)
      // Return decorations based on count
    }
  }
})

// Plugin with view
const viewPlugin = new Plugin({
  view(editorView) {
    const widget = document.createElement('div')
    widget.className = 'my-widget'
    editorView.dom.parentNode.appendChild(widget)
    
    return {
      update(view, prevState) {
        // Update widget
      },
      destroy() {
        widget.remove()
      }
    }
  }
})

// Filter transactions
const filterPlugin = new Plugin({
  filterTransaction(tr, state) {
    // Prevent certain changes
    return !tr.getMeta('forbidden')
  }
})

// Append transactions
const appendPlugin = new Plugin({
  appendTransaction(transactions, oldState, newState) {
    // Add automatic changes
    if (transactions.some(tr => tr.docChanged)) {
      const tr = newState.tr
      // Make additional changes
      return tr
    }
  }
})
```

### State Management Patterns

```javascript
// Reconfigure state
const newState = state.reconfigure({
  plugins: state.plugins.concat([newPlugin])
})

// Serialize/deserialize
const json = state.toJSON()
const restored = EditorState.fromJSON(
  {schema, plugins},
  json
)

// Track state history
let stateHistory = [state]
function updateState(newState) {
  stateHistory.push(newState)
  // Limit history size
  if (stateHistory.length > 100) {
    stateHistory = stateHistory.slice(-50)
  }
}
```

This module is central to ProseMirror's architecture, managing all editor state and providing the plugin system for extending functionality.