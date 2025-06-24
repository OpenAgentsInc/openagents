# Collaborative Editing in ProseMirror

## Overview

Collaborative editing allows multiple people to edit the same document simultaneously. The key principles are:

- Changes are applied immediately to the local document
- Changes are sent to peers
- Changes are merged automatically without manual conflict resolution

## Collaborative Editing Algorithm

The system uses a central authority to determine the order of changes:

1. Editors submit changes to the central authority
2. The authority accepts changes from one editor
3. Accepted changes are broadcast to all editors
4. Concurrent changes are rebased and resubmitted

## Authority Implementation

Here's a basic implementation of a central authority:

```javascript
class Authority {
  constructor(doc) {
    this.doc = doc
    this.steps = []
    this.stepClientIDs = []
    this.onNewSteps = []
  }

  receiveSteps(version, steps, clientID) {
    if (version != this.steps.length) return

    steps.forEach(step => {
      this.doc = step.apply(this.doc).doc
      this.steps.push(step)
      this.stepClientIDs.push(clientID)
    })
    this.onNewSteps.forEach(function(f) { f() })
  }

  stepsSince(version) {
    return {
      steps: this.steps.slice(version),
      clientIDs: this.stepClientIDs.slice(version)
    }
  }
}
```

## Collab Module

The `prosemirror-collab` module provides a plugin to manage collaborative editing:

```javascript
import {collab, sendableSteps, receiveTransaction} from "prosemirror-collab"

function collabEditor(authority, place) {
  let view = new EditorView(place, {
    state: EditorState.create({
      doc: authority.doc,
      plugins: [collab({version: authority.steps.length})]
    }),
    dispatchTransaction(transaction) {
      let newState = view.state.apply(transaction)
      view.updateState(newState)
      let sendable = sendableSteps(newState)
      if (sendable)
        authority.receiveSteps(sendable.version, sendable.steps, sendable.clientID)
    }
  })

  authority.onNewSteps.push(function() {
    let newData = authority.stepsSince(collab.getVersion(view.state))
    view.dispatch(
      receiveTransaction(view.state, newData.steps, newData.clientIDs)
    )
  })

  return view
}
```

## Key Concepts

### Version Tracking
The collab module tracks document versions to ensure steps are applied in the correct order:

```javascript
// Get current version
let version = collab.getVersion(state)

// Initialize with version
let plugin = collab({version: 42})
```

### Sendable Steps
Extract steps that need to be sent to the authority:

```javascript
let sendable = sendableSteps(state)
if (sendable) {
  // sendable.version - the version these steps start from
  // sendable.steps - array of steps to send
  // sendable.clientID - unique ID for this client
  sendToAuthority(sendable)
}
```

### Receiving Steps
Apply steps received from other clients:

```javascript
function applyRemoteSteps(view, steps, clientIDs) {
  let transaction = receiveTransaction(
    view.state,
    steps,
    clientIDs
  )
  view.dispatch(transaction)
}
```

## Full Example

Here's a complete example of setting up collaborative editing:

