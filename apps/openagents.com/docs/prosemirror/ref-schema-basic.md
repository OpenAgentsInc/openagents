# ProseMirror Schema Basic Module Reference

## Overview

The prosemirror-schema-basic module provides a pre-built schema with basic document elements like paragraphs, headings, lists, and common text formatting. It serves as a foundation for building rich text editors or as a reference for creating custom schemas.

## Installation

```bash
npm install prosemirror-schema-basic
```

## Exported Schema

### schema

The complete basic schema ready to use.

```javascript
import {schema} from 'prosemirror-schema-basic'

const state = EditorState.create({
  schema,
  // ... other options
})
```

## Node Types

### nodes

Object containing all node type specs.

```javascript
import {nodes} from 'prosemirror-schema-basic'
```

#### doc
The top-level document node.
- **Content**: `block+`
- **Marks**: None allowed

#### paragraph
A paragraph text block.
- **Content**: `inline*`
- **Group**: `block`
- **Parse**: `<p>` tags
- **Serialize**: `["p", 0]`

#### blockquote
A block quotation.
- **Content**: `block+`
- **Group**: `block`
- **Defining**: true
- **Parse**: `<blockquote>` tags
- **Serialize**: `["blockquote", 0]`

#### horizontal_rule
A horizontal rule.
- **Group**: `block`
- **Parse**: `<hr>` tags
- **Serialize**: `["hr"]`

#### heading
A heading with levels 1-6.
- **Content**: `inline*`
- **Group**: `block`
- **Defining**: true
- **Attributes**:
  - `level`: Number (default: 1)
- **Parse**: `<h1>` through `<h6>` tags
- **Serialize**: `["h" + attrs.level, 0]`

#### code_block
A code block for preformatted text.
- **Content**: `text*`
- **Marks**: None allowed
- **Group**: `block`
- **Code**: true
- **Defining**: true
- **Parse**: `<pre><code>` tags
- **Serialize**: `["pre", ["code", 0]]`

#### text
Inline text node.
- **Group**: `inline`

#### image
An inline image.
- **Inline**: true
- **Group**: `inline`
- **Draggable**: true
- **Attributes**:
  - `src`: String (required)
  - `alt`: String (optional)
  - `title`: String (optional)
- **Parse**: `<img[src]>` tags
- **Serialize**: `["img", attrs]`

#### hard_break
A hard line break.
- **Inline**: true
- **Group**: `inline`
- **Selectable**: false
- **Parse**: `<br>` tags
- **Serialize**: `["br"]`

## Mark Types

### marks

Object containing all mark type specs.

```javascript
import {marks} from 'prosemirror-schema-basic'
```

#### link
A hyperlink mark.
- **Attributes**:
  - `href`: String (required)
  - `title`: String (optional)
- **Inclusive**: false
- **Parse**: `<a[href]>` tags
- **Serialize**: `["a", attrs, 0]`

#### em
Emphasis (italic) mark.
- **Parse**: `<i>`, `<em>` tags, `font-style: italic`
- **Serialize**: `["em", 0]`

#### strong
Strong emphasis (bold) mark.
- **Parse**: `<strong>`, `<b>` tags, `font-weight: bold/400+`
- **Serialize**: `["strong", 0]`

#### code
Inline code mark.
- **Parse**: `<code>` tags
- **Serialize**: `["code", 0]`

## Usage Examples

### Using the Pre-built Schema

```javascript
import {schema} from 'prosemirror-schema-basic'
import {EditorState} from 'prosemirror-state'
import {EditorView} from 'prosemirror-view'

// Create editor with basic schema
const state = EditorState.create({
  schema,
  doc: schema.node('doc', null, [
    schema.node('heading', {level: 1}, [
      schema.text('Welcome')
    ]),
    schema.node('paragraph', null, [
      schema.text('This is a '),
      schema.text('paragraph', [schema.marks.strong.create()]),
      schema.text(' with '),
      schema.text('formatted', [schema.marks.em.create()]),
      schema.text(' text.')
    ])
  ])
})

const view = new EditorView(document.querySelector('#editor'), {
  state
})
```

### Extending the Basic Schema

