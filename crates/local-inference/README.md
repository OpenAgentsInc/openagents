# local-inference

Unified trait abstraction for local AI model inference backends. Provides a common interface for GPT-OSS, fm-bridge, and future local inference engines.

## Overview

The `local-inference` crate defines the `LocalModelBackend` trait that all local inference providers must implement. This enables OpenAgents applications to swap between different local model backends (GPT-OSS, fm-bridge, custom implementations) using a consistent API.

## Key Concepts

### LocalModelBackend Trait

The core trait that defines how OpenAgents interacts with local models:

```rust
#[async_trait]
pub trait LocalModelBackend: Send + Sync {
    async fn initialize(&mut self) -> Result<()>;
    async fn list_models(&self) -> Result<Vec<ModelInfo>>;
    async fn get_model_info(&self, model_id: &str) -> Result<ModelInfo>;
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse>;
    async fn complete_stream(&self, request: CompletionRequest) -> Result<mpsc::Receiver<Result<StreamChunk>>>;
    async fn is_ready(&self) -> bool;
    async fn shutdown(&mut self) -> Result<()>;
}
```

### Type Definitions

- **CompletionRequest**: Input to the model (prompt, parameters)
- **CompletionResponse**: Output from the model (text, usage stats)
- **StreamChunk**: Individual chunk in a streaming response
- **ModelInfo**: Metadata about an available model

## Usage

### Basic Completion

```rust
use local_inference::{LocalModelBackend, CompletionRequest};

async fn generate_text(backend: &impl LocalModelBackend) -> Result<String> {
    let request = CompletionRequest::new("llama-3-8b", "Hello, world!");
    let response = backend.complete(request).await?;
    Ok(response.text)
}
```

### Streaming Completion

```rust
use local_inference::{LocalModelBackend, CompletionRequest};

async fn stream_text(backend: &impl LocalModelBackend) -> Result<()> {
    let request = CompletionRequest::new("llama-3-8b", "Write a poem");
    let mut rx = backend.complete_stream(request).await?;

    while let Some(chunk_result) = rx.recv().await {
        match chunk_result {
            Ok(chunk) => print!("{}", chunk.text),
            Err(e) => eprintln!("Stream error: {}", e),
        }
    }

    Ok(())
}
```

### Listing Available Models

```rust
async fn show_models(backend: &impl LocalModelBackend) -> Result<()> {
    let models = backend.list_models().await?;

    for model in models {
        println!("Model: {}", model.id);
        println!("  Name: {}", model.name);
        println!("  Size: {}B parameters", model.parameters);
    }

    Ok(())
}
```

### Initialization Pattern

```rust
use local_inference::LocalModelBackend;

async fn setup_backend(mut backend: impl LocalModelBackend) -> Result<()> {
    // Initialize backend (load models, setup GPU, etc.)
    backend.initialize().await?;

    // Wait for backend to be ready
    while !backend.is_ready().await {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    println!("Backend ready!");
    Ok(())
}
```

## Implementing LocalModelBackend

To add a new local inference backend:

```rust
use async_trait::async_trait;
use local_inference::{
    LocalModelBackend, CompletionRequest, CompletionResponse,
    StreamChunk, ModelInfo, LocalModelError, Result
};
use tokio::sync::mpsc;

pub struct MyCustomBackend {
    // Your backend state
}

#[async_trait]
impl LocalModelBackend for MyCustomBackend {
    async fn initialize(&mut self) -> Result<()> {
        // Load models, setup resources
        Ok(())
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        // Return available models
        Ok(vec![
            ModelInfo {
                id: "my-model-7b".to_string(),
                name: "My Model 7B".to_string(),
                parameters: 7_000_000_000,
                context_length: 4096,
                description: Some("A custom model".to_string()),
            }
        ])
    }

    async fn get_model_info(&self, model_id: &str) -> Result<ModelInfo> {
        // Return info for specific model
        self.list_models()
            .await?
            .into_iter()
            .find(|m| m.id == model_id)
            .ok_or_else(|| LocalModelError::ModelNotFound(model_id.to_string()))
    }

    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        // Run inference
        let text = format!("Response to: {}", request.prompt);

        Ok(CompletionResponse {
            text,
            model: request.model,
            usage: UsageInfo {
                prompt_tokens: 10,
                completion_tokens: 20,
                total_tokens: 30,
            },
            finish_reason: Some("stop".to_string()),
        })
    }

    async fn complete_stream(
        &self,
        request: CompletionRequest,
    ) -> Result<mpsc::Receiver<Result<StreamChunk>>> {
        let (tx, rx) = mpsc::channel(32);

        // Spawn task to stream chunks
        tokio::spawn(async move {
            for word in request.prompt.split_whitespace() {
                let chunk = StreamChunk {
                    text: format!("{} ", word),
                    index: 0,
                    finish_reason: None,
                };

                if tx.send(Ok(chunk)).await.is_err() {
                    break;
                }
            }
        });

        Ok(rx)
    }

    async fn is_ready(&self) -> bool {
        true // or check if models are loaded
    }

    async fn shutdown(&mut self) -> Result<()> {
        // Cleanup resources
        Ok(())
    }
}
```

