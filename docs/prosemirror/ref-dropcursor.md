# ProseMirror Drop Cursor Module Reference

## Overview

The prosemirror-dropcursor module provides a visual cursor that appears when dragging content over the editor, indicating where the content will be inserted when dropped. It helps users understand exactly where their dragged content will land.

## Installation

```bash
npm install prosemirror-dropcursor
```

## Core Function

### dropCursor(options?)

Creates a drop cursor plugin.

```javascript
import {dropCursor} from 'prosemirror-dropcursor'

const dropCursorPlugin = dropCursor({
  color: 'black',      // CSS color for cursor (default: 'black')
  width: 1,           // Cursor width in pixels (default: 1)
  class: ''           // Additional CSS class (optional)
})
```

Options:
- `color`: CSS color string for the cursor line
- `width`: Width of the cursor in pixels
- `class`: Additional CSS class to apply to the cursor element

## How It Works

The drop cursor:
1. Monitors drag events over the editor
2. Calculates valid drop positions
3. Shows a visual indicator at the nearest valid position
4. Hides when dragging ends or leaves the editor

## CSS Requirements

The drop cursor creates elements with the class `.ProseMirror-drop-cursor`. Basic styling is applied inline, but you can override with CSS:

```css
/* Custom drop cursor styling */
.ProseMirror-drop-cursor {
  /* Default styles are applied inline */
  /* Override carefully to maintain functionality */
}

/* Custom class example */
.ProseMirror-drop-cursor.my-custom-cursor {
  box-shadow: 0 0 4px rgba(0, 100, 200, 0.5);
}
```

## Usage Examples

### Basic Setup

```javascript
import {EditorState} from 'prosemirror-state'
import {EditorView} from 'prosemirror-view'
import {dropCursor} from 'prosemirror-dropcursor'

// Create editor with drop cursor
const state = EditorState.create({
  schema,
  plugins: [
    dropCursor() // Default black, 1px cursor
  ]
})

const view = new EditorView(place, {
  state
})
```

### Custom Styled Drop Cursor

```javascript
// Blue drop cursor with custom width
const blueDropCursor = dropCursor({
  color: '#0066cc',
  width: 2
})

// Drop cursor with custom class
const customDropCursor = dropCursor({
  color: 'transparent',
  class: 'custom-drop-indicator'
})

// CSS for custom class
const style = document.createElement('style')
style.textContent = `
  .ProseMirror-drop-cursor.custom-drop-indicator {
    border-left: 3px solid #00aa00;
    margin-left: -1.5px;
    animation: pulse 1s infinite;
  }
  
  @keyframes pulse {
    0% { opacity: 1; }
    50% { opacity: 0.5; }
    100% { opacity: 1; }
  }
`
document.head.appendChild(style)
```

### Theme-Aware Drop Cursor

```javascript
// Drop cursor that matches theme
function themedDropCursor() {
  const isDarkMode = window.matchMedia('(prefers-color-scheme: dark)').matches
  
  return dropCursor({
    color: isDarkMode ? '#ffffff' : '#000000',
    width: isDarkMode ? 1 : 2
  })
}

// Update on theme change
window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', (e) => {
  // Recreate editor with new drop cursor
  view.updateState(view.state.reconfigure({
    plugins: [
      themedDropCursor(),
      ...otherPlugins
    ]
  }))
})
```

### Drop Cursor with Drag & Drop

```javascript
import {dropCursor} from 'prosemirror-dropcursor'
import {EditorView} from 'prosemirror-view'

// Complete drag & drop setup
const view = new EditorView(place, {
  state: EditorState.create({
    schema,
    plugins: [
      dropCursor({
        color: '#0080ff',
        width: 2
      })
    ]
  }),
  
  // Handle drop events
  handleDrop(view, event, slice, moved) {
    // Drop cursor will show during drag
    // This handles the actual drop
    console.log('Content dropped at position:', view.posAtCoords({
      left: event.clientX,
      top: event.clientY
    }))
    
    // Return false to use default behavior
    return false
  },
  
  // Optional: customize drag behavior
  handleDOMEvents: {
    dragover(view, event) {
      // Drop cursor updates automatically
      // Add custom behavior if needed
      event.dataTransfer.dropEffect = 'copy'
      return false
    }
  }
})
```

