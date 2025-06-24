# ProseMirror Collab Module Reference

## Overview

The prosemirror-collab module provides collaborative editing functionality for ProseMirror. It tracks document changes with version numbers and provides utilities for managing distributed updates between multiple editors.

## Installation

```bash
npm install prosemirror-collab
```

## Core Functions

### collab(config)

Creates a collaboration plugin.

```javascript
import {collab} from 'prosemirror-collab'

const collabPlugin = collab({
  version: 0,                    // Starting document version
  clientID: 'user-' + Date.now() // Unique client identifier
})
```

Configuration:
- `version`: Starting version number for the document
- `clientID`: Unique identifier for this editor instance (optional, auto-generated if not provided)

### sendableSteps(state)

Get steps that can be sent to other clients.

```javascript
import {sendableSteps} from 'prosemirror-collab'

const sendable = sendableSteps(state)
if (sendable) {
  const {version, steps, clientID, origins} = sendable
  // Send to server:
  // - version: document version these steps apply to
  // - steps: array of steps to apply
  // - clientID: ID of client that created steps
  // - origins: transaction origins for each step
}
```

### getVersion(state)

Get the current version number.

```javascript
import {getVersion} from 'prosemirror-collab'

const currentVersion = getVersion(state)
console.log('Document at version:', currentVersion)
```

### receiveTransaction(state, steps, clientIDs, options)

Apply steps received from other clients.

```javascript
import {receiveTransaction} from 'prosemirror-collab'

// Receive steps from server
const steps = stepsFromJSON(schema, stepsJSON)
const clientIDs = steps.map(() => otherClientID)

const transaction = receiveTransaction(
  state,
  steps,
  clientIDs,
  {version: serverVersion} // Optional: specify version
)

// Apply the transaction
const newState = state.apply(transaction)
```

Options:
- `version`: Version number to set after applying steps

## Plugin Metadata

The collab plugin uses transaction metadata:

```javascript
// Check if transaction is remote
if (tr.getMeta(collabKey)) {
  // This transaction came from receiveTransaction
}

// Mark transaction as rebaseable
tr.setMeta('rebaseable', true)
```

## Usage Examples

### Basic Collaborative Setup

```javascript
import {EditorState} from 'prosemirror-state'
import {collab, sendableSteps, getVersion, receiveTransaction} from 'prosemirror-collab'

// Initialize editor with collab
const state = EditorState.create({
  schema,
  plugins: [
    collab({version: 0})
  ]
})

// Handle local changes
function handleLocalChange(newState, oldState) {
  const sendable = sendableSteps(newState)
  if (sendable) {
    // Send to server
    sendToServer({
      version: sendable.version,
      steps: sendable.steps.map(s => s.toJSON()),
      clientID: sendable.clientID
    })
  }
}

// Handle remote changes
function handleRemoteSteps(stepsJSON, clientID, version) {
  const steps = stepsJSON.map(json => Step.fromJSON(schema, json))
  const tr = receiveTransaction(
    view.state,
    steps,
    steps.map(() => clientID),
    {version}
  )
  view.dispatch(tr)
}
```

### WebSocket Collaboration

```javascript
class CollaborationClient {
  constructor(view, websocketUrl) {
    this.view = view
    this.ws = new WebSocket(websocketUrl)
    this.clientID = Math.random().toString(36).slice(2)
    
    this.ws.onmessage = this.handleMessage.bind(this)
    this.ws.onopen = this.handleOpen.bind(this)
  }
  
  handleOpen() {
    // Request current document state
    this.ws.send(JSON.stringify({
      type: 'join',
      clientID: this.clientID
    }))
  }
  
  handleMessage(event) {
    const data = JSON.parse(event.data)
    
    switch (data.type) {
      case 'document':
        // Initial document sync
        const state = EditorState.fromJSON(
          {schema, plugins: this.view.state.plugins},
          data.doc
        )
        this.view.updateState(state)
        break
        
      case 'steps':
        // Receive steps from other clients
        if (data.clientID !== this.clientID) {
          const steps = data.steps.map(json => Step.fromJSON(schema, json))
          const tr = receiveTransaction(
            this.view.state,
            steps,
            steps.map(() => data.clientID),
            {version: data.version}
          )
          this.view.dispatch(tr)
        }
        break
    }
  }
  
  sendSteps() {
    const sendable = sendableSteps(this.view.state)
    if (sendable) {
      this.ws.send(JSON.stringify({
        type: 'steps',
        version: sendable.version,
        steps: sendable.steps.map(s => s.toJSON()),
        clientID: sendable.clientID
      }))
    }
  }
}

// Initialize collaboration
const collabClient = new CollaborationClient(view, 'ws://localhost:8080')

// Send steps after local changes
view.setProps({
  dispatchTransaction(tr) {
    const newState = view.state.apply(tr)
    view.updateState(newState)
    
    if (!tr.getMeta(collabKey)) {
      // Local change, send to server
      collabClient.sendSteps()
    }
  }
})
```

