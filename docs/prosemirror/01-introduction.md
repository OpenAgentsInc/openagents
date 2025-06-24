# ProseMirror Guide

## Introduction

ProseMirror is a set of tools for building rich text editors with a focus on giving developers full control over the document and editing process. Key principles include:

- Full control over document structure
- Modular and customizable design
- Document as a custom data structure with explicit constraints

### Essential Modules

Four core modules are required for editing:

1. `prosemirror-model`: Defines document model
2. `prosemirror-state`: Manages editor state and transactions
3. `prosemirror-view`: Implements user interface component
4. `prosemirror-transform`: Handles document modifications

### First Editor Example

```javascript
import {schema} from "prosemirror-schema-basic"
import {EditorState} from "prosemirror-state"
import {EditorView} from "prosemirror-view"

let state = EditorState.create({schema})
let view = new EditorView(document.body, {state})
```

### Transactions

ProseMirror uses a transaction-based approach to state updates:

```javascript
let view = new EditorView(document.body, {
  state,
  dispatchTransaction(transaction) {
    console.log("Document size changed")
    let newState = view.state.apply(transaction)
    view.updateState(newState)
  }
})
```

### Plugins

Plugins extend editor behavior:

```javascript
import {undo, redo, history} from "prosemirror-history"
import {keymap} from "prosemirror-keymap"

let state = EditorState.create({
  schema,
  plugins: [
    history(),
    keymap({"Mod-z": undo, "Mod-y": redo})
  ]
})
```

### Content Initialization

You can initialize a state with existing content:

```javascript
import {DOMParser} from "prosemirror-model"

let content = document.getElementById("content")
let state = EditorState.create({
  doc: DOMParser.fromSchema(schema).parse(content)
})
```

## Key Concepts

- **Schema**: Defines the allowed document structure
- **State**: Represents the current editor state
- **View**: Displays the state and handles user input
- **Transactions**: Describe state changes
- **Commands**: Functions that create transactions
- **Plugins**: Extend and customize editor behavior