# AI Gateway Setup for Adjutant Agent

This document describes how to set up the local AI Gateway server that provides LM (Language Model) backend for the Adjutant agent's DSPy pipeline.

## Overview

The Adjutant agent uses a local bun server that leverages the Vercel AI SDK to provide a unified interface to multiple AI providers. This architecture provides:

- **Single API key**: Access hundreds of models through one endpoint
- **Provider flexibility**: Easy switching between OpenAI, Anthropic, xAI, etc.
- **High reliability**: Automatic fallbacks and retries
- **Cost optimization**: No markup on tokens, BYOK support
- **Local control**: Server runs locally within Tauri app

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    Tauri Desktop App                             │
│  ┌─────────────────┐    ┌─────────────────┐    ┌───────────────┐ │
│  │   Frontend UI   │    │  Adjutant Agent │    │ Bun AI Server │ │
│  │                 │    │                 │    │               │ │
│  │ • Chat Interface│────│ • DSPy Pipeline │────│ • Vercel AI   │ │
│  │ • Agent Selector│    │ • Plan Mode     │    │ • Multi-LLM   │ │
│  │ • Stream Events │    │ • Signatures    │    │ • Local HTTP  │ │
│  └─────────────────┘    └─────────────────┘    └───────────────┘ │
│                                 │                        │       │
│                                 │                        │       │
│                          ┌─────────────┐         ┌─────────────┐ │
│                          │    dsrs     │         │   Server    │ │
│                          │ Predictors  │         │  Process    │ │
│                          │ Signatures  │         │ Management  │ │
│                          └─────────────┘         └─────────────┘ │
└─────────────────────────────────────────────────────────────────┘
                                   │
                                   ▼
                      ┌─────────────────────────────┐
                      │     Vercel AI Gateway       │
                      │  https://ai-gateway.vercel  │
                      │                             │
                      │ ┌─────────┐ ┌─────────────┐ │
                      │ │ OpenAI  │ │ Anthropic   │ │
                      │ └─────────┘ └─────────────┘ │
                      │ ┌─────────┐ ┌─────────────┐ │
                      │ │   xAI   │ │   Other     │ │
                      │ └─────────┘ └─────────────┘ │
                      └─────────────────────────────┘
```

## Setup Instructions

### 1. Prerequisites

- **Bun runtime** installed on system
- **AI Gateway API key** from Vercel
- **Tauri development environment** (already configured)

### 2. Environment Configuration

Create or update `.env` file in project root:

```bash
# Vercel AI Gateway Configuration
AI_GATEWAY_API_KEY=your_vercel_ai_gateway_api_key_here
AI_GATEWAY_BASE_URL=https://ai-gateway.vercel.sh/v1

# Local AI Server Configuration  
AI_SERVER_PORT=3001
AI_SERVER_HOST=localhost

# Default Model Configuration
DEFAULT_LLM_PROVIDER=anthropic
DEFAULT_LLM_MODEL=claude-sonnet-4.5
FALLBACK_LLM_MODEL=openai/gpt-4o

# DSPy Configuration
DSPY_LM_ENDPOINT=http://localhost:3001
DSPY_MAX_TOKENS=4096
DSPY_TEMPERATURE=0.7
```

### 3. Directory Structure

```
autopilot/
├── ai-server/                 # Local AI Gateway server
│   ├── package.json          # Bun dependencies
│   ├── server.ts             # Main server implementation
│   ├── routes/               # API routes
│   │   ├── chat.ts          # Chat completions endpoint
│   │   ├── embeddings.ts    # Embeddings endpoint
│   │   └── health.ts        # Health check endpoint
│   ├── middleware/           # Server middleware
│   │   ├── auth.ts          # API key validation
│   │   ├── cors.ts          # CORS configuration
│   │   └── logging.ts       # Request logging
│   ├── services/            # Business logic
│   │   ├── ai-gateway.ts    # Vercel AI SDK integration
│   │   ├── model-config.ts  # Model routing configuration
│   │   └── fallback.ts      # Provider fallback logic
│   └── types/               # TypeScript type definitions
│       ├── api.ts           # API request/response types
│       └── config.ts        # Configuration types
├── src-tauri/
│   └── src/
│       └── ai_server/       # Tauri server management
│           ├── mod.rs       # Server lifecycle management
│           ├── process.rs   # Bun process spawning
│           └── config.rs    # Server configuration
└── docs/
    └── ai-gateway-setup.md  # This documentation
```

## API Endpoints

### Chat Completions

```typescript
POST /v1/chat/completions
Content-Type: application/json
Authorization: Bearer <token>

{
  "model": "anthropic/claude-sonnet-4.5",
  "messages": [
    {
      "role": "user", 
      "content": "Decompose this task into exploration topics..."
    }
  ],
  "stream": false,
  "max_tokens": 4096,
  "temperature": 0.7
}
```

### Embeddings (for future use)

```typescript
POST /v1/embeddings
Content-Type: application/json
Authorization: Bearer <token>

