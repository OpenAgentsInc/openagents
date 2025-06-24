# ProseMirror Basic Editor Setup Example

## Overview
This example demonstrates setting up a basic ProseMirror editor with extended functionality, including:
- Using core ProseMirror libraries
- Creating a custom schema with list support
- Configuring an editor view with example plugins

## HTML Structure
```html
<div id="content">
  <p>Hello ProseMirror</p>
  <p>This is editable text. You can focus it and start typing.</p>
</div>
<div id="editor"></div>
```

## JavaScript Code
```javascript
import {EditorState} from "prosemirror-state"
import {EditorView} from "prosemirror-view"
import {Schema, DOMParser} from "prosemirror-model"
import {schema} from "prosemirror-schema-basic"
import {addListNodes} from "prosemirror-schema-list"
import {exampleSetup} from "prosemirror-example-setup"

// Mix the nodes from prosemirror-schema-list into the basic schema
const mySchema = new Schema({
  nodes: addListNodes(schema.spec.nodes, "paragraph block*", "block"),
  marks: schema.spec.marks
})

window.view = new EditorView(document.querySelector("#editor"), {
  state: EditorState.create({
    doc: DOMParser.fromSchema(mySchema).parse(document.querySelector("#content")),
    plugins: exampleSetup({schema: mySchema})
  })
})
```

## Key Features
The example setup includes:
- Input rules for smart quotes and Markdown-like behavior
- Keymaps with base bindings
- Drop cursor and gap cursor plugins
- Undo history
- Menu bar with common editing tasks

Note: This example is primarily for demonstration and learning purposes.