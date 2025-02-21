# Reasoning Integration Design

This document outlines how we integrate Groq's reasoning capabilities with our real-time sync engine to provide a seamless, WebSocket-based reasoning experience.

## Core Concepts

1. **WebSocket-Based Streaming**
   - Replace manual SSE parsing with WebSocket connections for real-time updates
   - Use the sync engine to handle connection management and reconnection
   - Stream both content and reasoning as separate fields in the WebSocket messages

2. **State Management**
   - Store reasoning state in Postgres using JSONB column
   - Maintain temporary reasoning state during streaming in frontend
   - Sync reasoning updates through WebSocket events
   - Use Zustand store to manage UI state

3. **Component Architecture**
   - Thinking component displays reasoning state
   - WebSocket events update thinking component in real-time
   - Support both streaming and non-streaming modes
   - Handle connection state and error cases

## WebSocket Protocol

### Connection Setup
```typescript
// Client connects and subscribes to chat
{
  type: "subscribe",
  scope: "chat",
  conversationId: "uuid",
  lastSyncId: 1234
}

// Server acknowledges
{
  type: "subscribed",
  scope: "chat", 
  lastSyncId: 1234
}
```

### Message Events
```typescript
// Client sends message
{
  type: "message",
  conversationId: "uuid",
  content: "What is 2+2?",
  useReasoning: true
}

// Server streams response
{
  type: "update",
  messageId: "uuid",
  delta: {
    content?: "The answer is 4",
    reasoning?: "Let's solve this step by step:\n1. We start with 2\n2. Adding another 2"
  }
}

// Server sends completion
{
  type: "complete",
  messageId: "uuid"
}
```

## Database Schema

The messages table includes a reasoning column:
```sql
CREATE TABLE messages (
  id UUID PRIMARY KEY,
  conversation_id UUID NOT NULL,
  content TEXT NOT NULL,
  reasoning JSONB,  -- Stores reasoning steps
  ...
);
```

## Frontend Integration

### Sync Engine Hook
```typescript
const { sendMessage, state } = useAgentSync({
  scope: "chat",
  conversationId: id,
  features: ["reasoning"] 
});
```

### Thinking Component
```typescript
<Thinking
  state={state.isStreaming ? "thinking" : "finished"}
  content={message.reasoning?.split('\n')}
  defaultOpen={state.isStreaming}
/>
```

## Backend Implementation

### WebSocket Handler
```rust
async fn handle_message(ws: WebSocket, state: AppState) {
    match message {
        Message::Text(text) => {
            let msg: ChatMessage = serde_json::from_str(&text)?;
            
            // Start Groq stream
            let stream = state.groq.chat_stream(
                msg.content,
                msg.use_reasoning
            ).await?;
            
            // Stream updates via WebSocket
            while let Some(update) = stream.next().await {
                ws.send(json!({
                    "type": "update",
                    "messageId": msg.id,
                    "delta": update
                })).await?;
            }
            
            // Send completion
            ws.send(json!({
                "type": "complete",
                "messageId": msg.id
            })).await?;
        }
    }
}
```

### Database Service
```rust
impl ChatDatabase {
    async fn create_message(&self, msg: NewMessage) -> Result<Message> {
        sqlx::query_as!(
            Message,
            "INSERT INTO messages (content, reasoning) VALUES ($1, $2)",
            msg.content,
            msg.reasoning
        )
        .execute(&self.pool)
        .await?
    }
}
```

## Key Benefits

1. **Real-Time Updates**
   - Instant feedback as reasoning progresses
   - Smooth UI updates via WebSocket
   - No manual polling or reconnection handling

2. **Better State Management** 
   - Centralized state in Zustand store
   - Consistent updates across components
   - Clear separation of concerns

3. **Improved Error Handling**
   - Automatic reconnection via sync engine
   - Error state propagation to UI
   - Graceful fallbacks

4. **Enhanced Performance**
   - Reduced HTTP overhead
   - Efficient binary protocol
   - Batched updates

## Implementation Steps

1. Update backend to use WebSocket handler
2. Modify Groq service to support streaming
3. Enhance frontend sync engine
4. Update thinking component
5. Add error handling
6. Test with various scenarios

## Future Improvements

1. **Offline Support**
   - Cache reasoning state
   - Queue updates when offline
   - Sync on reconnection

2. **Advanced Features**
   - Reasoning search/filter
   - Custom reasoning views
   - Progress indicators

3. **Performance**
   - Message batching
   - State compression
   - Selective updates