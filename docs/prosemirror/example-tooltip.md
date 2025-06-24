# ProseMirror Tooltip Example

## Overview

This example demonstrates creating a tooltip in ProseMirror that shows the size of the current text selection. The tooltip appears when text is selected and displays the number of characters selected.

## CSS Styles

```css
.tooltip {
  position: absolute;
  pointer-events: none;
  z-index: 20;
  background: white;
  border: 1px solid silver;
  border-radius: 2px;
  padding: 2px 10px;
  margin-bottom: 7px;
  transform: translateX(-50%);
}

.tooltip:before {
  content: "";
  height: 0;
  width: 0;
  position: absolute;
  left: 50%;
  margin-left: -5px;
  bottom: -6px;
  border: 5px solid transparent;
  border-bottom-width: 0;
  border-top-color: silver;
}

.tooltip:after {
  content: "";
  height: 0;
  width: 0;
  position: absolute;
  left: 50%;
  margin-left: -5px;
  bottom: -4.5px;
  border: 5px solid transparent;
  border-bottom-width: 0;
  border-top-color: white;
}

#editor {
  position: relative;
}
```

## JavaScript Code

```javascript
import {Plugin} from "prosemirror-state"
import {EditorView} from "prosemirror-view"
import {EditorState} from "prosemirror-state"
import {schema} from "prosemirror-schema-basic"
import {exampleSetup} from "prosemirror-example-setup"

// Plugin that shows selection size
let selectionSizePlugin = new Plugin({
  view(editorView) { return new SelectionSizeTooltip(editorView) }
})

// Tooltip view class
class SelectionSizeTooltip {
  constructor(view) {
    this.tooltip = document.createElement("div")
    this.tooltip.className = "tooltip"
    view.dom.parentNode.appendChild(this.tooltip)

    this.update(view, null)
  }

  update(view, lastState) {
    let state = view.state
    // Don't do anything if the document/selection didn't change
    if (lastState && lastState.doc.eq(state.doc) &&
        lastState.selection.eq(state.selection)) return

    // Hide the tooltip if the selection is empty
    if (state.selection.empty) {
      this.tooltip.style.display = "none"
      return
    }

    // Otherwise, reposition it and update its content
    this.tooltip.style.display = ""
    let {from, to} = state.selection
    // These are in screen coordinates
    let start = view.coordsAtPos(from), end = view.coordsAtPos(to)
    // The box in which the tooltip is positioned, to use as base
    let box = this.tooltip.offsetParent.getBoundingClientRect()
    // Find a center-ish x position from the selection endpoints (when
    // crossing lines, end may be more to the left)
    let left = Math.max((start.left + end.left) / 2, start.left + 3)
    this.tooltip.style.left = (left - box.left) + "px"
    this.tooltip.style.bottom = (box.bottom - start.top) + "px"
    this.tooltip.textContent = to - from
  }

  destroy() { 
    this.tooltip.remove() 
  }
}

// Create editor with tooltip plugin
window.view = new EditorView(document.querySelector("#editor"), {
  state: EditorState.create({
    schema,
    plugins: exampleSetup({schema}).concat(selectionSizePlugin)
  })
})
```

## Key Concepts

### Plugin Architecture
The tooltip is implemented as a ProseMirror plugin that:
- Creates a view component when initialized
- Updates the view on every editor state change
- Cleans up when destroyed

### Positioning Strategy
The tooltip positioning uses:
- `coordsAtPos()` to get screen coordinates for selection boundaries
- `offsetParent` to find the positioning context
- Centered positioning between selection start/end points
- Bottom-relative positioning (appears above selection)

### Performance Optimization
The update method includes an optimization:
- Checks if document or selection changed before updating
- Avoids unnecessary DOM updates when nothing relevant changed

### CSS Arrow Technique
The tooltip arrow is created using CSS borders:
- `:before` pseudo-element creates the outer border
- `:after` pseudo-element creates the inner fill
- Both use the border triangle trick for the arrow shape

## Usage Notes

- The tooltip automatically appears when text is selected
- It displays the number of characters in the selection
- The tooltip is positioned above the selection with an arrow pointing down
- It's styled to be unobtrusive and doesn't interfere with editing
- The `pointer-events: none` CSS ensures the tooltip doesn't block mouse interactions