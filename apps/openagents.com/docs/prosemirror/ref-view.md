# ProseMirror View Module Reference

## Overview

The prosemirror-view module implements the browser-based editing interface for ProseMirror. It handles rendering documents to the DOM, managing user input, and providing hooks for customizing the editing experience.

## Installation

```bash
npm install prosemirror-view
```

## Key Classes

### EditorView

The view component of ProseMirror. Handles the DOM representation of the editor and manages user interactions.

Constructor:
```javascript
new EditorView(place, props)
```
- `place`: DOM node or {mount: Node} or {root: Document, mount: Node}
- `props`: DirectEditorProps configuration

Properties:
- `state`: Current editor state
- `dom`: The editable DOM node
- `editable`: Whether the editor is editable
- `composing`: Whether a composition is active
- `props`: Current props
- `root`: Root document
- `isDestroyed`: Whether view is destroyed
- `docView`: Internal document view
- `lastSelectedViewDesc`: Last selected view descriptor
- `input`: Input handler
- `pluginViews`: Plugin view instances
- `domObserver`: DOM mutation observer

Methods:
- `update(props)`: Update view with new props
- `updateState(state)`: Update state (prefer dispatch)
- `dispatch(tr)`: Dispatch a transaction
- `focus()`: Focus the editor
- `blur()`: Blur the editor
- `hasFocus()`: Check if editor has focus
- `domAtPos(pos)`: Get DOM node at document position
- `nodeDOM(pos)`: Get DOM node for document node at position
- `posAtCoords(coords)`: Find position at viewport coordinates
- `coordsAtPos(pos, side)`: Get coordinates for position
- `defaultTextHeight`: Estimate default line height
- `posAtDOM(node, offset, bias)`: Position at DOM location
- `endOfTextblock(dir, state)`: Check if at textblock boundary
- `pasteHTML(html)`: Paste HTML content
- `pasteText(text)`: Paste plain text
- `destroy()`: Destroy the view
- `requiresGeckoHackNode`: Whether Firefox hack is needed
- `someProp(prop, f)`: Get prop value from plugins or props
- `hasContentDesc`: Whether view has content

### DirectEditorProps

Configuration object for EditorView:

```javascript
{
  // Core
  state: EditorState,              // Initial state
  dispatchTransaction?: (tr) => void,  // Handle transactions
  
  // DOM
  handleDOMEvents?: {[event: string]: (view, event) => boolean},
  
  // Input Events
  handleKeyDown?: (view, event) => boolean,
  handleKeyPress?: (view, event) => boolean,
  handleTextInput?: (view, from, to, text) => boolean,
  
  // Mouse Events
  handleClickOn?: (view, pos, node, nodePos, event, direct) => boolean,
  handleClick?: (view, pos, event) => boolean,
  handleDoubleClickOn?: (view, pos, node, nodePos, event, direct) => boolean,
  handleDoubleClick?: (view, pos, event) => boolean,
  handleTripleClickOn?: (view, pos, node, nodePos, event, direct) => boolean,
  handleTripleClick?: (view, pos, event) => boolean,
  
  // Paste/Drop
  handlePaste?: (view, event, slice) => boolean,
  handleDrop?: (view, event, slice, moved) => boolean,
  
  // Focus
  handleFocus?: (view, event) => boolean,
  handleBlur?: (view, event) => boolean,
  
  // Rendering
  nodeViews?: {[name: string]: NodeViewConstructor},
  markViews?: {[name: string]: MarkViewConstructor},
  
  // Clipboard
  clipboardSerializer?: DOMSerializer,
  clipboardTextSerializer?: (slice) => string,
  clipboardParser?: DOMParser,
  clipboardTextParser?: (text, $context) => Slice,
  
  // Transform
  transformPasted?: (slice, view) => Slice,
  transformPastedHTML?: (html, view) => string,
  transformPastedText?: (text, plain, view) => string,
  
  // Decorations
  decorations?: (state) => DecorationSet,
  
  // DOM Attributes
  attributes?: {[name: string]: string} | (state) => {[name: string]: string},
  
  // Scrolling
  scrollThreshold?: number | {top: number, right: number, bottom: number, left: number},
  scrollMargin?: number | {top: number, right: number, bottom: number, left: number},
  handleScrollToSelection?: (view) => boolean,
  
  // Other
  editable?: (state) => boolean,
  domParser?: DOMParser,
  transformCopied?: (slice, view) => Slice,
  createSelectionBetween?: (view, anchor, head) => Selection | null
}
```

