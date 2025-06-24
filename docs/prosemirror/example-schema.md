# ProseMirror Schema from Scratch Example

## Overview

This example demonstrates creating custom document schemas in ProseMirror, showing how to define:
- Simple text-only schemas
- Block nodes with custom structures  
- Inline nodes and marks
- Custom editing commands

## CSS Styles

```css
note, notegroup {
  display: block;
  border: 1px solid silver;
  border-radius: 3px;
  padding: 3px 6px;
  margin: 5px 0;
}

notegroup { 
  border-color: #66f 
}

p.boring { 
  background: #eee; 
  color: #444; 
}

shouting { 
  display: inline; 
  text-transform: uppercase; 
  font-weight: bold; 
}

star { 
  display: inline; 
  font-size: 190%; 
  line-height: 1; 
  vertical-align: -10%; 
  color: #a8f; 
  -webkit-text-stroke: 1px #75b; 
}
```

## JavaScript Code

### Schema 1: Text-only Schema

The simplest possible schema - a document containing only text:

```javascript
import {Schema} from "prosemirror-model"

const textSchema = new Schema({
  nodes: {
    text: {},
    doc: {content: "text*"}
  }
})
```

### Schema 2: Note Schema

A schema with custom block nodes for notes and note groups:

```javascript
const noteSchema = new Schema({
  nodes: {
    text: {},
    note: {
      content: "text*",
      toDOM() { return ["note", 0] },
      parseDOM: [{tag: "note"}]
    },
    notegroup: {
      content: "note+",
      toDOM() { return ["notegroup", 0] },
      parseDOM: [{tag: "notegroup"}]
    },
    doc: {
      content: "(note | notegroup)+"
    }
  }
})
```

### Schema 3: Star and Shouting Schema

A more complex schema with inline nodes, marks, and custom commands:

```javascript
const starSchema = new Schema({
  nodes: {
    text: {
      group: "inline",
    },
    star: {
      inline: true,
      group: "inline",
      toDOM() { return ["star", "🟊"] },
      parseDOM: [{tag: "star"}]
    },
    paragraph: {
      group: "block",
      content: "inline*",
      toDOM() { return ["p", 0] },
      parseDOM: [{tag: "p"}]
    },
    boring_paragraph: {
      group: "block",
      content: "text*",
      marks: "",
      toDOM() { return ["p", {class: "boring"}, 0] },
      parseDOM: [{tag: "p.boring", priority: 60}]
    },
    doc: {
      content: "block+"
    }
  },
  marks: {
    shouting: {
      toDOM() { return ["shouting", 0] },
      parseDOM: [{tag: "shouting"}]
    },
    link: {
      attrs: {href: {}},
      toDOM(node) { return ["a", {href: node.attrs.href}, 0] },
      parseDOM: [{tag: "a", getAttrs(dom) { return {href: dom.href} }}],
      inclusive: false
    }
  }
})
```

### Custom Commands and Keymap

```javascript
import {toggleMark} from "prosemirror-commands"
import {keymap} from "prosemirror-keymap"

let starKeymap = keymap({
  "Ctrl-b": toggleMark(starSchema.marks.shouting),
  "Ctrl-q": toggleLink,
  "Ctrl-Space": insertStar
})

function toggleLink(state, dispatch) {
  let {doc, selection} = state
  if (selection.empty) return false
  let attrs = null
  if (!doc.rangeHasMark(selection.from, selection.to, starSchema.marks.link)) {
    attrs = {href: prompt("Link to where?", "")}
    if (!attrs.href) return false
  }
  return toggleMark(starSchema.marks.link, attrs)(state, dispatch)
}

function insertStar(state, dispatch) {
  let type = starSchema.nodes.star
  let {$from} = state.selection
  if (!$from.parent.canReplaceWith($from.index(), $from.index(), type))
    return false
  dispatch(state.tr.replaceSelectionWith(type.create()))
  return true
}
```

### Editor Setup

```javascript
import {EditorState} from "prosemirror-state"
import {EditorView} from "prosemirror-view"
import {baseKeymap} from "prosemirror-commands"

// Create editor with star schema
window.view = new EditorView(document.querySelector("#editor"), {
  state: EditorState.create({
    schema: starSchema,
    plugins: [keymap(baseKeymap), starKeymap]
  })
})
```

## Key Concepts

### Schema Structure
- **nodes**: Define the document structure (paragraphs, text, custom nodes)
- **marks**: Define inline decorations (bold, links, etc.)
- **content expressions**: Control what nodes can contain
- **groups**: Categorize nodes (inline, block)

### Node Specifications
- **content**: What the node can contain (e.g., "text*", "inline*")
- **group**: Node category for content expressions
- **marks**: Which marks are allowed (empty string = no marks)
- **toDOM**: How to render the node to DOM
- **parseDOM**: How to parse DOM into the node

### Mark Specifications
- **attrs**: Attributes the mark can have
- **inclusive**: Whether mark extends to adjacent text
- **toDOM/parseDOM**: Rendering and parsing rules

### Custom Commands
Commands follow the pattern:
1. Check if action is valid in current state
2. If dispatch provided, perform the action
3. Return true if action was/would be valid

This example showcases the flexibility of ProseMirror's schema system, from simple text-only documents to complex structures with custom nodes and editing behaviors.