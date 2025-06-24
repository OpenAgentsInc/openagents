# The View Component in ProseMirror

## Overview

The ProseMirror editor view is a user interface component that:
- Displays an editor state to the user
- Allows editing actions
- Handles direct interaction with the editing surface

### Key Characteristics

The view's interaction is relatively narrow, focusing on:
- Typing
- Clicking
- Copying
- Pasting
- Dragging

More complex interactions like menus or key bindings are typically handled through plugins.

## Creating a View

```javascript
import {EditorView} from "prosemirror-view"
import {EditorState} from "prosemirror-state"
import {schema} from "prosemirror-schema-basic"

let state = EditorState.create({schema})
let view = new EditorView(document.querySelector("#editor"), {
  state,
  dispatchTransaction(transaction) {
    let newState = view.state.apply(transaction)
    view.updateState(newState)
  }
})
```

## Editable DOM

The view creates a DOM representation of the document using the schema's `toDOM` methods. Key behaviors include:

- Making the element editable
- Ensuring DOM selection matches editor state selection
- Registering event handlers to translate events into transactions

### Data Flow

The editor view follows a cyclic data flow:

```
DOM event → EditorView → Transaction → new EditorState → DOM update
```

## Props

Props define the view's behavior. Common props include:

```javascript
let view = new EditorView(dom, {
  state: myState,
  
  // Core props
  dispatchTransaction(tr) {
    // Handle transactions
  },
  
  // Control editing
  editable(state) {
    return true // or false to make read-only
  },
  
  // Event handlers
  handleDOMEvents: {
    mousedown(view, event) {
      // Return true to prevent default handling
      return false
    }
  },
  
  // Decorations
  decorations(state) {
    return myDecorations
  },
  
  // Node views
  nodeViews: {
    image: (node, view, getPos) => new ImageView(node, view, getPos)
  }
})
```

## Efficient Updating

The view optimizes updates by:
- Comparing old and new documents
- Leaving unchanged DOM nodes untouched
- Minimizing unnecessary redraws
- Using a virtual DOM-like reconciliation process

## Decorations

Decorations provide control over document rendering. Three types exist:

### 1. Node Decorations
Style or add attributes to a single node:

```javascript
Decoration.node(pos, pos + node.nodeSize, {
  class: "highlighted",
  style: "background: yellow"
})
```

### 2. Widget Decorations
Insert DOM elements that aren't part of the document:

```javascript
Decoration.widget(pos, () => {
  let span = document.createElement("span")
  span.textContent = "👉"
  return span
}, {side: -1})
```

### 3. Inline Decorations
Style or add attributes to a range of inline content:

```javascript
Decoration.inline(from, to, {
  class: "search-match",
  style: "background: yellow"
})
```

### Creating Decoration Sets

```javascript
let purplePlugin = new Plugin({
  props: {
    decorations(state) {
      return DecorationSet.create(state.doc, [
        Decoration.inline(0, state.doc.content.size, {style: "color: purple"})
      ])
    }
  }
})
```

### Efficient Decoration Updates

```javascript
let highlightPlugin = new Plugin({
  state: {
    init() {
      return DecorationSet.empty
    },
    apply(tr, set) {
      // Map decorations through changes
      set = set.map(tr.mapping, tr.doc)
      // Add new decorations if needed
      if (tr.getMeta(highlightPlugin)) {
        set = set.add(tr.doc, [
          Decoration.inline(tr.selection.from, tr.selection.to, {class: "highlight"})
        ])
      }
      return set
    }
  },
  props: {
    decorations(state) {
      return highlightPlugin.getState(state)
    }
  }
})
```

## Node Views

Node views allow custom rendering and interaction for specific document nodes.

### Basic Node View

```javascript
class ImageView {
  constructor(node, view, getPos) {
    // Create DOM representation
    this.dom = document.createElement("img")
    this.dom.src = node.attrs.src
    this.dom.alt = node.attrs.alt || ""
    
    // Handle interactions
    this.dom.addEventListener("click", e => {
      e.preventDefault()
      console.log("Image clicked at position", getPos())
    })
  }
  
  selectNode() {
    this.dom.classList.add("selected")
  }
  
  deselectNode() {
    this.dom.classList.remove("selected")
  }
  
  destroy() {
    // Cleanup if needed
  }
}

// Register the node view
let view = new EditorView(dom, {
  state,
  nodeViews: {
    image(node, view, getPos) {
      return new ImageView(node, view, getPos)
    }
  }
})
```

### Advanced Node View with Content

```javascript
class FootnoteView {
  constructor(node, view, getPos) {
    // Create container
    this.dom = document.createElement("footnote")
    
    // Create editable content area
    this.contentDOM = document.createElement("span")
    this.dom.appendChild(this.contentDOM)
    
    // Add UI elements
    let number = document.createElement("sup")
    number.textContent = node.attrs.number
    this.dom.insertBefore(number, this.contentDOM)
  }
  
  update(node) {
    // Return false if node type changed
    if (node.type.name !== "footnote") return false
    
    // Update attributes
    this.dom.querySelector("sup").textContent = node.attrs.number
    return true
  }
}
```

## View Methods

Common view methods:

```javascript
// Update state
view.updateState(newState)

// Get current position from DOM coordinates
let pos = view.posAtCoords({left: x, top: y})

// Get DOM coordinates from document position
let coords = view.coordsAtPos(pos)

// Focus the editor
view.focus()

// Check if view has focus
view.hasFocus()

// Destroy the view
view.destroy()
```

## Handling Events

```javascript
new EditorView(dom, {
  handleDOMEvents: {
    drop(view, event) {
      // Custom drop handling
      let pos = view.posAtCoords({left: event.clientX, top: event.clientY})
      if (pos) {
        // Handle drop at position
        return true // Prevent default
      }
    }
  },
  
  handleKeyDown(view, event) {
    // Handle before plugins
    if (event.key === "Tab") {
      // Custom tab handling
      return true
    }
  }
})
```

## Attributes and Classes

Control editor DOM attributes:

```javascript
new EditorView(dom, {
  attributes: {
    spellcheck: "false",
    class: "prose-mirror-editor"
  }
})
```