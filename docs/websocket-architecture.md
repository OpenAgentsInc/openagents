# WebSocket Architecture in OpenAgents

This document provides a comprehensive overview of the WebSocket implementation in OpenAgents, focusing on real-time chat and reasoning capabilities.

## Overview

The WebSocket system consists of several key components that work together to provide real-time communication between the frontend and backend:

1. WebSocket Transport Layer
2. Message Processing System
3. Chat Handler
4. Connection Management
5. State Synchronization

## Component Architecture

### 1. WebSocket Transport Layer

The transport layer handles the low-level WebSocket connection and message routing:

```rust
pub struct WebSocketTransport {
    pub state: Arc<WebSocketState>,
    pub app_state: AppState,
}
```

Key responsibilities:
- Connection establishment and teardown
- Message routing
- Error handling
- Connection state management

### 2. WebSocket State Management

The WebSocket state maintains active connections and shared services:

```rust
pub struct WebSocketState {
    pub connections: Arc<RwLock<HashMap<String, WebSocketSender>>>,
    pub github_service: Arc<GitHubService>,
    pub model_router: Arc<ModelRouter>,
}
```

Features:
- Connection tracking
- Service sharing
- Thread-safe state management
- Connection cleanup

### 3. Message Processing

Messages are processed through a dedicated processor:

```rust
pub struct MessageProcessor {
    app_state: AppState,
    user_id: String,
    ws_state: Arc<WebSocketState>,
}
```

Handles:
- Message parsing
- Protocol validation
- Message routing
- Error responses

### 4. Chat Handler

Specialized handler for chat-related messages:

```rust
pub struct ChatHandler {
    tx: mpsc::Sender<String>,
    state: AppState,
    user_id: String,
}
```

Responsibilities:
- Chat message processing
- Conversation management
- Streaming responses
- Reasoning integration

## Message Flow

1. **Connection Establishment**
   ```
   Client -> Server: WebSocket Upgrade Request
   Server -> Client: WebSocket Connection Established
   ```

2. **Subscription**
   ```
   Client -> Server: { type: "Subscribe", scope: "chat", ... }
   Server -> Client: { type: "Subscribed", scope: "chat", last_sync_id: 0 }
   ```

3. **Chat Message**
   ```
   Client -> Server: {
     type: "Message",
     id: "uuid",
     content: "message",
     use_reasoning: true
   }
   Server -> Client: {
     type: "Update",
     message_id: "uuid",
     delta: {
       content: "partial response",
       reasoning: "reasoning steps"
     }
   }
   ```

## Connection Lifecycle

1. **Initialization**
   - WebSocket upgrade request received
   - Connection ID generated
   - State initialized
   - Handlers created

2. **Active State**
   - Message processing
   - State synchronization
   - Error handling
   - Keep-alive management

3. **Termination**
   - Cleanup triggered
   - State removed
   - Resources freed
   - Connections closed

## Error Handling

The system implements multiple layers of error handling:

1. **Transport Level**
   - Connection errors
   - Protocol errors
   - State errors

2. **Message Level**
   - Parse errors
   - Validation errors
   - Processing errors

3. **Application Level**
   - Business logic errors
   - State errors
   - Authorization errors

## State Management

### Connection State

```rust
let (sender, receiver) = socket.split();
let (tx, rx) = mpsc::channel::<String>(32);
```

- Split socket for independent send/receive
- Channel for message passing
- State tracking per connection

### Shared State

```rust
pub struct AppState {
    pub ws_state: Arc<WebSocketState>,
    pub github_oauth: Arc<GitHubOAuth>,
    pub scramble_oauth: Arc<ScrambleOAuth>,
    pub pool: PgPool,
    pub frontend_url: String,
    pub groq: Arc<GroqService>,
}
```

- Thread-safe service sharing
- Connection pooling
- Configuration management

## Reasoning Integration

The system integrates with Groq for AI reasoning:

1. **Streaming Updates**
   ```rust
   while let Some(update) = stream.next().await {
       match update {
           Ok(delta) => {
               // Process content and reasoning updates
               if let Some(c) = delta.content { ... }
               if let Some(r) = delta.reasoning { ... }
           }
           Err(e) => { ... }
       }
   }
   ```

2. **State Management**
   ```rust
   let mut content = String::new();
   let mut reasoning = String::new();
   ```

## Key Considerations

1. **Thread Safety**
   - Use of Arc for shared state
   - RwLock for concurrent access
   - Channel-based communication

2. **Error Propagation**
   - Structured error types
   - Error context preservation
   - Client notification

3. **Resource Management**
   - Connection cleanup
   - Memory management
   - Resource pooling

4. **Performance**
   - Asynchronous processing
   - Message batching
   - Connection pooling

## Common Issues and Solutions

1. **Connection Drops**
   - Automatic reconnection
   - State preservation
   - Message queuing

2. **Message Ordering**
   - Sequential processing
   - Message IDs
   - State synchronization

3. **Resource Leaks**
   - Proper cleanup
   - Connection tracking
   - Resource limits

## Testing

Key test areas:

1. **Connection Management**
   - Connection establishment
   - Reconnection handling
   - Cleanup verification

2. **Message Processing**
   - Protocol compliance
   - Error handling
   - State management

3. **Integration Tests**
   - End-to-end flow
   - Error scenarios
   - Performance testing

## Future Improvements

1. **Connection Management**
   - Better reconnection logic
   - Connection pooling
   - Load balancing

2. **Message Processing**
   - Message compression
   - Binary protocols
   - Message prioritization

3. **State Management**
   - Distributed state
   - Cache integration
   - State persistence

4. **Monitoring**
   - Metrics collection
   - Performance monitoring
   - Error tracking

## Related Systems

- Chat System
- Authentication
- Database Layer
- AI Integration
- Frontend Components

## References

- [Axum WebSocket Documentation](https://docs.rs/axum/latest/axum/extract/ws/index.html)
- [Tokio Documentation](https://tokio.rs/docs/overview/)
- [WebSocket Protocol](https://tools.ietf.org/html/rfc6455)