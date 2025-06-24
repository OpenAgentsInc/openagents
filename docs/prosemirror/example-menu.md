# ProseMirror Menu Example

## Overview

This example demonstrates how to create a custom menu for a ProseMirror editor, highlighting an approach to building interactive menu components that dynamically update based on editor state.

## CSS Styles

```css
.menubar { 
  border-bottom: 1px solid rgba(0, 0, 0, 0.2); 
  line-height: 0.1; 
}

.menuicon { 
  display: inline-block; 
  border-right: 1px solid rgba(0, 0, 0, 0.2); 
  color: #888; 
  line-height: 1; 
  padding: 0 7px; 
  margin: 1px; 
  cursor: pointer; 
  text-align: center; 
  min-width: 1.4em; 
}

.strong, .heading { font-weight: bold; }
.em { font-style: italic; }
#editor { padding-top: 0 }
```

## JavaScript Code

```javascript
import {Plugin} from "prosemirror-state"
import {toggleMark, setBlockType, wrapIn} from "prosemirror-commands"
import {schema} from "prosemirror-schema-basic"

// Menu View Component
class MenuView {
  constructor(items, editorView) {
    this.items = items
    this.editorView = editorView

    this.dom = document.createElement("div")
    this.dom.className = "menubar"
    items.forEach(({dom}) => this.dom.appendChild(dom))
    this.update()

    this.dom.addEventListener("mousedown", e => {
      e.preventDefault()
      editorView.focus()
      items.forEach(({command, dom}) => {
        if (dom.contains(e.target))
          command(editorView.state, editorView.dispatch, editorView)
      })
    })
  }

  update() {
    this.items.forEach(({command, dom}) => {
      let active = command(this.editorView.state, null, this.editorView)
      dom.style.display = active ? "" : "none"
    })
  }

  destroy() { this.dom.remove() }
}

// Menu plugin
function menuPlugin(items) {
  return new Plugin({
    view(editorView) {
      let menuView = new MenuView(items, editorView)
      editorView.dom.parentNode.insertBefore(menuView.dom, editorView.dom)
      return menuView
    }
  })
}

// Helper to create menu icons
function icon(text, name) {
  let span = document.createElement("span")
  span.className = "menuicon " + name
  span.title = name
  span.textContent = text
  return span
}

// Helper to create heading menu items
function heading(level) {
  return {
    command: setBlockType(schema.nodes.heading, {level}),
    dom: icon("H" + level, "heading")
  }
}

// Create menu with various formatting options
let menu = menuPlugin([
  {command: toggleMark(schema.marks.strong), dom: icon("B", "strong")},
  {command: toggleMark(schema.marks.em), dom: icon("i", "em")},
  {command: setBlockType(schema.nodes.paragraph), dom: icon("p", "paragraph")},
  heading(1), heading(2), heading(3),
  {command: wrapIn(schema.nodes.blockquote), dom: icon(">", "blockquote")}
])

// Initialize editor with menu plugin
import {EditorState} from "prosemirror-state"
import {EditorView} from "prosemirror-view"
import {baseKeymap} from "prosemirror-commands"
import {keymap} from "prosemirror-keymap"

window.view = new EditorView(document.querySelector("#editor"), {
  state: EditorState.create({
    schema,
    plugins: [keymap(baseKeymap), menu]
  })
})
```

## Key Features

### Dynamic Menu Updates
- Menu items automatically show/hide based on whether their commands can be applied
- The `update()` method checks each command against the current editor state
- Items that aren't applicable in the current context are hidden

### Command-based Architecture
- Each menu item has a `command` function and a `dom` element
- Commands follow ProseMirror's command protocol
- Commands return `true` when they can be applied, enabling dynamic UI updates

### Plugin Integration
- Menu is implemented as a ProseMirror plugin
- Automatically manages menu lifecycle with the editor
- Inserts menu DOM before the editor element

### Design Considerations
- Simple approach suitable for basic menus
- For more advanced features (dropdowns, active states), consider using `prosemirror-menu` package
- Menu responds to clicks but maintains editor focus
- Prevents default mousedown behavior to avoid selection issues

## Usage Notes

This example demonstrates the fundamentals of creating a custom menu system. The approach:
1. Creates menu items with associated commands
2. Dynamically updates visibility based on command applicability
3. Handles user interaction while maintaining editor focus
4. Integrates cleanly with ProseMirror's plugin system