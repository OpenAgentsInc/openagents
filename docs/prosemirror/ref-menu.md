# ProseMirror Menu Module Reference

## Overview

The prosemirror-menu module provides a menu bar and menu items for ProseMirror, offering a traditional UI for executing editor commands. It includes pre-built menu items for common operations and utilities for creating custom menus.

## Installation

```bash
npm install prosemirror-menu
```

## Core Components

### MenuItem

Represents a single menu item.

```javascript
import {MenuItem} from 'prosemirror-menu'

const boldItem = new MenuItem({
  title: "Toggle bold",
  content: "B",  // Can be string or DOM node
  enable: state => toggleMark(schema.marks.strong)(state),
  active: state => isMarkActive(state, schema.marks.strong),
  run: toggleMark(schema.marks.strong)
})
```

Constructor options:
- `title`: Tooltip text
- `content`: Display content (string or DOM node)
- `enable`: Function `(state) => boolean` to check if enabled
- `active`: Function `(state) => boolean` to check if active
- `run`: Function `(state, dispatch, view, event)` to execute
- `select`: Alternative to `enable` with different signature
- `class`: CSS class to add
- `css`: CSS text to add to style

### Dropdown

A dropdown menu containing multiple items.

```javascript
import {Dropdown} from 'prosemirror-menu'

const headingDropdown = new Dropdown(
  [heading1Item, heading2Item, heading3Item],
  {label: "Heading"}
)
```

Constructor:
- `content`: Array of MenuItems
- `options`: Object with `label` and optional `title`, `class`

### DropdownSubmenu

A submenu that appears on hover.

```javascript
import {DropdownSubmenu} from 'prosemirror-menu'

const insertSubmenu = new DropdownSubmenu(
  [imageItem, videoItem, tableItem],
  {label: "Insert"}
)
```

### MenuBar

The main menu bar containing items.

```javascript
const menuBar = menuBar({
  content: [
    [boldItem, italicItem],     // Group 1
    [headingDropdown],          // Group 2
    [undoItem, redoItem]        // Group 3
  ],
  floating: false
})
```

Options:
- `content`: Array of arrays (groups) of menu items
- `floating`: Whether menu floats above content

## Pre-built Menu Items

### Text Formatting

```javascript
import {
  toggleMarkItem,
  wrapItem,
  blockTypeItem
} from 'prosemirror-menu'

// Mark toggling
const boldItem = toggleMarkItem(schema.marks.strong, {
  title: "Toggle bold",
  content: "B"
})

const italicItem = toggleMarkItem(schema.marks.em, {
  title: "Toggle italic", 
  content: "I"
})

const codeItem = toggleMarkItem(schema.marks.code, {
  title: "Toggle code",
  content: "Code"
})

const linkItem = toggleMarkItem(schema.marks.link, {
  title: "Add link",
  content: "Link"
})
```

### Block Formatting

```javascript
// Block type items
const paragraphItem = blockTypeItem(schema.nodes.paragraph, {
  title: "Change to paragraph",
  label: "Plain"
})

const heading1Item = blockTypeItem(schema.nodes.heading, {
  title: "Change to heading level 1",
  label: "H1",
  attrs: {level: 1}
})

const codeBlockItem = blockTypeItem(schema.nodes.code_block, {
  title: "Change to code block",
  label: "Code"
})

// Wrapping items
const blockquoteItem = wrapItem(schema.nodes.blockquote, {
  title: "Wrap in blockquote",
  content: "Quote"
})
```

### List Operations

```javascript
import {wrapListItem, liftItem} from 'prosemirror-menu'

const bulletListItem = wrapListItem(schema.nodes.bullet_list, {
  title: "Wrap in bullet list",
  content: "• List"
})

const orderedListItem = wrapListItem(schema.nodes.ordered_list, {
  title: "Wrap in ordered list",
  content: "1. List"
})

const liftItem = new MenuItem({
  title: "Lift out of enclosing block",
  run: lift,
  enable: state => lift(state),
  content: "Lift"
})
```

### History Items

```javascript
import {undoItem, redoItem} from 'prosemirror-menu'
import {undo, redo} from 'prosemirror-history'

// Pre-configured undo/redo items
// Or create custom ones:
const customUndoItem = new MenuItem({
  title: "Undo last change",
  run: undo,
  enable: state => undo(state),
  content: "↶"
})
```

## Icon Support

### Using Icons

