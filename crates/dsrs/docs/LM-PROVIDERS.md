# LM Providers

- **Status:** Accurate
- **Last verified:** d44f9cd3f
- **Source of truth:** `crates/dsrs/src/lm/`
- **Doc owner:** dsrs
- **If this doc conflicts with code, code wins.**

dsrs supports 14+ LM providers via rig-core, with special integrations for Codex SDK, Pylon, and GPT-OSS.

## Provider Hierarchy

```
┌─────────────────────────────────────────────────────────────┐
│                     DSRS LM PROVIDERS                        │
├─────────────────────────────────────────────────────────────┤
│  HIGH PRIORITY (Premium)                                     │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐          │
│  │ Codex SDK  │  │  Codex API │  │   GPT-4     │          │
│  │  (headless) │  │  (direct)   │  │  (OpenAI)   │          │
│  └─────────────┘  └─────────────┘  └─────────────┘          │
│                                                              │
│  MEDIUM PRIORITY (Balanced)                                  │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐          │
│  │   Gemini    │  │    Groq     │  │  Cerebras   │          │
│  │  (Google)   │  │   (fast)    │  │   (fast)    │          │
│  └─────────────┘  └─────────────┘  └─────────────┘          │
│                                                              │
│  LOW PRIORITY (Local/Cheap)                                  │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐          │
│  │   Ollama    │  │   Pylon     │  │  GPT-OSS    │          │
│  │  (local)    │  │(swarm/local)│  │ (llama.cpp) │          │
│  └─────────────┘  └─────────────┘  └─────────────┘          │
└─────────────────────────────────────────────────────────────┘
```

## Basic Configuration

```rust
use dsrs::prelude::*;

// Simple configuration
dsrs::configure(LM::new("codex-3-sonnet"));

// With caching
dsrs::configure(LM::new("codex-3-sonnet").with_cache(true));

// With custom adapter
dsrs::configure_with_adapter(
    LM::new("codex-3-sonnet"),
    MyCustomAdapter,
);
```

## Codex SDK Provider

## GPT-OSS Provider

Local inference via llama-server with OpenAI's open-weight gpt-oss models.

```rust
use dsrs::prelude::*;

// Using model string format
let lm = LM::builder()
    .model("gptoss:gpt-oss-20b".to_string())
    .temperature(0.3)
    .max_tokens(2048)
    .build()
    .await?;

dsrs::configure(lm, ChatAdapter);

// Or using base_url for OpenAI-compatible endpoint
let lm = LM::builder()
    .base_url("http://localhost:8000".to_string())
    .model("gpt-oss-20b".to_string())
    .build()
    .await?;
```

**Features:**

- **Structured Output**: Uses `/v1/chat/completions` with `response_format` for JSON schema constraints
- **Auto-start**: Arrow test binary can auto-start llama-server if not running
- **GBNF Grammar**: JSON schemas converted to GBNF for guaranteed output format
- **Harmony Format**: Supports GPT-OSS Harmony prompt format for optimal results

**Environment Variables:**

| Variable | Description |
|----------|-------------|
| `GPTOSS_BASE_URL` | Base URL for llama-server (default: `http://localhost:8000`) |
| `LLAMA_MODEL_PATH` | Path to `.gguf` model file |

## Pylon Provider

Distributed inference via Pylon swarm or local backends.

```rust
use dsrs::core::lm::pylon::PylonLM;

// Swarm mode (distributed)
let lm = PylonLM::new_swarm("wss://nexus.openagents.com")
    .with_budget_msats(10000)
    .with_timeout(Duration::from_secs(30));

// Local mode (Ollama/llama.cpp)
let lm = PylonLM::new_local()
    .with_backend("ollama")
    .with_model("llama2");

// Hybrid mode (try local first, fallback to swarm)
let lm = PylonLM::new_hybrid()
    .with_local_timeout(Duration::from_secs(5))
    .with_swarm_fallback(true);

dsrs::configure(lm.into());
```

**Modes:**

