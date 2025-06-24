# ProseMirror Dino Example

## Overview

This example demonstrates how to extend ProseMirror's basic schema to include custom inline nodes (dinosaurs) that can be inserted, selected, copied, and manipulated within a document.

## HTML Structure

```html
<div id="editor"></div>
<div id="content">
  <!-- Initial document content with dinosaurs -->
</div>
```

## CSS Styles

```css
img.dinosaur {
  height: 40px;
  vertical-align: bottom;
  border: 1px solid #0ae;
  border-radius: 4px;
  background: #ddf6ff;
}
```

## JavaScript Code

```javascript
import {Schema} from "prosemirror-model"
import {schema} from "prosemirror-schema-basic"
import {MenuItem} from "prosemirror-menu"
import {buildMenuItems} from "prosemirror-example-setup"
import {EditorState} from "prosemirror-state"
import {EditorView} from "prosemirror-view"
import {exampleSetup} from "prosemirror-example-setup"

// Supported dinosaur types
const dinos = ["brontosaurus", "stegosaurus", "triceratops", 
               "tyrannosaurus", "pterodactyl"]

// Custom node specification for dinosaur nodes
const dinoNodeSpec = {
  attrs: { type: { default: "brontosaurus" } },
  inline: true,
  group: "inline",
  draggable: true,

  toDOM: node => ["img", {
    "dino-type": node.attrs.type,
    src: `/img/dino/${node.attrs.type}.png`,
    title: node.attrs.type,
    class: "dinosaur"
  }],

  parseDOM: [{
    tag: "img[dino-type]",
    getAttrs: dom => {
      let type = dom.getAttribute("dino-type")
      return dinos.indexOf(type) > -1 ? { type } : false
    }
  }]
}

// Create schema with custom dinosaur node
const dinoSchema = new Schema({
  nodes: schema.spec.nodes.addBefore("image", "dino", dinoNodeSpec),
  marks: schema.spec.marks
})

// Get the dino node type from our schema
const dinoType = dinoSchema.nodes.dino

// Command to insert a specific dinosaur type
function insertDino(type) {
  return function(state, dispatch) {
    let {$from} = state.selection, index = $from.index()
    if (!$from.parent.canReplaceWith(index, index, dinoType))
      return false
    if (dispatch)
      dispatch(state.tr.replaceSelectionWith(dinoType.create({type})))
    return true
  }
}

// Build menu items
let menu = buildMenuItems(dinoSchema)

// Add a dino-inserting item for each type of dino
dinos.forEach(name => menu.insertMenu.content.push(new MenuItem({
  title: "Insert " + name,
  label: name.charAt(0).toUpperCase() + name.slice(1),
  enable(state) { return insertDino(name)(state) },
  run: insertDino(name)
})))

// Initialize editor
window.view = new EditorView(document.querySelector("#editor"), {
  state: EditorState.create({
    doc: DOMParser.fromSchema(dinoSchema).parse(document.querySelector("#content")),
    plugins: exampleSetup({schema: dinoSchema, menuContent: menu.fullMenu})
  })
})
```

## Key Concepts

### Custom Node Definition
The dinosaur node is defined with:
- **attrs**: Stores the dinosaur type
- **inline**: Makes it an inline node that flows with text
- **draggable**: Allows drag-and-drop functionality
- **toDOM**: Renders as an image with custom attributes
- **parseDOM**: Parses image elements with `dino-type` attribute

### Schema Extension
The example extends the basic schema by:
1. Taking the standard `prosemirror-schema-basic` schema
2. Adding the custom `dino` node before the `image` node
3. Preserving all existing marks

### Menu Integration
For each dinosaur type, the example:
1. Creates a menu item with a capitalized label
2. Uses the `insertDino` command for both enabling and execution
3. Adds items to the insert menu section

### Command Pattern
The `insertDino` command follows ProseMirror conventions:
- Checks if insertion is valid at the current position
- Returns `false` if insertion isn't possible
- Dispatches a transaction to replace selection with the dinosaur node
- Returns `true` on success

## Usage
- Dinosaurs can be inserted from the menu
- They behave like inline content (can be selected, copied, deleted)
- They're draggable within the document
- The schema ensures they can only be placed where inline content is allowed