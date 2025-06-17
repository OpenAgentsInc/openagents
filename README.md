# OpenAgents

OpenAgents is a platform for AI agents using open protocols.

Our previous flagship product (v4) is an agentic chat app live at [openagents.com](https://openagents.com).

This repo holds our new cross-platform version (v5), a work in progress.

## Architecture Overview

OpenAgents is built as a monorepo using pnpm workspaces, with a clean separation between reusable packages (libraries) and user-facing applications. The architecture emphasizes type safety through TypeScript and Effect.js, deterministic agent identities via Bitcoin standards, and local-first AI inference.

### Core Architecture Principles

- **Effect-Based**: Leverages Effect.js for functional programming patterns, dependency injection, and type-safe error handling
- **Bitcoin-Native**: Agents are designed to be economically self-sustaining with Lightning Network integration
- **Local-First AI**: Privacy-preserving AI inference through Ollama integration
- **Schema-Driven**: API contracts defined with `@effect/schema` for runtime validation
- **Multi-Format Builds**: Packages support both ESM and CommonJS for maximum compatibility

### How It All Fits Together

```
┌─────────────────────────────────────────────────────────────────┐
│                         Applications                             │
├─────────────────────────┬─────────────────────┬─────────────────┤
│   openagents.com        │      pylon          │   playground    │
│  (Main Website)         │  (SDK Demo)         │  (UI Testing)   │
├─────────────────────────┴─────────────────────┴─────────────────┤
│                         Core Packages                            │
├─────────────────┬───────────────┬───────────────────────────────┤
│      sdk        │     nostr     │            psionic            │
│ (Agent Runtime) │  (Protocol)   │      (Web Framework)          │
├─────────────────┴───────────────┴───────────────────────────────┤
│                      Support Packages                            │
├──────────────┬──────────────┬──────────────┬───────────────────┤
│      ui      │      ai      │     cli      │    storybook      │
│  (WebTUI)    │ (AI Provider)│  (CLI Demo)  │ (Component Dev)   │
└──────────────┴──────────────┴──────────────┴───────────────────┘
```

### Key Dependencies

- **SDK → Nostr**: Uses NIP-06 for deterministic key derivation from mnemonics
- **CLI → AI**: Integrates AI capabilities into command-line interface
- **OpenAgents.com → Psionic, SDK, Nostr, UI**: Main website uses all core components
- **Pylon → SDK**: Demonstrates SDK capabilities in a minimal app

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
- **`@openagentsinc/sdk`** - Bitcoin-powered digital agents SDK
- **`@openagentsinc/nostr`** - Effect-based Nostr protocol implementation

### Web Framework
- **`@openagentsinc/psionic`** - Hypermedia web framework using Bun and Elysia

### Interfaces
- **`@openagentsinc/cli`** - Command-line interface demo (placeholder for future development)
- **`@openagentsinc/ui`** - WebTUI terminal-inspired CSS components

### AI
- **`@openagentsinc/ai`** - Claude Code integration and AI provider abstraction

### Development Tools
- **`@openagentsinc/storybook`** - Component development and documentation

## Apps

- **`@openagentsinc/openagents.com`** - Main website built with Psionic
- **`@openagentsinc/pylon`** - OpenAgents SDK demo application
- **`@openagentsinc/playground`** - UI component testing environment