### Conflict Resolution

```javascript
// Server-side conflict resolution
class CollabServer {
  constructor(schema) {
    this.schema = schema
    this.version = 0
    this.steps = []
    this.doc = schema.nodes.doc.createAndFill()
    this.stepClientIDs = []
  }
  
  receiveSteps(version, stepsJSON, clientID) {
    // Check if steps are based on current version
    if (version !== this.version) {
      // Client is behind, needs to rebase
      return {
        success: false,
        version: this.version,
        steps: this.steps.slice(version).map(s => s.toJSON()),
        clientIDs: this.stepClientIDs.slice(version)
      }
    }
    
    // Apply steps
    const steps = stepsJSON.map(json => Step.fromJSON(this.schema, json))
    const result = this.applySteps(steps, clientID)
    
    if (result.failed) {
      // Steps don't apply cleanly
      return {success: false, error: 'Steps could not be applied'}
    }
    
    // Broadcast to other clients
    return {
      success: true,
      version: this.version,
      broadcast: {
        version: this.version - steps.length,
        steps: stepsJSON,
        clientID
      }
    }
  }
  
  applySteps(steps, clientID) {
    let doc = this.doc
    const failed = []
    
    for (let i = 0; i < steps.length; i++) {
      const result = steps[i].apply(doc)
      if (result.doc) {
        doc = result.doc
        this.version++
        this.steps.push(steps[i])
        this.stepClientIDs.push(clientID)
      } else {
        failed.push(i)
      }
    }
    
    this.doc = doc
    return {failed: failed.length ? failed : null}
  }
}
```

### Authority Server

```javascript
// Central authority pattern
class AuthorityServer {
  constructor(doc, schema) {
    this.doc = doc
    this.schema = schema
    this.version = 0
    this.clients = new Map()
    this.pending = []
  }
  
  addClient(clientID, connection) {
    this.clients.set(clientID, {
      connection,
      version: 0
    })
    
    // Send current document
    connection.send({
      type: 'init',
      version: this.version,
      doc: this.doc.toJSON()
    })
  }
  
  handleSteps(clientID, {version, steps}) {
    const client = this.clients.get(clientID)
    
    // Queue if client is behind
    if (version < this.version) {
      this.pending.push({clientID, version, steps})
      this.processPending()
      return
    }
    
    // Apply steps
    const result = this.applySteps(steps)
    if (result.ok) {
      client.version = this.version
      
      // Broadcast to all clients
      this.broadcast({
        type: 'steps',
        version: result.version,
        steps: steps,
        clientID: clientID
      }, clientID)
    }
  }
  
  processPending() {
    // Try to apply pending steps
    this.pending = this.pending.filter(({clientID, version, steps}) => {
      if (version === this.version) {
        this.handleSteps(clientID, {version, steps})
        return false
      }
      return version > this.version
    })
  }
  
  broadcast(message, excludeClient) {
    this.clients.forEach((client, clientID) => {
      if (clientID !== excludeClient) {
        client.connection.send(message)
      }
    })
  }
}
```

### Offline Support

```javascript
// Client with offline queue
class OfflineAwareCollab {
  constructor(view) {
    this.view = view
    this.online = navigator.onLine
    this.queue = []
    
    window.addEventListener('online', () => this.goOnline())
    window.addEventListener('offline', () => this.goOffline())
  }
  
  sendSteps(sendable) {
    if (this.online) {
      // Try to send immediately
      fetch('/collab/steps', {
        method: 'POST',
        body: JSON.stringify({
          version: sendable.version,
          steps: sendable.steps.map(s => s.toJSON()),
          clientID: sendable.clientID
        })
      }).catch(() => {
        // Failed, queue for later
        this.queueSteps(sendable)
      })
    } else {
      // Queue for when online
      this.queueSteps(sendable)
    }
  }
  
  queueSteps(sendable) {
    this.queue.push(sendable)
    // Persist queue to localStorage
    localStorage.setItem('collab-queue', JSON.stringify(this.queue))
  }
  
  goOnline() {
    this.online = true
    // Process queued steps
    this.processQueue()
  }
  
  goOffline() {
    this.online = false
  }
  
  async processQueue() {
    const queue = [...this.queue]
    this.queue = []
    
    for (const sendable of queue) {
      try {
        await this.sendSteps(sendable)
      } catch (error) {
        // Re-queue on failure
        this.queue.push(sendable)
      }
    }
  }
}
```

### Presence and Cursors

