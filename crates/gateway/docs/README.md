# Gateway Crate

- **Status:** Accurate
- **Last verified:** (see commit)
- **Source of truth:** terminology → [GLOSSARY.md](../../../GLOSSARY.md), behavior → code
- **If this doc conflicts with code, code wins.**

Unified abstraction layer for external AI service providers.

## Overview

The `gateway` crate provides a consistent interface for accessing various AI inference providers (Cerebras, OpenAI, OpenAI, etc.) through a common API. It handles authentication, request/response formatting, and provider-specific quirks.

## Quick Start

```rust
use gateway::{CerebrasGateway, InferenceGateway, ChatRequest, Message};

#[tokio::main]
async fn main() -> gateway::Result<()> {
    // Create gateway from environment (reads CEREBRAS_API_KEY)
    let gateway = CerebrasGateway::from_env()?;

    // Create a chat request
    let request = ChatRequest::new(
        "zai-glm-4.7",
        vec![
            Message::system("You are a helpful assistant."),
            Message::user("What is 2+2?"),
        ],
    );

    // Send request
    let response = gateway.chat(request).await?;

    println!("Response: {}", response.content().unwrap());
    println!("Tokens: {} prompt, {} completion",
        response.usage.prompt_tokens,
        response.usage.completion_tokens
    );

    Ok(())
}
```

## CLI Usage

The gateway is integrated into the Pylon CLI:

```bash
# Set API key
export CEREBRAS_API_KEY="csk-your-key-here"

# Or use .env.local file
echo 'CEREBRAS_API_KEY="csk-your-key-here"' >> .env.local

# Chat with a model
pylon gateway chat "Hello, world!"

# List available models
pylon gateway models

# Check health
pylon gateway health
```

## Architecture

```
┌─────────────────────────────────────────┐
│              Application                │
├─────────────────────────────────────────┤
│           InferenceGateway              │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐   │
│  │Cerebras │ │ OpenAI  │ │  Pylon  │   │
│  │ Gateway │ │ Gateway │ │ Gateway │   │
│  └────┬────┘ └────┬────┘ └────┬────┘   │
└───────┼──────────┼──────────┼──────────┘
        │          │          │
        ▼          ▼          ▼
   ┌─────────┐ ┌─────────┐ ┌─────────┐
   │Cerebras │ │ OpenAI  │ │  NIP-90 │
   │   API   │ │   API   │ │  Swarm  │
   └─────────┘ └─────────┘ └─────────┘
```

## Traits

### Gateway (Base)

```rust
pub trait Gateway: Send + Sync {
    fn gateway_type(&self) -> &str;    // "inference", "embedding", etc.
    fn provider(&self) -> &str;         // "cerebras", "openai", etc.
    fn name(&self) -> &str;             // Human-readable name
    fn is_configured(&self) -> bool;    // Has required credentials
    fn capabilities(&self) -> Vec<Capability>;
}
```

### InferenceGateway

```rust
#[async_trait]
pub trait InferenceGateway: Gateway {
    async fn models(&self) -> Result<Vec<ModelInfo>>;
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse>;
    async fn health(&self) -> GatewayHealth;
}
```

## Providers

### Cerebras (Implemented)

- **API**: OpenAI-compatible at `https://api.cerebras.ai/v1`
- **Models**: zai-glm-4.7, qwen-3-32b, llama-3.3-70b, llama3.1-8b
- **Features**: Chat completion, streaming, function calling, reasoning
- **Auth**: `CEREBRAS_API_KEY` environment variable

### OpenAI (Planned)

- **API**: `https://api.openai.com/v1`
- **Models**: gpt-4o, gpt-4-turbo, o1, o3
- **Auth**: `OPENAI_API_KEY` environment variable

### OpenAI (Planned)

- **API**: `https://api.openai.com/v1`
- **Models**: codex-3-opus, codex-3-sonnet, codex-3-haiku
- **Auth**: `OPENAI_API_KEY` environment variable

### Pylon Swarm (Planned)

- **Protocol**: NIP-90 Data Vending Machine
- **Payment**: Lightning/Bitcoin micropayments
- **Auth**: Pylon wallet

## Types

See [TYPES.md](./TYPES.md) for complete type documentation.

## Error Handling

```rust
use gateway::{GatewayError, Result};

match gateway.chat(request).await {
    Ok(response) => println!("{}", response.content().unwrap()),
    Err(GatewayError::NotConfigured(msg)) => eprintln!("Config error: {}", msg),
    Err(GatewayError::RateLimited) => eprintln!("Rate limited, retry later"),
    Err(GatewayError::Api { status, message }) => eprintln!("API error {}: {}", status, message),
    Err(e) => eprintln!("Error: {}", e),
}
```

## See Also

- [TYPES.md](./TYPES.md) - Complete type reference
- [PROVIDERS.md](./PROVIDERS.md) - Provider-specific documentation
- [../README.md](../README.md) - Crate README with architecture details