{
  "model": "openai/text-embedding-3-large",
  "input": ["Text to embed", "Another text"]
}
```

### Health Check

```typescript
GET /health

Response: 
{
  "status": "healthy",
  "timestamp": "2025-01-25T...",
  "models": ["anthropic/claude-sonnet-4.5", "openai/gpt-4o"],
  "uptime": 12345
}
```

## Model Configuration

### Supported Providers

| Provider | Models | Use Case |
|----------|---------|----------|
| **Anthropic** | `claude-sonnet-4.5`, `claude-haiku-3.5` | Planning, reasoning |
| **OpenAI** | `gpt-4o`, `gpt-4o-mini` | Fast responses, fallback |
| **xAI** | `grok-4` | Alternative reasoning |
| **Google** | `gemini-2.0-flash-exp` | Multimodal tasks |

### Routing Configuration

```typescript
// ai-server/services/model-config.ts
export const MODEL_ROUTING = {
  // Primary models for different DSPy signatures
  planning: {
    primary: "anthropic/claude-sonnet-4.5",
    fallback: "openai/gpt-4o",
    config: { temperature: 0.3, max_tokens: 4096 }
  },
  
  exploration: {
    primary: "anthropic/claude-haiku-3.5", 
    fallback: "openai/gpt-4o-mini",
    config: { temperature: 0.5, max_tokens: 2048 }
  },
  
  synthesis: {
    primary: "anthropic/claude-sonnet-4.5",
    fallback: "openai/gpt-4o", 
    config: { temperature: 0.7, max_tokens: 8192 }
  }
}
```

## DSPy Integration

### LM Configuration

The DSPy system connects to the local AI server through a custom LM implementation:

```rust
// src-tauri/src/agent/adjutant/lm_client.rs
pub struct LocalAiLM {
    base_url: String,
    api_key: String,
    client: reqwest::Client,
}