```javascript
import {Schema} from 'prosemirror-model'
import {nodes, marks} from 'prosemirror-schema-basic'

// Add custom nodes
const customNodes = {
  ...nodes,
  
  // Add figure node
  figure: {
    content: 'image caption?',
    group: 'block',
    defining: true,
    parseDOM: [{tag: 'figure'}],
    toDOM() { return ['figure', 0] }
  },
  
  // Add caption node
  caption: {
    content: 'inline*',
    parseDOM: [{tag: 'figcaption'}],
    toDOM() { return ['figcaption', 0] }
  },
  
  // Add video node
  video: {
    inline: false,
    group: 'block',
    draggable: true,
    attrs: {
      src: {},
      controls: {default: true},
      width: {default: null},
      height: {default: null}
    },
    parseDOM: [{
      tag: 'video[src]',
      getAttrs(dom) {
        return {
          src: dom.getAttribute('src'),
          controls: dom.hasAttribute('controls'),
          width: dom.getAttribute('width'),
          height: dom.getAttribute('height')
        }
      }
    }],
    toDOM(node) {
      const {src, controls, width, height} = node.attrs
      const attrs = {src}
      if (controls) attrs.controls = ''
      if (width) attrs.width = width
      if (height) attrs.height = height
      return ['video', attrs]
    }
  }
}

// Add custom marks
const customMarks = {
  ...marks,
  
  // Add underline mark
  underline: {
    parseDOM: [
      {tag: 'u'},
      {style: 'text-decoration=underline'}
    ],
    toDOM() { return ['u', 0] }
  },
  
  // Add strikethrough mark
  strikethrough: {
    parseDOM: [
      {tag: 's'},
      {tag: 'del'},
      {style: 'text-decoration=line-through'}
    ],
    toDOM() { return ['s', 0] }
  },
  
  // Add highlight mark
  highlight: {
    attrs: {
      color: {default: 'yellow'}
    },
    parseDOM: [{
      tag: 'mark',
      getAttrs(dom) {
        return {
          color: dom.style.backgroundColor || 'yellow'
        }
      }
    }],
    toDOM(mark) {
      return ['mark', {
        style: `background-color: ${mark.attrs.color}`
      }, 0]
    }
  }
}

// Create extended schema
const extendedSchema = new Schema({
  nodes: customNodes,
  marks: customMarks
})
```

### Modifying Existing Nodes

```javascript
import {nodes as basicNodes, marks as basicMarks} from 'prosemirror-schema-basic'

// Modify paragraph to add alignment
const alignedParagraph = {
  ...basicNodes.paragraph,
  attrs: {
    align: {default: 'left'}
  },
  parseDOM: [{
    tag: 'p',
    getAttrs(dom) {
      const align = dom.style.textAlign || 'left'
      return {align}
    }
  }],
  toDOM(node) {
    const {align} = node.attrs
    const style = align !== 'left' ? {style: `text-align: ${align}`} : {}
    return ['p', style, 0]
  }
}

// Modify heading to add ID
const headingWithId = {
  ...basicNodes.heading,
  attrs: {
    ...basicNodes.heading.attrs,
    id: {default: null}
  },
  parseDOM: basicNodes.heading.parseDOM.map(rule => ({
    ...rule,
    getAttrs(dom) {
      const baseAttrs = rule.getAttrs ? rule.getAttrs(dom) : {}
      return {
        ...baseAttrs,
        level: parseInt(dom.tagName.slice(1)),
        id: dom.getAttribute('id')
      }
    }
  })),
  toDOM(node) {
    const attrs = {id: node.attrs.id}
    return [`h${node.attrs.level}`, attrs, 0]
  }
}

// Create schema with modifications
const modifiedSchema = new Schema({
  nodes: {
    ...basicNodes,
    paragraph: alignedParagraph,
    heading: headingWithId
  },
  marks: basicMarks
})
```

### Creating Documents