```javascript
import {MenuItem} from 'prosemirror-menu'

// Font Awesome
const boldIconItem = new MenuItem({
  title: "Bold",
  content: iconElement("fa-bold"),
  run: toggleMark(schema.marks.strong)
})

// Helper function for icons
function iconElement(iconClass) {
  const span = document.createElement("span")
  span.className = `menu-icon fas ${iconClass}`
  return span
}

// SVG icons
function svgIcon(path) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg")
  svg.setAttribute("viewBox", "0 0 24 24")
  svg.setAttribute("width", "20")
  svg.setAttribute("height", "20")
  
  const pathEl = document.createElementNS("http://www.w3.org/2000/svg", "path")
  pathEl.setAttribute("d", path)
  svg.appendChild(pathEl)
  
  return svg
}

const saveItem = new MenuItem({
  title: "Save",
  content: svgIcon("M17 3H5C3.89 3 3 3.9 3 5V19C3..."),
  run: (state, dispatch, view) => {
    // Save logic
  }
})
```

## Styling

### Basic Menu Styles

```css
/* Menu bar */
.ProseMirror-menubar {
  border-bottom: 1px solid #ddd;
  padding: 4px;
  background: #f8f8f8;
  display: flex;
  flex-wrap: wrap;
  gap: 1px;
}

/* Menu groups */
.ProseMirror-menubar .ProseMirror-menuitem {
  margin-right: 3px;
}

/* Menu items */
.ProseMirror-menuitem {
  display: inline-block;
  padding: 4px 8px;
  border: 1px solid transparent;
  border-radius: 3px;
  cursor: pointer;
  background: white;
}

.ProseMirror-menuitem:hover {
  background: #f0f0f0;
  border-color: #ddd;
}

.ProseMirror-menuitem[disabled] {
  opacity: 0.5;
  cursor: not-allowed;
}

.ProseMirror-menuitem.ProseMirror-menuitem-active {
  background: #e0e0e0;
  border-color: #999;
}

/* Dropdowns */
.ProseMirror-dropdown {
  position: relative;
  display: inline-block;
}

.ProseMirror-dropdown-menu {
  position: absolute;
  top: 100%;
  left: 0;
  background: white;
  border: 1px solid #ddd;
  border-radius: 3px;
  box-shadow: 0 2px 5px rgba(0,0,0,0.1);
  padding: 4px;
  margin-top: 2px;
  min-width: 120px;
  z-index: 10;
}

.ProseMirror-dropdown-menu .ProseMirror-menuitem {
  display: block;
  width: 100%;
  text-align: left;
  margin: 2px 0;
}
```

## Usage Examples

### Basic Menu Setup

```javascript
import {EditorState} from 'prosemirror-state'
import {EditorView} from 'prosemirror-view'
import {menuBar} from 'prosemirror-menu'
import {buildMenuItems} from './menu-items'

// Create menu bar plugin
const menuPlugin = menuBar({
  content: buildMenuItems(schema).fullMenu
})

// Create editor with menu
const state = EditorState.create({
  schema,
  plugins: [menuPlugin]
})

const view = new EditorView(document.querySelector('#editor'), {
  state
})
```

### Custom Menu Building

```javascript
import {MenuItem, Dropdown} from 'prosemirror-menu'

function buildMenuItems(schema) {
  const marks = schema.marks
  const nodes = schema.nodes
  
  // Text formatting
  const textMenu = []
  
  if (marks.strong) {
    textMenu.push(new MenuItem({
      title: "Bold",
      content: "B",
      key: "Mod-b",
      run: toggleMark(marks.strong),
      active: state => isMarkActive(state, marks.strong),
      enable: state => toggleMark(marks.strong)(state)
    }))
  }
  
  if (marks.em) {
    textMenu.push(new MenuItem({
      title: "Italic",
      content: "I",
      key: "Mod-i",
      run: toggleMark(marks.em),
      active: state => isMarkActive(state, marks.em),
      enable: state => toggleMark(marks.em)(state)
    }))
  }
  
  // Block formatting
  const blockMenu = []
  
  if (nodes.heading) {
    const headingItems = []
    for (let i = 1; i <= 6; i++) {
      headingItems.push(blockTypeItem(nodes.heading, {
        title: `Heading ${i}`,
        label: `H${i}`,
        attrs: {level: i}
      }))
    }
    
    blockMenu.push(new Dropdown(headingItems, {
      label: "Headings",
      title: "Change heading level"
    }))
  }
  
  // Insert menu
  const insertMenu = []
  
  if (nodes.image) {
    insertMenu.push(new MenuItem({
      title: "Insert image",
      content: "Image",
      enable: state => canInsert(state, nodes.image),
      run(state, dispatch) {
        const src = prompt("Image URL:")
        if (src) {
          const node = nodes.image.create({src})
          dispatch(state.tr.replaceSelectionWith(node))
        }
      }
    }))
  }
  
  if (nodes.horizontal_rule) {
    insertMenu.push(new MenuItem({
      title: "Insert horizontal rule",
      content: "—",
      enable: state => canInsert(state, nodes.horizontal_rule),
      run(state, dispatch) {
        dispatch(state.tr.replaceSelectionWith(
          nodes.horizontal_rule.create()
        ))
      }
    }))
  }
  
  return {
    textMenu,
    blockMenu,
    insertMenu,
    fullMenu: [textMenu, blockMenu, insertMenu]
  }
}
```

