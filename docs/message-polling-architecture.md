# Message Polling Architecture

## Current Implementation

The OpenAgents Tauri app currently uses a polling mechanism to fetch messages from Claude Code sessions. This document explains how it works and potential improvements.

### Polling Mechanism

1. **Polling Interval**: Messages are fetched every 50ms using `setInterval`
2. **Session Management**: Each session has a unique ID that changes from temporary to real after initialization
3. **Message Fetching**: The frontend calls the `get_messages` Tauri command repeatedly

```typescript
// Polling effect in App.tsx
useEffect(() => {
  const fetchAllMessages = async () => {
    await Promise.all(sessions.map(session => {
      if (!session.isInitializing) {
        return fetchMessages(session.id);
      }
    }));
  };

  const interval = setInterval(fetchAllMessages, 50);
  return () => clearInterval(interval);
}, [sessions, fetchMessages]);
```

### Current Issues

1. **Inefficient**: Polling every 50ms creates unnecessary network overhead
2. **Messages appear all at once**: Claude Code streams responses, but our implementation only shows complete messages
3. **Latency**: Up to 50ms delay before new messages appear
4. **Resource usage**: Constant polling even when no messages are being sent

## Claude Code Streaming Format

Claude Code outputs messages in a streaming JSON format:
- `{"type":"system","subtype":"init"...}` - Session initialization
- `{"type":"assistant","message":{...}}` - Assistant messages (can be sent multiple times for the same message ID)
- `{"type":"result","subtype":"success"...}` - Final result

The same message ID appears multiple times as content is streamed.

## Potential Solutions

### 1. Event-Based Updates (Recommended)

Instead of polling, the backend could emit events when new messages arrive:

```rust
// Backend: Emit events when processing Claude output
app.emit("session-message", SessionMessage {
    session_id: session_id,
    message: parsed_message,
});

// Frontend: Listen for events
import { listen } from '@tauri-apps/api/event';

useEffect(() => {
  const unlisten = listen('session-message', (event) => {
    updateSessionMessages(event.payload.session_id, event.payload.message);
  });
  return () => { unlisten.then(fn => fn()); };
}, []);
```

### 2. WebSocket-like Streaming

While Tauri doesn't use WebSockets internally, it provides similar real-time capabilities:
- Tauri's event system uses IPC (Inter-Process Communication)
- Events can be emitted from Rust to JavaScript instantly
- No need for external WebSocket server

### 3. Server-Sent Events Pattern

The backend could maintain message queues per session and stream updates:
```rust
// Maintain a message queue per session
let message_queue = Arc::new(Mutex::new(Vec::new()));

// Process Claude output and queue messages
while let Some(line) = reader.next_line().await? {
    let message = parse_claude_output(line);
    message_queue.lock().await.push(message);
    app.emit("new-message", &message)?;
}
```

## Implementation Plan

1. **Modify Backend**:
   - Change `ClaudeSession::process_output` to emit events for each message chunk
   - Keep messages in memory for retrieval
   - Emit events for streaming updates

2. **Update Frontend**:
   - Replace polling with event listeners
   - Handle streaming messages (update existing messages with same ID)
   - Show partial messages as they arrive

3. **Benefits**:
   - Real-time message updates
   - Reduced resource usage
   - Better user experience with streaming responses
   - Lower latency

## Migration Path

1. Keep polling as fallback
2. Implement event-based updates
3. Test thoroughly
4. Remove polling mechanism once stable