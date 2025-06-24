# ProseMirror Schema List Module Reference

## Overview

The prosemirror-schema-list module provides list-related node types and commands for ProseMirror. It includes ordered lists, bullet lists, and list items, along with commands to manipulate list structure.

## Installation

```bash
npm install prosemirror-schema-list
```

## Node Types

### orderedList

Ordered (numbered) list node spec.

```javascript
import {orderedList} from 'prosemirror-schema-list'
```

Properties:
- **Content**: `list_item+`
- **Group**: `block`
- **Attributes**:
  - `order`: Starting number (default: 1)
- **Parse**: `<ol>` tags
- **Serialize**: `["ol", {"start": attrs.order == 1 ? null : attrs.order}, 0]`

### bulletList

Unordered (bullet) list node spec.

```javascript
import {bulletList} from 'prosemirror-schema-list'
```

Properties:
- **Content**: `list_item+`
- **Group**: `block`
- **Parse**: `<ul>` tags
- **Serialize**: `["ul", 0]`

### listItem

List item node spec.

```javascript
import {listItem} from 'prosemirror-schema-list'
```

Properties:
- **Content**: `paragraph block*`
- **Parse**: `<li>` tags
- **Serialize**: `["li", 0]`
- **Defining**: true

## Adding Lists to Schema

```javascript
import {Schema} from 'prosemirror-model'
import {orderedList, bulletList, listItem} from 'prosemirror-schema-list'
import {nodes as basicNodes, marks} from 'prosemirror-schema-basic'

// Add list nodes to schema
const mySchema = new Schema({
  nodes: {
    ...basicNodes,
    ordered_list: orderedList,
    bullet_list: bulletList,
    list_item: listItem
  },
  marks
})
```

## List Commands

### wrapInList(listType, attrs?)

Wrap the selection in a list with the given type.

```javascript
import {wrapInList} from 'prosemirror-schema-list'

const wrapInBulletList = wrapInList(schema.nodes.bullet_list)
const wrapInOrderedList = wrapInList(schema.nodes.ordered_list)

// With custom attributes
const wrapInNumberedList = wrapInList(
  schema.nodes.ordered_list, 
  {order: 5} // Start at 5
)
```

### splitListItem(itemType)

Split a list item into two.

```javascript
import {splitListItem} from 'prosemirror-schema-list'

const splitItem = splitListItem(schema.nodes.list_item)

// Typically bound to Enter key
keymap({
  "Enter": splitListItem(schema.nodes.list_item)
})
```

### liftListItem(itemType)

Lift a list item out of its parent list.

```javascript
import {liftListItem} from 'prosemirror-schema-list'

const liftItem = liftListItem(schema.nodes.list_item)

// Typically bound to Shift-Tab
keymap({
  "Shift-Tab": liftListItem(schema.nodes.list_item)
})
```

### sinkListItem(itemType)

Sink (indent) a list item into a nested list.

```javascript
import {sinkListItem} from 'prosemirror-schema-list'

const sinkItem = sinkListItem(schema.nodes.list_item)

// Typically bound to Tab
keymap({
  "Tab": sinkListItem(schema.nodes.list_item)
})
```

## Usage Examples

### Basic List Creation

```javascript
import {EditorState} from 'prosemirror-state'
import {EditorView} from 'prosemirror-view'
import {Schema} from 'prosemirror-model'
import {orderedList, bulletList, listItem} from 'prosemirror-schema-list'
import {keymap} from 'prosemirror-keymap'

// Schema with lists
const schema = new Schema({
  nodes: {
    doc: {content: "block+"},
    paragraph: {
      content: "inline*",
      group: "block",
      parseDOM: [{tag: "p"}],
      toDOM() { return ["p", 0] }
    },
    ordered_list: orderedList,
    bullet_list: bulletList,
    list_item: listItem,
    text: {group: "inline"}
  }
})

// Create document with lists
const doc = schema.node("doc", null, [
  schema.node("bullet_list", null, [
    schema.node("list_item", null, [
      schema.node("paragraph", null, [schema.text("First item")])
    ]),
    schema.node("list_item", null, [
      schema.node("paragraph", null, [schema.text("Second item")])
    ])
  ])
])

const state = EditorState.create({
  doc,
  schema
})
```

### List Keybindings

```javascript
import {splitListItem, liftListItem, sinkListItem} from 'prosemirror-schema-list'
import {keymap} from 'prosemirror-keymap'

const listKeymap = keymap({
  "Enter": splitListItem(schema.nodes.list_item),
  "Mod-[": liftListItem(schema.nodes.list_item),
  "Mod-]": sinkListItem(schema.nodes.list_item),
  "Tab": sinkListItem(schema.nodes.list_item),
  "Shift-Tab": liftListItem(schema.nodes.list_item)
})

// Add to editor
const state = EditorState.create({
  schema,
  plugins: [listKeymap]
})
```

