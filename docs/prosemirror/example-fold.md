# ProseMirror Code Folding Example

## Overview

This example demonstrates how to use node decorations to implement section folding functionality in a ProseMirror editor. Users can hide and show section content by clicking a toggle button.

## CSS Styles

```css
.ProseMirror section header { 
  margin: 0 -8px 0 -14px; 
  padding: 0 8px 0 14px; 
  background: #eee; 
  color: #888; 
  display: flex; 
  justify-content: space-between; 
}

.ProseMirror section header:before { 
  content: "section" 
}

.ProseMirror section header button { 
  background: transparent; 
  border: none; 
  font: inherit; 
  color: inherit; 
  font-size: 80%; 
}

.ProseMirror section h1 { 
  font-size: 100%; 
  line-height: 1.4; 
  margin: 0; 
}

.ProseMirror section { 
  margin-bottom: 2px; 
}
```

## JavaScript Code

```javascript
import {Schema} from "prosemirror-model"
import {schema as basicSchema} from "prosemirror-schema-basic"
import {Plugin} from "prosemirror-state"
import {Decoration, DecorationSet} from "prosemirror-view"
import {Selection} from "prosemirror-state"
import {EditorState} from "prosemirror-state"
import {EditorView} from "prosemirror-view"
import {baseKeymap} from "prosemirror-commands"
import {keymap} from "prosemirror-keymap"

// Custom Schema with sections
const schema = new Schema({
  nodes: basicSchema.spec.nodes.append({
    doc: {
      content: "section+"
    },
    section: {
      content: "heading block+",
      parseDOM: [{tag: "section"}],
      toDOM() { return ["section", 0] }
    }
  }),
  marks: basicSchema.spec.marks
})

// Section Node View
class SectionView {
  constructor(node, view, getPos, deco) {
    this.dom = document.createElement("section")
    this.header = this.dom.appendChild(document.createElement("header"))
    this.header.contentEditable = "false" 
    this.foldButton = this.header.appendChild(document.createElement("button"))
    this.foldButton.title = "Toggle section folding"
    this.foldButton.onmousedown = e => this.foldClick(view, getPos, e)
    this.contentDOM = this.dom.appendChild(document.createElement("div"))
    this.setFolded(deco.some(d => d.spec.foldSection))
  }

  setFolded(folded) {
    this.folded = folded
    this.foldButton.textContent = folded ? "▿" : "▵"
    this.contentDOM.style.display = folded ? "none" : ""
  }

  update(node, deco) {
    if (node.type.name != "section") return false
    let folded = deco.some(d => d.spec.foldSection)
    if (folded != this.folded) this.setFolded(folded)
    return true
  }

  foldClick(view, getPos, event) {
    event.preventDefault()
    setFolding(view, getPos(), !this.folded)
  }
}

// Folding Plugin
const foldPlugin = new Plugin({
  state: {
    init() { return DecorationSet.empty },
    apply(tr, value) {
      value = value.map(tr.mapping, tr.doc)
      let update = tr.getMeta(foldPlugin)
      if (update && update.fold) {
        let node = tr.doc.nodeAt(update.pos)
        if (node && node.type.name == "section")
          value = value.add(tr.doc, [Decoration.node(update.pos, update.pos + node.nodeSize, {}, {foldSection: true})])
      } else if (update) {
        let found = value.find(update.pos + 1, update.pos + 1)
        if (found.length) value = value.remove(found)
      }
      return value
    }
  },
  props: {
    decorations: state => foldPlugin.getState(state),
    nodeViews: {section: (node, view, getPos, decorations) => new SectionView(node, view, getPos, decorations)}
  }
})

// Helper function to toggle folding
function setFolding(view, pos, fold) {
  let section = view.state.doc.nodeAt(pos)
  if (section && section.type.name == "section") {
    let tr = view.state.tr.setMeta(foldPlugin, {pos, fold})
    let {from, to} = view.state.selection, endPos = pos + section.nodeSize
    if (from < endPos && to > pos) {
      let newSel = Selection.findFrom(view.state.doc.resolve(endPos), 1) ||
        Selection.findFrom(view.state.doc.resolve(pos), -1)
      if (newSel) tr.setSelection(newSel)
    }
    view.dispatch(tr)
  }
}

// Initialize editor
window.view = new EditorView(document.querySelector("#editor"), {
  state: EditorState.create({
    schema,
    plugins: [keymap(baseKeymap), foldPlugin]
  })
})
```

## Key Concepts

### Node Decorations
- Used to attach metadata to nodes without modifying the document
- The `foldSection` decoration spec indicates whether a section is folded
- Decorations persist across document changes using mapping

### Node Views
- Custom rendering for specific node types
- `SectionView` creates a custom header with fold button
- `contentDOM` property tells ProseMirror where to render child content
- Non-editable header prevents text selection in the control area

### Plugin State Management
- Plugin maintains `DecorationSet` as its state
- Transactions can include meta information to trigger folding/unfolding
- State updates are handled in the `apply` method

### Selection Handling
When folding a section that contains the selection:
- Moves selection outside the folded section
- Tries to place it after the section first, then before if needed
- Prevents selection from being "trapped" in hidden content

### Architecture Benefits
- Document structure remains unchanged when folding
- Folding state is purely presentational
- Content remains in the document and can be searched/manipulated
- Clean separation between document model and view layer

## Usage
- Click the arrow button in section headers to fold/unfold
- Folded sections hide their content but remain in the document
- Selection automatically moves when folding would hide it
- Multiple sections can be independently folded