### Dynamic Menu Items

```javascript
// Menu item that changes based on state
class DynamicMenuItem extends MenuItem {
  constructor(options) {
    super(options)
    this.options = options
  }
  
  render(view) {
    const el = super.render(view)
    
    // Update content based on state
    const state = view.state
    if (this.options.getContent) {
      const content = this.options.getContent(state)
      el.textContent = content
    }
    
    return el
  }
}

// Example: Toggle list button
const toggleListItem = new DynamicMenuItem({
  getContent(state) {
    return isInList(state) ? "Exit list" : "Bullet list"
  },
  title: "Toggle list",
  run(state, dispatch) {
    if (isInList(state)) {
      return liftListItem(schema.nodes.list_item)(state, dispatch)
    } else {
      return wrapInList(schema.nodes.bullet_list)(state, dispatch)
    }
  }
})
```

### Contextual Menus

```javascript
// Menu that shows different items based on selection
function contextualMenu(schema) {
  return new Plugin({
    view(editorView) {
      const menu = document.createElement("div")
      menu.className = "contextual-menu"
      menu.style.display = "none"
      editorView.dom.parentNode.appendChild(menu)
      
      return {
        update(view) {
          const {from, to} = view.state.selection
          
          if (from === to) {
            menu.style.display = "none"
            return
          }
          
          // Show menu near selection
          const coords = view.coordsAtPos(from)
          menu.style.display = "block"
          menu.style.left = coords.left + "px"
          menu.style.top = (coords.top - 40) + "px"
          
          // Update menu items
          menu.innerHTML = ""
          
          const items = getContextualItems(view.state)
          items.forEach(item => {
            menu.appendChild(item.render(view))
          })
        },
        
        destroy() {
          menu.remove()
        }
      }
    }
  })
}
```

### Floating Toolbar

```javascript
import {menuBar} from 'prosemirror-menu'

// Floating menu that follows selection
const floatingMenu = menuBar({
  floating: true,
  content: [
    [boldItem, italicItem, codeItem],
    [linkItem]
  ]
})

// Custom floating toolbar
class FloatingToolbar {
  constructor(items, view) {
    this.items = items
    this.view = view
    
    this.toolbar = document.createElement("div")
    this.toolbar.className = "floating-toolbar"
    this.toolbar.style.position = "absolute"
    this.toolbar.style.display = "none"
    
    items.forEach(item => {
      this.toolbar.appendChild(item.render(view))
    })
    
    document.body.appendChild(this.toolbar)
    this.update()
  }
  
  update() {
    const {from, to} = this.view.state.selection
    
    if (from === to) {
      this.toolbar.style.display = "none"
      return
    }
    
    const start = this.view.coordsAtPos(from)
    const end = this.view.coordsAtPos(to)
    const box = this.toolbar.getBoundingClientRect()
    
    const left = Math.max((start.left + end.left) / 2 - box.width / 2, 0)
    const top = start.top - box.height - 5
    
    this.toolbar.style.left = left + "px"
    this.toolbar.style.top = top + "px"
    this.toolbar.style.display = "block"
  }
  
  destroy() {
    this.toolbar.remove()
  }
}
```

### Menu with Keyboard Shortcuts

```javascript
// Display keyboard shortcuts in menu
function menuItemWithShortcut(command, key, options) {
  return new MenuItem({
    ...options,
    title: `${options.title} (${key})`,
    render(view) {
      const el = document.createElement("button")
      el.className = "ProseMirror-menuitem"
      
      const label = document.createElement("span")
      label.textContent = options.content || options.label
      el.appendChild(label)
      
      const shortcut = document.createElement("span")
      shortcut.className = "menu-shortcut"
      shortcut.textContent = key
      el.appendChild(shortcut)
      
      return el
    }
  })
}

// Usage
const boldWithShortcut = menuItemWithShortcut(
  toggleMark(schema.marks.strong),
  "Ctrl-B",
  {
    title: "Toggle bold",
    content: "Bold"
  }
)

// CSS for shortcuts
const shortcutStyles = `
.menu-shortcut {
  float: right;
  color: #666;
  font-size: 0.85em;
  margin-left: 1em;
}
`
```

### Advanced Menu Configuration

