# ProseMirror Example Setup Module Reference

## Overview

The prosemirror-example-setup module provides a quick way to set up a basic ProseMirror editor with sensible defaults. It bundles together common plugins and configurations, making it ideal for getting started quickly or as a reference for building custom setups.

## Installation

```bash
npm install prosemirror-example-setup
```

## Main Function

### exampleSetup(options)

Creates an array of plugins for a basic editor setup.

```javascript
import {exampleSetup} from 'prosemirror-example-setup'

const plugins = exampleSetup({
  schema: mySchema,
  mapKeys: {
    "Mod-Space": myCustomCommand
  },
  menuBar: true,
  history: true,
  floatingMenu: false,
  menuContent: myCustomMenuContent
})
```

Options:
- `schema`: The schema to use (required)
- `mapKeys`: Object mapping keys to commands (optional)
- `menuBar`: Whether to include menu bar (default: true)
- `floatingMenu`: Whether menu should float (default: false)
- `history`: Whether to include history support (default: true)
- `menuContent`: Custom menu content array (optional)

## What's Included

The example setup includes:

1. **Input rules**: Smart quotes, ellipsis, arrows, etc.
2. **Keymaps**: Standard keyboard shortcuts
3. **Drop cursor**: Visual feedback for drag & drop
4. **Gap cursor**: Navigation between blocks
5. **Menu bar**: UI for commands
6. **History**: Undo/redo support
7. **Base commands**: Essential editing commands

## Default Keybindings

```javascript
// Included keybindings:
{
  "Enter": splitListItem || splitBlock,
  "Mod-Enter": exitCode,
  "Backspace": deleteSelection || joinBackward,
  "Delete": deleteSelection || joinForward,
  "Mod-Delete": deleteSelection,
  "Mod-a": selectAll,
  "Mod-z": undo,
  "Mod-y": redo,
  "Mod-Shift-z": redo,
  "Mod-b": toggleMark(strong),
  "Mod-i": toggleMark(em),
  "Mod-`": toggleMark(code),
  "Shift-Ctrl-0": setBlockType(paragraph),
  "Shift-Ctrl-1": setBlockType(heading, {level: 1}),
  "Shift-Ctrl-2": setBlockType(heading, {level: 2}),
  "Shift-Ctrl-3": setBlockType(heading, {level: 3}),
  "Shift-Ctrl-\\": setBlockType(code_block),
  "Mod-[": liftListItem,
  "Mod-]": sinkListItem,
  "Mod->": wrapIn(blockquote),
  "Tab": sinkListItem,
  "Shift-Tab": liftListItem,
  "ArrowLeft": selectNodeBackward,
  "ArrowRight": selectNodeForward,
  "Mod-ArrowUp": joinUp,
  "Mod-ArrowDown": joinDown,
  "Alt-ArrowUp": joinUp,
  "Alt-ArrowDown": joinDown
}
```

## Default Input Rules

```javascript
// Smart typography
"..." → "…"        // Ellipsis
"--" → "—"         // Em dash
">>" → "»"         // Right guillemet
"<<" → "«"         // Left guillemet
"->" → "→"         // Right arrow
"<-" → "←"         // Left arrow
":)" → "🙂"        // Emoticons (if enabled)

// Markdown-style formatting
"> " → blockquote
"# " → heading 1
"## " → heading 2
"### " → heading 3
"```" → code block
"* " or "- " → bullet list
"1. " → ordered list
```

## Usage Examples

### Basic Setup

```javascript
import {EditorState} from 'prosemirror-state'
import {EditorView} from 'prosemirror-view'
import {Schema} from 'prosemirror-model'
import {schema} from 'prosemirror-schema-basic'
import {exampleSetup} from 'prosemirror-example-setup'

// Quick setup with defaults
const state = EditorState.create({
  schema,
  plugins: exampleSetup({schema})
})

const view = new EditorView(document.querySelector('#editor'), {
  state
})
```

### Custom Configuration

```javascript
import {exampleSetup} from 'prosemirror-example-setup'
import {buildMenuItems} from './custom-menu'

// Setup with custom options
const plugins = exampleSetup({
  schema: mySchema,
  
  // Custom keybindings
  mapKeys: {
    "Mod-Space": toggleHeading,
    "Mod-Shift-l": toggleBulletList,
    "F1": showHelp
  },
  
  // Custom menu
  menuBar: true,
  floatingMenu: false,
  menuContent: buildMenuItems(mySchema).fullMenu,
  
  // Include history
  history: true
})

