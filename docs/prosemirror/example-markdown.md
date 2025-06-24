# ProseMirror Markdown Example

## Overview

This example demonstrates a flexible Markdown editing interface that allows users to switch between a plain textarea and a rich ProseMirror editor while maintaining Markdown content.

## HTML Structure

```html
<div id="editor"></div>
<textarea id="content">
This is a comment written in [Markdown](http://commonmark.org). *You* may know the syntax...
</textarea>

<input type="radio" name="editor" value="markdown">
<input type="radio" name="editor" value="prosemirror">
```

## CSS

```css
.ProseMirror { 
  height: 120px; 
  overflow-y: auto; 
  box-sizing: border-box; 
}

textarea { 
  width: 100%; 
  height: 123px; 
  border: 1px solid silver; 
  box-sizing: border-box; 
}
```

## JavaScript

```javascript
import {EditorView} from "prosemirror-view"
import {EditorState} from "prosemirror-state"
import {schema, defaultMarkdownParser,
        defaultMarkdownSerializer} from "prosemirror-markdown"
import {exampleSetup} from "prosemirror-example-setup"

class MarkdownView {
  constructor(target, content) {
    this.textarea = target.appendChild(document.createElement("textarea"))
    this.textarea.value = content
  }

  get content() { return this.textarea.value }
  focus() { this.textarea.focus() }
  destroy() { this.textarea.remove() }
}

class ProseMirrorView {
  constructor(target, content) {
    this.view = new EditorView(target, {
      state: EditorState.create({
        doc: defaultMarkdownParser.parse(content),
        plugins: exampleSetup({schema})
      })
    })
  }

  get content() {
    return defaultMarkdownSerializer.serialize(this.view.state.doc)
  }
  focus() { this.view.focus() }
  destroy() { this.view.destroy() }
}

let place = document.querySelector("#editor")
let view = new MarkdownView(place, document.querySelector("#content").value)

document.querySelectorAll("input[type=radio]").forEach(button => {
  button.addEventListener("change", () => {
    if (!button.checked) return
    let View = button.value == "markdown" ? MarkdownView : ProseMirrorView
    if (view instanceof View) return
    let content = view.content
    view.destroy()
    view = new View(place, content)
    view.focus()
  })
})
```

## Key Features

- **Two-way Markdown editing**: Switch between rich editor and plain text
- **Content preservation**: Markdown content is maintained when switching views
- **ProseMirror markdown plugin**: Uses `prosemirror-markdown` for parsing and serialization
- **Clean API**: Simple interface for both editor types with consistent methods