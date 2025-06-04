Claude deep research. It didn't have all relevant file contents so take the following as recommendations that may need tweaking or ignoring.

---

I've completed the comprehensive research and created both the specification document and GitHub issue for the @openagentsinc/ai package integration.

## Key Deliverables:

### 1. **Specification Document (docs/ai.md)**
A thorough specification covering:
- **Architecture Overview**: Following OpenAgents' modular patterns with Effect.js integration
- **Provider Abstraction**: Unified interface supporting OpenAI, Anthropic, Vercel AI SDK v5, Goose, and Claude Code
- **Tool/Function Calling**: Unified interface across all providers with Effect-based execution
- **MCP Integration**: Abstracted by default with power-user configuration options
- **Routing & Orchestration**: Intelligent provider selection based on capabilities, cost, and availability
- **Schema Definitions**: Complete domain models using Effect Schema
- **Service Implementations**: Registry, router, orchestrator, and memory services
- **Testing Strategy**: Comprehensive unit and integration testing approaches
- **Open Questions**: Important considerations for implementation decisions

### 2. **GitHub Issue**
A detailed implementation plan with:
- **5-Phase Approach**: Foundation → Providers → Services → Advanced Features → Testing/Docs
- **Clear Deliverables**: Specific tasks for each phase
- **Success Criteria**: Measurable goals for completion
- **Technical Requirements**: Dependencies and environment specifications
- **Team Structure**: Suggested team assignments and timeline

## Key Architectural Decisions:

1. **Effect-First Design**: All operations return Effect types with proper error handling and dependency injection
2. **Provider Pattern**: Each provider in its own folder with shared adapters for common functionality
3. **Service Layer**: Clean separation between business logic and provider implementations
4. **MCP Abstraction**: Hidden by default but fully configurable for advanced users
5. **Unified Tool Interface**: Single tool calling interface that works across all providers
6. **Intelligent Routing**: Multiple strategies for provider selection (cost, performance, capability-based)

## Notable Features:

- **Claude Code Integration**: Special support for MAX subscribers with CLI wrapping
- **Goose MCP Support**: Full MCP server integration with extension ecosystem
- **Vercel AI SDK v5**: Adapter pattern to leverage the new V2 interfaces
- **Streaming Support**: First-class streaming with Effect Streams
- **Memory Management**: Session-based conversation memory with summarization
- **Type Safety**: Comprehensive schemas and runtime validation

The specification provides a solid foundation for building a production-ready AI integration package that follows best practices from both the OpenAgents and Effect.js ecosystems.

---

# OpenAgents AI Package Specification and Implementation Plan

## Executive Summary

This document outlines the architecture, design, and implementation plan for `@openagentsinc/ai`, a fully Effect-based AI integration package for the OpenAgents monorepo. The package will provide a unified interface for multiple AI providers including OpenAI, Anthropic, Vercel AI SDK v5, Goose (with MCP support), and Claude Code for MAX subscribers.

## Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Provider Integration Strategy](#provider-integration-strategy)
4. [Core Components](#core-components)
5. [Schema Definitions](#schema-definitions)
6. [Service Implementations](#service-implementations)
7. [Provider Implementations](#provider-implementations)
8. [Tool/Function Calling](#toolfunction-calling)
9. [MCP Integration](#mcp-integration)
10. [Routing and Orchestration](#routing-and-orchestration)
11. [Memory Management](#memory-management)
12. [Testing Strategy](#testing-strategy)
13. [Implementation Phases](#implementation-phases)
14. [Open Questions](#open-questions)
15. [Technical Decisions](#technical-decisions)
16. [GitHub Issue Template](#github-issue-template)

## Overview

### Goals

1. **Unified AI Interface**: Single API for interacting with multiple AI providers
2. **Effect-First Design**: Full integration with Effect.js patterns and best practices
3. **Provider Flexibility**: Support for OpenAI, Anthropic, Vercel AI SDK v5, Goose, and Claude Code
4. **Tool Unification**: Common interface for function/tool calling across providers
5. **MCP Support**: Model Context Protocol integration (abstracted by default)
6. **Intelligent Routing**: Smart provider selection based on capabilities, cost, and availability

### Non-Goals

1. Training or fine-tuning models
2. Implementing AI algorithms from scratch
3. Building a UI framework (that's for `@openagentsinc/ui`)
4. Direct model hosting

## Architecture

Following the OpenAgents architecture patterns from `docs/architecture.md`, the AI package will be structured as:

```
packages/ai/
├── src/
│   ├── index.ts                 # Main exports
│   ├── AiService.ts            # Core AI service
│   ├── providers/              # Provider implementations
│   │   ├── openai/
│   │   ├── anthropic/
│   │   ├── vercel-sdk/
│   │   ├── goose/
│   │   └── claude-code/
│   ├── routing/                # Provider routing logic
│   │   ├── Router.ts
│   │   └── strategies/
│   ├── tools/                  # Tool/function calling
│   │   ├── ToolRegistry.ts
│   │   ├── ToolExecutor.ts
│   │   └── adapters/
│   ├── mcp/                    # MCP integration
│   │   ├── McpClient.ts
│   │   └── McpServer.ts
│   ├── memory/                 # Conversation memory
│   │   └── MemoryService.ts
│   └── utils/                  # Shared utilities
├── test/
└── package.json
```

### Domain Layer Extensions

In `@openagentsinc/domain/src/ai/`:

```typescript
// Core AI types that other packages might need
export interface AiRequest extends Schema.Schema<AiRequest> {
  readonly prompt: string
  readonly model: ModelIdentifier
  readonly temperature?: number
  readonly maxTokens?: number
  readonly tools?: ReadonlyArray<AiTool>
  readonly systemPrompt?: string
}

export interface AiResponse extends Schema.Schema<AiResponse> {
  readonly content: string
  readonly usage: TokenUsage
  readonly model: ModelIdentifier
  readonly toolCalls?: ReadonlyArray<ToolCall>
  readonly finishReason: FinishReason
}

export interface AiStreamResponse extends Schema.Schema<AiStreamResponse> {
  readonly stream: Stream.Stream<AiChunk, AiError>
}
```

## Provider Integration Strategy

### 1. Native SDKs with Effect Wrappers

For OpenAI and Anthropic, we'll wrap their official SDKs:

```typescript
// OpenAI Provider
export class OpenAiProvider extends Effect.Service<OpenAiProvider>()("ai/OpenAiProvider", {
  effect: Effect.gen(function*() {
    const config = yield* OpenAiConfig
    const client = new OpenAI({ apiKey: config.apiKey })

    return {
      complete: (request: AiRequest) =>
        Effect.tryPromise({
          try: () => client.chat.completions.create({
            model: request.model,
            messages: [{ role: "user", content: request.prompt }],
            temperature: request.temperature,
            max_tokens: request.maxTokens
          }),
          catch: (error) => new OpenAiError({ error })
        }).pipe(
          Effect.map(transformToAiResponse)
        ),

      stream: (request: AiRequest) =>
        Effect.gen(function*() {
          const stream = yield* Effect.tryPromise({
            try: () => client.chat.completions.create({
              model: request.model,
              messages: [{ role: "user", content: request.prompt }],
              stream: true
            }),
            catch: (error) => new OpenAiError({ error })
          })

          return Stream.fromAsyncIterable(stream, (error) => new StreamError({ error }))
            .pipe(Stream.map(transformChunk))
        })
    }
  })
}) {}
```

### 2. Vercel AI SDK v5 Integration

Leverage the new v5 architecture with custom providers:

```typescript
// Vercel SDK Adapter
import { createOpenAI, createAnthropic } from '@ai-sdk/openai'
import { streamText, generateText } from 'ai'

export class VercelSdkProvider extends Effect.Service<VercelSdkProvider>()("ai/VercelSdkProvider", {
  effect: Effect.gen(function*() {
    const config = yield* AiConfig

    // Initialize multiple model providers
    const providers = {
      openai: createOpenAI({ apiKey: config.openaiKey }),
      anthropic: createAnthropic({ apiKey: config.anthropicKey })
    }

    return {
      complete: (request: AiRequest) =>
        Effect.tryPromise({
          try: async () => {
            const provider = getProviderForModel(request.model)
            const result = await generateText({
              model: provider(request.model),
              prompt: request.prompt,
              temperature: request.temperature,
              maxTokens: request.maxTokens,
              tools: request.tools ? transformTools(request.tools) : undefined
            })
            return result
          },
          catch: (error) => new VercelSdkError({ error })
        }).pipe(Effect.map(transformVercelResponse)),

      stream: (request: AiRequest) =>
        Effect.gen(function*() {
          const provider = getProviderForModel(request.model)
          const result = yield* Effect.tryPromise({
            try: () => streamText({
              model: provider(request.model),
              prompt: request.prompt,
              temperature: request.temperature
            }),
            catch: (error) => new VercelSdkError({ error })
          })

          return Stream.fromAsyncIterable(
            result.textStream,
            (error) => new StreamError({ error })
          )
        })
    }
  })
}) {}
```

### 3. Goose Integration with MCP

Goose will be wrapped as a custom provider with MCP support:

```typescript
export class GooseProvider extends Effect.Service<GooseProvider>()("ai/GooseProvider", {
  dependencies: [McpService.Default],
  effect: Effect.gen(function*() {
    const mcp = yield* McpService
    const config = yield* GooseConfig

    // Initialize Goose CLI in non-interactive mode
    const goose = yield* Effect.acquireRelease(
      Effect.tryPromise({
        try: () => spawn('goose', ['--non-interactive', '--json'], {
          stdio: ['pipe', 'pipe', 'pipe']
        }),
        catch: (error) => new GooseInitError({ error })
      }),
      (process) => Effect.sync(() => process.kill())
    )

    return {
      complete: (request: AiRequest) =>
        Effect.gen(function*() {
          // If tools are MCP tools, use MCP protocol
          if (request.tools?.some(isMcpTool)) {
            return yield* mcp.executeWithTools(request)
          }

          // Otherwise use standard Goose completion
          return yield* executeGooseCommand(goose, {
            command: 'complete',
            prompt: request.prompt,
            model: request.model,
            temperature: request.temperature
          })
        }),

      // MCP-specific methods
      listTools: () => mcp.listAvailableTools(),
      executeToolCall: (toolCall: ToolCall) => mcp.executeTool(toolCall)
    }
  })
}) {}
```

### 4. Claude Code Integration

Special handling for MAX subscribers:

```typescript
export class ClaudeCodeProvider extends Effect.Service<ClaudeCodeProvider>()("ai/ClaudeCodeProvider", {
  effect: Effect.gen(function*() {
    const auth = yield* ClaudeMaxAuth

    // Verify MAX subscription
    const isMaxSubscriber = yield* auth.verifyMaxSubscription()
    if (!isMaxSubscriber) {
      return yield* Effect.fail(new NotMaxSubscriberError())
    }

    // Initialize Claude Code SDK/CLI
    const claudeCode = yield* Effect.acquireRelease(
      Effect.tryPromise({
        try: () => initializeClaudeCodeCli({
          authToken: auth.token,
          nonInteractive: true
        }),
        catch: (error) => new ClaudeCodeInitError({ error })
      }),
      (cli) => Effect.sync(() => cli.cleanup())
    )

    return {
      complete: (request: AiRequest) =>
        Effect.gen(function*() {
          // Claude Code has special handling for code-related prompts
          const isCodeTask = yield* detectCodeTask(request.prompt)

          if (isCodeTask) {
            return yield* executeClaudeCodeTask(claudeCode, request)
          }

          // Fall back to standard completion
          return yield* executeStandardCompletion(claudeCode, request)
        }),

      // Claude Code specific features
      createProject: (spec: ProjectSpec) =>
        executeClaudeCodeCommand(claudeCode, 'create-project', spec),

      refactorCode: (code: string, instructions: string) =>
        executeClaudeCodeCommand(claudeCode, 'refactor', { code, instructions })
    }
  })
}) {}
```

## Core Components

### 1. AiService (Main Entry Point)

```typescript
export class AiService extends Effect.Service<AiService>()("ai/AiService", {
  dependencies: [
    ProviderRegistry.Default,
    Router.Default,
    MemoryService.Default,
    ToolExecutor.Default
  ],
  effect: Effect.gen(function*() {
    const providers = yield* ProviderRegistry
    const router = yield* Router
    const memory = yield* MemoryService
    const toolExecutor = yield* ToolExecutor

    return {
      // Main completion method with routing
      complete: (request: AiRequest, options?: AiOptions) =>
        Effect.gen(function*() {
          // Add conversation context if available
          const contextualRequest = yield* memory.enhanceRequest(request)

          // Route to appropriate provider
          const provider = yield* router.selectProvider(contextualRequest, options)

          // Execute request
          const response = yield* provider.complete(contextualRequest)

          // Store in memory
          yield* memory.store(contextualRequest, response)

          // Handle tool calls if present
          if (response.toolCalls?.length > 0) {
            const toolResults = yield* toolExecutor.executeAll(response.toolCalls)
            // Continue conversation with tool results
            return yield* complete({
              ...request,
              prompt: formatToolResults(toolResults)
            })
          }

          return response
        }),

      // Streaming variant
      stream: (request: AiRequest, options?: AiOptions) =>
        Effect.gen(function*() {
          const provider = yield* router.selectProvider(request, options)
          return yield* provider.stream(request)
        }),

      // List available models across all providers
      listModels: () =>
        Effect.gen(function*() {
          const allProviders = yield* providers.getAll()
          return yield* Effect.forEach(
            allProviders,
            (provider) => provider.listModels(),
            { concurrency: "unbounded" }
          ).pipe(Effect.map(models => models.flat()))
        }),

      // Tool management
      registerTool: (tool: AiTool) => toolExecutor.register(tool),

      // Memory management
      clearMemory: () => memory.clear(),
      getConversationHistory: () => memory.getHistory()
    }
  })
}) {}
```

### 2. Provider Registry

```typescript
export class ProviderRegistry extends Effect.Service<ProviderRegistry>()("ai/ProviderRegistry", {
  effect: Effect.gen(function*() {
    const providers = yield* Ref.make<Map<ProviderId, AiProvider>>(new Map())

    return {
      register: (id: ProviderId, provider: AiProvider) =>
        Ref.update(providers, map => new Map(map).set(id, provider)),

      get: (id: ProviderId) =>
        Ref.get(providers).pipe(
          Effect.map(map => map.get(id)),
          Effect.flatMap(Option.match({
            onNone: () => Effect.fail(new ProviderNotFoundError({ id })),
            onSome: Effect.succeed
          }))
        ),

      getAll: () => Ref.get(providers).pipe(Effect.map(map => Array.from(map.values()))),

      getByCapability: (capability: Capability) =>
        Ref.get(providers).pipe(
          Effect.map(map =>
            Array.from(map.values()).filter(p => p.capabilities.includes(capability))
          )
        )
    }
  })
}) {}
```

## Schema Definitions

### Core Schemas

```typescript
// Model identifiers
export const ModelIdentifier = Schema.Union(
  Schema.Literal("gpt-4", "gpt-4-turbo", "gpt-3.5-turbo"),
  Schema.Literal("claude-3-opus", "claude-3-sonnet", "claude-3-haiku"),
  Schema.Literal("llama-3.1-70b", "mistral-large"),
  Schema.String // Allow custom models
)

// Token usage tracking
export class TokenUsage extends Schema.Class<TokenUsage>("TokenUsage")({
  promptTokens: Schema.Number,
  completionTokens: Schema.Number,
  totalTokens: Schema.Number
}) {}

// Tool/Function definitions
export class AiTool extends Schema.Class<AiTool>("AiTool")({
  name: Schema.NonEmptyString,
  description: Schema.String,
  parameters: Schema.Record(Schema.String, Schema.Any),
  handler: Schema.optional(Schema.Any) // Will be Effect function
}) {}

// Tool calls from AI
export class ToolCall extends Schema.Class<ToolCall>("ToolCall")({
  id: Schema.String,
  name: Schema.String,
  arguments: Schema.Record(Schema.String, Schema.Any)
}) {}

// Streaming chunks
export class AiChunk extends Schema.Class<AiChunk>("AiChunk")({
  content: Schema.String,
  isFinished: Schema.Boolean,
  toolCallDelta: Schema.optional(ToolCall)
}) {}

// Error types
export class AiError extends Data.TaggedError("AiError")<{
  readonly message: string
  readonly cause?: unknown
}> {}

export class ProviderError extends Data.TaggedError("ProviderError")<{
  readonly provider: ProviderId
  readonly originalError: unknown
}> {}

export class ToolExecutionError extends Data.TaggedError("ToolExecutionError")<{
  readonly toolName: string
  readonly error: unknown
}> {}
```

### Provider-Specific Schemas

```typescript
// MCP-specific types
export class McpTool extends Schema.Class<McpTool>("McpTool")({
  name: Schema.String,
  description: Schema.String,
  inputSchema: Schema.Any, // JSON Schema
  serverName: Schema.String,
  serverVersion: Schema.String
}) {}

export class McpServerConfig extends Schema.Class<McpServerConfig>("McpServerConfig")({
  command: Schema.String,
  args: Schema.Array(Schema.String),
  env: Schema.optional(Schema.Record(Schema.String, Schema.String))
}) {}

// Claude Code specific
export class ClaudeCodeTask extends Schema.Class<ClaudeCodeTask>("ClaudeCodeTask")({
  type: Schema.Literal("create-project", "refactor", "debug", "test"),
  context: Schema.Record(Schema.String, Schema.Any)
}) {}

// Routing preferences
export class RoutingPreference extends Schema.Class<RoutingPreference>("RoutingPreference")({
  preferredProvider: Schema.optional(ProviderId),
  excludeProviders: Schema.optional(Schema.Array(ProviderId)),
  requireCapabilities: Schema.optional(Schema.Array(Capability)),
  maxCostPerRequest: Schema.optional(Schema.Number),
  preferSpeed: Schema.optional(Schema.Boolean)
}) {}
```

## Service Implementations

### Router Service

```typescript
export class Router extends Effect.Service<Router>()("ai/Router", {
  dependencies: [ProviderRegistry.Default, CostEstimator.Default],
  effect: Effect.gen(function*() {
    const registry = yield* ProviderRegistry
    const costEstimator = yield* CostEstimator

    return {
      selectProvider: (request: AiRequest, options?: AiOptions) =>
        Effect.gen(function*() {
          const preferences = options?.routing ?? {}

          // Get all available providers
          let providers = yield* registry.getAll()

          // Filter by model support
          providers = providers.filter(p => p.supportsModel(request.model))

          // Apply exclusions
          if (preferences.excludeProviders) {
            providers = providers.filter(p =>
              !preferences.excludeProviders!.includes(p.id)
            )
          }

          // Filter by capabilities
          if (preferences.requireCapabilities) {
            providers = providers.filter(p =>
              preferences.requireCapabilities!.every(cap =>
                p.capabilities.includes(cap)
              )
            )
          }

          // Filter by cost
          if (preferences.maxCostPerRequest) {
            const withCosts = yield* Effect.forEach(providers, p =>
              costEstimator.estimate(p, request).pipe(
                Effect.map(cost => ({ provider: p, cost }))
              )
            )
            providers = withCosts
              .filter(({ cost }) => cost <= preferences.maxCostPerRequest!)
              .map(({ provider }) => provider)
          }

          // Sort by preference
          if (preferences.preferSpeed) {
            providers.sort((a, b) => a.averageLatency - b.averageLatency)
          } else {
            // Default: sort by cost
            const costs = yield* Effect.forEach(providers, p =>
              costEstimator.estimate(p, request)
            )
            providers.sort((a, b) => {
              const costA = costs[providers.indexOf(a)]
              const costB = costs[providers.indexOf(b)]
              return costA - costB
            })
          }

          // Return best option or fail
          const selected = providers[0]
          if (!selected) {
            return yield* Effect.fail(new NoProviderAvailableError({ request }))
          }

          return selected
        })
    }
  })
}) {}
```

### Memory Service

```typescript
export class MemoryService extends Effect.Service<MemoryService>()("ai/MemoryService", {
  effect: Effect.gen(function*() {
    // In-memory storage with session support
    const sessions = yield* Ref.make<Map<SessionId, ConversationHistory>>(new Map())

    return {
      enhanceRequest: (request: AiRequest, sessionId?: SessionId) =>
        Effect.gen(function*() {
          if (!sessionId) return request

          const history = yield* Ref.get(sessions).pipe(
            Effect.map(map => map.get(sessionId))
          )

          if (!history || history.messages.length === 0) return request

          // Add conversation context
          return {
            ...request,
            systemPrompt: [
              request.systemPrompt,
              "Previous conversation context:",
              formatHistory(history)
            ].filter(Boolean).join("\n")
          }
        }),

      store: (request: AiRequest, response: AiResponse, sessionId?: SessionId) =>
        Effect.gen(function*() {
          if (!sessionId) return

          yield* Ref.update(sessions, map => {
            const newMap = new Map(map)
            const history = newMap.get(sessionId) ?? { messages: [] }

            history.messages.push({
              role: "user",
              content: request.prompt,
              timestamp: new Date()
            })

            history.messages.push({
              role: "assistant",
              content: response.content,
              timestamp: new Date()
            })

            // Limit history size
            if (history.messages.length > 50) {
              // Summarize older messages
              history.summary = yield* summarizeHistory(
                history.messages.slice(0, -20)
              )
              history.messages = history.messages.slice(-20)
            }

            newMap.set(sessionId, history)
            return newMap
          })
        }),

      clear: (sessionId?: SessionId) =>
        Effect.gen(function*() {
          if (sessionId) {
            yield* Ref.update(sessions, map => {
              const newMap = new Map(map)
              newMap.delete(sessionId)
              return newMap
            })
          } else {
            yield* Ref.set(sessions, new Map())
          }
        }),

      getHistory: (sessionId: SessionId) =>
        Ref.get(sessions).pipe(
          Effect.map(map => map.get(sessionId))
        )
    }
  })
}) {}
```

### Tool Executor

```typescript
export class ToolExecutor extends Effect.Service<ToolExecutor>()("ai/ToolExecutor", {
  effect: Effect.gen(function*() {
    const tools = yield* Ref.make<Map<string, AiTool>>(new Map())

    return {
      register: (tool: AiTool) =>
        Ref.update(tools, map => new Map(map).set(tool.name, tool)),

      execute: (toolCall: ToolCall) =>
        Effect.gen(function*() {
          const tool = yield* Ref.get(tools).pipe(
            Effect.map(map => map.get(toolCall.name)),
            Effect.flatMap(Option.match({
              onNone: () => Effect.fail(new ToolNotFoundError({ name: toolCall.name })),
              onSome: Effect.succeed
            }))
          )

          if (!tool.handler) {
            return yield* Effect.fail(new ToolNotExecutableError({ name: tool.name }))
          }

          // Validate arguments against schema
          const args = yield* Schema.decode(tool.parameters)(toolCall.arguments).pipe(
            Effect.mapError(error => new ToolArgumentError({ tool: tool.name, error }))
          )

          // Execute tool
          return yield* Effect.tryPromise({
            try: () => tool.handler(args),
            catch: (error) => new ToolExecutionError({ toolName: tool.name, error })
          })
        }),

      executeAll: (toolCalls: ReadonlyArray<ToolCall>) =>
        Effect.forEach(
          toolCalls,
          (call) => execute(call).pipe(
            Effect.map(result => ({ call, result })),
            Effect.catchAll(error => Effect.succeed({ call, error }))
          ),
          { concurrency: 5 }
        )
    }
  })
}) {}
```

## Provider Implementations

### OpenAI Provider with Advanced Features

```typescript
export class OpenAiProvider extends Effect.Service<OpenAiProvider>()("ai/OpenAiProvider", {
  effect: Effect.gen(function*() {
    const config = yield* OpenAiConfig
    const client = new OpenAI({ apiKey: config.apiKey })

    // Implement rate limiting
    const rateLimiter = yield* Semaphore.make(10)

    const withRateLimit = <A, E, R>(effect: Effect.Effect<A, E, R>) =>
      rateLimiter.withPermit(effect)

    return {
      id: "openai" as ProviderId,
      capabilities: ["chat", "tools", "vision", "streaming"],
      averageLatency: 1500, // ms

      supportsModel: (model: string) => model.startsWith("gpt-"),

      complete: (request: AiRequest) =>
        withRateLimit(
          Effect.gen(function*() {
            const messages = yield* formatMessages(request)

            const completion = yield* Effect.tryPromise({
              try: () => client.chat.completions.create({
                model: request.model,
                messages,
                temperature: request.temperature,
                max_tokens: request.maxTokens,
                tools: request.tools ? formatTools(request.tools) : undefined
              }),
              catch: (error) => new OpenAiError({ error })
            })

            return transformOpenAiResponse(completion)
          })
        ),

      stream: (request: AiRequest) =>
        withRateLimit(
          Effect.gen(function*() {
            const messages = yield* formatMessages(request)

            const stream = yield* Effect.tryPromise({
              try: () => client.chat.completions.create({
                model: request.model,
                messages,
                temperature: request.temperature,
                stream: true
              }),
              catch: (error) => new OpenAiError({ error })
            })

            return Stream.fromAsyncIterable(stream, (error) =>
              new StreamError({ provider: "openai", error })
            ).pipe(
              Stream.map(chunk => ({
                content: chunk.choices[0]?.delta?.content ?? "",
                isFinished: chunk.choices[0]?.finish_reason !== null,
                toolCallDelta: chunk.choices[0]?.delta?.tool_calls?.[0]
              }))
            )
          })
        ),

      listModels: () =>
        withRateLimit(
          Effect.tryPromise({
            try: async () => {
              const response = await client.models.list()
              return response.data
                .filter(m => m.id.startsWith("gpt-"))
                .map(m => ({
                  id: m.id,
                  name: m.id,
                  provider: "openai" as ProviderId
                }))
            },
            catch: (error) => new OpenAiError({ error })
          })
        )
    }
  })
}) {}
```

### Anthropic Provider with Claude-Specific Features

```typescript
export class AnthropicProvider extends Effect.Service<AnthropicProvider>()("ai/AnthropicProvider", {
  effect: Effect.gen(function*() {
    const config = yield* AnthropicConfig
    const client = new Anthropic({ apiKey: config.apiKey })

    return {
      id: "anthropic" as ProviderId,
      capabilities: ["chat", "tools", "vision", "streaming", "long-context"],
      averageLatency: 2000,

      supportsModel: (model: string) => model.startsWith("claude-"),

      complete: (request: AiRequest) =>
        Effect.gen(function*() {
          // Anthropic has different message format
          const messages = yield* formatAnthropicMessages(request)

          const response = yield* Effect.tryPromise({
            try: () => client.messages.create({
              model: request.model,
              messages,
              max_tokens: request.maxTokens ?? 4096,
              temperature: request.temperature,
              tools: request.tools ? formatAnthropicTools(request.tools) : undefined
            }),
            catch: (error) => new AnthropicError({ error })
          })

          return {
            content: response.content[0].type === 'text'
              ? response.content[0].text
              : '',
            usage: {
              promptTokens: response.usage.input_tokens,
              completionTokens: response.usage.output_tokens,
              totalTokens: response.usage.input_tokens + response.usage.output_tokens
            },
            model: request.model as ModelIdentifier,
            finishReason: response.stop_reason as FinishReason,
            toolCalls: response.content
              .filter(c => c.type === 'tool_use')
              .map(c => ({
                id: c.id,
                name: c.name,
                arguments: c.input
              }))
          }
        }),

      // Anthropic-specific feature: constitutional AI
      completeWithConstitution: (request: AiRequest, constitution: string) =>
        Effect.gen(function*() {
          const enhancedRequest = {
            ...request,
            systemPrompt: [
              request.systemPrompt,
              "Constitutional AI Principles:",
              constitution
            ].filter(Boolean).join("\n")
          }

          return yield* complete(enhancedRequest)
        })
    }
  })
}) {}
```

## Tool/Function Calling

### Unified Tool Interface

```typescript
// Tool definition that works across all providers
export interface UnifiedTool {
  name: string
  description: string
  parameters: JsonSchema
  execute: (args: unknown) => Effect.Effect<unknown, ToolError>
}

// Tool adapter for different providers
export class ToolAdapter extends Effect.Service<ToolAdapter>()("ai/ToolAdapter", {
  effect: Effect.gen(function*() {
    return {
      toOpenAi: (tool: UnifiedTool) => ({
        type: "function" as const,
        function: {
          name: tool.name,
          description: tool.description,
          parameters: tool.parameters
        }
      }),

      toAnthropic: (tool: UnifiedTool) => ({
        name: tool.name,
        description: tool.description,
        input_schema: tool.parameters
      }),

      toVercelSdk: (tool: UnifiedTool) => ({
        [tool.name]: {
          description: tool.description,
          parameters: tool.parameters,
          execute: async (args: unknown) => {
            const result = await Effect.runPromise(tool.execute(args))
            return result
          }
        }
      }),

      fromMcp: (mcpTool: McpTool) => ({
        name: mcpTool.name,
        description: mcpTool.description,
        parameters: mcpTool.inputSchema,
        execute: (args: unknown) =>
          McpService.pipe(
            Effect.andThen(mcp => mcp.executeTool({
              server: mcpTool.serverName,
              tool: mcpTool.name,
              arguments: args
            }))
          )
      })
    }
  })
}) {}
```

### Built-in Tools

```typescript
// Common tools that ship with the package
export const WebSearchTool: UnifiedTool = {
  name: "web_search",
  description: "Search the web for current information",
  parameters: {
    type: "object",
    properties: {
      query: { type: "string", description: "Search query" },
      num_results: { type: "number", default: 5 }
    },
    required: ["query"]
  },
  execute: (args) =>
    Effect.gen(function*() {
      const { query, num_results = 5 } = args as { query: string; num_results?: number }
      const searchService = yield* SearchService
      return yield* searchService.search(query, num_results)
    })
}

export const FileSystemTool: UnifiedTool = {
  name: "file_system",
  description: "Read and write files",
  parameters: {
    type: "object",
    properties: {
      operation: { type: "string", enum: ["read", "write", "list"] },
      path: { type: "string" },
      content: { type: "string" }
    },
    required: ["operation", "path"]
  },
  execute: (args) =>
    Effect.gen(function*() {
      const { operation, path, content } = args as any
      const fs = yield* FileSystemService

      switch (operation) {
        case "read":
          return yield* fs.readFile(path)
        case "write":
          return yield* fs.writeFile(path, content)
        case "list":
          return yield* fs.listDirectory(path)
        default:
          return yield* Effect.fail(new InvalidOperationError({ operation }))
      }
    })
}
```

## MCP Integration

### MCP Service

```typescript
export class McpService extends Effect.Service<McpService>()("ai/McpService", {
  effect: Effect.gen(function*() {
    const config = yield* McpConfig
    const servers = yield* Ref.make<Map<string, McpServer>>(new Map())

    return {
      // Start MCP server
      startServer: (serverConfig: McpServerConfig) =>
        Effect.gen(function*() {
          const server = yield* Effect.acquireRelease(
            Effect.tryPromise({
              try: async () => {
                const process = spawn(
                  serverConfig.command,
                  serverConfig.args,
                  {
                    env: { ...process.env, ...serverConfig.env },
                    stdio: ['pipe', 'pipe', 'pipe']
                  }
                )

                // Initialize JSON-RPC communication
                const transport = new StdioTransport(process)
                const client = new JsonRpcClient(transport)

                // Wait for initialization
                await client.request('initialize', {
                  protocolVersion: "1.0",
                  clientInfo: {
                    name: "openagents-ai",
                    version: "1.0.0"
                  }
                })

                return { process, client, config: serverConfig }
              },
              catch: (error) => new McpServerStartError({ error })
            }),
            (server) => Effect.sync(() => {
              server.process.kill()
            })
          )

          yield* Ref.update(servers, map =>
            new Map(map).set(serverConfig.command, server)
          )

          return server
        }),

      // List available tools from all servers
      listAvailableTools: () =>
        Effect.gen(function*() {
          const allServers = yield* Ref.get(servers)
          const toolLists = yield* Effect.forEach(
            Array.from(allServers.values()),
            (server) => Effect.tryPromise({
              try: () => server.client.request('tools/list'),
              catch: (error) => new McpRequestError({ error })
            }),
            { concurrency: 5 }
          )

          return toolLists.flatMap((response, index) =>
            response.tools.map(tool => ({
              ...tool,
              serverName: Array.from(allServers.keys())[index]
            }))
          )
        }),

      // Execute tool on appropriate server
      executeTool: (request: { server: string; tool: string; arguments: unknown }) =>
        Effect.gen(function*() {
          const server = yield* Ref.get(servers).pipe(
            Effect.map(map => map.get(request.server)),
            Effect.flatMap(Option.match({
              onNone: () => Effect.fail(new McpServerNotFoundError({ server: request.server })),
              onSome: Effect.succeed
            }))
          )

          return yield* Effect.tryPromise({
            try: () => server.client.request('tools/execute', {
              name: request.tool,
              arguments: request.arguments
            }),
            catch: (error) => new McpToolExecutionError({ error })
          })
        }),

      // Power user configuration
      configure: (settings: McpSettings) =>
        Effect.gen(function*() {
          // Update MCP-specific settings
          yield* Ref.update(config, c => ({ ...c, ...settings }))

          // Restart servers if needed
          if (settings.servers) {
            // Stop existing servers
            const existing = yield* Ref.get(servers)
            yield* Effect.forEach(
              existing.values(),
              (server) => Effect.sync(() => server.process.kill())
            )

            // Start new servers
            yield* Effect.forEach(
              settings.servers,
              (serverConfig) => startServer(serverConfig),
              { concurrency: 5 }
            )
          }
        })
    }
  })
}) {}
```

### MCP Configuration UI

```typescript
// For power users to configure MCP
export interface McpSettings {
  servers?: McpServerConfig[]
  defaultTimeout?: number
  enableLogging?: boolean
  customTransports?: Record<string, Transport>
}

// Configuration helper for common MCP servers
export const McpPresets = {
  filesystem: {
    command: "mcp-server-filesystem",
    args: ["--root", "/"],
    env: {}
  },
  github: (token: string) => ({
    command: "mcp-server-github",
    args: [],
    env: { GITHUB_TOKEN: token }
  }),
  database: (connectionString: string) => ({
    command: "mcp-server-database",
    args: ["--connection", connectionString],
    env: {}
  })
}
```

## Routing and Orchestration

### Advanced Routing Strategies

```typescript
export class RoutingStrategies {
  // Cost-optimized routing
  static costOptimized = (request: AiRequest) =>
    Effect.gen(function*() {
      const providers = yield* ProviderRegistry.getAll()
      const estimates = yield* Effect.forEach(
        providers,
        p => CostEstimator.estimate(p, request).pipe(
          Effect.map(cost => ({ provider: p, cost }))
        )
      )

      return estimates.sort((a, b) => a.cost - b.cost)[0].provider
    })

  // Latency-optimized routing
  static latencyOptimized = (request: AiRequest) =>
    Effect.gen(function*() {
      const providers = yield* ProviderRegistry.getAll()

      // Ping all providers
      const latencies = yield* Effect.forEach(
        providers,
        p => measureLatency(p).pipe(
          Effect.map(latency => ({ provider: p, latency }))
        )
      )

      return latencies.sort((a, b) => a.latency - b.latency)[0].provider
    })

  // Capability-based routing
  static capabilityBased = (requiredCapabilities: Capability[]) =>
    (request: AiRequest) =>
      Effect.gen(function*() {
        const providers = yield* ProviderRegistry.getByCapability(requiredCapabilities[0])

        // Filter by all capabilities
        const matching = providers.filter(p =>
          requiredCapabilities.every(cap => p.capabilities.includes(cap))
        )

        if (matching.length === 0) {
          return yield* Effect.fail(new NoProviderWithCapabilitiesError({ requiredCapabilities }))
        }

        // Use cost as tiebreaker
        return yield* costOptimized(request).pipe(
          Effect.map(p => matching.includes(p) ? p : matching[0])
        )
      })

  // Load-balanced routing
  static loadBalanced = (request: AiRequest) =>
    Effect.gen(function*() {
      const providers = yield* ProviderRegistry.getAll()
      const loads = yield* LoadTracker.getCurrentLoads()

      // Sort by current load
      const sorted = providers
        .map(p => ({ provider: p, load: loads.get(p.id) ?? 0 }))
        .sort((a, b) => a.load - b.load)

      return sorted[0].provider
    })

  // Fallback routing with retries
  static withFallbacks = (primary: ProviderId, fallbacks: ProviderId[]) =>
    (request: AiRequest) =>
      Effect.gen(function*() {
        const tryProvider = (id: ProviderId) =>
          ProviderRegistry.get(id).pipe(
            Effect.andThen(p => p.complete(request)),
            Effect.retry(
              Schedule.exponential(Duration.millis(100)).pipe(
                Schedule.jittered,
                Schedule.whileInput<any>(_ => _.retryable ?? true),
                Schedule.upTo(Duration.seconds(5))
              )
            )
          )

        // Try primary first
        const result = yield* tryProvider(primary).pipe(
          Effect.catchAll(() =>
            // Try fallbacks in order
            Effect.forEach(
              fallbacks,
              id => tryProvider(id),
              { concurrency: 1, discard: true }
            )
          )
        )

        return result
      })
}
```

### Orchestrator for Complex Workflows

```typescript
export class AiOrchestrator extends Effect.Service<AiOrchestrator>()("ai/AiOrchestrator", {
  dependencies: [AiService.Default],
  effect: Effect.gen(function*() {
    const ai = yield* AiService

    return {
      // Chain multiple AI calls
      chain: (steps: AiStep[]) =>
        Effect.gen(function*() {
          let context = {}
          const results = []

          for (const step of steps) {
            const request = yield* step.buildRequest(context)
            const response = yield* ai.complete(request)
            results.push(response)
            context = yield* step.updateContext(context, response)
          }

          return results
        }),

      // Parallel AI calls with aggregation
      parallel: (requests: AiRequest[], aggregator: (responses: AiResponse[]) => AiResponse) =>
        Effect.gen(function*() {
          const responses = yield* Effect.forEach(
            requests,
            req => ai.complete(req),
            { concurrency: 5 }
          )

          return aggregator(responses)
        }),

      // Map-reduce pattern
      mapReduce: <T>(
        items: T[],
        mapper: (item: T) => AiRequest,
        reducer: (responses: AiResponse[]) => AiResponse
      ) =>
        Effect.gen(function*() {
          const requests = items.map(mapper)
          return yield* parallel(requests, reducer)
        }),

      // Conditional branching
      conditional: (
        condition: AiRequest,
        onTrue: AiRequest,
        onFalse: AiRequest
      ) =>
        Effect.gen(function*() {
          const conditionResult = yield* ai.complete(condition)
          const decision = yield* parseBoolean(conditionResult.content)

          return yield* ai.complete(decision ? onTrue : onFalse)
        }),

      // Iterative refinement
      refine: (
        initial: AiRequest,
        refiner: (response: AiResponse) => AiRequest | null,
        maxIterations: number = 5
      ) =>
        Effect.gen(function*() {
          let request = initial
          let response: AiResponse

          for (let i = 0; i < maxIterations; i++) {
            response = yield* ai.complete(request)
            const nextRequest = refiner(response)

            if (!nextRequest) break
            request = nextRequest
          }

          return response!
        })
    }
  })
}) {}
```

## Memory Management

### Advanced Memory Features

```typescript
export class AdvancedMemoryService extends Effect.Service<AdvancedMemoryService>()("ai/AdvancedMemoryService", {
  dependencies: [AiService.Default],
  effect: Effect.gen(function*() {
    const ai = yield* AiService
    const embeddings = yield* EmbeddingService

    // Vector store for semantic search
    const vectorStore = yield* VectorStore.make()

    return {
      // Semantic memory search
      searchMemory: (query: string, sessionId: SessionId, topK: number = 5) =>
        Effect.gen(function*() {
          const queryEmbedding = yield* embeddings.embed(query)
          const results = yield* vectorStore.search(queryEmbedding, topK, {
            filter: { sessionId }
          })

          return results.map(r => r.metadata.message)
        }),

      // Auto-summarization of long conversations
      summarizeConversation: (messages: Message[]) =>
        Effect.gen(function*() {
          const chunks = chunkMessages(messages, 10)

          const summaries = yield* Effect.forEach(
            chunks,
            chunk => ai.complete({
              prompt: `Summarize this conversation excerpt: ${formatMessages(chunk)}`,
              model: "gpt-3.5-turbo" as ModelIdentifier,
              maxTokens: 200
            }),
            { concurrency: 3 }
          )

          // Combine summaries
          return yield* ai.complete({
            prompt: `Combine these summaries into one: ${summaries.map(s => s.content).join('\n')}`,
            model: "gpt-3.5-turbo" as ModelIdentifier,
            maxTokens: 500
          })
        }),

      // Extract entities and facts
      extractKnowledge: (conversation: Message[]) =>
        Effect.gen(function*() {
          const response = yield* ai.complete({
            prompt: `Extract key entities, facts, and relationships from this conversation: ${formatMessages(conversation)}`,
            model: "gpt-4" as ModelIdentifier,
            tools: [EntityExtractionTool]
          })

          return parseEntities(response)
        }),

      // Conversation analytics
      analyzeConversation: (sessionId: SessionId) =>
        Effect.gen(function*() {
          const history = yield* MemoryService.getHistory(sessionId)

          return {
            messageCount: history.messages.length,
            totalTokens: calculateTokens(history),
            topics: yield* extractTopics(history),
            sentiment: yield* analyzeSentiment(history),
            userIntent: yield* classifyIntent(history)
          }
        })
    }
  })
}) {}
```

## Testing Strategy

### Unit Tests

```typescript
// Test individual providers
describe("OpenAiProvider", () => {
  it("should complete requests", () =>
    Effect.gen(function*() {
      const provider = yield* OpenAiProvider
      const response = yield* provider.complete({
        prompt: "Hello",
        model: "gpt-3.5-turbo" as ModelIdentifier
      })

      expect(response.content).toBeDefined()
      expect(response.usage.totalTokens).toBeGreaterThan(0)
    }).pipe(
      Effect.provide(TestOpenAiProvider),
      Effect.runPromise
    ))
})

// Test routing logic
describe("Router", () => {
  it("should select cheapest provider", () =>
    Effect.gen(function*() {
      const router = yield* Router
      const provider = yield* router.selectProvider(
        { prompt: "test", model: "gpt-4" as ModelIdentifier },
        { routing: { preferredProvider: undefined } }
      )

      expect(provider.id).toBe("openai")
    }).pipe(
      Effect.provide(TestLayers.Router),
      Effect.runPromise
    ))
})
```

### Integration Tests

```typescript
describe("AiService Integration", () => {
  it("should handle tool calls", () =>
    Effect.gen(function*() {
      const ai = yield* AiService

      // Register tool
      yield* ai.registerTool(WebSearchTool)

      const response = yield* ai.complete({
        prompt: "Search for Effect.js documentation",
        model: "gpt-4" as ModelIdentifier,
        tools: [WebSearchTool]
      })

      expect(response.toolCalls).toHaveLength(1)
      expect(response.content).toContain("Effect")
    }).pipe(
      Effect.provide(IntegrationTestLayers.Full),
      Effect.runPromise
    ))
})
```

### Test Utilities

```typescript
// Mock providers for testing
export const TestProviders = {
  mockOpenAi: Layer.succeed(OpenAiProvider, {
    id: "openai" as ProviderId,
    capabilities: ["chat", "tools"],
    complete: (request) => Effect.succeed({
      content: `Mock response to: ${request.prompt}`,
      usage: { promptTokens: 10, completionTokens: 20, totalTokens: 30 },
      model: request.model,
      finishReason: "stop" as FinishReason
    }),
    stream: () => Effect.succeed(Stream.empty),
    listModels: () => Effect.succeed([])
  }),

  mockAnthropic: Layer.succeed(AnthropicProvider, {
    // Similar mock implementation
  })
}

// Test layer composition
export const TestLayers = {
  Router: Layer.mergeAll(
    TestProviders.mockOpenAi,
    TestProviders.mockAnthropic,
    ProviderRegistry.Default,
    CostEstimator.Default,
    Router.Default
  ),

  Full: Layer.mergeAll(
    TestProviders.mockOpenAi,
    TestProviders.mockAnthropic,
    ProviderRegistry.Default,
    Router.Default,
    MemoryService.Default,
    ToolExecutor.Default,
    AiService.Default
  )
}
```

## Implementation Phases

### Phase 1: Foundation (Week 1-2)
- Set up package structure
- Implement core schemas and types in domain
- Create base service definitions
- Set up build and test infrastructure

### Phase 2: Basic Providers (Week 3-4)
- Implement OpenAI provider
- Implement Anthropic provider
- Create provider registry
- Basic routing implementation

### Phase 3: Core Services (Week 5-6)
- Implement AiService
- Add memory management
- Tool executor implementation
- Basic testing suite

### Phase 4: Advanced Providers (Week 7-8)
- Vercel AI SDK v5 integration
- Goose provider with MCP
- Claude Code provider
- Provider-specific features

### Phase 5: Advanced Features (Week 9-10)
- Advanced routing strategies
- Orchestration patterns
- Enhanced memory with embeddings
- Performance optimizations

### Phase 6: Testing & Documentation (Week 11-12)
- Comprehensive test coverage
- Performance benchmarks
- API documentation
- Usage examples

## Open Questions

1. **Authentication Management**
   - Should we use Effect Config for all API keys?
   - How to handle Claude MAX authentication?
   - Token refresh strategies?

2. **Provider Selection**
   - Should we implement automatic failover?
   - How to handle provider-specific features in the unified interface?
   - Cost tracking and budgeting?

3. **MCP Configuration**
   - Default MCP servers to ship with?
   - How to handle MCP server discovery?
   - Security considerations for MCP?

4. **Streaming**
   - Unified streaming format across providers?
   - How to handle tool calls in streams?
   - Backpressure strategies?

5. **Memory Persistence**
   - Should memory be persistent by default?
   - Vector database choice (if any)?
   - Privacy considerations?

6. **Error Handling**
   - Retry strategies per provider?
   - How to surface provider-specific errors?
   - Graceful degradation?

7. **Performance**
   - Request batching across providers?
   - Caching strategies?
   - Rate limit coordination?

8. **Type Safety**
   - How strict should tool parameter validation be?
   - Runtime vs compile-time guarantees?
   - Schema evolution?

## Technical Decisions

### Decision 1: Effect-First vs Effect-Compatible
**Choice**: Effect-First
**Rationale**:
- Consistency with OpenAgents architecture
- Better type safety and error handling
- Cleaner dependency injection
- Natural fit for streaming and async operations

### Decision 2: Provider Abstraction Level
**Choice**: High-level abstraction with escape hatches
**Rationale**:
- Unified interface for common operations
- Provider-specific features accessible when needed
- Easier to add new providers
- Better testability

### Decision 3: Tool Calling Architecture
**Choice**: Unified tool interface with adapters
**Rationale**:
- Single tool definition works everywhere
- Adapters handle provider differences
- Easy to share tools between providers
- MCP tools integrate naturally

### Decision 4: Configuration Strategy
**Choice**: Effect Config with provider-specific sections
**Rationale**:
- Consistent with OpenAgents patterns
- Environment-based configuration
- Type-safe config schemas
- Easy testing with config overrides

### Decision 5: Memory Implementation
**Choice**: Pluggable memory with in-memory default
**Rationale**:
- Simple default that works out of box
- Can add persistence later
- Supports different strategies
- Privacy-friendly default

## GitHub Issue Template

```markdown
# AI Package Integration - Initial Implementation

## Overview
Implement `@openagentsinc/ai` package providing unified AI capabilities across multiple providers with full Effect.js integration.

## Objectives
- [ ] Create Effect-based AI service architecture
- [ ] Integrate multiple AI providers (OpenAI, Anthropic, Vercel AI SDK, Goose, Claude Code)
- [ ] Implement unified tool/function calling interface
- [ ] Add intelligent routing between providers
- [ ] Support MCP (Model Context Protocol) with power-user configuration
- [ ] Implement conversation memory management

## Technical Requirements
- Full Effect.js integration following OpenAgents patterns
- TypeScript with strict mode
- Comprehensive error handling with tagged errors
- Schema-first development
- Layer-based composition
- 90%+ test coverage

## Implementation Plan

### Phase 1: Foundation Setup ✅
**Timeline**: Week 1-2
**Deliverables**:
- [ ] Package structure creation
- [ ] Core schema definitions in `@openagentsinc/domain`
- [ ] Base service interfaces
- [ ] Build configuration
- [ ] Test infrastructure

### Phase 2: Core Providers
**Timeline**: Week 3-4
**Deliverables**:
- [ ] OpenAI provider implementation
- [ ] Anthropic provider implementation
- [ ] Provider registry service
- [ ] Basic routing implementation
- [ ] Provider tests

### Phase 3: Core Services
**Timeline**: Week 5-6
**Deliverables**:
- [ ] AiService implementation
- [ ] Memory management service
- [ ] Tool executor service
- [ ] Integration tests
- [ ] Basic examples

### Phase 4: Advanced Providers
**Timeline**: Week 7-8
**Deliverables**:
- [ ] Vercel AI SDK v5 adapter
- [ ] Goose provider with MCP support
- [ ] Claude Code provider for MAX subscribers
- [ ] Provider-specific features
- [ ] Advanced routing strategies

### Phase 5: Advanced Features
**Timeline**: Week 9-10
**Deliverables**:
- [ ] AI orchestration patterns
- [ ] Enhanced memory with search
- [ ] Performance optimizations
- [ ] Streaming improvements
- [ ] Advanced examples

### Phase 6: Polish & Documentation
**Timeline**: Week 11-12
**Deliverables**:
- [ ] Comprehensive documentation
- [ ] Performance benchmarks
- [ ] Migration guide
- [ ] Public API finalization
- [ ] Release preparation

## Success Criteria
- [ ] All providers working with unified interface
- [ ] Tool calling works across all providers
- [ ] M