### NodeView

Interface for custom node rendering:

```typescript
interface NodeView {
  // Required
  dom: Node,                       // DOM node representing this node
  
  // Optional
  contentDOM?: Node,               // Where to render node content
  update?: (node: Node, decorations: Decoration[], innerDecorations: DecorationSource) => boolean,
  selectNode?: () => void,         // Called when node is selected
  deselectNode?: () => void,       // Called when node is deselected
  setSelection?: (anchor: number, head: number, root: Document) => void,
  stopEvent?: (event: Event) => boolean,
  ignoreMutation?: (mutation: MutationRecord) => boolean,
  destroy?: () => void
}
```

NodeViewConstructor:
```typescript
type NodeViewConstructor = (
  node: Node,
  view: EditorView,
  getPos: () => number | undefined,
  decorations: Decoration[],
  innerDecorations: DecorationSource
) => NodeView
```

### MarkView

Interface for custom mark rendering:

```typescript
interface MarkView {
  dom: Node,
  contentDOM?: Node,
  destroy?: () => void,
  ignoreMutation?: (mutation: MutationRecord) => boolean
}
```

### Decoration

Decorations modify how the document is displayed without changing the document itself.

Static methods:
- `Decoration.widget(pos, toDOM, spec?)`: Widget decoration at position
- `Decoration.inline(from, to, attrs, spec?)`: Inline decoration
- `Decoration.node(from, to, attrs, spec?)`: Node decoration

Decoration spec:
```javascript
{
  inclusiveStart?: boolean,    // Include at range start
  inclusiveEnd?: boolean,      // Include at range end
  attributes?: Object,         // DOM attributes
  nodeName?: string,          // Wrapper node name
  class?: string,             // CSS class
  style?: string,             // CSS style
  // Widget-specific
  side?: number,              // Widget side (-1 or 1)
  marks?: Mark[],             // Widget marks
  stopEvent?: (event) => boolean,
  ignoreSelection?: boolean,   // Don't suppress native selection
  key?: string                // Identity key
}
```

### DecorationSet

Collection of decorations, organized for efficient mapping and drawing.

Static methods:
- `DecorationSet.create(doc, decorations)`: Create from array
- `DecorationSet.empty`: Empty set singleton

Methods:
- `find(start?, end?, predicate?)`: Find decorations in range
- `map(mapping, doc, options?)`: Map through document changes
- `add(doc, decorations)`: Add decorations
- `remove(decorations)`: Remove decorations

### DecorationSource

Interface for decoration sets that can be efficiently mapped:

```typescript
interface DecorationSource {
  map(mapping: Mapping, doc: Node): DecorationSource
  locals(view: NodeView): Decoration[]
  forChild(offset: number, child: Node): DecorationSource
  eq(other: DecorationSource): boolean
}
```

## Usage Examples

### Creating an Editor View

```javascript
import {EditorView} from 'prosemirror-view'
import {EditorState} from 'prosemirror-state'
import {schema} from './schema'

// Basic setup
const view = new EditorView(document.querySelector('#editor'), {
  state: EditorState.create({schema}),
  dispatchTransaction(transaction) {
    const newState = view.state.apply(transaction)
    view.updateState(newState)
  }
})

// With event handlers
const viewWithHandlers = new EditorView(document.querySelector('#editor'), {
  state,
  handleKeyDown(view, event) {
    if (event.key === 'Tab') {
      // Handle tab
      return true // Prevent default
    }
    return false
  },
  handleClick(view, pos, event) {
    console.log('Clicked at position:', pos)
    return false
  }
})
```

### Custom Node Views

```javascript
// Image node view with controls
class ImageView {
  constructor(node, view, getPos) {
    this.node = node
    this.view = view
    this.getPos = getPos
    
    // Create DOM
    this.dom = document.createElement('div')
    this.dom.className = 'image-wrapper'
    
    this.img = document.createElement('img')
    this.img.src = node.attrs.src
    this.dom.appendChild(this.img)
    
    // Add controls
    this.controls = document.createElement('div')
    this.controls.className = 'image-controls'
    this.dom.appendChild(this.controls)
  }
  
  update(node) {
    if (node.type !== this.node.type) return false
    this.node = node
    this.img.src = node.attrs.src
    return true
  }
  
  selectNode() {
    this.dom.classList.add('selected')
  }
  
  deselectNode() {
    this.dom.classList.remove('selected')
  }
  
  destroy() {
    // Cleanup
  }
}

// Register node view
const view = new EditorView(place, {
  state,
  nodeViews: {
    image: (node, view, getPos) => new ImageView(node, view, getPos)
  }
})
```

