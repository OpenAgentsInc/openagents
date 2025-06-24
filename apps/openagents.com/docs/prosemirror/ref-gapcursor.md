# ProseMirror Gap Cursor Module Reference

## Overview

The prosemirror-gapcursor module provides a special cursor type for navigating positions that don't allow regular text selection, such as between block nodes or around non-editable content. It renders as a horizontal line to indicate positions where text cannot be directly inserted.

## Installation

```bash
npm install prosemirror-gapcursor
```

## Core Components

### gapCursor()

Creates a gap cursor plugin.

```javascript
import {gapCursor} from 'prosemirror-gapcursor'

const gapCursorPlugin = gapCursor()
```

No configuration options - the plugin works automatically.

### GapCursor Class

A special selection subclass representing a gap cursor.

```javascript
import {GapCursor} from 'prosemirror-gapcursor'

// Create gap cursor at position
const cursor = new GapCursor(doc.resolve(pos))

// Check if selection is gap cursor
if (selection instanceof GapCursor) {
  console.log('Gap cursor at:', selection.pos)
}
```

Properties:
- `pos`: Position of the gap cursor
- `$pos`: Resolved position
- All standard Selection properties

## How Gap Cursors Work

Gap cursors appear in positions where:
1. Normal text selection cannot exist
2. Navigation would otherwise skip over the position
3. User needs to insert content between block nodes

Common scenarios:
- Between two block nodes (e.g., between images)
- Before the first block in the document
- After the last block in the document  
- Around non-editable nodes
- Between table cells (with table plugins)

## CSS Styling

The gap cursor requires CSS to be visible:

```css
/* Basic gap cursor styling */
.ProseMirror-gapcursor {
  display: none;
  pointer-events: none;
  position: absolute;
  margin-top: -2px;
}

.ProseMirror-gapcursor:after {
  content: "";
  display: block;
  position: absolute;
  top: -2px;
  width: 20px;
  border-top: 1px solid black;
  animation: ProseMirror-cursor-blink 1.1s steps(2, start) infinite;
}

@keyframes ProseMirror-cursor-blink {
  to {
    visibility: hidden;
  }
}

.ProseMirror-hideselection .ProseMirror-gapcursor {
  display: none;
}

.ProseMirror.ProseMirror-focused .ProseMirror-gapcursor {
  display: block;
}
```

## Usage Examples

### Basic Setup

```javascript
import {EditorState} from 'prosemirror-state'
import {EditorView} from 'prosemirror-view'
import {gapCursor} from 'prosemirror-gapcursor'

// Include gap cursor in plugins
const state = EditorState.create({
  schema,
  plugins: [
    gapCursor()
  ]
})

const view = new EditorView(place, {
  state
})
```

### Custom Gap Cursor Styling

```css
/* Custom gap cursor appearance */
.ProseMirror-gapcursor:after {
  content: "";
  display: block;
  position: absolute;
  top: -2px;
  width: 30px;
  height: 0;
  border-top: 2px solid #06c;
  border-radius: 1px;
  animation: ProseMirror-cursor-blink 1s infinite;
}

/* Different style for tables */
.ProseMirror table .ProseMirror-gapcursor:after {
  width: 15px;
  border-color: #999;
}

/* No animation variant */
.ProseMirror-gapcursor.no-blink:after {
  animation: none;
}
```

### Programmatic Gap Cursor Usage

```javascript
import {GapCursor} from 'prosemirror-gapcursor'
import {TextSelection} from 'prosemirror-state'

// Function to place gap cursor
function placeGapCursor(view, pos) {
  const $pos = view.state.doc.resolve(pos)
  
  // Check if gap cursor is valid at position
  if (GapCursor.valid($pos)) {
    const tr = view.state.tr.setSelection(new GapCursor($pos))
    view.dispatch(tr)
    return true
  }
  
  return false
}

// Find valid gap cursor positions
function findGapCursorPositions(doc) {
  const positions = []
  
  doc.descendants((node, pos) => {
    // Check before node
    const $before = doc.resolve(pos)
    if (GapCursor.valid($before)) {
      positions.push(pos)
    }
    
    // Check after node
    const $after = doc.resolve(pos + node.nodeSize)
    if (GapCursor.valid($after)) {
      positions.push(pos + node.nodeSize)
    }
  })
  
  return positions
}

// Navigate to nearest gap cursor
function moveToNearestGap(view, dir) {
  const {selection} = view.state
  const positions = findGapCursorPositions(view.state.doc)
  
  const currentPos = selection.from
  const validPositions = dir > 0 
    ? positions.filter(p => p > currentPos)
    : positions.filter(p => p < currentPos).reverse()
  
  if (validPositions.length > 0) {
    placeGapCursor(view, validPositions[0])
  }
}
```

### Gap Cursor with Custom Nodes