impl LM for LocalAiLM {
    async fn generate(&self, request: &CompletionRequest) -> Result<String> {
        let response = self.client
            .post(&format!("{}/v1/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&ChatCompletionRequest::from(request))
            .send()
            .await?;
            
        let completion: ChatCompletionResponse = response.json().await?;
        Ok(completion.choices[0].message.content.clone())
    }
}
```

### Pipeline Configuration

```rust
// Configure Adjutant with local LM
let lm = Arc::new(LocalAiLM::new(
    "http://localhost:3001".to_string(),
    "local-api-key".to_string(),
));

let pipeline = PlanModePipeline::new(workspace_path, config)
    .with_lm(lm);
```

## Server Lifecycle Management

### Tauri Server Startup

```rust
// src-tauri/src/ai_server/mod.rs
pub struct AiServerManager {
    process: Option<Child>,
    port: u16,
    host: String,
}

impl AiServerManager {
    pub async fn start(&mut self) -> Result<(), String> {
        // 1. Check if port is available
        self.ensure_port_available().await?;
        
        // 2. Spawn bun server process
        let mut cmd = Command::new("bun")
            .args(&["run", "server.ts"])
            .current_dir("ai-server")
            .env("PORT", self.port.to_string())
            .env("HOST", &self.host)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to start AI server: {}", e))?;
            
        // 3. Wait for server to be ready
        self.wait_for_ready().await?;
        self.process = Some(cmd);
        
        Ok(())
    }
    
    pub async fn stop(&mut self) -> Result<(), String> {
        if let Some(mut process) = self.process.take() {
            process.kill().map_err(|e| format!("Failed to stop server: {}", e))?;
        }
        Ok(())
    }
}
```

### App Initialization Sequence

```rust
// src-tauri/src/main.rs
#[tauri::command]
async fn app_ready(app: AppHandle) -> Result<(), String> {
    let ai_server = app.state::<Mutex<AiServerManager>>();
    let mut server = ai_server.lock().unwrap();
    
    // Start AI server on app ready
    server.start().await?;
    
    println!("AI Gateway server started on http://localhost:3001");
    Ok(())
}
```

## Error Handling & Resilience

### Server Health Monitoring

```typescript
// ai-server/middleware/health.ts
export class HealthMonitor {
  private lastCheck: Date = new Date();
  private isHealthy: boolean = true;
  
  async checkHealth(): Promise<HealthStatus> {
    try {
      // Test connection to Vercel AI Gateway
      const response = await fetch('https://ai-gateway.vercel.sh/v1/models');
      this.isHealthy = response.ok;
    } catch (error) {
      this.isHealthy = false;
    }
    
    this.lastCheck = new Date();
    return {
      status: this.isHealthy ? 'healthy' : 'unhealthy',
      lastCheck: this.lastCheck,
      uptime: process.uptime()
    };
  }
}
```

### Automatic Fallbacks

```typescript
// ai-server/services/fallback.ts
export class FallbackService {
  async generateWithFallback(request: ChatRequest): Promise<string> {
    const models = [request.model, ...this.getFallbackModels(request.model)];
    
    for (const model of models) {
      try {
        return await this.generateWithModel({ ...request, model });
      } catch (error) {
        console.warn(`Model ${model} failed, trying fallback:`, error);
        continue;
      }
    }
    
    throw new Error('All fallback models failed');
  }
}
```

## Security Considerations

### API Key Management

- **Environment Variables**: Store API keys in `.env` (not committed)
- **Local Authentication**: Use simple token-based auth for local server
- **No External Exposure**: Server only binds to localhost
- **Process Isolation**: Server runs as separate process, can be restarted

### Rate Limiting

```typescript
// ai-server/middleware/rate-limit.ts
export class RateLimiter {
  private requests: Map<string, number[]> = new Map();
  
  isAllowed(clientId: string, maxRequests: number = 60): boolean {
    const now = Date.now();
    const window = 60 * 1000; // 1 minute window
    
    const clientRequests = this.requests.get(clientId) || [];
    const recentRequests = clientRequests.filter(time => now - time < window);
    
    if (recentRequests.length >= maxRequests) {
      return false;
    }
    
    recentRequests.push(now);
    this.requests.set(clientId, recentRequests);
    return true;
  }
}
```

## Testing & Development

### Local Testing

```bash
# Start server manually for testing
cd ai-server
bun install
bun run dev

# Test endpoints
curl -X POST "http://localhost:3001/v1/chat/completions" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer test-key" \
  -d '{
    "model": "anthropic/claude-sonnet-4.5",
    "messages": [{"role": "user", "content": "Hello!"}]
  }'
```

### Integration Testing

```rust
// tests/ai_server_integration.rs
#[tokio::test]
async fn test_adjutant_planning_with_local_lm() {
    let server = AiServerManager::new("localhost".to_string(), 3001);
    server.start().await.expect("Server should start");
    
    let pipeline = PlanModePipeline::new(
        test_repo_path(),
        PlanModeConfig::default()
    ).with_lm(Arc::new(LocalAiLM::new(
        "http://localhost:3001".to_string(),
        "test-key".to_string(),
    )));
    
    let result = pipeline.execute_plan_mode("Add user authentication").await;
    assert!(result.is_ok());
    
    server.stop().await.expect("Server should stop");
}
```

## Monitoring & Observability

### Request Logging

```typescript
// ai-server/middleware/logging.ts
export function requestLogger(req: Request, res: Response, next: NextFunction) {
  const start = Date.now();
  
  res.on('finish', () => {
    const duration = Date.now() - start;
    console.log({
      method: req.method,
      url: req.url,
      status: res.statusCode,
      duration: `${duration}ms`,
      model: req.body?.model,
      tokens: res.locals?.usage?.total_tokens
    });
  });
  
  next();
}
```

### Usage Analytics

```typescript
// ai-server/services/analytics.ts
export class UsageAnalytics {
  private usage: Map<string, ModelUsage> = new Map();
  
  recordUsage(model: string, tokens: TokenUsage) {
    const current = this.usage.get(model) || { requests: 0, tokens: { input: 0, output: 0 } };
    current.requests++;
    current.tokens.input += tokens.input;
    current.tokens.output += tokens.output;
    this.usage.set(model, current);
  }
  
  getUsageSummary(): UsageSummary {
    return {
      totalRequests: Array.from(this.usage.values()).reduce((sum, usage) => sum + usage.requests, 0),
      totalTokens: Array.from(this.usage.values()).reduce((sum, usage) => sum + usage.tokens.input + usage.tokens.output, 0),
      modelBreakdown: Object.fromEntries(this.usage)
    };
  }
}
```

## Troubleshooting

### Common Issues

| Issue | Cause | Solution |
|-------|-------|----------|
| Server won't start | Port already in use | Change `AI_SERVER_PORT` in .env |
| API key errors | Invalid/missing AI Gateway key | Check `AI_GATEWAY_API_KEY` in .env |
| Connection timeouts | Vercel API issues | Check network/firewall settings |
| Model not found | Invalid model name | Check [supported models](https://vercel.com/ai-gateway/models) |

### Debug Commands

```bash
# Check if server is running
curl http://localhost:3001/health

# Check server logs
tail -f ai-server/logs/server.log

# Test specific model
curl -X POST "http://localhost:3001/v1/chat/completions" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $AI_GATEWAY_API_KEY" \
  -d '{"model": "anthropic/claude-sonnet-4.5", "messages": [{"role": "user", "content": "test"}]}'
```

---

This setup provides a robust, local AI Gateway that seamlessly integrates with the Adjutant agent's DSPy pipeline while maintaining flexibility and reliability through the Vercel AI SDK.
