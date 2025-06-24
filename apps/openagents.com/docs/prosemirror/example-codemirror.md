# ProseMirror Embedded Code Editor Example

## Overview

This example demonstrates how to create a custom node view in ProseMirror that embeds a CodeMirror editor directly into a code block, providing syntax highlighting, auto-indentation, and similar features.

## CSS Styles

```css
.CodeMirror { 
  border: 1px solid #eee; 
  height: auto; 
}

.CodeMirror pre { 
  white-space: pre !important 
}
```

## JavaScript Code

```javascript
import {EditorView as ProseMirrorView} from "prosemirror-view"
import {EditorState} from "prosemirror-state"
import {schema} from "prosemirror-schema-basic"
import {exampleSetup} from "prosemirror-example-setup"
import {
  EditorView as CodeMirror, keymap as cmKeymap, drawSelection
} from "@codemirror/view"
import {javascript} from "@codemirror/lang-javascript"
import {defaultKeymap} from "@codemirror/commands"
import {syntaxHighlighting, defaultHighlightStyle} from "@codemirror/language"
import {Selection, TextSelection} from "prosemirror-state"
import {exitCode} from "prosemirror-commands"
import {undo, redo} from "prosemirror-history"

// Custom node view for code blocks
class CodeBlockView {
  constructor(node, view, getPos) {
    this.node = node
    this.view = view
    this.getPos = getPos

    // Create CodeMirror instance
    this.cm = new CodeMirror({
      doc: this.node.textContent,
      extensions: [
        cmKeymap.of([
          ...this.codeMirrorKeymap(),
          ...defaultKeymap
        ]),
        drawSelection(),
        syntaxHighlighting(defaultHighlightStyle),
        javascript(),
        CodeMirror.updateListener.of(update => this.forwardUpdate(update))
      ]
    })

    // The editor's outer node is our DOM representation
    this.dom = this.cm.dom
    this.updating = false
  }

  forwardUpdate(update) {
    if (this.updating || !this.cm.hasFocus) return
    let offset = this.getPos() + 1, {main} = update.state.selection
    let selFrom = offset + main.from, selTo = offset + main.to
    let pmSel = this.view.state.selection
    if (update.docChanged || pmSel.from != selFrom || pmSel.to != selTo) {
      let tr = this.view.state.tr
      update.changes.iterChanges((fromA, toA, fromB, toB, text) => {
        if (text.length)
          tr.replaceWith(offset + fromA, offset + toA,
                         schema.text(text.toString()))
        else
          tr.delete(offset + fromA, offset + toA)
        offset += (toB - fromB) - (toA - fromA)
      })
      tr.setSelection(TextSelection.create(tr.doc, selFrom, selTo))
      this.view.dispatch(tr)
    }
  }

  setSelection(anchor, head) {
    this.cm.focus()
    this.updating = true
    this.cm.dispatch({selection: {anchor, head}})
    this.updating = false
  }

  codeMirrorKeymap() {
    let view = this.view
    return [
      {key: "ArrowUp", run: () => this.maybeEscape("line", -1)},
      {key: "ArrowLeft", run: () => this.maybeEscape("char", -1)},
      {key: "ArrowDown", run: () => this.maybeEscape("line", 1)},
      {key: "ArrowRight", run: () => this.maybeEscape("char", 1)},
      {key: "Ctrl-Enter", run: () => {
        if (!exitCode(view.state, view.dispatch)) return false
        view.focus()
        return true
      }},
      {key: "Ctrl-z", mac: "Cmd-z",
       run: () => undo(view.state, view.dispatch)},
      {key: "Shift-Ctrl-z", mac: "Shift-Cmd-z",
       run: () => redo(view.state, view.dispatch)},
      {key: "Ctrl-y", mac: "Cmd-y",
       run: () => redo(view.state, view.dispatch)}
    ]
  }

  maybeEscape(unit, dir) {
    let {state} = this.cm, {main} = state.selection
    if (!main.empty) return false
    if (unit == "line") main = state.doc.lineAt(main.head)
    if (dir < 0 ? main.from > 0 : main.to < state.doc.length) return false
    let targetPos = this.getPos() + (dir < 0 ? 0 : this.node.nodeSize)
    let selection = Selection.near(this.view.state.doc.resolve(targetPos), dir)
    let tr = this.view.state.tr.setSelection(selection).scrollIntoView()
    this.view.dispatch(tr)
    this.view.focus()
  }

  update(node) {
    if (node.type != this.node.type) return false
    this.node = node
    if (this.updating) return true
    let newText = node.textContent, curText = this.cm.state.doc.toString()
    if (newText != curText) {
      let start = 0, curEnd = curText.length, newEnd = newText.length
      while (start < curEnd &&
             curText.charCodeAt(start) == newText.charCodeAt(start)) {
        ++start
      }
      while (curEnd > start && newEnd > start &&
             curText.charCodeAt(curEnd - 1) == newText.charCodeAt(newEnd - 1)) {
        curEnd--
        newEnd--
      }
      this.updating = true
      this.cm.dispatch({
        changes: {
          from: start, to: curEnd,
          insert: newText.slice(start, newEnd)
        }
      })
      this.updating = false
    }
    return true
  }

  selectNode() { 
    this.cm.focus() 
  }

  deselectNode() { 
    this.cm.contentDOM.blur() 
  }

  stopEvent() { 
    return true 
  }

  destroy() {
    this.cm.destroy()
  }
}

// Initialize ProseMirror with CodeMirror integration
window.view = new ProseMirrorView(document.querySelector("#editor"), {
  state: EditorState.create({
    doc: schema.node("doc", null, [
      schema.node("paragraph", null, [schema.text("Some text")]),
      schema.node("code_block", null, [schema.text(`function hello(name) {
  return "Hello, " + name
}`)]),
      schema.node("paragraph", null, [schema.text("More text")])
    ]),
    plugins: exampleSetup({schema})
  }),
  nodeViews: {
    code_block: (node, view, getPos) => new CodeBlockView(node, view, getPos)
  }
})
```

## Key Concepts

### Node View Integration
- The `CodeBlockView` class wraps a CodeMirror instance
- The `dom` property exposes CodeMirror's DOM element to ProseMirror
- Content synchronization happens bidirectionally

### Content Synchronization
- **CodeMirror → ProseMirror**: The `forwardUpdate` method listens for CodeMirror changes and creates ProseMirror transactions
- **ProseMirror → CodeMirror**: The `update` method receives ProseMirror changes and updates CodeMirror
- A flag (`updating`) prevents infinite update loops

### Navigation
- Arrow keys at editor boundaries escape to ProseMirror
- Ctrl-Enter exits the code block entirely
- The `maybeEscape` method handles boundary detection

### Selection Management
- Selection changes in CodeMirror are forwarded to ProseMirror
- The `setSelection` method allows ProseMirror to control CodeMirror's selection
- Selection synchronization maintains consistent editing state

### Event Handling
- `stopEvent` prevents ProseMirror from handling events inside CodeMirror
- Undo/redo commands are forwarded to ProseMirror for unified history
- Focus management ensures proper editor activation

## Benefits
- Rich code editing features within structured documents
- Syntax highlighting and language-specific behaviors
- Seamless navigation between prose and code
- Unified undo/redo history
- Consistent selection and clipboard handling