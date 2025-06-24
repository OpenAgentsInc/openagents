# ProseMirror Collaborative Editing Example

## Overview

This example demonstrates real-time collaborative editing using ProseMirror, featuring:
- Synchronized document editing across multiple users
- Out-of-line annotations with text selection and commenting functionality
- Server-side collaboration service

## Description

The collaborative editing demo shows how multiple users can edit the same document simultaneously with changes synchronized in real-time. Users can:

1. **Edit together**: All connected users see the same document and changes appear instantly
2. **Add annotations**: Select text and click the speech bubble icon to add comments
3. **See other users**: Annotations and edits from other users are visible

## Key Concepts

### Collaborative Architecture

While the full implementation is not shown, collaborative editing in ProseMirror typically involves:

```javascript
// Conceptual overview of collaborative editing setup
import {collab, receiveTransaction, sendableSteps} from "prosemirror-collab"

// Plugin for collaborative editing
const collabPlugin = collab({
  version: 0,  // Document version
  clientID: Math.floor(Math.random() * 0xFFFFFFFF)  // Unique client ID
})

// Handle incoming changes from server
function receiveUpdates(state, updates) {
  let tr = receiveTransaction(
    state,
    updates.steps,     // Steps from other users
    updates.clientIDs  // Client IDs for each step
  )
  return view.updateState(state.apply(tr))
}

// Send local changes to server
function sendUpdates(state) {
  let sendable = sendableSteps(state)
  if (sendable) {
    // Send to server: sendable.steps, sendable.clientID
    server.send({
      version: sendable.version,
      steps: sendable.steps.map(s => s.toJSON()),
      clientID: sendable.clientID
    })
  }
}
```

### Annotation System

Annotations are typically implemented using:

```javascript
// Conceptual annotation implementation
class AnnotationPlugin {
  constructor() {
    this.annotations = []
  }

  addAnnotation(from, to, text) {
    this.annotations.push({
      id: generateID(),
      from,
      to,
      text,
      author: currentUser
    })
  }

  decorations(state) {
    return DecorationSet.create(state.doc, 
      this.annotations.map(ann => 
        Decoration.inline(ann.from, ann.to, {
          class: "comment",
          annotation: ann
        })
      )
    )
  }
}
```

## CSS Styles

```css
.comment { 
  background-color: #ff8; 
}

.currentComment { 
  background-color: #fe0; 
}

/* Additional styles for collaborative features */
.ProseMirror-selectednode {
  outline: 2px solid #8cf;
}

.tooltip {
  position: absolute;
  pointer-events: none;
  z-index: 20;
  background: white;
  border: 1px solid silver;
  border-radius: 2px;
  padding: 2px 10px;
  margin-bottom: 7px;
  transform: translateX(-50%);
}

.tooltip:before {
  content: "";
  height: 0;
  width: 0;
  position: absolute;
  left: 50%;
  margin-left: -5px;
  bottom: -6px;
  border: 5px solid transparent;
  border-bottom-width: 0;
  border-top-color: silver;
}
```

## Implementation Requirements

A complete collaborative editing system requires:

### Client-Side
1. **Collaboration plugin**: Track document version and client ID
2. **WebSocket connection**: Real-time communication with server
3. **Conflict resolution**: Operational transformation or CRDT
4. **Annotation management**: Store and sync annotations
5. **User presence**: Show active users and cursors

### Server-Side
1. **Document storage**: Persist current document state
2. **Version control**: Track document versions
3. **Step relay**: Broadcast changes to all clients
4. **Conflict resolution**: Rebase conflicting changes
5. **Connection management**: Handle user sessions

## Architecture Flow

```
User A Edit → Local Transform → Send to Server
                                      ↓
User B Edit → Local Transform → Server Rebases → Broadcast to All
                                      ↓
                              Apply Remote Changes → Update Views
```

## Advanced Features

### Presence Awareness
```javascript
// Show other users' cursors
const presencePlugin = new Plugin({
  state: {
    init: () => ({ users: {} }),
    apply(tr, value) {
      // Update user positions
      let meta = tr.getMeta(presencePlugin)
      if (meta) {
        value = {...value}
        value.users[meta.user] = meta.selection
      }
      return value
    }
  },
  props: {
    decorations(state) {
      // Create cursor decorations for other users
    }
  }
})
```

### Offline Support
- Queue changes when offline
- Sync when connection restored
- Handle conflicts on reconnection

## Resources

- Full implementation: [GitHub Repository](https://github.com/ProseMirror/website/tree/master/src/collab/)
- Related packages:
  - `prosemirror-collab`: Collaboration plugin
  - `prosemirror-transform`: Operational transformation
  - WebSocket libraries for real-time communication

## Note

Since this is a public demo, content moderation may be necessary in production implementations to handle inappropriate content from users.