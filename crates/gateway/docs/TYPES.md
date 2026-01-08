# Gateway Types Reference

Complete reference for all types in the gateway crate.

## Core Types

### Capability

Capabilities a gateway can provide.

```rust
pub enum Capability {
    TextGeneration,    // Basic text completion
    ChatCompletion,    // Chat-style messages
    Streaming,         // Token streaming
    FunctionCalling,   // Tool/function calling
    Vision,            // Image input
    Embedding,         // Vector embeddings
    ImageGeneration,   // Image output
    SpeechToText,      // Audio transcription
    TextToSpeech,      // Voice synthesis
    Reasoning,         // Extended thinking/reasoning
}
```

### GatewayHealth

Health check response.

```rust
pub struct GatewayHealth {
    pub available: bool,           // Gateway is reachable
    pub latency_ms: Option<u64>,   // Response time
    pub error: Option<String>,     // Error message if unavailable
    pub last_check: i64,           // Unix timestamp
}
```

### ModelInfo

Information about an available model.

```rust
pub struct ModelInfo {
    pub id: String,                    // Model ID (e.g., "zai-glm-4.7")
    pub name: String,                  // Display name
    pub provider: String,              // Provider name
    pub context_length: u32,           // Max context tokens
    pub capabilities: Vec<Capability>, // What this model can do
    pub pricing: Option<ModelPricing>, // Cost information
}
```

### ModelPricing

Pricing information for a model.

```rust
pub struct ModelPricing {
    pub input_per_million: f64,   // USD per 1M input tokens
    pub output_per_million: f64,  // USD per 1M output tokens
}
```

## Chat Types

### Role

Role of a message sender.

```rust
pub enum Role {
    System,     // System instructions
    User,       // User input
    Assistant,  // Model response
    Tool,       // Tool/function result
}
```

### Message

A message in a conversation.

```rust
pub struct Message {
    pub role: Role,
    pub content: String,
    pub name: Option<String>,  // Optional participant name
}

// Convenience constructors
Message::system("You are helpful.")
Message::user("Hello!")
Message::assistant("Hi there!")
```

### ChatRequest

Request for chat completion.

```rust
pub struct ChatRequest {
    pub model: String,                  // Model ID
    pub messages: Vec<Message>,         // Conversation history
    pub temperature: Option<f32>,       // Sampling temperature (0.0-2.0)
    pub max_tokens: Option<u32>,        // Max response tokens
    pub top_p: Option<f32>,             // Nucleus sampling
    pub stop: Option<Vec<String>>,      // Stop sequences
    pub stream: bool,                   // Enable streaming
}

// Builder pattern
let request = ChatRequest::new("zai-glm-4.7", messages)
    .with_temperature(0.7)
    .with_max_tokens(1000)
    .with_top_p(0.95);
```

### ChatResponse

Response from chat completion.

```rust
pub struct ChatResponse {
    pub id: String,              // Response ID
    pub object: String,          // Always "chat.completion"
    pub created: i64,            // Unix timestamp
    pub model: String,           // Model used
    pub choices: Vec<Choice>,    // Generated responses
    pub usage: Usage,            // Token usage
}

// Get first response content
response.content() // -> Option<&str>
```

### Choice

A single completion choice.

```rust
pub struct Choice {
    pub index: u32,                   // Choice index
    pub message: Message,             // Generated message
    pub finish_reason: Option<String>, // Why generation stopped
}
```

### Usage

Token usage statistics.

```rust
pub struct Usage {
    pub prompt_tokens: u32,      // Input tokens
    pub completion_tokens: u32,  // Output tokens
    pub total_tokens: u32,       // Total
}
```

## Error Types

### GatewayError

All possible gateway errors.

```rust
pub enum GatewayError {
    /// Gateway not configured (missing API key, etc.)
    NotConfigured(String),

    /// HTTP request failed
    Http(reqwest::Error),

    /// API returned an error
    Api { status: u16, message: String },

    /// JSON parsing error
    Json(serde_json::Error),

    /// Rate limit exceeded
    RateLimited,

    /// Request timeout
    Timeout,

    /// Model not found
    ModelNotFound(String),
}
```

### Result

Convenience type alias.

```rust
pub type Result<T> = std::result::Result<T, GatewayError>;
```

## Example Usage

```rust
use gateway::{
    CerebrasGateway, InferenceGateway, Gateway,
    ChatRequest, ChatResponse, Message, Role,
    Capability, ModelInfo, GatewayHealth,
    GatewayError, Result,
};

async fn example() -> Result<()> {
    let gateway = CerebrasGateway::from_env()?;

    // Check capabilities
    if gateway.capabilities().contains(&Capability::Reasoning) {
        println!("Gateway supports reasoning!");
    }

    // List models
    for model in gateway.models().await? {
        println!("{}: {} tokens context", model.id, model.context_length);
        if let Some(pricing) = model.pricing {
            println!("  ${}/M input, ${}/M output",
                pricing.input_per_million,
                pricing.output_per_million
            );
        }
    }

    // Chat
    let request = ChatRequest::new("zai-glm-4.7", vec![
        Message::user("Explain quantum computing in one sentence."),
    ]).with_max_tokens(100);

    let response = gateway.chat(request).await?;
    println!("{}", response.content().unwrap_or("No response"));

    Ok(())
}
```