```javascript
// Track user presence
class PresencePlugin {
  constructor(clientID, color) {
    this.clientID = clientID
    this.color = color
    this.decorations = DecorationSet.empty
    this.presences = new Map()
  }
  
  plugin() {
    return new Plugin({
      state: {
        init: () => this,
        apply: (tr, pluginState) => {
          // Update local selection
          if (tr.selectionSet && !tr.getMeta(collabKey)) {
            this.broadcastPresence(tr.selection)
          }
          
          // Update decorations
          const presence = tr.getMeta('presence')
          if (presence) {
            pluginState.updatePresence(presence)
          }
          
          return pluginState
        }
      },
      props: {
        decorations: (state) => {
          return this.getState(state).decorations
        }
      }
    })
  }
  
  updatePresence({clientID, selection}) {
    if (selection) {
      this.presences.set(clientID, selection)
    } else {
      this.presences.delete(clientID)
    }
    
    this.updateDecorations()
  }
  
  updateDecorations() {
    const decorations = []
    
    this.presences.forEach((selection, clientID) => {
      const color = this.getClientColor(clientID)
      
      // Add cursor decoration
      if (selection.empty) {
        decorations.push(
          Decoration.widget(selection.head, () => {
            const cursor = document.createElement('span')
            cursor.className = 'collab-cursor'
            cursor.style.borderColor = color
            return cursor
          }, {
            side: 1,
            key: `cursor-${clientID}`
          })
        )
      } else {
        // Add selection decoration
        decorations.push(
          Decoration.inline(
            selection.from,
            selection.to,
            {
              class: 'collab-selection',
              style: `background-color: ${color}40`
            },
            {key: `selection-${clientID}`}
          )
        )
      }
    })
    
    this.decorations = DecorationSet.create(this.view.state.doc, decorations)
  }
}
```

### Version Recovery

```javascript
// Recover from version mismatch
class VersionRecovery {
  constructor(view) {
    this.view = view
    this.retryCount = 0
    this.maxRetries = 3
  }
  
  async sendSteps(sendable) {
    try {
      const response = await fetch('/collab/steps', {
        method: 'POST',
        body: JSON.stringify(sendable)
      })
      
      const result = await response.json()
      
      if (!result.success && result.version !== undefined) {
        // Version mismatch, need to sync
        await this.syncToVersion(result.version, result.steps)
        
        // Retry sending
        if (this.retryCount < this.maxRetries) {
          this.retryCount++
          const newSendable = sendableSteps(this.view.state)
          if (newSendable) {
            return this.sendSteps(newSendable)
          }
        }
      }
      
      this.retryCount = 0
    } catch (error) {
      console.error('Failed to send steps:', error)
    }
  }
  
  async syncToVersion(version, stepsJSON) {
    // Apply missing steps
    const steps = stepsJSON.map(json => Step.fromJSON(schema, json))
    const tr = receiveTransaction(
      this.view.state,
      steps,
      steps.map(() => 'server'),
      {version}
    )
    
    this.view.dispatch(tr)
  }
}
```

## Best Practices

1. **Use unique client IDs**: Generate stable, unique IDs for each client
2. **Handle version conflicts**: Implement proper version checking and recovery
3. **Queue offline changes**: Store changes locally when offline
4. **Implement presence**: Show other users' cursors and selections
5. **Optimize step transmission**: Batch steps to reduce network traffic
6. **Handle connection failures**: Implement reconnection logic
7. **Validate steps server-side**: Don't trust client-submitted steps

## Integration Example

```javascript
import {EditorState} from 'prosemirror-state'
import {EditorView} from 'prosemirror-view'
import {collab, sendableSteps, receiveTransaction} from 'prosemirror-collab'
import {Step} from 'prosemirror-transform'

// Complete collaborative editor setup
function createCollaborativeEditor(place, schema, docID) {
  const clientID = 'client-' + Math.random().toString(36).slice(2)
  
  const state = EditorState.create({
    schema,
    plugins: [
      collab({version: 0, clientID})
    ]
  })
  
  const view = new EditorView(place, {
    state,
    dispatchTransaction(tr) {
      const newState = view.state.apply(tr)
      view.updateState(newState)
      
      // Send local changes
      if (!tr.getMeta(collabKey)) {
        const sendable = sendableSteps(newState)
        if (sendable) {
          socket.emit('steps', {
            docID,
            ...sendable,
            steps: sendable.steps.map(s => s.toJSON())
          })
        }
      }
    }
  })
  
  // Socket connection
  const socket = io('/collab')
  
  socket.on('steps', ({version, steps: stepsJSON, clientID: fromClient}) => {
    const steps = stepsJSON.map(json => Step.fromJSON(schema, json))
    const tr = receiveTransaction(
      view.state,
      steps,
      steps.map(() => fromClient),
      {version}
    )
    view.dispatch(tr)
  })
  
  socket.on('init', ({version, doc}) => {
    const state = EditorState.fromJSON(
      {schema, plugins: view.state.plugins},
      {doc, selection: view.state.selection}
    )
    view.updateState(state)
  })
  
  // Join document
  socket.emit('join', {docID, clientID})
  
  return {view, socket}
}
```

This module enables real-time collaborative editing in ProseMirror applications.