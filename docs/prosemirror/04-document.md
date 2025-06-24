# ProseMirror Documents

## Core Characteristics

ProseMirror defines documents as a custom data structure with several key properties:

- Documents are tree-shaped, similar to the DOM
- Nodes are immutable values, not stateful objects
- Each document has a single, canonical representation
- Adjacent text nodes with identical marks are always combined
- Empty text nodes are not allowed

## Document Structure

A document is fundamentally a node containing a fragment of child nodes. The structure looks like:

```javascript
{
  type: NodeType,
  content: Fragment,
  attrs: Object,
  marks: [Mark]
}
```

### Key Methods and Properties

- `isBlock` and `isInline`: Determine node type
- `inlineContent`: Checks if node expects inline content
- `isTextblock`: Identifies block nodes with inline content
- `isLeaf`: Indicates nodes without child content

## Creating Documents

Documents are created through the schema:

```javascript
import {schema} from "prosemirror-schema-basic"

let doc = schema.node("doc", null, [
  schema.node("paragraph", null, [schema.text("One.")]),
  schema.node("horizontal_rule"),
  schema.node("paragraph", null, [schema.text("Two!")])
])
```

## Indexing and Positions

ProseMirror supports two indexing approaches:
1. Tree-based: Accessing nodes directly
2. Flat sequence: Representing positions as integer tokens

Positions follow specific rules:
- Document start is position 0
- Entering/leaving nodes counts as tokens
- Each character in text nodes is a token
- Leaf nodes count as single tokens

### Position Example

For a document like `<p>ab</p>`, positions are:
- 0: Start of document
- 1: Start of paragraph
- 2: Between 'a' and 'b'
- 3: After 'b'
- 4: End of paragraph
- 5: End of document

## Resolving Positions

Use `doc.resolve(pos)` to get detailed position information:

```javascript
let $pos = doc.resolve(5)
$pos.depth      // Nesting depth
$pos.parent     // Parent node
$pos.node()     // Ancestor at given depth
$pos.before()   // Position before ancestor
$pos.after()    // Position after ancestor
```

## Updating Documents

Since documents are immutable, updates create entirely new document values. Recommended update methods include:

- `Node.replace()`: Replace document ranges
- `Node.copy()`: Create shallow updates
- Fragment methods like `replaceChild()` and `append()`

### Replace Example

```javascript
// Replace content between two positions
let newDoc = doc.replace(from, to, slice)
```

## Slices

Slices represent document fragments, potentially with "open" nodes at start or end. They're useful for operations like copy-paste.

```javascript
let slice1 = doc.slice(0, 3)  // First paragraph
let slice2 = doc.slice(5)     // From position 5 to end
```

Properties of slices:
- `openStart`: Depth of open nodes at start
- `openEnd`: Depth of open nodes at end
- `content`: The fragment content
- `size`: Total size of the slice

## Common Operations

### Iterating Over Content

```javascript
doc.descendants((node, pos, parent) => {
  // Process each node
  console.log(node.type.name, pos)
})
```

### Finding Nodes

```javascript
// Find first heading
let heading = doc.firstChild(child => child.type.name == "heading")

// Get node at position
let nodeAt = doc.nodeAt(pos)
```

### Text Content

```javascript
// Get all text content
let text = doc.textContent

// Get text between positions
let textBetween = doc.textBetween(from, to, separator)
```

## Performance Considerations

- Document updates share unchanged nodes between versions
- Avoid recreating large portions unnecessarily
- Use methods like `replace()` rather than manual reconstruction
- Position resolution is optimized for repeated access