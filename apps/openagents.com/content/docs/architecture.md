---
title: Architecture Overview
date: 2024-12-17
summary: Understanding the OpenAgents monorepo structure and design principles
category: guide
order: 4
---

# Architecture Overview

OpenAgents is built as a monorepo using pnpm workspaces, with a clean separation between reusable packages (libraries) and user-facing applications. The architecture emphasizes type safety through TypeScript and Effect.js, deterministic agent identities via Bitcoin standards, and local-first AI inference.

## Core Architecture Principles

### 1. Effect-Based Design

The codebase leverages Effect.js for:
- **Functional programming patterns**: Composable, type-safe operations
- **Dependency injection**: Service-based architecture with Layers
- **Type-safe error handling**: Tagged errors with proper error types
- **Resource management**: Proper cleanup and lifecycle management

### 2. Bitcoin-Native Vision

While still in development, the architecture is designed for:
- **Economic agents**: Self-sustaining entities that earn Bitcoin
- **Lightning integration**: Micropayments for AI services
- **Deterministic identities**: BIP39 mnemonics for agent keys

### 3. Local-First AI

Privacy and control through:
- **Ollama integration**: Local LLM inference
- **No cloud dependencies**: Everything runs on your hardware
- **Model flexibility**: Support for any Ollama-compatible model

### 4. Schema-Driven Development

- **Runtime validation**: Using `@effect/schema` for data validation
- **Contract-first APIs**: Define interfaces before implementation
- **Type generation**: Schemas provide TypeScript types automatically

## Repository Structure

```
openagents/
├── apps/                    # User-facing applications
│   ├── openagents.com/      # Main website (Psionic + WebTUI)
│   ├── pylon/               # SDK demo application
│   └── playground/          # UI component testing
├── packages/                # Reusable libraries
│   ├── sdk/                 # Core OpenAgents SDK
│   ├── nostr/               # Nostr protocol implementation
│   ├── ai/                  # AI provider integrations
│   ├── cli/                 # Command-line interface (demo)
│   ├── ui/                  # WebTUI component library
│   ├── psionic/             # Web framework
│   ├── autotest/            # Browser automation testing
│   └── storybook/           # Component development
├── docs/                    # Technical documentation
├── CLAUDE.md               # AI assistant instructions
└── README.md               # Project overview
```

## Package Architecture

### Core Packages

#### SDK (`@openagentsinc/sdk`)
The heart of OpenAgents, providing:
- **Agent management**: Creation, lifecycle, identity
- **AI inference**: Integration with Ollama for local LLMs
- **Placeholder features**: Lightning payments, container deployment
- **Effect integration**: Service-based architecture ready

```typescript
// Example SDK architecture
export namespace Agent {
  export function create(config?: AgentConfig): AgentIdentity
  export function createFromMnemonic(mnemonic: string): Promise<AgentIdentity>
}

export namespace Inference {
  export function infer(request: InferenceRequest): Promise<InferenceResponse>
  export function* inferStream(request: InferenceRequest): AsyncGenerator<InferenceChunk>
}
```

#### Nostr (`@openagentsinc/nostr`)
Effect-based Nostr protocol implementation:
- **NIP support**: Comprehensive protocol implementation
- **Key derivation**: NIP-06 for deterministic identities
- **Event handling**: Type-safe event creation and validation
- **Relay management**: WebSocket connections with reconnection

#### AI (`@openagentsinc/ai`)
Unified AI provider integration:
- **Claude Code**: Integration for MAX subscribers
- **Provider abstraction**: Consistent interface across providers
- **Effect services**: Proper dependency injection
- **Streaming support**: Real-time token generation

### Web Framework

#### Psionic (`@openagentsinc/psionic`)
Hypermedia web framework:
- **Server-side rendering**: HTML-first responses
- **Component explorer**: Built-in UI development tools
- **Markdown service**: Blog and documentation support
- **Bun + Elysia**: Fast, modern runtime

#### UI (`@openagentsinc/ui`)
WebTUI terminal-inspired components:
- **ASCII aesthetics**: Box-drawing characters
- **Theme system**: Multiple built-in themes
- **Attribute selectors**: `is-="component"` pattern
- **Zero dependencies**: Pure CSS implementation

### Development Tools

#### Autotest (`@openagentsinc/autotest`)
Browser automation and testing:
- **Visual regression**: Screenshot comparison
- **Server management**: Start/stop dev servers
- **Test orchestration**: Multi-route testing
- **Error detection**: Console and network monitoring

## Application Architecture

### OpenAgents.com

The main website demonstrates best practices:

```typescript
// Route structure
src/
├── routes/
│   ├── home.ts         # Landing page
│   ├── docs.ts         # Documentation system
│   └── blog.ts         # Blog with markdown
├── components/
│   └── shared-header.ts # Reusable navigation
└── styles.ts           # Shared styles
```

### Pylon

SDK demonstration application showing:
- Agent creation and management
- AI inference examples
- Real-time streaming
- Ollama integration

## Development Workflow

### Multi-Format Build Process

Each package builds in this order:

1. **ESM Build**: TypeScript → ES modules
2. **Annotation**: Pure call annotations for tree-shaking
3. **CJS Build**: Babel transformation for CommonJS
4. **Packaging**: Effect build utils final packaging

```json
{
  "scripts": {
    "build": "pnpm build-esm && pnpm build-annotate && pnpm build-cjs",
    "build-esm": "tsc -b tsconfig.build.json",
    "build-annotate": "pnpm tsx ../../scripts/annotate.ts",
    "build-cjs": "babel dist/esm --plugins @babel/transform-export-namespace-from"
  }
}
```

### Effect Codegen

Automatic export generation for Effect packages:

```bash
# Generate exports after adding new files
pnpm --filter=@openagentsinc/sdk codegen

# DO NOT run on UI package (contains JSX)
```

### TypeScript Configuration

- **Composite builds**: Incremental compilation
- **Project references**: Clean package boundaries
- **Strict mode**: Maximum type safety
- **Effect LSP**: Enhanced IntelliSense

## Design Patterns

### Service Pattern

All major functionality uses Effect services:

```typescript
export class MyService extends Effect.Service<MyService>()("app/MyService", {
  effect: Effect.gen(function*() {
    // Dependencies
    const config = yield* ConfigService
    
    // Service implementation
    return {
      doSomething: (input: string) => Effect.succeed(process(input))
    }
  })
}) {}
```

### Layer Composition

Services are wired together with Layers:

```typescript
const MainLayer = Layer.mergeAll(
  ConfigLive,
  DatabaseLive,
  MyServiceLive
)

const program = Effect.gen(function*() {
  const service = yield* MyService
  return yield* service.doSomething("input")
})

// Run with dependencies
program.pipe(
  Effect.provide(MainLayer),
  Effect.runPromise
)
```

### Error Handling

Tagged errors for type-safe error handling:

```typescript
export class NetworkError extends Data.TaggedError("NetworkError")<{
  readonly url: string
  readonly statusCode: number
}> {}

// Handle specific errors
Effect.catchTag("NetworkError", (error) =>
  Console.error(`Network error: ${error.statusCode}`)
)
```

## Key Technologies

### Runtime & Build
- **Bun**: Fast JavaScript runtime and bundler
- **pnpm**: Efficient package management
- **TypeScript**: Type safety and modern JavaScript
- **Effect**: Functional programming and service architecture

### Web Technologies
- **Elysia**: Bun-first web framework
- **markdown-it**: Markdown processing
- **Playwright**: Browser automation

### AI & Crypto
- **Ollama**: Local LLM inference
- **Nostr**: Decentralized communication protocol
- **Bitcoin/Lightning**: Payment infrastructure (planned)

## Current Limitations

### Work in Progress
- SDK is minimal with many placeholder features
- Lightning payments not yet implemented
- Container deployment is stubbed
- Some packages (CLI) are demo placeholders

### Design Decisions
- No client-side framework (server-rendered HTML)
- Local-first AI (no cloud providers by default)
- Effect.js learning curve for contributors

## Future Architecture

### Planned Features
- **Container isolation**: Firecracker VMs for agents
- **Lightning integration**: Real payment flows
- **Distributed agents**: Multi-node deployment
- **Plugin system**: Extensible agent capabilities

### Scaling Considerations
- **Horizontal scaling**: Stateless agent design
- **Event sourcing**: Audit trail and replay
- **CQRS pattern**: Separate read/write paths
- **Microservices**: Service mesh for large deployments

## Best Practices

### Code Organization
1. **Feature folders**: Group related code together
2. **Barrel exports**: Clean public APIs
3. **Type-first**: Define types before implementation
4. **Test coverage**: Unit and integration tests

### Dependency Management
1. **Exact versions**: No version ranges
2. **Minimal dependencies**: Prefer built-in solutions
3. **Tree-shaking**: Use pure annotations
4. **Monorepo benefits**: Share code efficiently

### Performance
1. **Lazy loading**: Load code on demand
2. **Caching**: Use Effect's built-in caching
3. **Streaming**: Process data incrementally
4. **Resource pooling**: Reuse expensive resources

---

*For detailed implementation patterns and Effect.js usage, see the [technical architecture guide](/docs/architecture) in the docs folder.*