```javascript
// Schema with nodes that create gaps
const schema = new Schema({
  nodes: {
    doc: {content: "block+"},
    paragraph: {
      content: "inline*",
      group: "block",
      parseDOM: [{tag: "p"}],
      toDOM() { return ["p", 0] }
    },
    image: {
      inline: false,
      group: "block",
      draggable: true,
      attrs: {
        src: {},
        alt: {default: null}
      },
      parseDOM: [{
        tag: "img[src]",
        getAttrs(dom) {
          return {
            src: dom.getAttribute("src"),
            alt: dom.getAttribute("alt")
          }
        }
      }],
      toDOM(node) {
        return ["img", node.attrs]
      }
    },
    figure: {
      content: "image caption?",
      group: "block",
      defining: true,
      parseDOM: [{tag: "figure"}],
      toDOM() { return ["figure", 0] }
    },
    caption: {
      content: "inline*",
      parseDOM: [{tag: "figcaption"}],
      toDOM() { return ["figcaption", 0] }
    },
    text: {group: "inline"}
  }
})

// Gap cursors will automatically work between images, figures, etc.
```

### Key Bindings for Gap Cursor

```javascript
import {keymap} from 'prosemirror-keymap'
import {GapCursor} from 'prosemirror-gapcursor'

// Custom keybindings for gap cursor navigation
const gapCursorKeymap = keymap({
  // Move to gap cursor positions with Ctrl+Arrow
  "Ctrl-ArrowLeft": (state, dispatch, view) => {
    const {selection} = state
    
    if (selection.empty) {
      // Try to find gap cursor position to the left
      for (let pos = selection.from - 1; pos >= 0; pos--) {
        const $pos = state.doc.resolve(pos)
        if (GapCursor.valid($pos)) {
          if (dispatch) {
            dispatch(state.tr.setSelection(new GapCursor($pos)))
          }
          return true
        }
      }
    }
    
    return false
  },
  
  "Ctrl-ArrowRight": (state, dispatch, view) => {
    const {selection} = state
    
    if (selection.empty) {
      // Try to find gap cursor position to the right
      for (let pos = selection.from + 1; pos <= state.doc.content.size; pos++) {
        const $pos = state.doc.resolve(pos)
        if (GapCursor.valid($pos)) {
          if (dispatch) {
            dispatch(state.tr.setSelection(new GapCursor($pos)))
          }
          return true
        }
      }
    }
    
    return false
  }
})
```

### Gap Cursor in Complex Layouts

```javascript
// Handle gap cursors in nested structures
function handleComplexGapCursor(view) {
  // Custom node view that manages gap cursors
  class ComplexNodeView {
    constructor(node, view, getPos) {
      this.node = node
      this.view = view
      this.getPos = getPos
      
      this.dom = document.createElement('div')
      this.dom.className = 'complex-node'
      
      // Add click handlers for gap cursor placement
      this.dom.addEventListener('click', e => {
        const rect = this.dom.getBoundingClientRect()
        const pos = this.getPos()
        
        if (e.clientY < rect.top + 10) {
          // Click near top - place gap cursor before
          placeGapCursor(view, pos)
        } else if (e.clientY > rect.bottom - 10) {
          // Click near bottom - place gap cursor after
          placeGapCursor(view, pos + this.node.nodeSize)
        }
      })
    }
  }
  
  return ComplexNodeView
}

// Table-specific gap cursor handling
function tableGapCursor(state, dispatch) {
  const {selection} = state
  
  if (selection instanceof CellSelection) {
    // Convert cell selection to gap cursor at edge
    const $pos = state.doc.resolve(selection.to)
    
    if (GapCursor.valid($pos)) {
      if (dispatch) {
        dispatch(state.tr.setSelection(new GapCursor($pos)))
      }
      return true
    }
  }
  
  return false
}
```

### Visual Feedback for Gap Cursor

```javascript
// Plugin to add visual hints for gap cursor positions
const gapCursorHints = new Plugin({
  props: {
    decorations(state) {
      const decorations = []
      const {selection} = state
      
      // Only show hints when near potential gap positions
      if (selection.empty) {
        state.doc.descendants((node, pos) => {
          const $pos = state.doc.resolve(pos)
          
          if (GapCursor.valid($pos) && Math.abs(selection.from - pos) < 10) {
            decorations.push(
              Decoration.widget(pos, () => {
                const hint = document.createElement('div')
                hint.className = 'gap-cursor-hint'
                hint.title = 'Click to place cursor here'
                return hint
              })
            )
          }
        })
      }
      
      return DecorationSet.create(state.doc, decorations)
    }
  }
})

// CSS for hints
const hintStyles = `
.gap-cursor-hint {
  position: absolute;
  width: 100%;
  height: 4px;
  background: rgba(0, 100, 200, 0.2);
  cursor: text;
  transition: background 0.2s;
}

