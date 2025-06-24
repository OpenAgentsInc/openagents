# Schemas in ProseMirror

## Overview

In ProseMirror, each document has an associated schema that defines the structure and allowed content of the document. The schema specifies:

- What types of nodes can exist
- How nodes can be nested
- What attributes nodes can have
- What marks can be applied to content

## Node Types

Every node in a document has a type that represents its semantic meaning and properties. When defining a schema, you enumerate node types using a spec object:

```javascript
const trivialSchema = new Schema({
  nodes: {
    doc: {content: "paragraph+"},
    paragraph: {content: "text*"},
    text: {inline: true},
  }
})
```

Key points about node types:
- Every schema must define a top-level node type (default is "doc")
- Must include a "text" type for text content
- Inline nodes must declare `inline: true`

## Content Expressions

Content expressions control valid child node sequences. They use syntax like:
- `"paragraph"`: Exactly one paragraph
- `"paragraph+"`: One or more paragraphs
- `"paragraph*"`: Zero or more paragraphs
- `"caption?"`: Zero or one caption
- `{2}`: Exactly two nodes
- `{1,5}`: One to five nodes
- `{2,}`: Two or more nodes

Expressions can be combined:
- `"heading paragraph+"`: A heading followed by one or more paragraphs
- `"(paragraph | blockquote)+"`: Alternating paragraphs and blockquotes

### Node Groups

You can create node groups to simplify content expressions:

```javascript
const groupSchema = new Schema({
  nodes: {
    doc: {content: "block+"},
    paragraph: {group: "block", content: "text*"},
    blockquote: {group: "block", content: "block+"},
    text: {}
  }
})
```

Here, `"block+"` is equivalent to `"(paragraph | blockquote)+"`.

## Marks

Marks add styling or metadata to inline content. A schema declares allowed mark types:

```javascript
const markSchema = new Schema({
  nodes: {
    doc: {content: "block+"},
    paragraph: {group: "block", content: "text*", marks: "_"},
    heading: {group: "block", content: "text*", marks: ""},
    text: {inline: true}
  },
  marks: {
    strong: {},
    em: {},
    link: {
      attrs: {
        href: {},
        title: {default: null}
      },
      inclusive: false
    }
  }
})
```

Mark properties:
- `marks: "_"`: Allows all marks (default)
- `marks: ""`: Disallows all marks
- `marks: "em strong"`: Only allows specific marks
- `inclusive`: Whether mark extends when typing at its boundary
- `excludes`: Marks that can't coexist with this mark

## Attributes

Both nodes and marks can have attributes to store metadata:

```javascript
{
  image: {
    inline: true,
    attrs: {
      alt: {default: ""},
      src: {},
      title: {default: null}
    },
    group: "inline"
  }
}
```

Attribute options:
- `default`: Value used when creating without explicit attribute
- Required attributes have no default
- Can use functions to compute defaults

## Serialization and Parsing

### DOM Serialization

Schemas define how nodes render to HTML:

```javascript
const schema = new Schema({
  nodes: {
    doc: {content: "block+"},
    paragraph: {
      group: "block",
      content: "inline*",
      toDOM() { return ["p", 0] }
    },
    text: {inline: true}
  }
})
```

The `toDOM` method returns:
- Element name
- Optional attributes object
- Content hole (0) or array of children

### DOM Parsing

Define parsing rules to convert HTML to document nodes:

```javascript
paragraph: {
  content: "inline*",
  group: "block",
  parseDOM: [{tag: "p"}]
}
```

Parse rules can include:
- `tag`: CSS selector for matching elements
- `attrs`: Extract attributes from DOM
- `getAttrs`: Function to extract/validate attributes
- `priority`: Rule precedence

## Schema Utils

Helper functions from `prosemirror-schema-basic`:
- Pre-defined node types
- Common mark types
- Standard schema configurations

Example using schema-basic:

```javascript
import {schema} from "prosemirror-schema-basic"

// Use the pre-built schema
let myDoc = schema.node("doc", null, [
  schema.node("paragraph", null, [
    schema.text("Hello world!")
  ])
])
```

## Extending Schemas

You can build new schemas based on existing ones:

```javascript
import {Schema} from "prosemirror-model"
import {schema} from "prosemirror-schema-basic"

const mySchema = new Schema({
  nodes: schema.spec.nodes.append({
    customBlock: {
      group: "block",
      content: "text*",
      toDOM() { return ["div", {"class": "custom"}, 0] }
    }
  }),
  marks: schema.spec.marks
})
```