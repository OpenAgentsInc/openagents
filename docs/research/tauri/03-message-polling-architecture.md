# Message Polling Architecture

## Current Implementation

The OpenAgents Tauri app currently uses a polling mechanism to fetch messages from Claude Code sessions. This document explains how it works, its limitations, and potential improvements.

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

1. **Inefficient**: Polling every 50ms creates unnecessary IPC overhead
2. **Messages appear all at once**: Claude Code streams responses, but our implementation only shows complete messages
3. **Latency**: Up to 50ms delay before new messages appear
4. **Resource usage**: Constant polling even when no messages are being sent

## Claude Code Streaming Format

Claude Code outputs messages in a streaming JSON format:
- `{"type":"system","subtype":"init"...}` - Session initialization
- `{"type":"assistant","message":{...}}` - Assistant messages (can be sent multiple times for the same message ID)
- `{"type":"result","subtype":"success"...}` - Final result

The same message ID appears multiple times as content is streamed, allowing for progressive updates.

## Real-time Streaming Solution (Implemented)

### Frontend Changes
- Added `message_id_map` to track Claude message IDs to our internal UUIDs
- Messages now update progressively as content streams in
- Handles duplicate message IDs by updating existing messages instead of creating new ones

### Backend Changes
```rust
// In handle_assistant_message
if let Some(&existing_uuid) = self.message_id_map.get(message_id) {
    // Update existing message
    if let Some(msg) = self.messages.iter_mut().find(|m| m.id == existing_uuid) {
        msg.content = text_content;
        // Notify subscribers of update
    }
} else {
    // Create new message
    let new_uuid = Uuid::new_v4();
    self.message_id_map.insert(message_id.to_string(), new_uuid);
    // Add message
}
```

## Future Improvements: Event-Based Architecture

### 1. Tauri Event System (Recommended)

Replace polling with Tauri's built-in event system:

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

### 2. Benefits of Event-Based Approach

- **Real-time updates**: Zero latency message delivery
- **Resource efficient**: No constant polling
- **Simpler code**: Direct event handling instead of polling logic
- **Better streaming**: Natural fit for progressive message updates

### 3. Implementation Plan

1. **Backend Changes**:
   - Modify `process_output_line` to emit Tauri events
   - Add event types for message creation, updates, and completion
   - Keep current API for backward compatibility

2. **Frontend Changes**:
   - Add event listeners for each session
   - Remove polling mechanism
   - Update state directly from events

3. **Migration Strategy**:
   - Implement events alongside polling
   - Test thoroughly with both systems
   - Gradually phase out polling
   - Remove polling code once stable

## Performance Comparison

| Aspect | Current (Polling) | Future (Events) |
|--------|------------------|-----------------|
| Latency | 0-50ms | ~0ms |
| CPU Usage | Constant | On-demand |
| IPC Calls | 20/second/session | As needed |
| Code Complexity | Medium | Low |
| Streaming Support | Manual tracking | Native |

## Conclusion

While the current polling implementation works and now supports real-time streaming through message ID tracking, moving to an event-based architecture would provide better performance, lower resource usage, and cleaner code. The Tauri event system is purpose-built for this use case and would be the natural evolution of the current implementation.