const state = EditorState.create({
  schema: mySchema,
  plugins
})
```

### Building Custom Setup

```javascript
// Use example setup as reference for custom setup
import {inputRules, smartQuotes, emDash, ellipsis} from 'prosemirror-inputrules'
import {keymap} from 'prosemirror-keymap'
import {history} from 'prosemirror-history'
import {dropCursor} from 'prosemirror-dropcursor'
import {gapCursor} from 'prosemirror-gapcursor'
import {menuBar} from 'prosemirror-menu'
import {baseKeymap} from 'prosemirror-commands'

function myCustomSetup(options) {
  const plugins = []
  
  // Input rules
  plugins.push(inputRules({
    rules: [
      ...smartQuotes,
      ellipsis,
      emDash,
      // Custom rules
      myCustomInputRule
    ]
  }))
  
  // Keymaps
  plugins.push(keymap({
    ...baseKeymap,
    ...options.customKeys
  }))
  
  // UI enhancements
  plugins.push(dropCursor({color: options.dropCursorColor}))
  plugins.push(gapCursor())
  
  // Menu
  if (options.menuBar !== false) {
    plugins.push(menuBar({
      content: options.menuContent || defaultMenuContent,
      floating: options.floatingMenu
    }))
  }
  
  // History
  if (options.history !== false) {
    plugins.push(history())
    plugins.push(keymap({
      "Mod-z": undo,
      "Mod-y": redo,
      "Mod-Shift-z": redo
    }))
  }
  
  return plugins
}
```

### Extending Example Setup

```javascript
import {exampleSetup} from 'prosemirror-example-setup'
import {Plugin} from 'prosemirror-state'

// Get base plugins and add custom ones
const basePlugins = exampleSetup({
  schema,
  menuBar: false  // We'll add custom menu
})

const customPlugins = [
  ...basePlugins,
  
  // Custom menu
  myCustomMenuPlugin(),
  
  // Additional features
  collaborationPlugin(),
  spellCheckPlugin(),
  autoSavePlugin(),
  
  // Custom behavior
  new Plugin({
    props: {
      handleDOMEvents: {
        focus(view) {
          console.log('Editor focused')
          return false
        }
      }
    }
  })
]

const state = EditorState.create({
  schema,
  plugins: customPlugins
})
```

### Minimal Setup

```javascript
// Minimal setup without menu
const minimalPlugins = exampleSetup({
  schema,
  menuBar: false
})

// Or build minimal manually
const minimalCustom = [
  inputRules({rules: smartQuotes}),
  keymap(baseKeymap),
  history(),
  keymap({
    "Mod-z": undo,
    "Mod-y": redo
  })
]
```

### Schema Requirements

The example setup expects certain node and mark types:

```javascript
// Expected schema structure
const requiredSchema = new Schema({
  nodes: {
    doc: {content: "block+"},
    paragraph: {group: "block", content: "inline*"},
    blockquote: {group: "block", content: "block+"},
    horizontal_rule: {group: "block"},
    heading: {group: "block", attrs: {level: {default: 1}}},
    code_block: {group: "block", content: "text*"},
    text: {group: "inline"},
    image: {group: "inline"},
    hard_break: {group: "inline"}
  },
  marks: {
    link: {},
    em: {},
    strong: {},
    code: {}
  }
})

// Check schema compatibility
function checkSchemaCompatibility(schema) {
  const missing = []
  
  // Check nodes
  const nodes = ['paragraph', 'blockquote', 'heading', 'code_block']
  nodes.forEach(name => {
    if (!schema.nodes[name]) {
      missing.push(`node: ${name}`)
    }
  })
  
  // Check marks
  const marks = ['strong', 'em', 'code']
  marks.forEach(name => {
    if (!schema.marks[name]) {
      missing.push(`mark: ${name}`)
    }
  })
  
  if (missing.length) {
    console.warn('Schema missing:', missing)
  }
  
  return missing.length === 0
}
```

### Customizing Menu Items

```javascript
import {buildMenuItems} from 'prosemirror-example-setup'
import {MenuItem} from 'prosemirror-menu'

// Get default menu items
const defaultItems = buildMenuItems(schema)

