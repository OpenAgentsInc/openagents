# ProseMirror Lint Example

## Overview

This example demonstrates a document linter for ProseMirror that can:
- Find problems in a document
- Highlight issues with decorations
- Provide quick fixes for common problems
- Show interactive warning icons

## CSS Styles

```css
#editor { position: relative }
.problem { 
  background: #fdd; 
  border-bottom: 1px solid #f22; 
  margin-bottom: -1px; 
}
.lint-icon { 
  display: inline-block; 
  position: absolute; 
  right: 2px; 
  cursor: pointer; 
  border-radius: 100px; 
  background: #f22; 
  color: white; 
  font-family: times, georgia, serif; 
  font-size: 15px; 
  font-weight: bold; 
  width: 1.1em; 
  height: 1.1em; 
  text-align: center; 
  padding-left: .5px; 
  line-height: 1.1em 
}
.lint-icon:before { content: "!" }
.ProseMirror { padding-right: 20px }
```

## JavaScript Code

```javascript
import {Decoration, DecorationSet} from "prosemirror-view"
import {Plugin} from "prosemirror-state"
import {TextSelection} from "prosemirror-state"

// Words you probably shouldn't use
const badWords = /\b(obviously|clearly|evidently|simply)\b/ig
// Matches punctuation with a space before it
const badPunc = / ([,\.!?:]) ?/g

function lint(doc) {
  let result = [], lastHeadLevel = null

  function record(msg, from, to, fix) {
    result.push({msg, from, to, fix})
  }

  // For each node in the document
  doc.descendants((node, pos) => {
    if (node.isText) {
      // Scan text nodes for suspicious patterns
      let m
      while (m = badWords.exec(node.text))
        record(`Try not to say '${m[0]}'`,
               pos + m.index, pos + m.index + m[0].length)
      
      // Check punctuation
      while (m = badPunc.exec(node.text))
        record("Suspicious spacing around punctuation",
               pos + m.index, pos + m.index + m[0].length,
               fixPunc(m[1] + " "))
    } else if (node.type.name == "heading") {
      // Check heading levels
      let level = node.attrs.level
      if (lastHeadLevel != null && level > lastHeadLevel + 1)
        record(`Heading too small (${level} under ${lastHeadLevel})`,
               pos + 1, pos + 1 + node.content.size,
               fixHeader(lastHeadLevel + 1))
      lastHeadLevel = level
    } else if (node.type.name == "image" && !node.attrs.alt) {
      // Ensure images have alt text
      record("Image without alt text", pos, pos + 1, addAlt)
    }
  })

  return result
}

// Fix functions
function fixPunc(replacement) {
  return function({state, dispatch}) {
    dispatch(state.tr.replaceWith(this.from, this.to,
                                  state.schema.text(replacement)))
  }
}

function fixHeader(level) {
  return function({state, dispatch}) {
    dispatch(state.tr.setNodeMarkup(this.from - 1, null, {level}))
  }
}

function addAlt({state, dispatch}) {
  let alt = prompt("Alt text", "")
  if (alt) {
    let attrs = Object.assign({}, state.doc.nodeAt(this.from).attrs, {alt})
    dispatch(state.tr.setNodeMarkup(this.from, null, attrs))
  }
}

// Decoration functions
function lintDeco(doc) {
  let decos = []
  lint(doc).forEach(prob => {
    decos.push(
      Decoration.inline(prob.from, prob.to, {class: "problem"}),
      Decoration.widget(prob.from, lintIcon(prob), {key: prob.msg})
    )
  })
  return DecorationSet.create(doc, decos)
}

function lintIcon(prob) {
  return () => {
    let icon = document.createElement("div")
    icon.className = "lint-icon"
    icon.title = prob.msg
    icon.problem = prob
    return icon
  }
}

// Lint plugin
let lintPlugin = new Plugin({
  state: {
    init(_, {doc}) { return lintDeco(doc) },
    apply(tr, old) { return tr.docChanged ? lintDeco(tr.doc) : old }
  },
  props: {
    decorations(state) { return this.getState(state) },
    handleClick(view, _, event) {
      if (/lint-icon/.test(event.target.className)) {
        let {from, to} = event.target.problem
        view.dispatch(
          view.state.tr
            .setSelection(TextSelection.create(view.state.doc, from, to))
            .scrollIntoView())
        return true
      }
    },
    handleDoubleClick(view, _, event) {
      if (/lint-icon/.test(event.target.className)) {
        let prob = event.target.problem
        if (prob.fix) {
          prob.fix(view)
          view.focus()
          return true
        }
      }
    }
  }
})

// Initialize editor with lint plugin
import {EditorState} from "prosemirror-state"
import {EditorView} from "prosemirror-view"
import {schema} from "prosemirror-schema-basic"
import {exampleSetup} from "prosemirror-example-setup"

window.view = new EditorView(document.querySelector("#editor"), {
  state: EditorState.create({
    schema,
    plugins: exampleSetup({schema}).concat(lintPlugin)
  })
})
```

## Linting Rules

### 1. Word Choice
Detects potentially weak words:
- "obviously"
- "clearly" 
- "evidently"
- "simply"

### 2. Punctuation Spacing
Finds spaces before punctuation marks (incorrect in English):
- Space before comma: ` ,`
- Space before period: ` .`
- Space before exclamation: ` !`
- Space before question mark: ` ?`
- Space before colon: ` :`

### 3. Heading Structure
Ensures heading levels don't skip:
- Can't have H3 directly after H1
- Must increment by at most one level

### 4. Image Accessibility
Checks that all images have alt text for accessibility

## User Interaction

### Visual Feedback
- **Problem highlighting**: Pink background with red underline
- **Warning icons**: Red circles with exclamation marks
- **Tooltips**: Hover over icons to see issue description

### Fixing Problems
- **Single click on icon**: Selects the problematic text
- **Double click on icon**: Applies automatic fix (if available)
- **Manual fixing**: Edit the highlighted text directly

## Architecture

### Plugin State
- Recalculates decorations when document changes
- Maintains decoration set for efficient updates
- Only re-lints when document is modified

### Decoration Strategy
- **Inline decorations**: Highlight problematic text
- **Widget decorations**: Add interactive warning icons
- Icons positioned absolutely to avoid affecting layout

### Fix Functions
- Return functions that can be called with view context
- Use transactions to make changes
- Some fixes are automatic (punctuation), others interactive (alt text)

## Extensibility

To add new lint rules:
1. Add pattern matching in the `lint` function
2. Call `record()` with problem details
3. Optionally provide a fix function
4. The plugin will automatically handle decoration and interaction