| Mode | Use Case | Cost |
|------|----------|------|
| Swarm | Distributed inference | ~10-100 msats/call |
| Local | Privacy-sensitive | Free |
| Hybrid | Best of both | Variable |

## Multi-Provider (LaneMux)

Automatic provider selection based on availability and cost.

```rust
use dsrs::core::lm::LaneMux;

let mux = LaneMux::new()
    .add_lane("fast", GroqLM::new())
    .add_lane("cheap", PylonLM::new_swarm("wss://nexus.openagents.com"))
    .add_lane("local", PylonLM::new_local());

// Auto-detect available providers
let mux = LaneMux::auto_detect().await?;

dsrs::configure(mux.into());
```

**Lane Selection:**

```rust
// Explicit lane selection
let result = predictor.forward_with_lane(inputs, "premium").await?;

// Automatic selection
let result = predictor.forward(inputs).await?;  // Uses best available
```

## Provider Comparison

| Provider | Speed | Cost | Quality | Local |
|----------|-------|------|---------|-------|
| GPT-4 | Medium | $$$ | Excellent | No |
| Gemini Pro | Fast | $$ | Good | No |
| Groq | Very Fast | $ | Good | No |
| Cerebras | Very Fast | $ | Good | No |
| Ollama | Variable | Free | Variable | Yes |
| GPT-OSS | Variable | Free | Good | Yes |
| Pylon Swarm | Medium | ¢ | Variable | No |
| Pylon Local | Variable | Free | Variable | Yes |

## Cost Tracking

All providers track usage:

```rust
let result = predictor.forward(inputs).await?;

println!("Tokens: {}", result.lm_usage.total_tokens);
println!("Cost: {} msats", result.lm_usage.cost_msats);
```

**Cost aggregation:**

```rust
// Track across multiple calls
let mut total_usage = LmUsage::default();

for input in inputs_batch {
    let result = predictor.forward(input).await?;
    total_usage = total_usage + result.lm_usage;
}

println!("Total cost: {} msats", total_usage.cost_msats);
println!("Total tokens: {}", total_usage.total_tokens);
```

## Provider-Specific Features

### OpenAI/Codex

```rust
let lm = LM::new("codex-3-opus")
    .with_max_tokens(4096)
    .with_temperature(0.7);
```

### OpenAI/GPT

```rust
let lm = LM::new("gpt-4-turbo")
    .with_max_tokens(4096)
    .with_temperature(0.7)
    .with_seed(42);  // Deterministic
```

### Ollama

```rust
let lm = LM::new("ollama/llama2")
    .with_base_url("http://localhost:11434")
    .with_num_ctx(8192);
```

### Groq

```rust
let lm = LM::new("groq/mixtral-8x7b")
    .with_max_tokens(32768);  // Large context
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `OPENAI_API_KEY` | OpenAI API key |
| `GOOGLE_API_KEY` | Google AI API key |
| `GROQ_API_KEY` | Groq API key |
| `OLLAMA_HOST` | Ollama server URL |
| `PYLON_RELAY_URL` | Pylon relay URL |
| `GPTOSS_BASE_URL` | GPT-OSS server URL (default: `http://localhost:8000`) |
| `LLAMA_MODEL_PATH` | Path to `.gguf` model file for auto-discovery |

## Error Handling

```rust
match predictor.forward(inputs).await {
    Ok(result) => { /* success */ }
    Err(e) if e.is::<RateLimitError>() => {
        // Retry with backoff
    }
    Err(e) if e.is::<QuotaExceededError>() => {
        // Switch to cheaper provider
    }
    Err(e) => {
        // Log and propagate
    }
}
```

## Best Practices

1. **Use LaneMux for production** - Auto-fallback on failures
2. **Set budget limits** - Prevent runaway costs
3. **Enable caching** - Avoid duplicate calls
4. **Monitor usage** - Track cost_msats across calls
5. **Use local for development** - Free and fast iteration
6. **Use swarm for optimization** - Cheap, high-volume calls
