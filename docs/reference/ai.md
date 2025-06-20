# @openagentsinc/ai Package Specification

## Overview

The `@openagentsinc/ai` package provides a unified, Effect-based interface for integrating multiple AI providers into OpenAgents applications. Built on Effect patterns and inspired by the Effect AI framework, it offers provider-agnostic AI capabilities with support for OpenAI, Anthropic, Vercel AI SDK v5, Goose (with MCP), and Claude Code.

## Architecture

### Core Design Principles

1. **Effect-First**: All operations return Effect types with proper error handling
2. **Provider Agnostic**: Write once, run with any provider
3. **Schema-Driven**: Runtime validation and type safety via Effect Schema
4. **Layer-Based**: Clean dependency injection and composition
5. **Tool Unified**: Single tool interface works across all providers

### Package Structure

```
packages/ai/
├── src/
│   ├── index.ts                 # Public API exports
│   ├── AiService.ts            # Main service interface
│   ├── models/                 # Provider-agnostic model definitions
│   │   ├── AiModel.ts         # Based on Effect AI patterns
│   │   ├── AiPlan.ts          # Execution planning with retries/fallbacks
│   │   └── AiProvider.ts      # Provider interface
│   ├── providers/
│   │   ├── openai/            # OpenAI integration
│   │   ├── anthropic/         # Anthropic integration
│   │   ├── vercel/            # Vercel AI SDK v5 adapter
│   │   ├── goose/             # Goose CLI integration
│   │   └── claude-code/       # Claude Code SDK wrapper
│   ├── tools/                  # Tool/function calling
│   │   ├── AiToolkit.ts       # Effect AI toolkit patterns
│   │   ├── ToolAdapter.ts     # Provider-specific adapters
│   │   └── UnifiedTool.ts     # Common tool interface
│   ├── routing/
│   │   ├── Router.ts          # Intelligent provider selection
│   │   └── strategies/        # Cost, latency, capability routing
│   ├── mcp/                    # Model Context Protocol
│   │   ├── McpClient.ts       # MCP client implementation
│   │   ├── McpServer.ts       # Server lifecycle management
│   │   └── McpAbstraction.ts  # User-friendly abstractions
│   └── memory/                 # Conversation management
│       └── MemoryService.ts    # Session-based memory
├── test/
└── package.json
```

## Provider Integration Strategy

### 1. Native Effect AI Providers (OpenAI, Anthropic)

Leverage the existing Effect AI packages:

```typescript
import { OpenAiLanguageModel } from "@effect/ai-openai"
import { AnthropicLanguageModel } from "@effect/ai-anthropic"

// Use Effect AI's AiModel pattern
const Gpt4o = OpenAiLanguageModel.model("gpt-4o")
const Claude3 = AnthropicLanguageModel.model("claude-3-opus")
```

### 2. Vercel AI SDK v5 Custom Provider

Create a custom OpenAI-compatible provider:

```typescript
export class VercelAiProvider extends createCustomProvider({
  baseURL: "https://api.vercel.ai/v5",
  headers: { "X-Provider": "vercel-ai-sdk" },

  // Map Vercel's unified interface to Effect AI patterns
  async completion(params) {
    const result = await generateText({
      model: params.model,
      prompt: params.messages,
      tools: params.tools
    })
    return transformToEffectAiResponse(result)
  }
})
```

### 3. Goose Integration via CLI

Wrap Goose's CLI in non-interactive mode:

```typescript
export class GooseProvider extends Effect.Service<GooseProvider>()("ai/GooseProvider", {
  effect: Effect.gen(function*() {
    const goose = yield* Effect.acquireRelease(
      startGooseCli({ nonInteractive: true }),
      (cli) => Effect.sync(() => cli.terminate())
    )

    return {
      complete: (request) => executeGooseCommand(goose, {
        prompt: request.prompt,
        model: request.model,
        mcpServers: request.mcpServers // Optional MCP configuration
      })
    }
  })
}) {}
```

### 4. Claude Code for MAX Subscribers

Integrate Claude Code SDK with subscription verification:

```typescript
export class ClaudeCodeProvider extends Effect.Service<ClaudeCodeProvider>()("ai/ClaudeCodeProvider", {
  effect: Effect.gen(function*() {
    const apiKey = yield* Config.redacted("ANTHROPIC_API_KEY")

    // Initialize Claude Code CLI/SDK
    const claude = yield* initializeClaudeCode({
      apiKey,
      nonInteractive: true
    })

    return {
      // Special handling for code-related tasks
      complete: (request) => {
        if (isCodeTask(request)) {
          return claude.runCommand({
            prompt: request.prompt,
            continueSession: request.sessionId
          })
        }
        return claude.complete(request)
      }
    }
  })
}) {}
```

