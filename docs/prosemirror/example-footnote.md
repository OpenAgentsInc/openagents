# ProseMirror Footnote Example

## Overview

This example demonstrates how to implement footnotes in ProseMirror, showing a custom approach to creating inline nodes with editable content through a nested editor view.

## HTML Structure

```html
<div id="editor"></div>
<div id="content">
  <p>This paragraph has a footnote<footnote>Which is a piece of text placed at the bottom of a page...</footnote> in it.</p>
</div>
```

## CSS Styles

```css
.ProseMirror { counter-reset: prosemirror-footnote; }
footnote { 
  display: inline-block; 
  position: relative; 
  cursor: pointer; 
}
footnote::after { 
  content: counter(prosemirror-footnote); 
  vertical-align: super; 
  font-size: 75%; 
  counter-increment: prosemirror-footnote; 
}
.footnote-tooltip {
  cursor: auto;
  position: absolute;
  left: -30px;
  top: calc(100% + 10px);
  background: silver;
  padding: 3px;
  border-radius: 2px;
  width: 500px;
}
```

## JavaScript Code

```javascript
import {Schema} from "prosemirror-model"
import {EditorState, EditorView} from "prosemirror-view"
import {StepMap} from "prosemirror-transform"
import {keymap} from "prosemirror-keymap"
import {undo, redo} from "prosemirror-history"
import {schema} from "prosemirror-schema-basic"
import {exampleSetup, buildMenuItems} from "prosemirror-example-setup"
import {MenuItem} from "prosemirror-menu"

// Footnote Node Specification
const footnoteSpec = {
  group: "inline",
  content: "text*",
  inline: true,
  atom: true,
  toDOM: () => ["footnote", 0],
  parseDOM: [{tag: "footnote"}]
}

// Create schema with footnote node
const footnoteSchema = new Schema({
  nodes: schema.spec.nodes.addBefore("image", "footnote", footnoteSpec),
  marks: schema.spec.marks
})

// Footnote Node View
class FootnoteView {
  constructor(node, view, getPos) {
    this.node = node
    this.outerView = view
    this.getPos = getPos
    
    // Create DOM representation
    this.dom = document.createElement("footnote")
    this.innerView = null
  }

  selectNode() {
    this.dom.classList.add("ProseMirror-selectednode")
    if (!this.innerView) this.open()
  }

  deselectNode() {
    this.dom.classList.remove("ProseMirror-selectednode")
    if (this.innerView) this.close()
  }

  open() {
    // Create tooltip container
    let tooltip = this.dom.appendChild(document.createElement("div"))
    tooltip.className = "footnote-tooltip"
    
    // Create inner editor
    this.innerView = new EditorView(tooltip, {
      state: EditorState.create({
        doc: this.node,
        plugins: [keymap({
          "Mod-z": () => undo(this.outerView.state, this.outerView.dispatch),
          "Mod-y": () => redo(this.outerView.state, this.outerView.dispatch)
        })]
      }),
      dispatchTransaction: this.dispatchInner.bind(this),
      handleDOMEvents: {
        mousedown: () => {
          // Focus inner editor if outer editor has focus
          if (this.outerView.hasFocus()) this.innerView.focus()
        }
      }
    })
  }

  close() {
    this.innerView.destroy()
    this.innerView = null
    this.dom.textContent = ""
  }

  dispatchInner(tr) {
    let {state, transactions} = this.innerView.state.applyTransaction(tr)
    this.innerView.updateState(state)

    if (!tr.getMeta("fromOutside")) {
      let outerTr = this.outerView.state.tr, offsetMap = StepMap.offset(this.getPos() + 1)
      for (let i = 0; i < transactions.length; i++) {
        let steps = transactions[i].steps
        for (let j = 0; j < steps.length; j++)
          outerTr.step(steps[j].map(offsetMap))
      }
      if (outerTr.docChanged) this.outerView.dispatch(outerTr)
    }
  }

  update(node) {
    if (!node.sameMarkup(this.node)) return false
    this.node = node
    if (this.innerView) {
      let state = this.innerView.state
      let start = node.content.findDiffStart(state.doc.content)
      if (start != null) {
        let {a: endA, b: endB} = node.content.findDiffEnd(state.doc.content)
        let overlap = start - Math.min(endA, endB)
        if (overlap > 0) { endA += overlap; endB += overlap }
        this.innerView.dispatch(
          state.tr
            .replace(start, endB, node.slice(start, endA))
            .setMeta("fromOutside", true))
      }
    }
    return true
  }

  destroy() {
    if (this.innerView) this.close()
  }

  stopEvent(event) {
    return this.innerView && this.innerView.dom.contains(event.target)
  }

  ignoreMutation() { return true }
}

// Command to insert footnote
function insertFootnote(state, dispatch) {
  let {$from} = state.selection, index = $from.index()
  if (!$from.parent.canReplaceWith(index, index, footnoteSchema.nodes.footnote))
    return false
  if (dispatch) {
    let footnote = footnoteSchema.nodes.footnote.create()
    dispatch(state.tr.replaceSelectionWith(footnote))
  }
  return true
}

// Build menu with footnote option
let menu = buildMenuItems(footnoteSchema)
menu.insertMenu.content.push(new MenuItem({
  title: "Insert footnote",
  label: "Footnote",
  select: insertFootnote,
  run: insertFootnote
}))

// Initialize editor
window.view = new EditorView(document.querySelector("#editor"), {
  state: EditorState.create({
    doc: DOMParser.fromSchema(footnoteSchema).parse(document.querySelector("#content")),
    plugins: exampleSetup({schema: footnoteSchema, menuContent: menu.fullMenu})
  }),
  nodeViews: {
    footnote(node, view, getPos) { return new FootnoteView(node, view, getPos) }
  }
})
```

## Key Concepts

### Nested Editor Views
- The footnote contains a complete ProseMirror editor instance
- Inner editor is created when footnote is selected
- Changes in inner editor are mapped to outer document

### Transaction Mapping
- Steps from inner editor are mapped to outer document coordinates
- Uses `StepMap.offset` to adjust positions
- Prevents infinite loops with `fromOutside` meta flag

### Node View Lifecycle
- **selectNode/deselectNode**: Opens/closes the inner editor
- **update**: Syncs content when outer document changes
- **stopEvent**: Prevents outer editor from handling inner events
- **ignoreMutation**: Tells ProseMirror not to re-render on DOM changes

### CSS Counter Magic
- Uses CSS counters to automatically number footnotes
- `counter-reset` on editor container
- `counter-increment` in footnote::after
- Numbers update automatically as footnotes are added/removed

### Keyboard Handling
- Undo/redo in inner editor affects outer document
- Mouse events properly focus the appropriate editor
- All standard editing operations work within footnotes

## Benefits
- Fully editable inline footnotes
- Automatic numbering via CSS
- Clean separation between display and editing states
- Maintains document structure integrity
- Seamless integration with ProseMirror's transaction system