## Built-in Implementations

### GPT-OSS Backend

```rust
use gpt_oss::GptOssClient;

let mut backend = GptOssClient::builder()
    .base_url("http://localhost:8000")
    .default_model("gpt-oss-20b")
    .build()?;
backend.initialize().await?;

// Now use as LocalModelBackend
```

### FM-Bridge Backend

```rust
use fm_bridge::FMClient;

let mut backend = FMClient::new();
backend.initialize().await?;

// Now use as LocalModelBackend
```

## Advanced Usage

### Request Configuration

```rust
use local_inference::CompletionRequest;

let request = CompletionRequest {
    model: "llama-3-8b".to_string(),
    prompt: "Write a haiku about Rust".to_string(),
    max_tokens: Some(100),
    temperature: Some(0.7),
    top_p: Some(0.9),
    stop_sequences: Some(vec!["\n\n".to_string()]),
    stream: false,
};
```

### Error Handling

```rust
use local_inference::LocalModelError;

match backend.complete(request).await {
    Ok(response) => println!("Success: {}", response.text),
    Err(LocalModelError::ModelNotFound(id)) => {
        eprintln!("Model '{}' not found", id);
    }
    Err(LocalModelError::InferenceError(msg)) => {
        eprintln!("Inference failed: {}", msg);
    }
    Err(LocalModelError::ConfigError(msg)) => {
        eprintln!("Configuration error: {}", msg);
    }
    Err(e) => eprintln!("Error: {}", e),
}
```

### Backend Lifecycle Management

```rust
async fn run_with_backend(backend: impl LocalModelBackend) -> Result<()> {
    let mut backend = backend;

    // Initialize
    backend.initialize().await?;

    // Use backend
    let response = backend.complete(
        CompletionRequest::new("llama-3-8b", "Hello!")
    ).await?;

    println!("Response: {}", response.text);

    // Cleanup
    backend.shutdown().await?;

    Ok(())
}
```

## Testing

### Mock Backend for Tests

```rust
use local_inference::{LocalModelBackend, CompletionRequest, CompletionResponse};

struct MockBackend {
    responses: HashMap<String, String>,
}

#[async_trait]
impl LocalModelBackend for MockBackend {
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        let text = self.responses
            .get(&request.prompt)
            .cloned()
            .unwrap_or_else(|| "mock response".to_string());

        Ok(CompletionResponse {
            text,
            model: request.model,
            usage: UsageInfo::default(),
            finish_reason: Some("stop".to_string()),
        })
    }

    // Implement other required methods...
}

#[tokio::test]
async fn test_with_mock() {
    let mut mock = MockBackend::default();
    mock.responses.insert("test".to_string(), "result".to_string());

    let response = mock.complete(CompletionRequest::new("model", "test")).await.unwrap();
    assert_eq!(response.text, "result");
}
```

## Integration with OpenAgents

### Legacy Autopilot Usage (disabled in v0.1)

```rust
// Legacy example (removed from Autopilot v0.1)
use local_inference::LocalModelBackend;
use gpt_oss::GptOssClient;

let backend: Box<dyn LocalModelBackend> = match config.backend {
    "gpt-oss" => Box::new(GptOssClient::new(gpt_oss_config)),
    "fm-bridge" => Box::new(FMClient::new()),
    _ => return Err(anyhow!("Unknown backend")),
};

backend.initialize().await?;
```

### GUI Integration

```rust
// In crates/desktop/src/main.rs
use local_inference::LocalModelBackend;

struct AppState {
    backend: Arc<RwLock<Box<dyn LocalModelBackend>>>,
}

async fn handle_completion_request(
    state: Arc<AppState>,
    prompt: String,
) -> Result<String> {
    let backend = state.backend.read().await;
    let request = CompletionRequest::new("default-model", &prompt);
    let response = backend.complete(request).await?;
    Ok(response.text)
}
```

## Performance Considerations

1. **Initialization**: Call `initialize()` once during app startup, not per-request
2. **Streaming**: Use `complete_stream()` for long outputs to show partial results
3. **Model Loading**: Large models may take seconds to load during initialization
4. **Resource Cleanup**: Always call `shutdown()` when done to free GPU memory

## Dependencies

```toml
[dependencies]
async-trait = "0.1"
tokio = { version = "1.0", features = ["sync"] }
serde = { version = "1.0", features = ["derive"] }
thiserror = "2.0"
```

## See Also

- [gpt-oss](../gpt-oss/README.md) - GPT-OSS backend implementation
- [fm-bridge](../fm-bridge/README.md) - Foundation Models bridge backend
- [acp-adapter](../acp-adapter/README.md) - Agent Client Protocol adapter

## License

MIT