## Core Services

### AiService (Main Entry Point)

```typescript
export class AiService extends Effect.Service<AiService>()("ai/AiService", {
  effect: Effect.gen(function*() {
    const router = yield* Router
    const memory = yield* MemoryService
    const toolkit = yield* AiToolkit

    return {
      // Main completion with automatic routing
      complete: (prompt: string, options?: AiOptions) =>
        Effect.gen(function*() {
          const request = yield* buildRequest(prompt, options)
          const provider = yield* router.selectProvider(request)

          // Use Effect AI's Provider.use pattern
          return yield* provider.use(
            AiLanguageModel.generateText(request)
          )
        }),

      // Streaming with backpressure control
      stream: (prompt: string, options?: AiOptions) =>
        Effect.gen(function*() {
          const provider = yield* router.selectProvider(request)
          return yield* provider.use(
            AiLanguageModel.streamText(request)
          )
        }),

      // Execute with specific plan (retries, fallbacks)
      executeWithPlan: (plan: AiPlan, prompt: string) =>
        Effect.gen(function*() {
          const executor = yield* plan
          return yield* executor.use(
            AiLanguageModel.generateText({ prompt })
          )
        })
    }
  })
}) {}
```

### Tool Integration

Unified tool interface that adapts to each provider:

```typescript
// Define tools using Effect AI patterns
class SearchTool extends AiTool.make("search", {
  description: "Search the web",
  parameters: {
    query: Schema.String,
    limit: Schema.Number.pipe(Schema.optional)
  },
  success: Schema.Array(SearchResult),
  failure: Schema.Never
}) {}

// Create toolkit
class AppToolkit extends AiToolkit.make(SearchTool, FileSystemTool) {}

// Implement handlers
const ToolHandlers = AppToolkit.toLayer(
  Effect.gen(function*() {
    const search = yield* SearchService
    return {
      search: ({ query, limit }) => search.execute(query, limit),
      fileSystem: ({ operation, path }) => handleFileOp(operation, path)
    }
  })
)
```

### MCP Integration

Abstract MCP complexity while allowing power-user configuration:

```typescript
export class McpService extends Effect.Service<McpService>()("ai/McpService", {
  effect: Effect.gen(function*() {
    return {
      // Simple API for common use cases
      enableFileSystem: () => startMcpServer(McpPresets.filesystem),
      enableGitHub: (token: string) => startMcpServer(McpPresets.github(token)),

      // Power user API
      startCustomServer: (config: McpServerConfig) => startMcpServer(config),

      // Auto-discovery of MCP tools
      discoverTools: () => Effect.gen(function*() {
        const servers = yield* getAllMcpServers()
        const tools = yield* Effect.forEach(servers, s => s.listTools())
        return tools.flat().map(adaptMcpToolToUnified)
      })
    }
  })
}) {}
```

## Routing Strategies

### Intelligent Provider Selection

```typescript
export const RouterStrategies = {
  // Cost-optimized (default)
  costOptimized: routeByCost(),

  // Performance-optimized
  latencyOptimized: routeByLatency(),

  // Capability-based
  capabilityBased: (caps: Capability[]) => routeByCapabilities(caps),

  // Model-specific
  modelSpecific: (model: ModelId) => routeToProviderForModel(model),

  // With fallbacks
  withFallbacks: (primary: ProviderId, fallbacks: ProviderId[]) =>
    AiPlan.make(
      { model: primary, attempts: 3, while: isRetryable },
      ...fallbacks.map(id => ({ model: id, attempts: 2 }))
    )
}
```

## Example Usage

### Basic Completion

```typescript
import { AiService } from "@openagentsinc/ai"

const program = Effect.gen(function*() {
  const ai = yield* AiService

  // Automatic provider selection
  const response = yield* ai.complete("Explain quantum computing")
  console.log(response.text)

  // With specific provider
  const anthropicResponse = yield* ai.complete(
    "Write a poem",
    { preferredProvider: "anthropic" }
  )
})
```

### With Tools

```typescript
const programWithTools = Effect.gen(function*() {
  const ai = yield* AiService

  const response = yield* ai.complete(
    "Search for Effect documentation and summarize it",
    { tools: AppToolkit }
  )
})

// Provide tool implementations
programWithTools.pipe(
  Effect.provide(ToolHandlers),
  Effect.runPromise
)
```

### Advanced Execution Plan

