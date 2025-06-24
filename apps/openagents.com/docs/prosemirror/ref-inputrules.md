# ProseMirror Input Rules Module Reference

## Overview

The prosemirror-inputrules module allows you to define rules that automatically transform text as the user types. This is useful for features like auto-formatting, smart quotes, and markdown-style shortcuts.

## Installation

```bash
npm install prosemirror-inputrules
```

## Core Classes and Functions

### inputRules(config)

Creates a plugin that enables input rules.

```javascript
import {inputRules} from 'prosemirror-inputrules'

const inputRulesPlugin = inputRules({
  rules: [
    // Array of InputRule instances
  ]
})
```

Configuration:
- `rules`: Array of InputRule instances to apply

### InputRule

Class representing a single input rule.

```javascript
new InputRule(match, handler)
```

Parameters:
- `match`: RegExp to match against text before cursor
- `handler`: Function or replacement to apply when matched

Handler signature:
```typescript
(
  state: EditorState,
  match: RegExpMatchArray,
  start: number,
  end: number
) => Transaction | null
```

### Pre-built Rules

#### smartQuotes

Automatically converts straight quotes to smart quotes.

```javascript
import {smartQuotes} from 'prosemirror-inputrules'

const rules = smartQuotes
// Includes rules for:
// - "..." → "..."
// - '...' → '...'
```

#### ellipsis

Converts three dots to ellipsis character.

```javascript
import {ellipsis} from 'prosemirror-inputrules'

// ... → …
```

#### emDash

Converts double dash to em dash.

```javascript
import {emDash} from 'prosemirror-inputrules'

// -- → —
```

### Helper Functions

#### wrappingInputRule(regexp, nodeType, getAttrs?, joinPredicate?)

Creates a rule that wraps text in a node when matched.

```javascript
import {wrappingInputRule} from 'prosemirror-inputrules'

// Create blockquote with "> "
const blockquoteRule = wrappingInputRule(
  /^\s*>\s$/,
  schema.nodes.blockquote
)

// Create heading with "# "
const headingRule = wrappingInputRule(
  /^(#{1,6})\s$/,
  schema.nodes.heading,
  match => ({level: match[1].length})
)
```

Parameters:
- `regexp`: Pattern to match
- `nodeType`: Node type to wrap with
- `getAttrs`: Function to derive node attributes from match (optional)
- `joinPredicate`: Function to test if nodes can be joined (optional)

#### textblockTypeInputRule(regexp, nodeType, getAttrs?)

Creates a rule that changes a text block type when matched.

```javascript
import {textblockTypeInputRule} from 'prosemirror-inputrules'

// Convert to code block with ```
const codeBlockRule = textblockTypeInputRule(
  /^```$/,
  schema.nodes.code_block
)

// Convert to heading
const headingRule = textblockTypeInputRule(
  /^(#{1,6})\s$/,
  schema.nodes.heading,
  match => ({level: match[1].length})
)
```

## Usage Examples

### Basic Input Rules

```javascript
import {InputRule, inputRules} from 'prosemirror-inputrules'

// Simple text replacement
const copyrightRule = new InputRule(
  /\(c\)$/,
  '©'
)

// Arrow replacements
const arrowRules = [
  new InputRule(/->$/, '→'),
  new InputRule(/<-$/, '←'),
  new InputRule(/<->$/, '↔'),
  new InputRule(/=>$/, '⇒'),
  new InputRule(/<=/, '⇐')
]

// Emoji shortcuts
const emojiRules = [
  new InputRule(/:smile:$/, '😊'),
  new InputRule(/:heart:$/, '❤️'),
  new InputRule(/:fire:$/, '🔥'),
  new InputRule(/:100:$/, '💯')
]

const plugin = inputRules({
  rules: [
    copyrightRule,
    ...arrowRules,
    ...emojiRules,
    ...smartQuotes,
    ellipsis,
    emDash
  ]
})
```

### Markdown-Style Formatting

```javascript
// Bold with **text**
const boldRule = new InputRule(
  /\*\*([^*]+)\*\*$/,
  (state, match, start, end) => {
    const [fullMatch, text] = match
    const mark = schema.marks.strong.create()
    
    return state.tr
      .delete(start, end)
      .insertText(text, start)
      .addMark(start, start + text.length, mark)
  }
)

// Italic with *text*
const italicRule = new InputRule(
  /(?:^|[^*])\*([^*]+)\*$/,
  (state, match, start, end) => {
    const [fullMatch, text] = match
    const mark = schema.marks.em.create()
    const startPos = match[0].startsWith('*') ? start : start + 1
    
    return state.tr
      .delete(startPos, end)
      .insertText(text, startPos)
      .addMark(startPos, startPos + text.length, mark)
  }
)

// Code with `text`
const codeRule = new InputRule(
  /`([^`]+)`$/,
  (state, match, start, end) => {
    const [fullMatch, text] = match
    const mark = schema.marks.code.create()
    
    return state.tr
      .delete(start, end)
      .insertText(text, start)
      .addMark(start, start + text.length, mark)
  }
)

// Link with [text](url)
const linkRule = new InputRule(
  /\[([^\]]+)\]\(([^)]+)\)$/,
  (state, match, start, end) => {
    const [fullMatch, text, url] = match
    const mark = schema.marks.link.create({href: url})
    
    return state.tr
      .delete(start, end)
      .insertText(text, start)
      .addMark(start, start + text.length, mark)
  }
)
```

### List Creation Rules

```javascript
// Bullet list with - or *
const bulletListRule = wrappingInputRule(
  /^\s*([-*])\s$/,
  schema.nodes.bullet_list
)