// Customize menu content
const customMenuContent = [
  // Keep some defaults
  defaultItems.toggleStrong,
  defaultItems.toggleEm,
  
  // Add custom items
  new MenuItem({
    title: "Toggle highlight",
    run: toggleMark(schema.marks.highlight),
    active: state => isMarkActive(state, schema.marks.highlight),
    content: "H"
  }),
  
  // Group items
  [defaultItems.wrapBulletList, defaultItems.wrapOrderedList],
  
  // Custom dropdown
  new Dropdown(
    [heading1, heading2, heading3],
    {label: "Heading"}
  )
]

const plugins = exampleSetup({
  schema,
  menuContent: customMenuContent
})
```

### Mobile-Friendly Setup

```javascript
// Adjust setup for mobile
const isMobile = /mobile/i.test(navigator.userAgent)

const mobileSetup = exampleSetup({
  schema,
  
  // Floating menu on mobile
  floatingMenu: isMobile,
  
  // Simplified menu for mobile
  menuContent: isMobile ? simplifiedMenu : fullMenu,
  
  // Additional mobile-specific keys
  mapKeys: isMobile ? {
    "Enter": chainCommands(
      newlineInCode,
      createParagraphNear,
      liftEmptyBlock,
      splitBlock
    )
  } : {}
})

// Add mobile-specific plugins
if (isMobile) {
  mobileSetup.push(
    touchGesturesPlugin(),
    virtualKeyboardPlugin()
  )
}
```

### Debugging Setup

```javascript
// Debug version of example setup
function debugExampleSetup(options) {
  const plugins = exampleSetup(options)
  
  // Add debug plugin
  plugins.push(new Plugin({
    view() {
      return {
        update(view, prevState) {
          console.log('State updated:', {
            docChanged: !view.state.doc.eq(prevState.doc),
            selectionChanged: !view.state.selection.eq(prevState.selection),
            tr: view.state.tr
          })
        }
      }
    }
  }))
  
  // Log all transactions
  plugins.push(new Plugin({
    filterTransaction(tr) {
      console.log('Transaction:', {
        steps: tr.steps.length,
        selectionSet: tr.selectionSet,
        docChanged: tr.docChanged,
        metadata: tr.getMeta()
      })
      return true
    }
  }))
  
  return plugins
}
```

## CSS Requirements

The example setup requires basic CSS for the menu:

```css
/* Editor wrapper */
.ProseMirror {
  position: relative;
  border: 1px solid #ddd;
  min-height: 300px;
  padding: 10px;
}

/* Focus style */
.ProseMirror:focus {
  outline: none;
  border-color: #66afe9;
}

/* Menu bar (included via prosemirror-menu) */
.ProseMirror-menubar {
  border-bottom: 1px solid #ddd;
  padding: 5px;
  background: #f7f7f7;
}

/* Gap cursor (included via prosemirror-gapcursor) */
.ProseMirror-gapcursor {
  display: none;
  pointer-events: none;
  position: absolute;
}

.ProseMirror-gapcursor:after {
  content: "";
  display: block;
  position: absolute;
  top: -2px;
  width: 20px;
  border-top: 1px solid black;
  animation: ProseMirror-cursor-blink 1.1s steps(2, start) infinite;
}

@keyframes ProseMirror-cursor-blink {
  to { visibility: hidden; }
}

.ProseMirror.ProseMirror-focused .ProseMirror-gapcursor {
  display: block;
}

/* Selected node outline */
.ProseMirror-selectednode {
  outline: 2px solid #8cf;
}
```

## Complete Example

```javascript
import {EditorState} from 'prosemirror-state'
import {EditorView} from 'prosemirror-view'
import {Schema} from 'prosemirror-model'
import {addListNodes} from 'prosemirror-schema-list'
import {exampleSetup} from 'prosemirror-example-setup'

