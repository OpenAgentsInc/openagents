# ProseMirror Transform Module Reference

## Overview

The prosemirror-transform module provides document transformation functionality, implementing the core editing operations in ProseMirror. It defines how documents can be modified through a series of steps that can be applied, inverted, and mapped.

## Installation

```bash
npm install prosemirror-transform
```

## Key Classes

### Transform

The main class for building up and applying document transformations. Extends from the basic Transform class to provide high-level transformation methods.

Key properties:
- `doc`: Current document state
- `steps`: Array of transformation steps
- `docs`: Array of documents after each step
- `mapping`: Mapping to track position changes

Key methods:
- `step(step)`: Apply a single transformation step
- `replace(from, to, slice)`: Replace a range with new content
- `replaceWith(from, to, node)`: Replace range with a single node
- `delete(from, to)`: Delete a range
- `insert(pos, node)`: Insert a node at position
- `replaceRange(from, to, slice)`: Replace range fitting slice structure
- `replaceRangeWith(from, to, node)`: Replace range with single node
- `deleteRange(from, to)`: Delete range between positions
- `lift(range, target)`: Lift nodes out of parent
- `join(pos, depth)`: Join nodes around position
- `wrap(range, wrappers)`: Wrap range in node types
- `setBlockType(from, to, type, attrs)`: Change block type
- `setNodeMarkup(pos, type, attrs, marks)`: Change node's type/attrs
- `addMark(from, to, mark)`: Add mark to range
- `removeMark(from, to, mark)`: Remove mark from range
- `clearIncompatible(pos, parentType)`: Clear incompatible content

### Step

Abstract base class for transformation steps. Each step represents an atomic change to a document.

Key methods:
- `apply(doc)`: Apply step to document
- `invert(doc)`: Create inverse step
- `map(mapping)`: Map step through changes
- `merge(other)`: Try to merge with another step
- `toJSON()`: Serialize to JSON
- `fromJSON()`: Deserialize from JSON

### ReplaceStep

Replaces a range of the document with a slice of content.

Properties:
- `from`: Start position
- `to`: End position
- `slice`: Content to insert
- `structure`: Whether this is a structural replacement

### ReplaceAroundStep

Replaces around a range, useful for operations like wrapping.

Properties:
- `from`: Start position
- `to`: End position
- `gapFrom`: Gap start
- `gapTo`: Gap end
- `slice`: Content to insert
- `insert`: Size of inserted content
- `structure`: Whether structural

### AddMarkStep

Adds a mark to a given range.

Properties:
- `from`: Start position
- `to`: End position
- `mark`: Mark to add

### RemoveMarkStep

Removes a mark from a given range.

Properties:
- `from`: Start position
- `to`: End position
- `mark`: Mark to remove

### AddNodeMarkStep

Adds a mark to a specific node.

Properties:
- `pos`: Node position
- `mark`: Mark to add

### RemoveNodeMarkStep

Removes a mark from a specific node.

Properties:
- `pos`: Node position
- `mark`: Mark to remove

### AttrStep

Changes attributes on a node.

Properties:
- `pos`: Node position
- `attr`: Attribute name
- `value`: New value

### DocAttrStep

Changes attributes on the document node.

Properties:
- `attr`: Attribute name
- `value`: New value

### StepMap

Maps positions through a single step's changes.

Key methods:
- `mapResult(pos, assoc)`: Map a position with success/failure info
- `map(pos, assoc)`: Map a position

### Mapping

Tracks position mappings through multiple steps.

Properties:
- `maps`: Array of step maps
- `from`: Starting point in maps array
- `to`: Ending point in maps array

Key methods:
- `mapResult(pos, assoc)`: Map position with result
- `map(pos, assoc)`: Map position
- `slice(from, to)`: Get sub-mapping
- `copy()`: Create copy
- `appendMap(map)`: Add new map
- `appendMapping(mapping)`: Combine mappings
- `appendMappingInverted(mapping)`: Append inverted

### MapResult

Result of mapping a position.

Properties:
- `pos`: Mapped position
- `delInfo`: Information about deletions (if position was deleted)

## Transform Builder Functions

### canSplit(doc, pos, depth, typesAfter)

Test whether the blocks before and after a position can be split.

### canJoin(doc, pos)

Test whether the blocks around a position can be joined.

### joinPoint(doc, pos, dir)

Find a position where nodes can be joined.

### insertPoint(doc, pos, nodeType)

Find position where a node can be inserted.

### dropPoint(doc, pos, slice)

Find position where a slice can be dropped.

### liftTarget(range)

Find what node a range can be lifted to.

### findWrapping(range, nodeType)

Find wrapping that can be applied to a range.

### replaceStep(doc, from, to, slice)

Create a replace step.

## Usage Examples

### Basic Transformations

```javascript
import {Transform} from 'prosemirror-transform'

// Create a transform
const tr = new Transform(doc)

// Delete text
tr.delete(5, 10)

// Insert a node
const paragraph = schema.nodes.paragraph.create(null, schema.text("New text"))
tr.insert(5, paragraph)

// Add a mark
const strong = schema.marks.strong.create()
tr.addMark(0, 10, strong)

// Apply the transformation
const newDoc = tr.doc
```

### Working with Steps

```javascript
// Create a custom step
const step = new ReplaceStep(5, 10, Slice.empty)

// Apply step
const result = step.apply(doc)
if (result.doc) {
  // Step succeeded
  const newDoc = result.doc
}

// Invert a step
const inverted = step.invert(doc)

// Map positions through step
const mapped = step.getMap().map(15)
```

### Complex Operations

```javascript
// Wrap content in a blockquote
const blockquoteType = schema.nodes.blockquote
tr.wrap(range, [{type: blockquoteType}])

// Change paragraph to heading
const headingType = schema.nodes.heading
tr.setBlockType(from, to, headingType, {level: 2})

// Join adjacent blocks
if (canJoin(doc, pos)) {
  tr.join(pos)
}

// Lift content out of parent
const target = liftTarget(range)
if (target != null) {
  tr.lift(range, target)
}
```

### Position Mapping

```javascript
// Track positions through transformations
const mapping = new Mapping()

// Add steps to mapping
steps.forEach(step => {
  mapping.appendMap(step.getMap())
})

// Map positions
const oldPos = 10
const newPos = mapping.map(oldPos)

// Map with deletion info
const result = mapping.mapResult(oldPos)
if (result.delInfo) {
  console.log("Position was deleted")
}
```

This module is essential for implementing any editing operations in ProseMirror, providing the infrastructure for document mutations, undo/redo, and collaborative editing.