### List Manipulation Commands

```javascript
import {wrapInList} from 'prosemirror-schema-list'
import {chainCommands} from 'prosemirror-commands'

// Toggle list command
function toggleList(listType) {
  return (state, dispatch, view) => {
    const {$from, $to} = state.selection
    const range = $from.blockRange($to)
    
    if (!range) return false
    
    const parentList = findParentNode(
      node => node.type === listType
    )(state.selection)
    
    if (parentList) {
      // Already in list, lift out
      return liftListItem(schema.nodes.list_item)(state, dispatch, view)
    } else {
      // Not in list, wrap in one
      return wrapInList(listType)(state, dispatch, view)
    }
  }
}

// Create numbered list starting at specific number
function createNumberedList(startNumber) {
  return wrapInList(schema.nodes.ordered_list, {order: startNumber})
}

// Convert between list types
function convertListType(fromType, toType) {
  return (state, dispatch) => {
    const {$from, $to} = state.selection
    const range = $from.blockRange($to)
    
    if (!range) return false
    
    const parentList = findParentNode(
      node => node.type === fromType
    )(state.selection)
    
    if (!parentList) return false
    
    if (dispatch) {
      const tr = state.tr
      tr.setNodeMarkup(parentList.pos, toType, parentList.node.attrs)
      dispatch(tr)
    }
    
    return true
  }
}
```

### Nested Lists

```javascript
// Create nested list structure
const nestedDoc = schema.node("doc", null, [
  schema.node("bullet_list", null, [
    schema.node("list_item", null, [
      schema.node("paragraph", null, [schema.text("Parent item")]),
      schema.node("bullet_list", null, [
        schema.node("list_item", null, [
          schema.node("paragraph", null, [schema.text("Nested item 1")])
        ]),
        schema.node("list_item", null, [
          schema.node("paragraph", null, [schema.text("Nested item 2")])
        ])
      ])
    ]),
    schema.node("list_item", null, [
      schema.node("paragraph", null, [schema.text("Another parent")])
    ])
  ])
])

// Command to create nested list
function nestListItem(state, dispatch) {
  const {$from, $to} = state.selection
  const range = $from.blockRange($to, 
    node => node.type === schema.nodes.list_item
  )
  
  if (!range || range.depth < 2) return false
  
  const listItem = range.parent
  const before = listItem.lastChild
  
  if (!before || before.type !== schema.nodes.bullet_list) {
    // Create new nested list
    if (dispatch) {
      const inner = schema.nodes.bullet_list.create(null, [
        schema.nodes.list_item.create(null, [
          schema.nodes.paragraph.create()
        ])
      ])
      
      const tr = state.tr.insert(range.end - 1, inner)
      dispatch(tr)
    }
    return true
  }
  
  return false
}
```

### List Item with Multiple Blocks

```javascript
// Extended list item that can contain multiple blocks
const richListItem = {
  content: "paragraph (paragraph | code_block | blockquote)*",
  parseDOM: [{tag: "li"}],
  toDOM() { return ["li", 0] },
  defining: true
}

// Schema with rich list items
const richSchema = new Schema({
  nodes: {
    ...basicNodes,
    ordered_list: orderedList,
    bullet_list: bulletList,
    list_item: richListItem
  },
  marks
})

// Create rich list items
const richList = richSchema.node("bullet_list", null, [
  richSchema.node("list_item", null, [
    richSchema.node("paragraph", null, [
      richSchema.text("List item with multiple blocks:")
    ]),
    richSchema.node("code_block", null, [
      richSchema.text("console.log('Code in list');")
    ]),
    richSchema.node("blockquote", null, [
      richSchema.node("paragraph", null, [
        richSchema.text("A quote within a list item")
      ])
    ])
  ])
])
```

### Smart List Behavior