```javascript
import {EditorState} from "prosemirror-state"
import {EditorView} from "prosemirror-view"
import {schema} from "prosemirror-schema-basic"
import {collab, sendableSteps, receiveTransaction} from "prosemirror-collab"

// Central authority
class CollabAuthority {
  constructor(doc) {
    this.doc = doc
    this.steps = []
    this.stepClientIDs = []
    this.listeners = []
  }

  receiveSteps(version, steps, clientID) {
    // Reject if version doesn't match
    if (version !== this.steps.length) {
      return {status: "rejected"}
    }

    // Apply and store steps
    steps.forEach(step => {
      let result = step.apply(this.doc)
      if (result.failed) {
        return {status: "rejected"}
      }
      this.doc = result.doc
      this.steps.push(step)
      this.stepClientIDs.push(clientID)
    })

    // Notify listeners
    this.listeners.forEach(fn => fn())
    return {status: "accepted"}
  }

  stepsSince(version) {
    return {
      steps: this.steps.slice(version),
      clientIDs: this.stepClientIDs.slice(version)
    }
  }

  subscribe(fn) {
    this.listeners.push(fn)
  }
}

// Create collaborative editor
function createCollabEditor(authority, mountPoint) {
  let state = EditorState.create({
    doc: authority.doc,
    plugins: [
      collab({
        version: authority.steps.length
      })
    ]
  })

  let view = new EditorView(mountPoint, {
    state,
    dispatchTransaction(tr) {
      let newState = view.state.apply(tr)
      view.updateState(newState)

      // Send local changes
      let sendable = sendableSteps(newState)
      if (sendable) {
        authority.receiveSteps(
          sendable.version,
          sendable.steps,
          sendable.clientID
        )
      }
    }
  })

  // Receive remote changes
  authority.subscribe(() => {
    let version = collab.getVersion(view.state)
    let {steps, clientIDs} = authority.stepsSince(version)
    if (steps.length) {
      view.dispatch(
        receiveTransaction(view.state, steps, clientIDs)
      )
    }
  })

  return view
}
```

## Network Transport

In practice, you'll need to send steps over a network:

```javascript
// WebSocket example
class NetworkedAuthority {
  constructor(doc, socket) {
    this.doc = doc
    this.socket = socket
    this.version = 0
    
    socket.on('steps', data => {
      this.receiveSteps(data.version, data.steps, data.clientID)
    })
  }

  sendSteps(version, steps, clientID) {
    this.socket.emit('steps', {
      version,
      steps: steps.map(s => s.toJSON()),
      clientID
    })
  }

  // ... rest of authority implementation
}
```

## Handling Conflicts

The collab module automatically handles conflicts through rebasing:

1. Local changes are applied immediately
2. If remote changes arrive first, local changes are rebased
3. Rebased changes are resubmitted
4. Process continues until all changes are accepted

## Optimizations

### Compression
Store compressed step history:

```javascript
import {compress, uncompress} from "prosemirror-compress-steps"

// Compress steps for storage
let compressed = compress(steps)

// Uncompress when needed
let steps = uncompress(compressed, schema)
```

### Debouncing
Batch steps before sending:

```javascript
let timeout
function debouncedSend(view) {
  clearTimeout(timeout)
  timeout = setTimeout(() => {
    let sendable = sendableSteps(view.state)
    if (sendable) {
      sendToServer(sendable)
    }
  }, 250)
}
```

## Best Practices

1. **Keep authority simple** - It should only order steps, not validate content
2. **Handle network failures** - Implement reconnection and step resending
3. **Use unique client IDs** - Essential for tracking step origins
4. **Compress old steps** - Reduce memory usage for long editing sessions
5. **Consider cursor positions** - Share selections for better collaboration UX

## Advanced Features

### Shared Cursors
```javascript
// Plugin to share cursor positions
import {Plugin} from "prosemirror-state"
import {Decoration, DecorationSet} from "prosemirror-view"

const cursorsPlugin = new Plugin({
  state: {
    init() { return {cursors: []} },
    apply(tr, value) {
      // Update cursor positions based on transaction
      return value
    }
  },
  props: {
    decorations(state) {
      // Create decorations for remote cursors
      return DecorationSet.create(state.doc, [
        // ... cursor decorations
      ])
    }
  }
})
```

### Presence Awareness
Track who's currently editing:

```javascript
class PresenceAuthority extends Authority {
  constructor(doc) {
    super(doc)
    this.presence = new Map()
  }

  updatePresence(clientID, data) {
    this.presence.set(clientID, {
      ...data,
      lastSeen: Date.now()
    })
  }

  getActiveUsers() {
    const now = Date.now()
    return Array.from(this.presence.entries())
      .filter(([_, data]) => now - data.lastSeen < 30000)
      .map(([id, data]) => ({id, ...data}))
  }
}
```