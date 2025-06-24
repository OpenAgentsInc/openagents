# ProseMirror Commands Module Reference

## Overview

The prosemirror-commands module provides a collection of common editing commands that can be used with ProseMirror. Commands are functions that take an editor state and dispatch function, and perform some action.

## Installation

```bash
npm install prosemirror-commands
```

## Command Type

All commands follow this signature:
```typescript
type Command = (
  state: EditorState,
  dispatch?: (tr: Transaction) => void,
  view?: EditorView
) => boolean
```

- Returns `true` if the command is applicable
- When `dispatch` is provided, executes the command
- When `dispatch` is omitted, only checks if command can run

## Core Commands

### Text Manipulation

#### deleteSelection(state, dispatch)
Delete the current selection.

#### joinBackward(state, dispatch, view)
Join the selected block or, if there is a text selection, delete backward from the selection.

#### joinForward(state, dispatch, view)
Join the selected block or, if there is a text selection, delete forward from the selection.

#### joinUp(state, dispatch)
Join the selected block with the block above it.

#### joinDown(state, dispatch)
Join the selected block with the block below it.

#### lift(state, dispatch)
Lift the selected block out of its parent block.

#### newlineInCode(state, dispatch)
When in a code block, create a new line and indent it properly.

#### exitCode(state, dispatch)
Exit from a code block, creating a new paragraph below.

### Mark Commands

#### toggleMark(markType, attrs, options)
Create a command that toggles a mark on the selection.

```javascript
const toggleBold = toggleMark(schema.marks.strong)
const toggleLink = toggleMark(schema.marks.link, {href: "http://example.com"})
```

Options:
- `excludeMarks`: Marks to remove when enabling this mark

### Block Commands

#### setBlockType(nodeType, attrs)
Create a command that sets the selected block(s) to the given node type.

```javascript
const makeHeading1 = setBlockType(schema.nodes.heading, {level: 1})
const makeParagraph = setBlockType(schema.nodes.paragraph)
```

#### wrapIn(nodeType, attrs)
Create a command that wraps the selection in a node of the given type.

```javascript
const wrapInBlockquote = wrapIn(schema.nodes.blockquote)
```

### List Commands

#### splitBlock(state, dispatch)
Split the parent block of the selection.

#### liftEmptyBlock(state, dispatch)
Lift an empty block out of its parent.

#### splitBlockKeepMarks(state, dispatch)
Split block but preserve marks at the split point.

### Selection Commands

#### selectParentNode(state, dispatch)
Select the parent node of the current selection.

#### selectAll(state, dispatch)
Select the entire document.

#### selectTextblockStart(state, dispatch)
Move cursor to the start of the current text block.

#### selectTextblockEnd(state, dispatch)
Move cursor to the end of the current text block.

### Utility Commands

#### autoJoin(command, isJoinable)
Wrap a command to automatically join adjacent nodes after running.

```javascript
const joinableBlockquote = (node) => node.type === schema.nodes.blockquote
const autoJoinBlockquotes = autoJoin(wrapIn(schema.nodes.blockquote), joinableBlockquote)
```

#### chainCommands(...commands)
Chain multiple commands together, trying each until one succeeds.

```javascript
const backspace = chainCommands(
  deleteSelection,
  joinBackward,
  selectNodeBackward
)
```

#### baseKeymap
An object containing basic key bindings:

```javascript
{
  "Enter": chainCommands(newlineInCode, createParagraphNear, liftEmptyBlock, splitBlock),
  "Backspace": chainCommands(deleteSelection, joinBackward, selectNodeBackward),
  "Mod-Backspace": deleteSelection,
  "Delete": chainCommands(deleteSelection, joinForward, selectNodeForward),
  "Mod-Delete": deleteSelection,
  "Mod-a": selectAll,
  "Mod-z": undo,
  "Mod-y": redo,
  "Mod-Shift-z": redo
}
```

### Advanced Commands

#### createParagraphNear(state, dispatch)
Create a new paragraph near the selection.

#### selectNodeBackward(state, dispatch, view)
Select the node before the cursor if any.

#### selectNodeForward(state, dispatch, view)
Select the node after the cursor if any.

#### deleteRange(from, to)
Delete a range of the document.

```javascript
const deleteFirst10Chars = deleteRange(0, 10)
```

#### wrapInList(nodeType, attrs)
Wrap the selection in a list node.

```javascript
const wrapInBulletList = wrapInList(schema.nodes.bullet_list)
```

## Usage Examples

### Basic Command Usage

```javascript
import {splitBlock, joinBackward, toggleMark} from 'prosemirror-commands'
import {keymap} from 'prosemirror-keymap'

// Check if command can run
if (splitBlock(view.state)) {
  // Command is available
}

// Execute command
splitBlock(view.state, view.dispatch)

// In keymap
const myKeymap = keymap({
  "Enter": splitBlock,
  "Backspace": joinBackward,
  "Mod-b": toggleMark(schema.marks.strong)
})
```