### Multiple Drop Cursor Styles

```javascript
// Different cursors for different content types
class SmartDropCursor {
  constructor() {
    this.currentType = null
  }
  
  plugin() {
    return new Plugin({
      props: {
        handleDOMEvents: {
          dragenter: (view, event) => {
            // Detect content type being dragged
            const types = event.dataTransfer.types
            
            if (types.includes('Files')) {
              this.currentType = 'file'
            } else if (types.includes('text/html')) {
              this.currentType = 'html'
            } else {
              this.currentType = 'text'
            }
            
            // Update drop cursor style
            this.updateDropCursor(view)
            return false
          }
        }
      }
    })
  }
  
  updateDropCursor(view) {
    const plugins = view.state.plugins.filter(p => 
      !p.spec.key || p.spec.key.key !== 'drop-cursor'
    )
    
    const dropCursorConfig = {
      file: {color: '#00aa00', width: 3},
      html: {color: '#aa0000', width: 2},
      text: {color: '#000000', width: 1}
    }[this.currentType]
    
    plugins.push(dropCursor(dropCursorConfig))
    
    view.updateState(view.state.reconfigure({plugins}))
  }
}
```

### Drop Cursor with Visual Feedback

```javascript
// Enhanced drop cursor with preview
const enhancedDropCursor = () => {
  return new Plugin({
    props: {
      decorations(state) {
        // Could add decorations around drop position
        return DecorationSet.empty
      },
      
      handleDOMEvents: {
        dragover(view, event) {
          const pos = view.posAtCoords({
            left: event.clientX,
            top: event.clientY
          })
          
          if (pos) {
            // Add custom visual feedback
            const $pos = view.state.doc.resolve(pos.pos)
            console.log('Hovering over:', $pos.parent.type.name)
          }
          
          return false
        }
      }
    }
  })
}

// Use with standard drop cursor
const plugins = [
  dropCursor({color: '#0066cc', width: 2}),
  enhancedDropCursor()
]
```

### Conditional Drop Cursor

```javascript
// Show drop cursor only for certain conditions
const conditionalDropCursor = () => {
  let showCursor = true
  
  return new Plugin({
    filterTransaction(tr, state) {
      // Disable drop cursor in read-only mode
      showCursor = view.editable
      return true
    },
    
    appendTransaction(transactions, oldState, newState) {
      if (!showCursor) {
        // Remove drop cursor decorations
        // (This is a conceptual example)
      }
      return null
    }
  })
}

// Drop cursor for specific node types only
const selectiveDropCursor = dropCursor({
  color: '#006600',
  width: 2
})

// Override drop position calculation
const customDropPosition = (view, event) => {
  const pos = view.posAtCoords({
    left: event.clientX,
    top: event.clientY
  })
  
  if (pos) {
    const $pos = view.state.doc.resolve(pos.pos)
    
    // Only allow drops in certain contexts
    if ($pos.parent.type.name === 'code_block') {
      return null // No drop in code blocks
    }
    
    return pos
  }
  
  return null
}
```

### Drop Cursor with Animation

```css
/* Animated drop cursor */
@keyframes drop-cursor-pulse {
  0% {
    opacity: 1;
    transform: scaleY(1);
  }
  50% {
    opacity: 0.6;
    transform: scaleY(1.2);
  }
  100% {
    opacity: 1;
    transform: scaleY(1);
  }
}

.ProseMirror-drop-cursor {
  animation: drop-cursor-pulse 1s ease-in-out infinite;
}

/* Gradient drop cursor */
.ProseMirror-drop-cursor.gradient {
  width: 3px !important;
  background: linear-gradient(
    to bottom,
    transparent 0%,
    #0080ff 20%,
    #0080ff 80%,
    transparent 100%
  ) !important;
  border: none !important;
}
```

### Debug Drop Cursor

