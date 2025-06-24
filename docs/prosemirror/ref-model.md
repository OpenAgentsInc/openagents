# ProseMirror Model Module Reference

## Overview

The ProseMirror model module defines the core data structures and content model for ProseMirror documents. It provides a flexible, persistent tree-based representation of document structure.

## Installation

```bash
npm install prosemirror-model
```

## Key Classes

### Node

Represents a node in the document tree. Key characteristics:

- Persistent data structure (immutable)
- Represents document content at each level
- Contains:
  - `type`: NodeType defining the node's structure
  - `attrs`: Node attributes
  - `marks`: Formatting marks
  - `content`: Child nodes (Fragment)
  - `text`: Text content (for text nodes)
  - `nodeSize`: Size of the node in the document

Key methods:
- `child(index)`: Get child node at given index
- `slice(from, to)`: Extract a slice of the content
- `replace(from, to, slice)`: Create new node with replacement
- `nodeAt(pos)`: Find node at given position
- `childAfter(pos)`: Get child after position
- `forEach(f)`: Iterate over child nodes
- `descendants(f)`: Iterate over all descendant nodes
- `textContent`: Get all text content
- `eq(other)`: Compare nodes for equality
- `cut(from, to)`: Cut out a sub-document

### Mark

Represents formatting applied to nodes, such as emphasis or links.

Properties:
- `type`: MarkType
- `attrs`: Mark attributes

Methods:
- `addToSet(set)`: Add mark to a mark set
- `removeFromSet(set)`: Remove mark from a mark set
- `isInSet(set)`: Check if mark is in set
- `eq(other)`: Compare marks

### Fragment

Represents a sequence of nodes, used to manage child node collections.

Properties:
- `size`: Total size of the fragment
- `childCount`: Number of children

Methods:
- `nodeAt(pos)`: Find node at position
- `child(index)`: Get child at index
- `forEach(f)`: Iterate over nodes
- `cut(from, to)`: Extract sub-fragment
- `append(other)`: Concatenate fragments
- `eq(other)`: Compare fragments

Static methods:
- `Fragment.from()`: Create fragment from various inputs
- `Fragment.empty`: Empty fragment singleton

### Schema

Defines the allowed structure and types of nodes and marks in a document.

Properties:
- `nodes`: Registry of node types
- `marks`: Registry of mark types
- `topNodeType`: The document's root node type

Methods:
- `node()`: Create a node
- `text()`: Create a text node
- `mark()`: Create a mark
- `nodeFromJSON()`: Deserialize node from JSON
- `markFromJSON()`: Deserialize mark from JSON

### Slice

Represents a piece of a document that can be inserted or copied.

Properties:
- `content`: Fragment containing the slice content
- `openStart`: Depth of open nodes at start
- `openEnd`: Depth of open nodes at end

Methods:
- `eq(other)`: Compare slices
- `toJSON()`: Serialize to JSON

Static methods:
- `Slice.empty`: Empty slice singleton
- `Slice.fromJSON()`: Deserialize from JSON

### NodeType

Defines a type of node.

Properties:
- `name`: Type name
- `schema`: Parent schema
- `spec`: Type specification
- `contentMatch`: Content expression match
- `inlineContent`: Whether content is inline
- `isBlock`: Whether this is a block node
- `isText`: Whether this is a text node

Methods:
- `create()`: Create a node of this type
- `createChecked()`: Create with validation
- `createAndFill()`: Create and auto-fill required content
- `validContent()`: Check if fragment is valid content
- `allowsMarkType()`: Check if mark type is allowed
- `allowsMarks()`: Check which marks are allowed
- `compatibleContent()`: Check content compatibility

### MarkType

Defines a type of mark.

Properties:
- `name`: Type name
- `schema`: Parent schema
- `spec`: Type specification
- `inclusive`: Whether mark expands when typing

Methods:
- `create()`: Create a mark of this type
- `removeFromSet()`: Remove from mark set
- `isInSet()`: Check if in mark set
- `excludes()`: Check if excludes another mark

## Key Concepts

- Documents are tree-structured
- Nodes are immutable
- Content is represented as nested fragments
- Strict typing through schemas
- Supports complex document structures
- Positions are represented as integers counting from document start

## Usage Examples

### Creating a Schema

```javascript
import {Schema} from 'prosemirror-model'

const mySchema = new Schema({
  nodes: {
    doc: {
      content: "block+"
    },
    paragraph: {
      content: "inline*",
      group: "block",
      parseDOM: [{tag: "p"}],
      toDOM() { return ["p", 0] }
    },
    blockquote: {
      content: "block+",
      group: "block",
      parseDOM: [{tag: "blockquote"}],
      toDOM() { return ["blockquote", 0] }
    },
    text: {
      group: "inline"
    }
  },
  marks: {
    em: {
      parseDOM: [{tag: "i"}, {tag: "em"}],
      toDOM() { return ["em", 0] }
    },
    strong: {
      parseDOM: [{tag: "strong"}, {tag: "b"}],
      toDOM() { return ["strong", 0] }
    }
  }
})
```

### Creating Documents

```javascript
// Create a document
const doc = mySchema.node("doc", null, [
  mySchema.node("paragraph", null, [
    mySchema.text("Hello "),
    mySchema.text("world", [mySchema.mark("strong")])
  ])
])

// Create from JSON
const jsonDoc = {
  type: "doc",
  content: [{
    type: "paragraph",
    content: [{
      type: "text",
      text: "Hello world"
    }]
  }]
}
const docFromJSON = mySchema.nodeFromJSON(jsonDoc)
```

### Working with Fragments

```javascript
// Create fragments
const fragment = Fragment.from([
  mySchema.node("paragraph", null, [mySchema.text("First")]),
  mySchema.node("paragraph", null, [mySchema.text("Second")])
])

// Iterate over nodes
fragment.forEach((node, offset, index) => {
  console.log(`Node ${index} at offset ${offset}:`, node.type.name)
})
```

### Working with Marks

```javascript
// Create marks
const em = mySchema.mark("em")
const strong = mySchema.mark("strong")

// Apply marks to text
const markedText = mySchema.text("Bold and italic", [em, strong])

// Work with mark sets
const marks = markedText.marks
const withoutEm = em.removeFromSet(marks)
```

This module provides the foundational data model for creating and manipulating rich text documents in ProseMirror.