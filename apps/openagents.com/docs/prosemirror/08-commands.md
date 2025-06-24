# Commands in ProseMirror

In ProseMirror, a _command_ is a function that implements an editing action, which can be triggered by key combinations or menu interactions.

## Command Interface

Commands have a specific interface:
- Take an editor state as the first argument
- Take an optional dispatch function as the second argument
- Return a boolean indicating whether the command can be executed

Here's a simple example of a command:

```javascript
function deleteSelection(state, dispatch) {
  if (state.selection.empty) return false
  if (dispatch) dispatch(state.tr.deleteSelection())
  return true
}
```

Key characteristics:
- Returns `false` if the command is not applicable
- When `dispatch` is provided, it creates and dispatches a transaction
- Can be queried for applicability without executing by passing `null` as the dispatch argument

## Command Variations

Some commands might interact with the DOM or view:

```javascript
function blinkView(_state, dispatch, view) {
  if (dispatch) {
    view.dom.style.background = "yellow"
    setTimeout(() => view.dom.style.background = "", 1000)
  }
  return true
}
```

## Built-in Commands

The `prosemirror-commands` module provides several editing commands:

### Selection Commands
```javascript
import {deleteSelection, selectAll, selectParentNode} from "prosemirror-commands"

// Delete current selection
deleteSelection(state, dispatch)

// Select entire document
selectAll(state, dispatch)

// Select parent node
selectParentNode(state, dispatch)
```

### Text Manipulation
```javascript
import {joinBackward, joinForward, splitBlock} from "prosemirror-commands"

// Join with previous block (like backspace at start)
joinBackward(state, dispatch)

// Join with next block (like delete at end)
joinForward(state, dispatch)

// Split current block
splitBlock(state, dispatch)
```

### Mark Commands
```javascript
import {toggleMark} from "prosemirror-commands"

// Create command to toggle bold
const toggleBold = toggleMark(schema.marks.strong)

// Create command to toggle link with attributes
const toggleLink = toggleMark(schema.marks.link, {href: "https://example.com"})
```

### List Commands
```javascript
import {wrapInList, splitListItem, liftListItem, sinkListItem} from "prosemirror-schema-list"

// Wrap selection in a list
const wrapInBulletList = wrapInList(schema.nodes.bullet_list)

// Split list item (Enter key behavior)
const splitListItemCommand = splitListItem(schema.nodes.list_item)

// Lift item out of list
const liftListItemCommand = liftListItem(schema.nodes.list_item)

// Sink item deeper into list
const sinkListItemCommand = sinkListItem(schema.nodes.list_item)
```

## Command Chaining

The `chainCommands` utility allows combining multiple commands:

```javascript
import {chainCommands} from "prosemirror-commands"

// Example: Backspace behavior chain
const backspace = chainCommands(
  deleteSelection,
  joinBackward,
  selectNodeBackward
)

// Example: Enter key behavior
const enter = chainCommands(
  newlineInCode,
  createParagraphNear,
  liftEmptyBlock,
  splitBlock
)
```

## Creating Custom Commands

### Basic Structure
```javascript
function myCommand(state, dispatch, view) {
  // Check if command is applicable
  if (!canApplyCommand(state)) {
    return false
  }
  
  // Execute if dispatch provided
  if (dispatch) {
    let tr = state.tr
    // Modify transaction
    tr.insertText("Hello")
    dispatch(tr)
  }
  
  return true
}
```

### Command with Options
```javascript
function insertText(text) {
  return function(state, dispatch) {
    if (dispatch) {
      dispatch(state.tr.insertText(text))
    }
    return true
  }
}

// Usage
const insertHello = insertText("Hello")
```

### Complex Command Example
```javascript
function insertHorizontalRule(state, dispatch) {
  let {$from, $to} = state.selection
  
  // Check if we can insert here
  if (!$from.parent.type.spec.code && $from.parent.canReplaceWith($from.index(), $to.index(), schema.nodes.horizontal_rule)) {
    if (dispatch) {
      let tr = state.tr.replaceSelectionWith(schema.nodes.horizontal_rule.create())
      dispatch(tr)
    }
    return true
  }
  
  return false
}
```

## Using Commands with Keymaps

```javascript
import {keymap} from "prosemirror-keymap"

const myKeymap = keymap({
  "Mod-b": toggleMark(schema.marks.strong),
  "Mod-i": toggleMark(schema.marks.em),
  "Mod-Enter": insertHorizontalRule,
  "Alt-ArrowUp": joinUp,
  "Alt-ArrowDown": joinDown,
  "Backspace": chainCommands(deleteSelection, joinBackward),
  "Delete": chainCommands(deleteSelection, joinForward)
})

// Add to plugin list
let state = EditorState.create({
  schema,
  plugins: [myKeymap]
})
```

## Command Helpers

### Testing Commands
```javascript
// Check if command can be executed
if (toggleBold(view.state, null)) {
  console.log("Bold can be toggled")
}

// Execute command
toggleBold(view.state, view.dispatch)
```

### Menu Integration
```javascript
class MenuItem {
  constructor(spec) {
    this.spec = spec
  }
  
  isActive(state) {
    return this.spec.active?.(state) || false
  }
  
  isEnabled(state) {
    return this.spec.command(state, null)
  }
  
  execute(state, dispatch, view) {
    return this.spec.command(state, dispatch, view)
  }
}

const boldItem = new MenuItem({
  command: toggleMark(schema.marks.strong),
  active: state => isMarkActive(state, schema.marks.strong)
})
```

## Best Practices

1. **Always check applicability** - Return false when command cannot be applied
2. **Make commands pure** - Don't modify state directly
3. **Support querying** - Work correctly when dispatch is null
4. **Chain fallbacks** - Use chainCommands for flexible behavior
5. **Provide feedback** - Return true/false to indicate success

## Advanced Patterns

### Conditional Commands
```javascript
function conditionalCommand(condition, command) {
  return (state, dispatch, view) => {
    if (!condition(state)) return false
    return command(state, dispatch, view)
  }
}
```

### Commands with Side Effects
```javascript
function saveDocument(state, dispatch, view) {
  if (dispatch) {
    // Perform save
    fetch('/save', {
      method: 'POST',
      body: JSON.stringify(state.doc.toJSON())
    })
    
    // Optionally update state
    dispatch(state.tr.setMeta('saved', true))
  }
  return true
}
```