```javascript
// Debug plugin to log drop cursor behavior
const debugDropCursor = () => {
  return new Plugin({
    props: {
      handleDOMEvents: {
        dragenter(view, event) {
          console.log('Drag entered editor')
          return false
        },
        
        dragover(view, event) {
          const pos = view.posAtCoords({
            left: event.clientX,
            top: event.clientY
          })
          
          if (pos) {
            const $pos = view.state.doc.resolve(pos.pos)
            console.log('Drop position:', {
              pos: pos.pos,
              parent: $pos.parent.type.name,
              parentOffset: $pos.parentOffset
            })
          }
          
          return false
        },
        
        dragleave(view, event) {
          console.log('Drag left editor')
          return false
        },
        
        drop(view, event) {
          console.log('Content dropped')
          return false
        }
      }
    }
  })
}

// Use for debugging
const debugPlugins = [
  dropCursor({color: 'red', width: 3}),
  debugDropCursor()
]
```

## Integration with Other Features

### With File Uploads

```javascript
// Drop cursor for file uploads
const fileDropCursor = dropCursor({
  color: '#00aa00',
  width: 3,
  class: 'file-drop-cursor'
})

// Handle file drops
const handleFileDrop = (view, event, slice, moved) => {
  const files = event.dataTransfer.files
  
  if (files.length > 0) {
    const pos = view.posAtCoords({
      left: event.clientX,
      top: event.clientY
    })
    
    if (pos) {
      // Insert placeholder at drop position
      const placeholder = schema.nodes.image.create({
        src: 'placeholder.png',
        alt: 'Uploading...'
      })
      
      const tr = view.state.tr.insert(pos.pos, placeholder)
      view.dispatch(tr)
      
      // Upload file and replace placeholder
      uploadFile(files[0]).then(url => {
        // Replace placeholder with actual image
      })
    }
    
    return true
  }
  
  return false
}
```

## Best Practices

1. **Choose visible colors**: Ensure cursor contrasts with background
2. **Consider width**: Thicker cursors are easier to see but may obscure content
3. **Test with content**: Verify cursor appears correctly with various node types
4. **Handle edge cases**: Test dragging to document boundaries
5. **Accessibility**: Consider users with visual impairments
6. **Performance**: Drop cursor updates frequently during drag
7. **Mobile support**: Touch devices may have different drag behaviors

## Complete Example

```javascript
import {EditorState} from 'prosemirror-state'
import {EditorView} from 'prosemirror-view'
import {dropCursor} from 'prosemirror-dropcursor'
import {Schema} from 'prosemirror-model'

// Schema that supports draggable content
const schema = new Schema({
  nodes: {
    doc: {content: "block+"},
    paragraph: {
      content: "inline*",
      group: "block",
      draggable: false,
      parseDOM: [{tag: "p"}],
      toDOM() { return ["p", 0] }
    },
    image: {
      inline: true,
      group: "inline",
      draggable: true,
      attrs: {
        src: {},
        alt: {default: null}
      },
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

// Create editor with drop cursor
const state = EditorState.create({
  schema,
  plugins: [
    dropCursor({
      color: '#0080ff',
      width: 2
    })
  ]
})

const view = new EditorView(document.querySelector('#editor'), {
  state,
  
  // Make images draggable
  handleDOMEvents: {
    dragstart(view, event) {
      const pos = view.posAtCoords({
        left: event.clientX,
        top: event.clientY
      })
      
      if (pos) {
        const $pos = view.state.doc.resolve(pos.pos)
        const node = $pos.nodeAfter
        
        if (node && node.type.name === 'image') {
          event.dataTransfer.effectAllowed = 'move'
          event.dataTransfer.setData('text/html', event.target.outerHTML)
          return true
        }
      }
      
      return false
    }
  }
})

// Add styles
const style = document.createElement('style')
style.textContent = `
  #editor {
    border: 1px solid #ddd;
    padding: 1em;
    min-height: 200px;
  }
  
  #editor img {
    cursor: move;
    max-width: 100%;
  }
  
  #editor img:hover {
    outline: 2px solid #0080ff;
  }
`
document.head.appendChild(style)
```

This module provides essential visual feedback for drag-and-drop operations in ProseMirror.