```typescript
// Define a plan with retries and fallbacks
const ResilientPlan = AiPlan.make(
  {
    model: OpenAiLanguageModel.model("gpt-4"),
    attempts: 3,
    schedule: Schedule.exponential("100 millis"),
    while: (error) => error._tag === "NetworkError"
  },
  {
    model: AnthropicLanguageModel.model("claude-3-opus"),
    attempts: 2
  },
  {
    model: GooseProvider.model("llama-3.1-70b"),
    attempts: 1
  }
)

// Use the plan
const resilientProgram = Effect.gen(function*() {
  const ai = yield* AiService
  const response = yield* ai.executeWithPlan(
    ResilientPlan,
    "Complex task requiring high reliability"
  )
})
```

### Claude Code Integration

```typescript
const codeProgram = Effect.gen(function*() {
  const ai = yield* AiService

  // Automatically routes to Claude Code for MAX subscribers
  const refactored = yield* ai.complete(
    "Refactor this function to use Effect patterns: " + codeSnippet,
    {
      preferredProvider: "claude-code",
      sessionId: "refactoring-session-123"
    }
  )
})
```

## Configuration

### Environment Variables

```bash
# Provider API Keys
OPENAI_API_KEY=sk-...
ANTHROPIC_API_KEY=sk-ant-...
VERCEL_AI_TOKEN=...

# Claude MAX Authentication
CLAUDE_CODE_AUTH_TOKEN=...

# MCP Configuration (optional)
MCP_FILESYSTEM_ENABLED=true
MCP_GITHUB_TOKEN=ghp_...
```

### Layer Configuration

```typescript
const AiLive = Layer.mergeAll(
  // Provider configurations
  OpenAiClient.layerConfig({
    apiKey: Config.redacted("OPENAI_API_KEY")
  }),
  AnthropicClient.layerConfig({
    apiKey: Config.redacted("ANTHROPIC_API_KEY")
  }),

  // Custom provider layers
  VercelAiProvider.layer,
  GooseProvider.layer,
  ClaudeCodeProvider.layer,

  // Core services
  Router.layer,
  MemoryService.layer,
  McpService.layer,
  AiService.layer
)
```

## Testing

### Unit Test Example

```typescript
describe("AiService", () => {
  it("routes to cheapest provider", () =>
    Effect.gen(function*() {
      const ai = yield* AiService
      const response = yield* ai.complete("test prompt")

      expect(response.model).toContain("gpt-3.5")
    }).pipe(
      Effect.provide(TestProviders.layer),
      Effect.runPromise
    )
  )
})
```

### Integration Test Example

```typescript
describe("Tool Integration", () => {
  it("executes web search tool", () =>
    Effect.gen(function*() {
      const ai = yield* AiService
      const response = yield* ai.complete(
        "Search for Effect",
        { tools: SearchToolkit }
      )

      expect(response.toolCalls).toHaveLength(1)
      expect(response.content).toContain("Effect")
    }).pipe(
      Effect.provide(IntegrationTestLayers.Full),
      Effect.runPromise
    )
  )
})
```

## Performance Considerations

1. **Request Batching**: Automatic batching for multiple concurrent requests
2. **Streaming**: First-class streaming support with backpressure
3. **Caching**: Optional response caching with TTL
4. **Rate Limiting**: Built-in rate limit handling per provider
5. **Connection Pooling**: Reuse HTTP connections for efficiency

## Security Considerations

1. **API Key Management**: Use Effect Config with redacted values
2. **Request Validation**: Schema validation on all inputs
3. **Response Sanitization**: Clean provider responses
4. **MCP Sandboxing**: Run MCP servers in isolated processes
5. **Audit Logging**: Track all AI interactions

## Open Questions

1. **Provider Authentication**
   - Should we support OAuth flows for certain providers?
   - How to handle token refresh for long-running sessions?

2. **MCP Configuration**
   - Default MCP servers to enable out-of-box?
   - GUI for MCP server management?

3. **Streaming Unification**
   - How to handle provider-specific streaming formats?
   - Standardize chunk format across providers?

4. **Cost Tracking**
   - Built-in cost estimation and tracking?
   - Budget enforcement mechanisms?

5. **Caching Strategy**
   - Cache identical requests across sessions?
   - Semantic similarity caching?

## Migration Path

For teams already using AI providers directly:

```typescript
// Before: Direct OpenAI usage
const openai = new OpenAI({ apiKey })
const completion = await openai.chat.completions.create({...})

// After: Effect AI integration
const program = Effect.gen(function*() {
  const ai = yield* AiService
  const response = yield* ai.complete(prompt)
})
```

## Next Steps

1. Implement core service architecture
2. Create provider adapters starting with OpenAI/Anthropic
3. Build unified tool interface
4. Add routing strategies
5. Implement MCP abstraction layer
6. Create comprehensive test suite
7. Write migration guide
8. Performance benchmarks