```javascript
// Menu with custom rendering
class IconMenu {
  constructor(schema) {
    this.schema = schema
  }
  
  buildMenu() {
    return [
      // Format group
      [
        this.iconItem("format_bold", toggleMark(schema.marks.strong)),
        this.iconItem("format_italic", toggleMark(schema.marks.em)),
        this.iconItem("format_underline", toggleMark(schema.marks.underline)),
        this.iconItem("code", toggleMark(schema.marks.code))
      ],
      
      // Block group
      [
        this.dropdownItem("text_fields", [
          this.textItem("P", setBlockType(schema.nodes.paragraph)),
          this.textItem("H1", setBlockType(schema.nodes.heading, {level: 1})),
          this.textItem("H2", setBlockType(schema.nodes.heading, {level: 2})),
          this.textItem("H3", setBlockType(schema.nodes.heading, {level: 3}))
        ]),
        this.iconItem("format_quote", wrapIn(schema.nodes.blockquote)),
        this.iconItem("format_list_bulleted", wrapInList(schema.nodes.bullet_list)),
        this.iconItem("format_list_numbered", wrapInList(schema.nodes.ordered_list))
      ],
      
      // Insert group
      [
        this.iconItem("image", this.insertImage.bind(this)),
        this.iconItem("link", this.insertLink.bind(this)),
        this.iconItem("horizontal_rule", this.insertHR.bind(this))
      ],
      
      // History group
      [
        this.iconItem("undo", undo),
        this.iconItem("redo", redo)
      ]
    ]
  }
  
  iconItem(icon, command) {
    return new MenuItem({
      content: this.materialIcon(icon),
      run: command,
      enable: state => command(state),
      active: state => {
        // Check if command is active
        if (command.isActive) {
          return command.isActive(state)
        }
        return false
      }
    })
  }
  
  materialIcon(name) {
    const i = document.createElement("i")
    i.className = "material-icons"
    i.textContent = name
    return i
  }
}
```

## Best Practices

1. **Group related items**: Organize menu items into logical groups
2. **Show state**: Use active/disabled states to show command availability
3. **Provide tooltips**: Include helpful title attributes
4. **Consider mobile**: Menus may need different layouts on small screens
5. **Keyboard shortcuts**: Display shortcuts in tooltips or menu items
6. **Icons vs text**: Use icons for common operations, text for clarity
7. **Test all commands**: Ensure menu items work in all contexts

## Complete Example

```javascript
import {EditorState} from 'prosemirror-state'
import {EditorView} from 'prosemirror-view'
import {menuBar, MenuItem, Dropdown} from 'prosemirror-menu'
import {toggleMark, setBlockType, wrapIn} from 'prosemirror-commands'
import {undo, redo} from 'prosemirror-history'

// Complete menu setup
function createMenuPlugin(schema) {
  const menu = menuBar({
    content: [
      // Formatting
      [
        new MenuItem({
          title: "Bold (Ctrl-B)",
          content: "B",
          run: toggleMark(schema.marks.strong),
          active: state => isMarkActive(state, schema.marks.strong),
          enable: state => toggleMark(schema.marks.strong)(state)
        }),
        new MenuItem({
          title: "Italic (Ctrl-I)",
          content: "I",
          run: toggleMark(schema.marks.em),
          active: state => isMarkActive(state, schema.marks.em),
          enable: state => toggleMark(schema.marks.em)(state)
        })
      ],
      
      // Blocks
      [
        new Dropdown([
          blockTypeItem(schema.nodes.paragraph, {
            title: "Paragraph",
            label: "¶"
          }),
          blockTypeItem(schema.nodes.heading, {
            title: "Heading 1",
            label: "H1",
            attrs: {level: 1}
          }),
          blockTypeItem(schema.nodes.heading, {
            title: "Heading 2", 
            label: "H2",
            attrs: {level: 2}
          })
        ], {label: "Type", title: "Text type"}),
        
        wrapItem(schema.nodes.blockquote, {
          title: "Blockquote",
          content: "❝"
        })
      ],
      
      // History
      [
        new MenuItem({
          title: "Undo",
          content: "↶",
          run: undo,
          enable: state => undo(state)
        }),
        new MenuItem({
          title: "Redo",
          content: "↷",
          run: redo,
          enable: state => redo(state)
        })
      ]
    ]
  })
  
  return menu
}

// Helper functions
function isMarkActive(state, type) {
  const {from, $from, to, empty} = state.selection
  if (empty) {
    return type.isInSet(state.storedMarks || $from.marks())
  } else {
    return state.doc.rangeHasMark(from, to, type)
  }
}

function canInsert(state, nodeType) {
  const $from = state.selection.$from
  for (let d = $from.depth; d >= 0; d--) {
    const index = $from.index(d)
    if ($from.node(d).canReplaceWith(index, index, nodeType)) {
      return true
    }
  }
  return false
}

// Initialize editor
const state = EditorState.create({
  schema,
  plugins: [
    createMenuPlugin(schema),
    keymap(baseKeymap),
    history()
  ]
})

const view = new EditorView(document.querySelector('#editor'), {
  state
})
```

This module provides flexible menu creation for ProseMirror editors.