.gap-cursor-hint:hover {
  background: rgba(0, 100, 200, 0.4);
}
`
```

### Testing Gap Cursor Positions

```javascript
// Utility to test gap cursor behavior
function testGapCursorPositions(doc) {
  const results = []
  
  for (let pos = 0; pos <= doc.content.size; pos++) {
    try {
      const $pos = doc.resolve(pos)
      const valid = GapCursor.valid($pos)
      
      if (valid) {
        results.push({
          pos,
          before: $pos.nodeBefore?.type.name || 'start',
          after: $pos.nodeAfter?.type.name || 'end',
          parent: $pos.parent.type.name
        })
      }
    } catch (e) {
      // Invalid position
    }
  }
  
  return results
}

// Debug gap cursor placement
function debugGapCursor(view) {
  const positions = testGapCursorPositions(view.state.doc)
  console.log('Valid gap cursor positions:', positions)
  
  // Visualize all possible positions
  const decorations = positions.map(({pos}) => 
    Decoration.widget(pos, () => {
      const marker = document.createElement('span')
      marker.className = 'gap-cursor-debug'
      marker.textContent = '|'
      marker.style.color = 'red'
      return marker
    })
  )
  
  // Temporarily show markers
  const plugin = new Plugin({
    props: {
      decorations: () => DecorationSet.create(view.state.doc, decorations)
    }
  })
  
  // Add and remove after delay
  view.updateState(view.state.reconfigure({
    plugins: view.state.plugins.concat(plugin)
  }))
  
  setTimeout(() => {
    view.updateState(view.state.reconfigure({
      plugins: view.state.plugins.filter(p => p !== plugin)
    }))
  }, 3000)
}
```

## Best Practices

1. **Always include CSS**: Gap cursor is invisible without proper styling
2. **Test with keyboard**: Ensure arrow key navigation works properly
3. **Consider mobile**: Touch interfaces may need special handling
4. **Custom nodes**: Test gap cursor behavior with all custom nodes
5. **Visual feedback**: Consider adding hints for gap cursor positions
6. **Accessibility**: Ensure screen readers announce gap cursor positions
7. **Performance**: Gap cursor checks run frequently, keep them efficient

## Complete Example

```javascript
import {EditorState} from 'prosemirror-state'
import {EditorView} from 'prosemirror-view'
import {gapCursor, GapCursor} from 'prosemirror-gapcursor'
import {keymap} from 'prosemirror-keymap'

// Schema with gaps between blocks
const schema = new Schema({
  nodes: {
    doc: {content: "block+"},
    paragraph: {
      content: "inline*",
      group: "block",
      parseDOM: [{tag: "p"}],
      toDOM() { return ["p", 0] }
    },
    horizontal_rule: {
      group: "block",
      parseDOM: [{tag: "hr"}],
      toDOM() { return ["hr"] }
    },
    image: {
      inline: false,
      group: "block",
      attrs: {src: {}, alt: {default: null}},
      parseDOM: [{tag: "img[src]", getAttrs(dom) {
        return {
          src: dom.getAttribute("src"),
          alt: dom.getAttribute("alt")
        }
      }}],
      toDOM(node) { return ["img", node.attrs] }
    },
    text: {group: "inline"}
  }
})

// Create editor with gap cursor
const state = EditorState.create({
  schema,
  doc: schema.nodes.doc.create(null, [
    schema.nodes.paragraph.create(null, [schema.text("Before image")]),
    schema.nodes.image.create({src: "image.png"}),
    schema.nodes.horizontal_rule.create(),
    schema.nodes.paragraph.create(null, [schema.text("After rule")])
  ]),
  plugins: [
    gapCursor(),
    keymap({
      "Mod-g": (state, dispatch) => {
        // Toggle gap cursor at current position
        const $pos = state.selection.$from
        if (GapCursor.valid($pos)) {
          if (dispatch) {
            dispatch(state.tr.setSelection(new GapCursor($pos)))
          }
          return true
        }
        return false
      }
    })
  ]
})

const view = new EditorView(document.querySelector('#editor'), {
  state
})

// Add CSS
const style = document.createElement('style')
style.textContent = `
  .ProseMirror-gapcursor {
    display: none;
    pointer-events: none;
    position: absolute;
  }
  
  .ProseMirror-gapcursor:after {
    content: "";
    display: block;
    position: absolute;
    top: -2px;
    width: 20px;
    border-top: 1px solid black;
    animation: ProseMirror-cursor-blink 1.1s steps(2, start) infinite;
  }
  
  @keyframes ProseMirror-cursor-blink {
    to { visibility: hidden; }
  }
  
  .ProseMirror.ProseMirror-focused .ProseMirror-gapcursor {
    display: block;
  }
`
document.head.appendChild(style)
```

This module is essential for providing complete cursor navigation in editors with block-level content.