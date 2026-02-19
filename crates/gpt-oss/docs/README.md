# gpt-oss

Rust client for OpenAI's GPT-OSS open-weight models via llama-server.

## Overview

gpt-oss provides a client library for interacting with llama-server running GPT-OSS models. It supports:

- **Completions API** (`/v1/completions`) - Text generation with optional JSON schema constraints
- **Chat Completions API** (`/v1/chat/completions`) - Chat-style completions with `response_format` for structured output
- **Responses API** (`/v1/responses`) - Tool calling and reasoning effort (OpenAI-compatible)
- **Harmony Rendering** - GPT-OSS specific prompt format for optimal results
- **Server Management** - Auto-start llama-server if not running

## Quick Start

```rust
use gpt_oss::{GptOssClient, GptOssRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = GptOssClient::with_base_url("http://localhost:8000")?;

    // Simple completion
    let response = client.complete_simple("gpt-oss-20b", "What is Rust?").await?;
    println!("{}", response);

    Ok(())
}
```

## Structured Output

Use the chat completions API with `response_format` to constrain output to a JSON schema:

```rust
use gpt_oss::{
    GptOssClient, ChatCompletionsRequest, ChatMessage,
    ResponseFormat, JsonSchemaSpec
};
use serde_json::json;

let client = GptOssClient::with_base_url("http://localhost:8000")?;

let schema = json!({
    "type": "object",
    "properties": {
        "name": {"type": "string"},
        "age": {"type": "integer"}
    },
    "required": ["name", "age"]
});

let request = ChatCompletionsRequest {
    model: "gpt-oss-20b".to_string(),
    messages: vec![
        ChatMessage {
            role: "user".to_string(),
            content: "Generate a person with name and age.".to_string(),
        }
    ],
    max_tokens: Some(100),
    temperature: None,
    top_p: None,
    stop: None,
    response_format: Some(ResponseFormat::JsonSchema {
        json_schema: JsonSchemaSpec {
            name: Some("person".to_string()),
            schema,
            strict: Some(true),
        },
    }),
    stream: false,
};

let response = client.chat_completions(request).await?;
println!("{}", response.content()); // {"name": "Alice", "age": 30}
```

The `response_format` with `json_schema` is converted to GBNF grammar by llama-server, guaranteeing the output matches the schema.

## Streaming

```rust
use gpt_oss::{GptOssClient, GptOssRequest};
use tokio_stream::StreamExt;

let client = GptOssClient::with_base_url("http://localhost:8000")?;

let request = GptOssRequest {
    model: "gpt-oss-20b".to_string(),
    prompt: "Write a haiku about Rust:".to_string(),
    max_tokens: Some(100),
    temperature: Some(0.7),
    top_p: None,
    stop: None,
    stream: true,
    json_schema: None,
};

let mut stream = client.stream(request).await?;
while let Some(chunk) = stream.next().await {
    let chunk = chunk?;
    print!("{}", chunk.delta());
}
```

## Server Management

Auto-start llama-server if not running:

```rust
use gpt_oss::LlamaServerManager;
use std::time::Duration;

// Check if llama-server binary is available
if LlamaServerManager::is_available() {
    let mut manager = LlamaServerManager::new()
        .with_model("/path/to/model.gguf");

    manager.start()?;
    manager.wait_ready_timeout(Duration::from_secs(30)).await?;

    // Server is ready, use client...

    // Server stops when manager is dropped
}
```

## Harmony Format

For GPT-OSS models, use the Harmony renderer for optimal prompt formatting:

```rust
use gpt_oss::{HarmonyRenderer, HarmonyRole, HarmonyTurn};

let renderer = HarmonyRenderer::gpt_oss()?;
let turns = vec![
    HarmonyTurn::new(HarmonyRole::User, "What is 2+2?"),
];

let prompt = renderer.render_prompt(&turns, &[])?;
// Renders with proper GPT-OSS special tokens
```

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `GPT_OSS_URL` | Base URL for llama-server | `http://localhost:8000` |
| `GPTOSS_BASE_URL` | Alternative base URL env var | `http://localhost:8000` |
| `LLAMA_MODEL_PATH` | Path to model file for auto-discovery | - |

## API Types

### Request Types

- `GptOssRequest` - Completions API request
- `ChatCompletionsRequest` - Chat completions with structured output
- `GptOssResponsesRequest` - Responses API request (tools, reasoning)

### Response Types

- `GptOssResponse` - Completions API response
- `ChatCompletionsResponse` - Chat completions response
- `GptOssResponsesResponse` - Responses API response
- `GptOssStreamChunk` - Streaming chunk

### Structured Output Types

- `ResponseFormat` - Output format specification (text, json_object, json_schema)
- `JsonSchemaSpec` - JSON schema definition for constrained output

## Integration with dsrs

The gpt-oss client integrates with dsrs for DSPy workflows:

```rust
use dsrs::prelude::*;

// Using gptoss: provider prefix
let lm = LM::builder()
    .model("gptoss:gpt-oss-20b".to_string())
    .temperature(0.3)
    .build()
    .await?;

dsrs::configure(lm, ChatAdapter);
```

The dsrs `GptOssCompletionModel` automatically:
1. Extracts output schema from DSPy signature
2. Sends request with `response_format` for JSON schema constraint
3. Parses JSON response and formats back to DSPy field markers

## License

MIT
