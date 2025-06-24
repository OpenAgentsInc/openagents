# ProseMirror Upload Handling Example

## Overview

This example demonstrates how to handle asynchronous file uploads in ProseMirror, specifically image uploads, by:
- Inserting a placeholder immediately when a file is selected
- Uploading the file in the background
- Replacing the placeholder with the actual uploaded image

## HTML Structure

```html
<input type="file" id="image-upload">
<div class="ProseMirror">
  <p>This paragraph needs an image.</p>
</div>
```

## CSS Styles

```css
placeholder {
  display: inline;
  border: 1px solid #ccc;
  color: #ccc;
}

placeholder:after {
  content: "☁";
  font-size: 200%;
  line-height: 0.1;
  font-weight: bold;
}

.ProseMirror img {
  max-width: 100px;
}
```

## JavaScript Code

```javascript
import {Plugin} from "prosemirror-state"
import {Decoration, DecorationSet} from "prosemirror-view"

let placeholderPlugin = new Plugin({
  state: {
    init() { return DecorationSet.empty },
    apply(tr, set) {
      set = set.map(tr.mapping, tr.doc)
      let action = tr.getMeta(this)
      if (action && action.add) {
        let widget = document.createElement("placeholder")
        let deco = Decoration.widget(action.add.pos, widget, {id: action.add.id})
        set = set.add(tr.doc, [deco])
      } else if (action && action.remove) {
        set = set.remove(set.find(null, null,
                                  spec => spec.id == action.remove.id))
      }
      return set
    }
  },
  props: {
    decorations(state) { return this.getState(state) }
  }
})

function findPlaceholder(state, id) {
  let decos = placeholderPlugin.getState(state)
  let found = decos.find(null, null, spec => spec.id == id)
  return found.length ? found[0].from : null
}

document.querySelector("#image-upload").addEventListener("change", e => {
  if (view.state.selection.$from.parent.inlineContent && e.target.files.length)
    startImageUpload(view, e.target.files[0])
  view.focus()
})

function startImageUpload(view, file) {
  // A fresh object to act as the ID for this upload
  let id = {}

  // Replace the selection with a placeholder
  let tr = view.state.tr
  if (!tr.selection.empty) tr.deleteSelection()
  tr.setMeta(placeholderPlugin, {add: {id, pos: tr.selection.from}})
  view.dispatch(tr)

  uploadFile(file).then(url => {
    let pos = findPlaceholder(view.state, id)
    // If the content around the placeholder has been deleted, drop the image
    if (pos == null) return
    // Otherwise, insert it at the placeholder's position, and remove the placeholder
    view.dispatch(view.state.tr
                  .replaceWith(pos, pos, schema.nodes.image.create({src: url}))
                  .setMeta(placeholderPlugin, {remove: {id}}))
  }, () => {
    // On failure, just clean up the placeholder
    view.dispatch(tr.setMeta(placeholderPlugin, {remove: {id}}))
  })
}

// This function should be implemented to upload a file and return
// a promise that resolves to the uploaded file's URL.
function uploadFile(file) {
  // Implementation would go here
  // Return a promise that resolves to a URL string
}
```

## Key Concepts

### Placeholder Plugin
The plugin manages decorations that represent upload placeholders:
- **add action**: Creates a placeholder widget at a specific position
- **remove action**: Removes the placeholder when upload completes or fails
- Uses unique IDs to track individual uploads

### Upload Flow
1. User selects a file through the file input
2. A placeholder is immediately inserted at the cursor position
3. File upload begins asynchronously
4. On success: placeholder is replaced with the actual image
5. On failure: placeholder is removed

### Benefits
- **Immediate feedback**: Users see a placeholder right away
- **Non-blocking**: Upload happens in the background
- **Clean failure handling**: Placeholders are removed if upload fails
- **Maintains document flow**: Other editing can continue during upload

Note: The actual `uploadFile` implementation would depend on your backend API and file handling requirements.