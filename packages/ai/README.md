# @openagentsinc/ai

Unified AI provider integration with Effect.js, providing a consistent interface for multiple AI services.

## Features

- 🤖 **Claude Code Integration** - Full support for Claude Code CLI/SDK for MAX subscribers
- 🔄 **Session Management** - Continue conversations with session IDs
- 🌊 **Streaming Support** - Real-time streaming responses
- ⚡ **Effect-based** - Leverages Effect for type-safe error handling
- 🔌 **Extensible** - Easy to add new AI providers
- 📦 **Provider Agnostic** - Single API that works across providers

## Installation

```bash
pnpm add @openagentsinc/ai
```

## Claude Code Integration

The package includes full integration with Claude Code for MAX subscribers.

### Prerequisites

- Claude Code CLI installed (`claude` command available)
- Authenticated via Claude Code (no API key needed)

### Basic Usage

```typescript
import { Effect } from "effect"
import { NodeCommandExecutor } from "@effect/platform-node"
import { ClaudeCodeClient, ClaudeCodeClientLive, ClaudeCodeConfigDefault } from "@openagentsinc/ai"

const program = Effect.gen(function* () {
  const client = yield* ClaudeCodeClient
  
  // Check availability
  const isAvailable = yield* client.checkAvailability()
  
  // Send a prompt
  const response = yield* client.prompt("Hello Claude!")
  console.log(response.content)
})

// Run the program
program.pipe(
  Effect.provide(ClaudeCodeClientLive),
  Effect.provide(ClaudeCodeConfigDefault),
  Effect.provide(NodeCommandExecutor.layer),
  Effect.runPromise
)
```

### Configuration

```typescript
import { makeClaudeCodeConfig } from "@openagentsinc/ai"

const customConfig = makeClaudeCodeConfig({
  model: "claude-3-opus-20240229",
  outputFormat: "json",
  systemPrompt: "You are a helpful assistant",
  defaultTimeout: 60000
})
```

### Session Management

Continue conversations using session IDs:

```typescript
const conversation = Effect.gen(function* () {
  const client = yield* ClaudeCodeClient
  
  // Start conversation
  const response1 = yield* client.prompt("My name is Alice")
  
  // Continue with session
  if ("session_id" in response1 && response1.session_id) {
    const response2 = yield* client.continueSession(
      response1.session_id,
      "What was my name?"
    )
  }
})
```

### Streaming Responses

```typescript
const streaming = Effect.gen(function* () {
  const client = yield* ClaudeCodeClient
  
  const stream = client.streamPrompt("Tell me a story")
  
  yield* stream.pipe(
    Stream.tap(chunk => Console.log(chunk)),
    Stream.runDrain
  )
})
```

### Error Handling

The integration provides typed errors for different failure scenarios:

- `ClaudeCodeNotFoundError` - CLI not found
- `ClaudeCodeExecutionError` - Command execution failed
- `ClaudeCodeParseError` - Failed to parse response
- `ClaudeCodeSessionError` - Invalid session

```typescript
import { ClaudeCodeNotFoundError } from "@openagentsinc/ai"

const handled = program.pipe(
  Effect.catchTag("ClaudeCodeNotFoundError", (error) =>
    Console.error("Please install Claude Code CLI")
  )
)
```

## AI Service Provider

Use the high-level AI service for provider-agnostic access:

```typescript
import { AiService, ClaudeCodeProviderLive } from "@openagentsinc/ai"

const program = Effect.gen(function* () {
  const ai = yield* AiService
  
  const response = yield* ai.complete("Generate a haiku")
  console.log(response.content)
})

program.pipe(
  Effect.provide(ClaudeCodeProviderLive),
  Effect.runPromise
)
```

## Examples

See the `examples/` directory for more usage examples:

- `claude-code-example.ts` - Comprehensive Claude Code examples
- More examples coming soon...

## Testing

```bash
# Run tests
pnpm test

# Run examples
pnpm tsx examples/claude-code-example.ts
```

## License

CC0-1.0