### Creating Custom Commands

```javascript
// Simple command
function insertHardBreak(state, dispatch) {
  if (!state.selection.empty) return false
  
  if (dispatch) {
    const br = state.schema.nodes.hard_break.create()
    dispatch(state.tr.replaceSelectionWith(br))
  }
  
  return true
}

// Command with parameters
function insertText(text) {
  return (state, dispatch) => {
    if (dispatch) {
      dispatch(state.tr.insertText(text))
    }
    return true
  }
}

// Complex command
function wrapInCustomBlock(state, dispatch) {
  const {$from, $to} = state.selection
  const range = $from.blockRange($to)
  
  if (!range) return false
  
  const wrapping = findWrapping(range, state.schema.nodes.custom_block)
  if (!wrapping) return false
  
  if (dispatch) {
    dispatch(state.tr.wrap(range, wrapping))
  }
  
  return true
}
```

### Combining Commands

```javascript
import {chainCommands, deleteSelection, joinForward} from 'prosemirror-commands'

// Chain commands - try each in order
const deleteForward = chainCommands(
  deleteSelection,
  joinForward,
  (state, dispatch) => {
    // Custom fallback
    if (dispatch) {
      const pos = state.selection.from
      if (pos < state.doc.content.size) {
        dispatch(state.tr.delete(pos, pos + 1))
      }
    }
    return true
  }
)

// Conditional command
function smartDelete(state, dispatch, view) {
  if (state.selection.empty) {
    return joinForward(state, dispatch, view)
  } else {
    return deleteSelection(state, dispatch)
  }
}
```

### Working with Marks

```javascript
import {toggleMark} from 'prosemirror-commands'

// Simple mark toggle
const toggleBold = toggleMark(schema.marks.strong)
const toggleItalic = toggleMark(schema.marks.em)

// Mark with attributes
const toggleLink = toggleMark(schema.marks.link, {
  href: window.prompt('Enter URL:') || ''
})

// Complex mark command
function toggleCode(state, dispatch) {
  const {empty, $cursor, ranges} = state.selection
  
  if (empty && !$cursor) return false
  
  if (dispatch) {
    if ($cursor) {
      // Toggle stored marks
      const marks = state.storedMarks || $cursor.marks()
      const has = marks.some(mark => mark.type === schema.marks.code)
      if (has) {
        dispatch(state.tr.removeStoredMark(schema.marks.code))
      } else {
        dispatch(state.tr.addStoredMark(schema.marks.code.create()))
      }
    } else {
      // Toggle on ranges
      const tr = state.tr
      for (const range of ranges) {
        const has = state.doc.rangeHasMark(range.$from.pos, range.$to.pos, schema.marks.code)
        tr[has ? 'removeMark' : 'addMark'](range.$from.pos, range.$to.pos, schema.marks.code.create())
      }
      dispatch(tr)
    }
  }
  
  return true
}
```

### List Manipulation

```javascript
import {wrapIn, lift, autoJoin} from 'prosemirror-commands'

// Wrap in list
const wrapInBulletList = wrapIn(schema.nodes.bullet_list)
const wrapInOrderedList = wrapIn(schema.nodes.ordered_list)

// Toggle list
function toggleList(listType) {
  return (state, dispatch) => {
    const {$from, $to} = state.selection
    const range = $from.blockRange($to)
    
    if (!range) return false
    
    const wrapping = range && findWrapping(range, listType)
    
    if (range.depth >= 2 && range.$from.node(range.depth - 1).type === listType) {
      // Already in list, lift out
      return lift(state, dispatch)
    } else if (wrapping) {
      // Can wrap in list
      return autoJoin(wrapIn(listType), (before, after) => {
        return before.type === after.type && before.type === listType
      })(state, dispatch)
    }
    
    return false
  }
}
```

### Building Keymaps

```javascript
import {baseKeymap, setBlockType, wrapIn} from 'prosemirror-commands'
import {keymap} from 'prosemirror-keymap'

// Extend base keymap
const myKeymap = keymap({
  ...baseKeymap,
  "Mod-b": toggleMark(schema.marks.strong),
  "Mod-i": toggleMark(schema.marks.em),
  "Mod-`": toggleMark(schema.marks.code),
  "Shift-Ctrl-1": setBlockType(schema.nodes.heading, {level: 1}),
  "Shift-Ctrl-2": setBlockType(schema.nodes.heading, {level: 2}),
  "Shift-Ctrl-3": setBlockType(schema.nodes.heading, {level: 3}),
  "Shift-Ctrl-0": setBlockType(schema.nodes.paragraph),
  "Mod->": wrapIn(schema.nodes.blockquote),
  "Mod-Enter": chainCommands(exitCode, insertHardBreak),
  "Shift-Enter": insertHardBreak,
  "Ctrl-Alt-Backspace": joinUp,
  "Ctrl-Alt-Delete": joinDown
})
```

This module provides essential editing commands that form the foundation of most ProseMirror-based editors.