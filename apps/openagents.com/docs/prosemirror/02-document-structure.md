# Document Structure in ProseMirror

## Overview

ProseMirror defines its own unique data structure to represent content documents. Unlike the browser DOM, ProseMirror models documents with a distinctive approach to representing content, especially inline content.

## Key Characteristics

### Structure and Representation

A ProseMirror document is a node that contains a fragment of zero or more child nodes. The key differences from the DOM include:

1. Inline content is represented as a flat sequence with markup attached as metadata
2. Each document has a single valid representation
3. Adjacent text nodes with identical marks are always combined
4. Empty text nodes are not allowed

### Example Comparison

HTML representation:
```html
<p>This is <strong>strong text with <em>emphasis</em></strong></p>
```

ProseMirror representation:
- Paragraph node
- Text: "This is"
- Text: "strong text with"
- Text: "emphasis"
- Marks applied separately to text segments

## Node Properties

Nodes come with properties that describe their role in the document:

- `isBlock` and `isInline`: Node type classification
- `inlineContent`: Indicates nodes expecting inline content
- `isTextblock`: Block nodes with inline content
- `isLeaf`: Nodes without content

## Identity and Persistence

ProseMirror nodes are treated as immutable values:
- Nodes can appear in multiple data structures
- Creating a change produces a new document value
- Unchanged sub-nodes are shared between document versions

## Data Structure

A typical node object includes:
- Node type
- Content fragment
- Attributes
- Active marks

## Code Example

```javascript
import {schema} from "prosemirror-schema-basic"

let doc = schema.node("doc", null, [
  schema.node("paragraph", null, [schema.text("One.")]),
  schema.node("horizontal_rule"),
  schema.node("paragraph", null, [schema.text("Two!")])
])
```

## Indexing

ProseMirror supports two indexing approaches:
1. Tree-based: Accessing nodes directly
2. Flat sequence: Representing positions as integer tokens

## Slices

Slices represent document fragments that may have open boundaries. They maintain depth information to preserve context when cutting and pasting content.