// Ordered list with 1.
const orderedListRule = wrappingInputRule(
  /^\s*(\d+)\.\s$/,
  schema.nodes.ordered_list,
  match => ({order: parseInt(match[1])})
)

// Task list with [ ] or [x]
const taskListRule = wrappingInputRule(
  /^\s*\[([ x])\]\s$/,
  schema.nodes.task_list,
  match => ({checked: match[1] === 'x'})
)
```

### Advanced Input Rules

```javascript
// Math expressions with $$
const mathRule = new InputRule(
  /\$\$([^$]+)\$\$$/,
  (state, match, start, end) => {
    const [fullMatch, expression] = match
    const node = schema.nodes.math_inline.create({
      expression
    })
    
    return state.tr
      .delete(start, end)
      .insert(start, node)
  }
)

// Footnote references
const footnoteRule = new InputRule(
  /\[\^(\d+)\]$/,
  (state, match, start, end) => {
    const [fullMatch, id] = match
    const node = schema.nodes.footnote_ref.create({
      id: parseInt(id)
    })
    
    return state.tr
      .delete(start, end)
      .insert(start, node)
  }
)

// Custom block creation
const customBlockRule = new InputRule(
  /^::(\w+)\s$/,
  (state, match, start, end) => {
    const [fullMatch, blockType] = match
    const nodeType = schema.nodes[blockType]
    
    if (!nodeType) return null
    
    const $start = state.doc.resolve(start)
    const range = $start.blockRange()
    
    if (!range) return null
    
    return state.tr
      .delete(start, end)
      .setBlockType(range.start, range.end, nodeType)
  }
)
```

### Contextual Input Rules

```javascript
// Only apply in certain contexts
class ContextualInputRule extends InputRule {
  constructor(match, handler, context) {
    super(match, (state, match, start, end) => {
      const $pos = state.doc.resolve(start)
      
      // Check context
      if (!context($pos)) return null
      
      return handler(state, match, start, end)
    })
  }
}

