# Gateway Providers

Documentation for each supported gateway provider.

## Cerebras

Fast inference on Cerebras Wafer-Scale Engine hardware.

### Configuration

```bash
# Required
export CEREBRAS_API_KEY="csk-your-key-here"

# Optional (defaults shown)
export CEREBRAS_ENDPOINT="https://api.cerebras.ai/v1"
```

Or in `.env.local`:
```
CEREBRAS_API_KEY="csk-your-key-here"
```

Get your API key at: https://cloud.cerebras.ai

### Available Models

| Model ID | Name | Context | Input $/M | Output $/M |
|----------|------|---------|-----------|------------|
| `zai-glm-4.7` | Z.ai GLM 4.7 | 131k | $2.25 | $2.75 |
| `qwen-3-32b` | Qwen 3 32B | 128k | $0.15 | $0.30 |
| `llama-3.3-70b` | Llama 3.3 70B | 128k | $0.85 | $1.20 |
| `llama3.1-8b` | Llama 3.1 8B | 128k | $0.10 | $0.10 |

### Capabilities

- Chat Completion
- Streaming
- Function/Tool Calling
- Reasoning (GLM 4.7)
- Structured Outputs

### GLM 4.7 Specific Features

GLM 4.7 is optimized for coding and agentic workflows:

- **Reasoning**: Enabled by default. Disable with `disable_reasoning: true`
- **Clear Thinking**: Control reasoning trace preservation with `clear_thinking: false`
- **~1000 tokens/sec**: Extremely fast inference

### Rate Limits

| Tier | Requests/min | Tokens/min | Daily Tokens |
|------|-------------|------------|--------------|
| Free | 10 | 150k | 1M |
| Developer | 250 | 250k | Unlimited |

### Example

```rust
use gateway::{CerebrasGateway, InferenceGateway, ChatRequest, Message};

let gateway = CerebrasGateway::from_env()?;

// Use GLM 4.7 for coding tasks
let request = ChatRequest::new("zai-glm-4.7", vec![
    Message::system("You are an expert programmer."),
    Message::user("Write a function to calculate fibonacci numbers."),
]).with_max_tokens(500);

let response = gateway.chat(request).await?;
println!("{}", response.content().unwrap());
```

### CLI

```bash
# Chat with GLM 4.7
pylon gateway chat "Write hello world in Rust" -m zai-glm-4.7

# Use cheaper model for simple tasks
pylon gateway chat "What is 2+2?" -m llama3.1-8b

# List models
pylon gateway models

# Check health
pylon gateway health
```

---

## OpenAI (Planned)

OpenAI API integration.

### Configuration

```bash
export OPENAI_API_KEY="sk-your-key-here"
```

### Models (Planned)

- `gpt-4o` - Latest GPT-4
- `gpt-4-turbo` - Fast GPT-4
- `o1` - Reasoning model
- `o3` - Latest reasoning model

---

## Anthropic (Planned)

Anthropic Claude API integration.

### Configuration

```bash
export ANTHROPIC_API_KEY="sk-ant-your-key-here"
```

### Models (Planned)

- `claude-3-opus` - Most capable
- `claude-3-sonnet` - Balanced
- `claude-3-haiku` - Fastest

---

## Pylon Swarm (Planned)

Decentralized inference via NIP-90 on the OpenAgents network.

### Configuration

No API key needed - uses your Pylon wallet for Bitcoin micropayments.

### How It Works

1. Submit inference job as NIP-90 event (kind 5050)
2. Providers bid on the job
3. Select provider and pay Lightning invoice
4. Receive result as NIP-90 result event (kind 6050)

### Benefits

- Decentralized - no single provider
- Bitcoin-native payments
- Privacy - no accounts needed
- Access to local compute (Apple FM, Ollama, etc.)

---

## Adding New Providers

To add a new provider, implement the `InferenceGateway` trait:

```rust
use async_trait::async_trait;
use crate::{
    Gateway, InferenceGateway, Capability, GatewayHealth,
    ModelInfo, ChatRequest, ChatResponse, Result,
};

pub struct MyGateway {
    client: reqwest::Client,
    api_key: String,
    endpoint: String,
}

impl Gateway for MyGateway {
    fn gateway_type(&self) -> &str { "inference" }
    fn provider(&self) -> &str { "myprovider" }
    fn name(&self) -> &str { "My Provider" }
    fn is_configured(&self) -> bool { !self.api_key.is_empty() }
    fn capabilities(&self) -> Vec<Capability> {
        vec![Capability::ChatCompletion]
    }
}

#[async_trait]
impl InferenceGateway for MyGateway {
    async fn models(&self) -> Result<Vec<ModelInfo>> {
        // Return available models
    }

    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        // Make API call and return response
    }

    async fn health(&self) -> GatewayHealth {
        // Check if API is reachable
    }
}
```

Then add it to the CLI in `crates/pylon/src/cli/gateway.rs`:

```rust
fn create_gateway(provider: &str) -> anyhow::Result<Box<dyn InferenceGateway>> {
    match provider {
        "cerebras" => Ok(Box::new(CerebrasGateway::from_env()?)),
        "myprovider" => Ok(Box::new(MyGateway::from_env()?)),
        _ => anyhow::bail!("Unknown provider"),
    }
}
```
