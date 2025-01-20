# Solver Service Documentation

The Solver service is responsible for automatically analyzing and solving GitHub issues using AI assistance. It integrates with DeepSeek's API for analysis and solution generation, providing real-time updates through WebSocket connections.

## Architecture

### Core Components

```
src/server/services/solver/
├── mod.rs              # Main solver service implementation
└── ws/                 # WebSocket-based solver implementation
    ├── mod.rs          # WebSocket coordination and flow control
    ├── url_parsing.rs  # GitHub URL parsing and validation
    ├── files_analysis.rs    # File analysis with DeepSeek
    ├── solution_generation.rs # Solution generation with DeepSeek
    └── html_formatting.rs    # HTML template formatting
```

### Module Responsibilities

1. **mod.rs (Main Service)**
   - Service initialization
   - Configuration management
   - Service coordination
   - Error handling

2. **ws/mod.rs (WebSocket Coordinator)**
   - WebSocket connection management
   - Progress updates
   - Flow coordination
   - State management

3. **ws/url_parsing.rs**
   - GitHub URL validation
   - Repository URL extraction
   - Issue number parsing
   - Error reporting

4. **ws/files_analysis.rs**
   - Repository structure analysis
   - Relevant file identification
   - DeepSeek integration for file analysis
   - Real-time analysis updates

5. **ws/solution_generation.rs**
   - Solution planning
   - Code change generation
   - Implementation steps
   - Risk analysis

6. **ws/html_formatting.rs**
   - HTML template management
   - Response formatting
   - Syntax highlighting
   - Layout structuring

## Implementation Details

### State Management

The service uses tokio's async-safe primitives for state management:

```rust
// Shared state for streaming updates
let state = Arc::new(Mutex::new((String::new(), String::new())));
```

### Async Processing

Proper async handling with tokio:

```rust
Box::pin(async move {
    let mut guard = state.lock().await;
    // Process updates
    drop(guard);  // Drop guard before await points
    Ok(())
})
```

### Progress Updates

Real-time updates via WebSocket:

```rust
SolverUpdate::Progress {
    stage: SolverStage::Analysis,
    message: "Analyzing files...".into(),
    data: Some(json!({...})),
}
```

## Process Flow

1. **Initialization**
   - Parse GitHub issue URL
   - Validate repository access
   - Initialize WebSocket connection

2. **Repository Analysis**
   - Generate repository map
   - Identify relevant files
   - Stream analysis progress

3. **Solution Generation**
   - Analyze issue requirements
   - Generate code changes
   - Create implementation plan
   - Stream solution progress

4. **Response Formatting**
   - Format HTML response
   - Include reasoning sections
   - Add syntax highlighting
   - Structure layout

## WebSocket Updates

The service provides real-time updates through several stages:

```typescript
type SolverUpdate = {
    type: 'progress' | 'complete' | 'error'
    stage?: 'init' | 'repomap' | 'analysis' | 'solution' | 'pr'
    message: string
    data?: any
}
```

### Update Stages

1. **Init**
   - Connection established
   - URL validation
   - Initial setup

2. **Repomap**
   - Repository structure analysis
   - File hierarchy generation
   - Access verification

3. **Analysis**
   - File relevance analysis
   - Context gathering
   - Dependency mapping

4. **Solution**
   - Code change generation
   - Implementation planning
   - Risk assessment

5. **PR**
   - Solution formatting
   - Final preparation
   - Completion notification

## Error Handling

The service implements comprehensive error handling:

1. **URL Validation**
   ```rust
   if !issue_url.contains("/issues/") {
       return Err(anyhow::anyhow!("Invalid GitHub URL format"));
   }
   ```

2. **State Management**
   ```rust
   let guard = state.lock().await;
   if let Err(e) = process_update() {
       let _ = update_tx.send(SolverUpdate::Error {...});
   }
   ```

3. **API Integration**
   - DeepSeek API error handling
   - GitHub API error handling
   - Network error recovery

## Testing

The modular structure enables focused testing:

1. **Unit Tests**
   - Individual module testing
   - Mock API responses
   - State management tests

2. **Integration Tests**
   - End-to-end flow testing
   - WebSocket communication
   - Error handling scenarios

## Future Improvements

1. **Performance**
   - Implement response caching
   - Optimize state management
   - Add request batching

2. **Features**
   - Add solution templates
   - Implement retry mechanisms
   - Add progress persistence

3. **Integration**
   - Add more AI providers
   - Enhance GitHub integration
   - Add CI/CD hooks

## Usage Example

```rust
// Initialize solver service
let solver = SolverService::new();

// Create WebSocket connection
let (tx, rx) = broadcast::channel(100);

// Process issue
let result = solver
    .solve_issue_with_ws(issue_url, tx)
    .await?;

// Handle result
match result {
    Ok(solution) => // Handle solution
    Err(e) => // Handle error
}
```

## Configuration

Required environment variables:
- `DEEPSEEK_API_KEY`: DeepSeek API authentication
- `GITHUB_TOKEN`: GitHub API access token
- `AIDER_API_KEY`: Aider service API key

## Dependencies

- tokio: Async runtime and primitives
- serde_json: JSON handling
- anyhow: Error handling
- tracing: Logging and diagnostics
- html-escape: HTML content escaping