# @openagentsinc/ai

Unified AI provider integration with Effect for OpenAgents applications.

## Installation

```bash
pnpm add @openagentsinc/ai
```

## Features

- **Provider Agnostic**: Single API that works with OpenAI, Anthropic, and more
- **Effect-First**: Full integration with Effect patterns
- **Unified Tools**: Common tool/function interface across all providers
- **Intelligent Routing**: Automatic provider selection based on various strategies
- **Memory Management**: Built-in conversation history and session management

## Usage

```typescript
import { AiService } from "@openagentsinc/ai"
import { Effect } from "effect"

const program = Effect.gen(function*() {
  const ai = yield* AiService
  
  const response = yield* ai.complete("Hello, AI!")
  console.log(response)
})
```

## License

CC0-1.0