```javascript
// Smart Enter key handling
function smartSplitListItem(itemType) {
  return (state, dispatch, view) => {
    const {$from, $to} = state.selection
    const node = $from.node(-1)
    
    if (node.type !== itemType) {
      return splitListItem(itemType)(state, dispatch, view)
    }
    
    // Check if list item is empty
    if (node.content.size === 0 || 
        (node.content.size === 2 && node.firstChild.isTextblock && 
         node.firstChild.content.size === 0)) {
      // Empty item - lift out of list
      if (liftListItem(itemType)(state, dispatch, view)) {
        return true
      }
    }
    
    // Check if at end of item
    if ($from.parentOffset === node.content.size - 2) {
      // At end - create new item after
      return splitListItem(itemType)(state, dispatch, view)
    }
    
    // Otherwise, normal split
    return splitListItem(itemType)(state, dispatch, view)
  }
}

// Smart backspace handling
function smartJoinListItem(itemType) {
  return (state, dispatch, view) => {
    const {$from} = state.selection
    
    // Only at start of list item
    if ($from.parentOffset !== 0) return false
    
    const node = $from.node(-1)
    if (node.type !== itemType) return false
    
    // Try to join with previous item
    const before = $from.node(-2).maybeChild($from.indexAfter(-2) - 1)
    
    if (before && before.type === itemType) {
      // Join with previous item
      if (dispatch) {
        const tr = state.tr
        const beforePos = $from.pos - $from.parentOffset - 2
        tr.join(beforePos + before.nodeSize)
        dispatch(tr)
      }
      return true
    }
    
    // Otherwise, try to lift
    return liftListItem(itemType)(state, dispatch, view)
  }
}
```

### List Utilities

```javascript
// Check if in list
function isInList(state, listType) {
  const {$from} = state.selection
  
  for (let d = $from.depth; d > 0; d--) {
    if ($from.node(d).type === listType) {
      return true
    }
  }
  
  return false
}

// Get list depth
function getListDepth(state) {
  const {$from} = state.selection
  let depth = 0
  
  for (let d = $from.depth; d > 0; d--) {
    const node = $from.node(d)
    if (node.type === schema.nodes.bullet_list || 
        node.type === schema.nodes.ordered_list) {
      depth++
    }
  }
  
  return depth
}

// Find parent list
function findParentList(state) {
  const {$from} = state.selection
  
  for (let d = $from.depth; d > 0; d--) {
    const node = $from.node(d)
    if (node.type === schema.nodes.bullet_list || 
        node.type === schema.nodes.ordered_list) {
      return {node, pos: $from.before(d), depth: d}
    }
  }
  
  return null
}
```

### List Styling

```css
/* Custom list styling */
.ProseMirror ul {
  list-style-type: disc;
}

.ProseMirror ul ul {
  list-style-type: circle;
}

.ProseMirror ul ul ul {
  list-style-type: square;
}

.ProseMirror ol {
  list-style-type: decimal;
}

.ProseMirror ol ol {
  list-style-type: lower-alpha;
}

.ProseMirror ol ol ol {
  list-style-type: lower-roman;
}

/* List item spacing */
.ProseMirror li {
  margin: 0.25em 0;
}

.ProseMirror li p {
  margin: 0;
}

/* Nested list indentation */
.ProseMirror ul, .ProseMirror ol {
  padding-left: 2em;
}
```

### Complete List Plugin

```javascript
import {Plugin} from 'prosemirror-state'
import {splitListItem, liftListItem, sinkListItem, wrapInList} from 'prosemirror-schema-list'

function createListPlugin(schema) {
  return new Plugin({
    props: {
      handleKeyDown(view, event) {
        const {state, dispatch} = view
        
        // Enter key
        if (event.key === 'Enter' && !event.shiftKey) {
          if (smartSplitListItem(schema.nodes.list_item)(state, dispatch, view)) {
            event.preventDefault()
            return true
          }
        }
        
        // Tab key
        if (event.key === 'Tab') {
          if (event.shiftKey) {
            if (liftListItem(schema.nodes.list_item)(state, dispatch, view)) {
              event.preventDefault()
              return true
            }
          } else {
            if (sinkListItem(schema.nodes.list_item)(state, dispatch, view)) {
              event.preventDefault()
              return true
            }
          }
        }
        
        // Backspace at start of item
        if (event.key === 'Backspace') {
          const {$from} = state.selection
          if ($from.parentOffset === 0) {
            const itemType = schema.nodes.list_item
            if ($from.node(-1).type === itemType) {
              if (smartJoinListItem(itemType)(state, dispatch, view)) {
                event.preventDefault()
                return true
              }
            }
          }
        }
        
        return false
      }
    }
  })
}
```

## Best Practices

1. **Consistent keybindings**: Use standard Tab/Shift-Tab for indentation
2. **Smart Enter handling**: Empty items should exit list
3. **Preserve list type**: Maintain ordered/unordered when manipulating
4. **Limit nesting**: Consider maximum nesting depth
5. **Accessible markup**: Ensure proper semantic HTML
6. **Style consistently**: Match platform conventions
7. **Test edge cases**: Empty lists, deeply nested items

This module provides comprehensive list support for ProseMirror editors.