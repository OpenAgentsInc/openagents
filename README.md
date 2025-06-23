# OpenAgents

OpenAgents is a platform for Bitcoin-powered AI agents built with Effect and open protocols.

Our flagship chat application showcases AI agents that can interact through the Nostr protocol, accept Bitcoin payments via Lightning, and leverage multiple AI providers for intelligent responses.

This monorepo contains our fully Effect-based architecture with comprehensive type safety, streaming support, and production-ready AI integrations.

## Architecture Overview

OpenAgents is built as a monorepo using pnpm workspaces, with a clean separation between reusable packages (libraries) and user-facing applications. The architecture emphasizes type safety through TypeScript and Effect, deterministic agent identities via Bitcoin standards, and local-first AI inference.

### Core Architecture Principles

- **Fully Effect-Based**: Complete Effect integration throughout the stack - no Promise/async mixing
- **Streaming-First**: SSE-based streaming with proper Effect Stream to Web Stream conversion
- **Multi-Provider AI**: Unified interface for Cloudflare (free), OpenRouter (100+ models), and Ollama (local)
- **Bitcoin-Native**: Agents use deterministic identities (NIP-06) with Lightning Network payments
- **Type-Safe Services**: Effect Services with Layer-based dependency injection
- **Schema-Driven**: Runtime validation with `@effect/schema` for all API contracts
- **Production-Ready**: Comprehensive error handling, proper resource management, and monitoring

### How It All Fits Together

```
┌─────────────────────────────────────────────────────────────────┐
│                         Applications                             │
├─────────────────────────────────────────────────────────────────┤
│                     openagents.com                               │
│            (Chat App with AI & Bitcoin Agents)                   │
├─────────────────────────────────────────────────────────────────┤
│                      Web Framework                               │
├─────────────────────────────────────────────────────────────────┤
│                        psionic                                   │
│          (Effect-based HTTP server & routing)                    │
├─────────────────────────────────────────────────────────────────┤
│                         Core Packages                            │
├─────────────────┬───────────────┬───────────────────────────────┤
│      sdk        │     nostr     │            relay              │
│ (Agent Runtime) │  (Protocol)   │    (Database & Events)        │
├─────────────────┴───────────────┴───────────────────────────────┤
│                      AI & Interface Packages                     │
├─────────────┬──────────────┬────────────┬──────────────────────┤
│     ai      │      ui      │    cli     │     autotest         │
│ (Providers) │   (Styling)  │  (Demo)    │    (Testing)         │
└─────────────┴──────────────┴────────────┴──────────────────────┘
```

### Key Dependencies

- **Psionic → Effect HTTP**: Full Effect-based HTTP server with streaming support
- **SDK → Nostr**: NIP-06 deterministic key derivation for agent identities
- **AI → Effect Streams**: Streaming AI responses with proper layer management
- **OpenAgents.com → All Packages**: Integrates full stack for chat application
- **Relay → PlanetScale**: MySQL database with Drizzle ORM for persistence

### Development Workflow

```bash
# Install dependencies
pnpm i

# Set up git hooks for quality checks
pnpm setup-hooks

# Build all packages (required before testing)
pnpm build

# Run tests
pnpm test

# Type checking
pnpm check

# Lint code
pnpm lint

# Generate Effect exports (not for UI package!)
pnpm --filter=@openagentsinc/sdk codegen
```

## Packages

### Core
- **[`@openagentsinc/sdk`](packages/sdk/)** - Bitcoin-powered agent runtime with Lightning payments
- **[`@openagentsinc/nostr`](packages/nostr/)** - Effect-based Nostr protocol with NIP support
- **[`@openagentsinc/relay`](packages/relay/)** - Database layer with PlanetScale MySQL integration

### Web Framework
- **[`@openagentsinc/psionic`](packages/psionic/)** - Effect-based HTTP framework with streaming support

### AI & Providers
- **[`@openagentsinc/ai`](packages/ai/)** - Multi-provider AI abstraction (Cloudflare, OpenRouter, Ollama)
  - Cloudflare: Free models including Llama 4 Scout
  - OpenRouter: 100+ premium models (GPT-4, Claude, etc.)
  - Ollama: Local inference for privacy

### UI & Testing
- **[`@openagentsinc/ui`](packages/ui/)** - Custom Tailwind theme with Basecoat CSS
- **[`@openagentsinc/autotest`](packages/autotest/)** - Browser automation and visual testing
- **[`@openagentsinc/cli`](packages/cli/)** - Command-line interface (demo)

## Apps

- **[`openagents.com`](apps/openagents.com/)** - Production chat application
  - Real-time AI streaming with multiple providers
  - Model selection UI with 20+ models
  - Conversation persistence with MySQL
  - Full Effect integration (no Promises!)

## Documentation

### Architecture Guides (REQUIRED READING)

Before working on this codebase, you **MUST** read the relevant guides:

- **[Effect Architecture Guide](docs/guides/effect-architecture-guide.md)** - Core Effect patterns and Psionic framework
- **[Streaming Architecture Guide](docs/guides/streaming-architecture.md)** - Critical SSE streaming patterns
- **[AI Provider Integration Guide](docs/guides/ai-provider-integration.md)** - Adding and configuring AI providers
- **[Language Model Integration Guide](docs/guides/language-model-integration.md)** - Model configuration and UI
- **[Effect Quick Reference](docs/guides/effect-quick-reference.md)** - Common patterns and anti-patterns

### Additional Documentation

- **[CLAUDE.md](CLAUDE.md)** - Coding agent instructions and forbidden patterns
- **[Database Migration Guide](DATABASE_MIGRATION_GUIDE.md)** - PlanetScale schema management
- **[Autotest Documentation](docs/autotest.md)** - Browser testing framework

## Key Features

### 🚀 Production-Ready Streaming
- Server-Sent Events (SSE) for real-time AI responses
- Proper Effect Stream to Web Stream conversion
- Anti-buffering headers for CDN compatibility
- Error recovery and timeout handling

### 🤖 Multi-Provider AI Support
- **Cloudflare Workers AI**: Free tier with Llama models
- **OpenRouter**: Access to 100+ models with single API
- **Ollama**: Local models for privacy-conscious users
- Unified streaming interface across all providers

### ⚡ Effect-Based Architecture
- No `async`/`await` or Promises in route handlers
- Proper service layer composition
- Type-safe error handling with tagged errors
- Resource management with finalizers

### 🔐 Bitcoin & Nostr Integration
- Deterministic agent identities (NIP-06)
- Lightning Network payment capabilities
- Nostr protocol for decentralized communication
- Agent-to-agent messaging support

## Getting Started

```bash
# Clone the repository
git clone https://github.com/OpenAgentsInc/openagents.git
cd openagents

# Install dependencies
pnpm install

# Set up git hooks
pnpm setup-hooks

# Build all packages
pnpm build

# Run tests
pnpm test

# Start development (in separate terminals)
cd packages/relay && pnpm dev  # Database server on :3003
cd apps/openagents.com && pnpm dev  # Main app on :3000
```

## Environment Setup

Create `.env` files based on `.env.example`:

```bash
# Required for AI features
CLOUDFLARE_ACCOUNT_ID=your_account_id
CLOUDFLARE_API_KEY=your_api_key

# Optional for premium models
OPENROUTER_API_KEY=your_api_key

# Database (PlanetScale)
DATABASE_URL=mysql://...
```

## Contributing

1. Read the architecture guides (seriously, read them!)
2. Follow Effect patterns - no Promise mixing
3. Run tests before pushing
4. Update documentation for new features

## License

MIT - See [LICENSE](LICENSE) for details
