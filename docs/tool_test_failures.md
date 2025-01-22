# Tool Test Failures Analysis and Resolution

## Categories of Errors

### 1. Missing Mockall Imports and Setup
```rust
error[E0433]: failed to resolve: use of undeclared type `MockGitHubService`
error[E0433]: failed to resolve: use of undeclared type `MockDeepSeekService`
error[E0433]: failed to resolve: use of undeclared type `MockWebSocketState`
error[E0433]: failed to resolve: use of undeclared type `MockToolExecutorFactory`
```

Resolution:
1. Add mockall dependency to Cargo.toml:
```toml
[dev-dependencies]
mockall = "0.11"
```

2. Add proper imports in test modules:
```rust
use mockall::predicate::*;
use mockall::mock;
```

3. Add #[automock] attribute to traits:
```rust
#[cfg_attr(test, automock)]
pub trait GitHubService {
    // ...
}
```

### 2. Missing Predicate Functions
```rust
error[E0425]: cannot find function `eq` in this scope
error[E0425]: cannot find function `always` in this scope
```

Resolution:
1. Import predicate functions:
```rust
use mockall::predicate::*;
```

2. Use fully qualified paths:
```rust
.with(predicate::eq("test"), predicate::always())
```

### 3. Error Handling Implementation
```rust
error[E0277]: the trait bound `ToolError: StdError` is not satisfied
error[E0277]: the trait bound `ToolError: Into<anyhow::Error>` is not satisfied
```

Resolution:
1. Implement std::error::Error for ToolError:
```rust
impl std::error::Error for ToolError {}

impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidArguments(msg) => write!(f, "Invalid arguments: {}", msg),
            Self::ExecutionFailed(msg) => write!(f, "Execution failed: {}", msg),
            Self::PermissionDenied(msg) => write!(f, "Permission denied: {}", msg),
            Self::ResourceNotFound(msg) => write!(f, "Resource not found: {}", msg),
            Self::RateLimitExceeded => write!(f, "Rate limit exceeded"),
            Self::NetworkError(msg) => write!(f, "Network error: {}", msg),
        }
    }
}
```

### 4. Missing Service Methods
```rust
error[E0599]: no method named `create_pull_request` found for struct `Arc<GitHubService>`
error[E0599]: no method named `get_file_contents` found for struct `Arc<GitHubService>`
```

Resolution:
1. Add missing methods to GitHubService:
```rust
impl GitHubService {
    pub async fn create_pull_request(&self, owner: &str, repo: &str, title: &str, 
        description: &str, head: &str, base: &str) -> Result<Value> {
        // Implementation
    }

    pub async fn get_file_contents(&self, owner: &str, repo: &str, 
        path: &str, branch: &str) -> Result<String> {
        // Implementation
    }
}
```

### 5. DeepSeek Integration Issues
```rust
error[E0599]: no method named `chat_stream_with_tools` found for struct `Arc<DeepSeekService>`
error[E0599]: no variant named `ToolCall` found for enum `StreamUpdate`
```

Resolution:
1. Update StreamUpdate enum:
```rust
pub enum StreamUpdate {
    Content(String),
    Reasoning(String),
    ToolCall { name: String, arguments: Value },
    Done,
}
```

2. Add chat_stream_with_tools method:
```rust
impl DeepSeekService {
    pub async fn chat_stream_with_tools(&self, prompt: String, 
        tools: Vec<Tool>) -> impl Stream<Item = StreamUpdate> {
        // Implementation
    }
}
```

### 6. Router Type Mismatches
```rust
error[E0308]: mismatched types
expected struct `MethodRouter<()>`
found struct `MethodRouter<Arc<ChatHandler>>`
```

Resolution:
1. Update route definition:
```rust
pub fn routes(chat_handler: Arc<ChatHandler>) -> Router {
    Router::new()
        .route("/", get(chat_home))
        .route("/:id", get(chat_session))
        .route("/tools/toggle", post(move |form| toggle_tool(chat_handler.clone(), form)))
}
```

## Implementation Strategy

1. Fix Core Types First:
   - Implement Error traits for ToolError
   - Update StreamUpdate enum
   - Add missing service methods

2. Fix Test Infrastructure:
   - Add mockall setup
   - Import predicates
   - Set up proper mocking attributes

3. Fix Integration Issues:
   - Update router types
   - Fix service method calls
   - Update stream handling

4. Add Missing Tests:
   - Add service method tests
   - Add stream processing tests
   - Add error handling tests

## Testing Guidelines

1. Service Mocking:
```rust
#[cfg_attr(test, automock)]
#[async_trait]
pub trait GitHubService {
    async fn get_issue(&self, owner: &str, repo: &str, number: i32) -> Result<Value>;
    // ... other methods
}
```

2. Test Setup:
```rust
#[tokio::test]
async fn test_tool_execution() {
    let mut mock_service = MockGitHubService::new();
    mock_service
        .expect_get_issue()
        .with(predicate::eq("owner"), predicate::eq("repo"), predicate::eq(123))
        .returning(|_, _, _| Ok(json!({ "number": 123 })));
    
    let tools = GitHubTools::new(Arc::new(mock_service));
    // Test implementation
}
```

3. Stream Testing:
```rust
#[tokio::test]
async fn test_stream_processing() {
    let mut mock_service = MockDeepSeekService::new();
    mock_service
        .expect_chat_stream_with_tools()
        .returning(|_, _| {
            stream::iter(vec![
                StreamUpdate::Content("test".to_string()),
                StreamUpdate::Done,
            ])
        });
    // Test implementation
}
```