// Define schema
const mySchema = new Schema({
  nodes: addListNodes(
    {
      doc: {content: "block+"},
      paragraph: {
        content: "inline*",
        group: "block",
        parseDOM: [{tag: "p"}],
        toDOM() { return ["p", 0] }
      },
      blockquote: {
        content: "block+",
        group: "block",
        defining: true,
        parseDOM: [{tag: "blockquote"}],
        toDOM() { return ["blockquote", 0] }
      },
      horizontal_rule: {
        group: "block",
        parseDOM: [{tag: "hr"}],
        toDOM() { return ["hr"] }
      },
      heading: {
        attrs: {level: {default: 1}},
        content: "inline*",
        group: "block",
        defining: true,
        parseDOM: [
          {tag: "h1", attrs: {level: 1}},
          {tag: "h2", attrs: {level: 2}},
          {tag: "h3", attrs: {level: 3}},
          {tag: "h4", attrs: {level: 4}},
          {tag: "h5", attrs: {level: 5}},
          {tag: "h6", attrs: {level: 6}}
        ],
        toDOM(node) { return ["h" + node.attrs.level, 0] }
      },
      code_block: {
        content: "text*",
        marks: "",
        group: "block",
        code: true,
        defining: true,
        parseDOM: [{tag: "pre", preserveWhitespace: "full"}],
        toDOM() { return ["pre", ["code", 0]] }
      },
      text: {
        group: "inline"
      },
      image: {
        inline: true,
        attrs: {
          src: {},
          alt: {default: null},
          title: {default: null}
        },
        group: "inline",
        draggable: true,
        parseDOM: [{
          tag: "img[src]",
          getAttrs(dom) {
            return {
              src: dom.getAttribute("src"),
              title: dom.getAttribute("title"),
              alt: dom.getAttribute("alt")
            }
          }
        }],
        toDOM(node) { return ["img", node.attrs] }
      },
      hard_break: {
        inline: true,
        group: "inline",
        selectable: false,
        parseDOM: [{tag: "br"}],
        toDOM() { return ["br"] }
      }
    },
    "doc",
    "paragraph"
  ),
  marks: {
    link: {
      attrs: {
        href: {},
        title: {default: null}
      },
      inclusive: false,
      parseDOM: [{
        tag: "a[href]",
        getAttrs(dom) {
          return {
            href: dom.getAttribute("href"),
            title: dom.getAttribute("title")
          }
        }
      }],
      toDOM(node) { return ["a", node.attrs, 0] }
    },
    em: {
      parseDOM: [{tag: "i"}, {tag: "em"}, {style: "font-style=italic"}],
      toDOM() { return ["em", 0] }
    },
    strong: {
      parseDOM: [
        {tag: "strong"},
        {tag: "b"},
        {style: "font-weight", getAttrs: value => /^(bold(er)?|[5-9]\d{2,})$/.test(value) && null}
      ],
      toDOM() { return ["strong", 0] }
    },
    code: {
      parseDOM: [{tag: "code"}],
      toDOM() { return ["code", 0] }
    }
  }
})

// Create editor
const state = EditorState.create({
  doc: mySchema.nodes.doc.create(null, [
    mySchema.nodes.heading.create({level: 1}, [
      mySchema.text("Welcome to ProseMirror")
    ]),
    mySchema.nodes.paragraph.create(null, [
      mySchema.text("This is an example editor with "),
      mySchema.text("rich text", [mySchema.marks.strong.create()]),
      mySchema.text(" editing capabilities.")
    ])
  ]),
  plugins: exampleSetup({
    schema: mySchema,
    menuBar: true,
    history: true
  })
})

const view = new EditorView(document.querySelector('#editor'), {
  state,
  dispatchTransaction(transaction) {
    console.log('Document size:', transaction.doc.content.size)
    const newState = view.state.apply(transaction)
    view.updateState(newState)
  }
})

// Add CSS
const style = document.createElement('style')
style.textContent = `
  #editor {
    background: white;
    color: black;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
    font-size: 16px;
    line-height: 1.5;
  }
  
  .ProseMirror {
    min-height: 140px;
    overflow-y: auto;
    box-sizing: border-box;
    -moz-box-sizing: border-box;
  }
  
  .ProseMirror:focus {
    outline: none;
  }
  
  .ProseMirror p { margin-bottom: 1em }
  .ProseMirror h1 { margin: .67em 0; font-size: 2em }
  .ProseMirror h2 { margin: .75em 0; font-size: 1.5em }
  .ProseMirror h3 { margin: .83em 0; font-size: 1.17em }
  .ProseMirror h4 { margin: 1.12em 0 }
  .ProseMirror h5 { margin: 1.5em 0; font-size: .83em }
  .ProseMirror h6 { margin: 1.67em 0; font-size: .75em }
  .ProseMirror pre { white-space: pre-wrap }
  .ProseMirror blockquote { margin: 1em 40px }
`
document.head.appendChild(style)
```

This module provides everything needed to create a functional ProseMirror editor quickly.