// Heading rule only at start of line
const headingOnlyAtStart = new ContextualInputRule(
  /^(#{1,6})\s$/,
  (state, match, start, end) => {
    const level = match[1].length
    const $start = state.doc.resolve(start)
    
    return state.tr
      .delete(start, end)
      .setBlockType($start.pos, $start.pos, schema.nodes.heading, {level})
  },
  $pos => $pos.parentOffset === 0
)

// Code block only in doc or blockquote
const codeBlockInContext = new ContextualInputRule(
  /^```(\w+)?\s$/,
  (state, match, start, end) => {
    const language = match[1] || ''
    return state.tr
      .delete(start, end)
      .setBlockType(start, start, schema.nodes.code_block, {language})
  },
  $pos => {
    const parent = $pos.parent.type.name
    return parent === 'doc' || parent === 'blockquote'
  }
)
```

### Undo-Aware Rules

```javascript
// Rule that can be undone as single action
const undoableRule = new InputRule(
  /--$/,
  (state, match, start, end) => {
    const tr = state.tr.delete(start, end).insertText('—', start)
    
    // Mark for grouping with next input
    tr.setMeta('inputRule', true)
    
    return tr
  }
)

// In history plugin configuration
import {history} from 'prosemirror-history'

const historyPlugin = history({
  newGroupDelay: 500,
  // Group input rule transformations
  isTransaction: tr => !tr.getMeta('inputRule')
})
```

### Complex Transformations

```javascript
// Table creation with |
const tableRule = new InputRule(
  /^\|(.+)\|$/,
  (state, match, start, end) => {
    const cellsText = match[1].split('|').map(cell => cell.trim())
    
    const rows = []
    const row = schema.nodes.table_row.create(
      null,
      cellsText.map(text => 
        schema.nodes.table_cell.create(
          null,
          schema.nodes.paragraph.create(null, schema.text(text))
        )
      )
    )
    rows.push(row)
    
    const table = schema.nodes.table.create(null, rows)
    
    const $start = state.doc.resolve(start)
    const range = $start.blockRange()
    
    if (!range) return null
    
    return state.tr
      .delete(range.start, range.end)
      .insert(range.start, table)
  }
)

// Auto-list continuation
const continueListRule = new InputRule(
  /^(\s*)([-*]|\d+\.)\s$/,
  (state, match, start, end) => {
    const $start = state.doc.resolve(start)
    const listItem = $start.node(-1)
    
    if (listItem.type !== schema.nodes.list_item) return null
    
    const list = $start.node(-2)
    const listType = list.type
    
    // Check if current item is empty
    if (listItem.content.size === 0) {
      // Exit list
      const $listStart = state.doc.resolve($start.start(-2))
      const range = $listStart.blockRange()
      
      if (range && liftTarget(range) != null) {
        return state.tr.lift(range, liftTarget(range)).delete(start, end)
      }
    }
    
    return null
  }
)
```

### Configurable Rules

```javascript
// Factory for creating configurable rules
function createAutoLinkRule(options = {}) {
  const {
    protocols = ['http://', 'https://', 'ftp://'],
    emailPattern = /[\w._%+-]+@[\w.-]+\.[A-Za-z]{2,}/
  } = options
  
  const urlPattern = new RegExp(
    `(${protocols.join('|')})[^\\s]+$`
  )
  
  return [
    // URL auto-linking
    new InputRule(urlPattern, (state, match, start, end) => {
      const url = match[0]
      const mark = schema.marks.link.create({href: url})
      
      return state.tr
        .addMark(start, end, mark)
        .insertText(' ')
    }),
    
    // Email auto-linking
    new InputRule(emailPattern, (state, match, start, end) => {
      const email = match[0]
      const mark = schema.marks.link.create({href: `mailto:${email}`})
      
      return state.tr
        .addMark(start, end, mark)
        .insertText(' ')
    })
  ]
}
```

## Best Practices

1. **Test thoroughly**: Input rules can interfere with normal typing
2. **Make rules specific**: Avoid overly broad patterns
3. **Consider context**: Some rules should only apply in certain positions
4. **Handle edge cases**: Test at document boundaries
5. **Provide escape hatches**: Allow users to type literal characters
6. **Performance**: Complex regex can impact typing performance
7. **User expectations**: Follow common conventions (markdown, etc.)

## Complete Example

```javascript
import {EditorState} from 'prosemirror-state'
import {EditorView} from 'prosemirror-view'
import {inputRules, wrappingInputRule, textblockTypeInputRule,
        smartQuotes, emDash, ellipsis} from 'prosemirror-inputrules'

// Create comprehensive input rules
function buildInputRules(schema) {
  const rules = []
  
  // Smart typography
  rules.push(...smartQuotes)
  rules.push(emDash)
  rules.push(ellipsis)
  
  // Block formatting
  if (schema.nodes.blockquote) {
    rules.push(wrappingInputRule(/^\s*>\s$/, schema.nodes.blockquote))
  }
  
  if (schema.nodes.heading) {
    rules.push(textblockTypeInputRule(
      /^(#{1,6})\s$/,
      schema.nodes.heading,
      match => ({level: match[1].length})
    ))
  }
  
  if (schema.nodes.code_block) {
    rules.push(textblockTypeInputRule(
      /^```([a-z]+)?\s$/,
      schema.nodes.code_block,
      match => ({language: match[1]})
    ))
  }
  
  // Lists
  if (schema.nodes.bullet_list) {
    rules.push(wrappingInputRule(
      /^\s*([-*])\s$/,
      schema.nodes.bullet_list
    ))
  }
  
  if (schema.nodes.ordered_list) {
    rules.push(wrappingInputRule(
      /^\s*(\d+)\.\s$/,
      schema.nodes.ordered_list,
      match => ({start: parseInt(match[1])})
    ))
  }
  
  // Inline formatting
  if (schema.marks.strong) {
    rules.push(new InputRule(
      /\*\*([^*]+)\*\*$/,
      (state, match, start, end) => {
        const mark = schema.marks.strong.create()
        return state.tr
          .delete(start, end)
          .insertText(match[1], start)
          .addMark(start, start + match[1].length, mark)
      }
    ))
  }
  
  return inputRules({rules})
}

// Initialize editor with input rules
const state = EditorState.create({
  schema,
  plugins: [
    buildInputRules(schema)
  ]
})

const view = new EditorView(document.querySelector('#editor'), {
  state
})
```

This module enables automatic text transformations that enhance the writing experience.