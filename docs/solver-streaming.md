# Solver Streaming Architecture

## Current Implementation

### WebSocket Connection Flow
1. Client connects via WebSocket to `/ws` endpoint
2. Server creates new connection state in `SolverWsState`
3. Two async tasks are spawned:
   - `send_task`: Handles sending messages to client
   - `recv_task`: Handles receiving messages from client

### Message Flow

#### Client to Server
1. Client sends issue URL via WebSocket
2. Server parses URL and starts solver process
3. Solver process generates updates through various stages

#### Server to Client Updates
Current update types (in `SolverUpdate` enum):
```rust
Progress { stage, message, data }
Complete { result }
Error { message, details }
```

### Current Issues

1. **Redundant Updates**
   - The same progress bar is being updated hundreds of times
   - Each stage update overwrites the previous content
   - HTML sections are not properly targeted for updates

2. **Content Streaming**
   - DeepSeek responses are not being streamed in real-time
   - Updates are batched and sent as complete chunks
   - No partial updates for reasoning or solution text

3. **HTML Structure Problems**
   - Single div being updated repeatedly
   - No separation between different content types
   - HTMX swap targets not properly utilized

## Ideal Implementation

### HTML Structure
```html
<div id="solver-container">
    <!-- Progress bar section -->
    <div id="progress-bar" hx-swap="innerHTML">
        <!-- Progress updates here -->
    </div>

    <!-- File analysis section -->
    <div id="files-section">
        <div id="files-list" hx-swap="innerHTML">
            <!-- Files list updates here -->
        </div>
        <div id="files-reasoning" hx-swap="appendChild">
            <!-- Streaming reasoning about files -->
        </div>
    </div>

    <!-- Solution section -->
    <div id="solution-section">
        <div id="solution-reasoning" hx-swap="appendChild">
            <!-- Streaming solution reasoning -->
        </div>
        <div id="solution-code" hx-swap="innerHTML">
            <!-- Final code solution -->
        </div>
    </div>
</div>
```

### Streaming Updates
1. **Progress Updates**
   - Update progress bar only when stage changes
   - Use specific progress div target

2. **Content Streaming**
   - Stream DeepSeek responses in real-time
   - Append new content to appropriate sections
   - Use different swap strategies:
     - `innerHTML` for replaceable content
     - `appendChild` for streaming content

3. **WebSocket Message Types**
```rust
enum WsUpdate {
    // Replace entire progress bar
    ProgressUpdate {
        stage: SolverStage,
        message: String,
    },
    // Append to files reasoning
    FilesReasoningChunk {
        content: String,
    },
    // Replace files list
    FilesList {
        files: Vec<String>,
    },
    // Append to solution reasoning
    SolutionReasoningChunk {
        content: String,
    },
    // Replace solution code
    SolutionCode {
        code: String,
    },
    // Final completion
    Complete {
        summary: String,
    }
}
```

## Implementation Steps

1. **HTML Template Updates**
   - Create new HTML structure with proper div IDs
   - Add HTMX swap attributes
   - Style sections for streaming content

2. **WebSocket Handler Changes**
   - Modify `transport.rs` to handle new message types
   - Implement proper targeting of HTML sections
   - Add streaming support for content chunks

3. **DeepSeek Integration**
   - Modify DeepSeek service to stream responses
   - Implement chunk processing
   - Add proper error handling for stream interruptions

4. **Progress Updates**
   - Reduce frequency of progress bar updates
   - Only send updates on stage changes
   - Add better progress indicators

5. **Testing**
   - Add tests for streaming functionality
   - Verify proper HTML targeting
   - Test error handling and recovery

## Code Changes Needed

1. **transport.rs**
```rust
impl SolverWsState {
    pub async fn send_update(&self, update: WsUpdate) {
        let conns = self.connections.read().await;
        let html = match update {
            WsUpdate::ProgressUpdate { stage, message } => {
                format!(
                    "<div id='progress-bar' hx-swap-oob='true'>{}</div>",
                    render_progress(stage, message)
                )
            },
            WsUpdate::FilesReasoningChunk { content } => {
                format!(
                    "<div id='files-reasoning' hx-swap-oob='true'>{}</div>",
                    escape_html(&content)
                )
            },
            // ... other update types
        };
        
        for tx in conns.values() {
            let _ = tx.send(Message::Text(html.clone())).await;
        }
    }
}
```

2. **DeepSeek Service**
```rust
impl DeepSeekService {
    pub async fn stream_reasoning(&self, prompt: String) -> impl Stream<Item = Result<String>> {
        // Implement streaming from DeepSeek API
        // Return chunks as they arrive
    }
}
```

3. **Solver Service**
```rust
impl SolverService {
    pub async fn process_with_streaming(&self, issue: Issue, tx: mpsc::Sender<WsUpdate>) {
        // Stream file analysis
        let mut reasoning_stream = self.deepseek.stream_reasoning(prompt).await;
        while let Some(chunk) = reasoning_stream.next().await {
            tx.send(WsUpdate::FilesReasoningChunk { 
                content: chunk? 
            }).await?;
        }
        
        // Continue with other streaming updates
    }
}
```

## Next Steps

1. Implement new HTML template structure
2. Update WebSocket handler to use new message types
3. Modify DeepSeek service to support streaming
4. Test and verify proper streaming behavior
5. Add error handling and recovery
6. Update documentation with final implementation details

## Current Problems to Fix

1. **Progress Bar Updates**
   - In `transport.rs`, we're sending the same progress bar update multiple times
   - Need to track current stage and only send updates on changes
   - Add debouncing for progress updates

2. **Content Duplication**
   - Each update is causing full content replacement
   - Need to implement proper HTMX swap strategies
   - Use append for streaming content

3. **DeepSeek Integration**
   - Currently waiting for full response
   - Need to implement streaming from DeepSeek API
   - Handle streaming errors gracefully

4. **HTML Structure**
   - Current structure doesn't support proper streaming
   - Need to update templates with proper IDs and swap attributes
   - Add styling for streaming content

5. **Error Handling**
   - Add better error recovery
   - Implement reconnection logic
   - Handle partial updates properly

## Implementation Priority

1. Fix progress bar updates (highest priority, most visible issue)
2. Update HTML structure to support streaming
3. Implement proper HTMX swap strategies
4. Add DeepSeek streaming support
5. Improve error handling and recovery

This will give us a proper streaming implementation that shows real-time updates to users while maintaining a clean and efficient codebase.