```javascript
import {schema} from 'prosemirror-schema-basic'

// Helper functions for creating content
function doc(...content) {
  return schema.node('doc', null, content)
}

function p(...content) {
  return schema.node('paragraph', null, content)
}

function h(level, ...content) {
  return schema.node('heading', {level}, content)
}

function blockquote(...content) {
  return schema.node('blockquote', null, content)
}

function codeBlock(...lines) {
  return schema.node('code_block', null, 
    lines.length ? [schema.text(lines.join('\n'))] : []
  )
}

function img(src, alt, title) {
  return schema.node('image', {src, alt, title})
}

function hr() {
  return schema.node('horizontal_rule')
}

function br() {
  return schema.node('hard_break')
}

function link(href, ...content) {
  const mark = schema.marks.link.create({href})
  return content.map(node => 
    node.mark ? node.mark(node.marks.concat(mark)) : node
  )
}

function strong(...content) {
  const mark = schema.marks.strong.create()
  return content.map(node => node.mark(node.marks.concat(mark)))
}

function em(...content) {
  const mark = schema.marks.em.create()
  return content.map(node => node.mark(node.marks.concat(mark)))
}

function code(...content) {
  const mark = schema.marks.code.create()
  return content.map(node => node.mark(node.marks.concat(mark)))
}

// Create complex document
const myDoc = doc(
  h(1, schema.text('My Document')),
  p(
    schema.text('This is a paragraph with '),
    ...strong(schema.text('bold')),
    schema.text(' and '),
    ...em(schema.text('italic')),
    schema.text(' text.')
  ),
  p(
    schema.text('Here is a '),
    ...link('https://example.com', schema.text('link')),
    schema.text(' and some '),
    ...code(schema.text('inline code')),
    schema.text('.')
  ),
  blockquote(
    p(schema.text('This is a blockquote.')),
    p(schema.text('It can contain multiple paragraphs.'))
  ),
  codeBlock(
    'function hello() {',
    '  console.log("Hello, world!");',
    '}'
  ),
  hr(),
  p(
    schema.text('An image: '),
    img('image.png', 'Alt text', 'Title text')
  )
)
```

### Schema Validation

```javascript
import {schema} from 'prosemirror-schema-basic'

// Validate content against schema
function validateContent(content, nodeType) {
  try {
    nodeType.checkContent(content)
    return {valid: true}
  } catch (error) {
    return {valid: false, error: error.message}
  }
}

// Check if marks are allowed
function checkMarks(nodeType, marks) {
  return marks.every(mark => nodeType.allowsMarkType(mark.type))
}

// Example validation
const para = schema.nodes.paragraph.create()
const content = Fragment.from([
  schema.text('Hello'),
  schema.nodes.image.create({src: 'test.png'}) // Invalid in paragraph
])

const validation = validateContent(content, schema.nodes.paragraph)
console.log(validation) // {valid: false, error: "..."}
```

### Serialization Examples

```javascript
import {schema} from 'prosemirror-schema-basic'
import {DOMSerializer} from 'prosemirror-model'

// Get the built-in serializer
const serializer = DOMSerializer.fromSchema(schema)

// Serialize to DOM
const doc = schema.node('doc', null, [
  schema.node('paragraph', null, [
    schema.text('Hello, '),
    schema.text('world', [schema.marks.strong.create()]),
    schema.text('!')
  ])
])

const dom = serializer.serializeFragment(doc.content)
// Results in: <p>Hello, <strong>world</strong>!</p>

// Custom serialization
const customSerializer = new DOMSerializer({
  paragraph(node) {
    return ['p', {class: 'my-paragraph'}, 0]
  },
  heading(node) {
    return [`h${node.attrs.level}`, {
      class: `heading-${node.attrs.level}`
    }, 0]
  }
}, serializer.marks)
```

## Best Practices

1. **Use as foundation**: The basic schema is a good starting point
2. **Extend carefully**: When extending, maintain consistency with basic schema conventions
3. **Preserve groups**: Keep the `block` and `inline` group structure
4. **Test content expressions**: Validate that your extensions work with existing content rules
5. **Document changes**: Clearly document any deviations from the basic schema
6. **Consider marks**: Ensure new nodes specify which marks they allow
7. **Maintain parseDOM/toDOM**: Keep serialization round-trip compatible

## Migration Guide

If migrating from a custom schema to schema-basic:

```javascript
// Map custom node names to basic schema
const nodeMapping = {
  'header': 'heading',
  'quote': 'blockquote',
  'code': 'code_block',
  'break': 'hard_break',
  'img': 'image'
}

// Transform function
function migrateDocument(doc, fromSchema, toSchema) {
  function mapNode(node) {
    const mappedType = nodeMapping[node.type.name] || node.type.name
    const nodeType = toSchema.nodes[mappedType]
    
    if (!nodeType) {
      // Handle unknown node type
      return toSchema.text(node.textContent)
    }
    
    // Map attributes
    const attrs = {}
    if (mappedType === 'heading' && node.attrs.size) {
      attrs.level = node.attrs.size // Convert size to level
    } else {
      Object.assign(attrs, node.attrs)
    }
    
    // Recursively map content
    const content = []
    node.content.forEach(child => {
      content.push(mapNode(child))
    })
    
    return nodeType.create(attrs, content.length ? content : null)
  }
  
  return mapNode(doc)
}
```

This module provides a solid foundation for building rich text editing experiences with ProseMirror.