### Working with Decorations

```javascript
import {Decoration, DecorationSet} from 'prosemirror-view'

// Widget decoration
const widget = Decoration.widget(pos, () => {
  const span = document.createElement('span')
  span.className = 'widget'
  span.textContent = '→'
  return span
}, {side: 1})

// Inline decoration
const highlight = Decoration.inline(from, to, {
  class: 'highlight'
})

// Node decoration
const selected = Decoration.node(from, to, {
  class: 'selected-node'
})

// Create decoration set
const decorations = DecorationSet.create(doc, [widget, highlight, selected])

// Use in view
const decoratedView = new EditorView(place, {
  state,
  decorations(state) {
    return decorations
  }
})

// Plugin with decorations
const highlightPlugin = new Plugin({
  state: {
    init(_, state) {
      return DecorationSet.empty
    },
    apply(tr, set, oldState, state) {
      // Map decorations through changes
      set = set.map(tr.mapping, tr.doc)
      
      // Add new decorations
      if (tr.getMeta('highlight')) {
        const deco = Decoration.inline(tr.selection.from, tr.selection.to, {
          class: 'highlight'
        })
        set = set.add(tr.doc, [deco])
      }
      
      return set
    }
  },
  props: {
    decorations(state) {
      return this.getState(state)
    }
  }
})
```

### Handling Paste and Drop

```javascript
const pasteView = new EditorView(place, {
  state,
  handlePaste(view, event, slice) {
    // Custom paste handling
    console.log('Pasted:', slice)
    
    // Modify the slice
    const modified = new Slice(
      slice.content.map(node => {
        // Transform nodes
        return node
      }),
      slice.openStart,
      slice.openEnd
    )
    
    // Apply modified slice
    const tr = view.state.tr.replaceSelection(modified)
    view.dispatch(tr)
    
    return true // Handled
  },
  
  transformPastedHTML(html) {
    // Clean HTML before parsing
    return html.replace(/<script[^>]*>.*?<\/script>/gi, '')
  },
  
  handleDrop(view, event, slice, moved) {
    // Handle file drops
    if (event.dataTransfer.files.length > 0) {
      const file = event.dataTransfer.files[0]
      // Handle file upload
      return true
    }
    return false
  }
})
```

### DOM Interaction

```javascript
// Get position from DOM
const pos = view.posAtDOM(domNode, offset)

// Get DOM from position
const {node, offset} = view.domAtPos(15)

// Get coordinates
const coords = view.coordsAtPos(10) // {left, right, top, bottom}

// Find position at coordinates
const pos = view.posAtCoords({left: 100, top: 50})
if (pos) {
  console.log('Position:', pos.pos, 'Inside:', pos.inside)
}

// Check text block boundaries
if (view.endOfTextblock('forward')) {
  console.log('At end of text block')
}
```

### Custom Mark Views

```javascript
class TooltipMark {
  constructor(mark, view) {
    this.mark = mark
    
    this.dom = document.createElement('span')
    this.dom.className = 'tooltip-mark'
    this.dom.title = mark.attrs.title
    
    this.contentDOM = this.dom
  }
  
  destroy() {
    // Cleanup if needed
  }
}

const tooltipView = new EditorView(place, {
  state,
  markViews: {
    tooltip: (mark, view) => new TooltipMark(mark, view)
  }
})
```

### Focus and Selection Management

```javascript
// Focus handling
view.focus()
view.blur()

if (view.hasFocus()) {
  console.log('Editor is focused')
}

// Custom selection creation
const customView = new EditorView(place, {
  state,
  createSelectionBetween(view, anchor, head) {
    // Create custom selection between positions
    if (/* some condition */) {
      return TextSelection.create(view.state.doc, anchor.pos, head.pos)
    }
    return null // Use default
  }
})
```

This module provides the visual editing interface for ProseMirror, handling all DOM interaction, rendering, and user input processing.