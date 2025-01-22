# Chat System Architecture

The OpenAgents chat system provides a real-time chat interface powered by WebSocket connections and AI language models. The system is built with Rust, using Axum for HTTP routing and WebSocket handling.

## System Components

### 1. WebSocket Module (`src/server/ws/`)

The WebSocket module handles real-time communication:

```
src/server/ws/
├── handlers/
│   ├── mod.rs (MessageHandler trait)
│   ├── chat.rs (Chat handler)
│   └── solver.rs (Solver handler)
├── transport.rs (WebSocket state and connection management)
└── types.rs (Message type definitions)
```

#### Message Handler Trait

```rust
#[async_trait]
pub trait MessageHandler {
    type Message;
    
    async fn handle_message(&self, msg: Self::Message, conn_id: String) -> Result<()>;
    async fn broadcast(&self, msg: Self::Message) -> Result<()>;
}
```

#### Chat Handler

The ChatHandler implements the MessageHandler trait and:
- Processes incoming user messages
- Manages AI model interactions
- Handles streaming responses
- Maintains chat history

### 2. HTTP Routes (`src/server/routes/`)

Main chat endpoints:
- `/chat` - Home chat interface
- `/chat/{uuid}` - Individual chat sessions
- `/ws` - WebSocket endpoint

### 3. Templates (`templates/`)

Template structure for the chat interface:
- `layouts/chat_base.html` - Main chat layout
- `layouts/chat_content.html` - Chat content area
- Supports both full page loads and HTMX partial updates

### 4. WebSocket Communication

#### Connection Management

- Unique connection IDs using UUID v4
- Connection state tracking
- Automatic reconnection handling
- Connection status indicators

#### Message Types

```rust
pub enum ChatMessage {
    UserMessage { content: String },
    AgentResponse { content: String },
    Error { message: String }
}
```

#### Message Flow

1. Client connects to WebSocket endpoint
2. Server assigns unique connection ID
3. Client sends message in JSON format
4. Server routes message to appropriate handler
5. Handler processes message and generates response
6. Server streams response back to client
7. Client updates UI with response

### 5. AI Integration

The chat system integrates with DeepSeek for AI responses:

- Streaming response support
- Error handling and retry logic
- Response formatting
- Context management
- Rate limiting

## Data Flow

1. User Input
   - User types message
   - UI sends message via WebSocket
   - Input disabled during processing

2. Message Processing
   - Server receives WebSocket message
   - ChatHandler processes message
   - AI model generates response
   - Response streamed back to client

3. Response Handling
   - Client receives streamed response
   - UI updates in real-time
   - Message history updated
   - Input re-enabled

## Error Handling

The system implements comprehensive error handling:

- WebSocket connection errors
- Message processing errors
- AI model errors
- Network timeouts
- Rate limiting
- Invalid message formats

## Future Enhancements

Planned improvements include:

1. Chat History
   - Persistent storage
   - Session management
   - History export

2. Model Settings
   - Model selection
   - Parameter adjustment
   - Context length control

3. Enhanced UI
   - Message search
   - Code highlighting
   - File attachments
   - Rich text formatting

4. System Features
   - User authentication
   - Chat sharing
   - Export options
   - Usage tracking

## Development Guidelines

When working on the chat system:

1. Message Handling
   - Always use typed message structs
   - Implement proper error handling
   - Support message streaming
   - Maintain connection state

2. UI Updates
   - Follow existing styling
   - Maintain responsive design
   - Support both desktop and mobile
   - Keep consistent spacing

3. WebSocket Management
   - Handle reconnection gracefully
   - Implement proper cleanup
   - Monitor connection health
   - Log important events

4. Testing
   - Unit test message handlers
   - Test WebSocket connections
   - Verify error handling
   - Test UI components