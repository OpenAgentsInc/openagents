# Psionic Packaging Strategy

This document outlines the future packaging and distribution strategy for Psionic as it evolves from a prototype to a production-ready framework.

## Current State

The current structure in `packages/psionic/` is set up as an **application** rather than a **framework package**. This is intentional for initial development but will need restructuring before public release.

## Future Architecture

Psionic will be split into two separate packages:

### 1. Framework Package: `@openagentsinc/psionic`

The core framework that developers import into their projects.

```
packages/psionic/
├── src/
│   ├── core/                    # Core framework code
│   │   ├── HypermediaService.ts # Effect service for rendering
│   │   ├── RelayService.ts      # WebSocket relay management
│   │   ├── ComponentRegistry.ts # Server component registry
│   │   ├── Router.ts           # Hypermedia routing
│   │   └── index.ts
│   ├── components/             # Built-in components
│   │   ├── layouts/
│   │   │   ├── BaseLayout.ts
│   │   │   └── index.ts
│   │   ├── primitives/
│   │   │   ├── Button.ts
│   │   │   ├── Card.ts
│   │   │   └── index.ts
│   │   └── index.ts
│   ├── client/                 # Minimal client runtime
│   │   ├── psionic-client.js   # WebSocket handling
│   │   └── htmx-config.js      # HTMX configuration
│   ├── types/                  # TypeScript definitions
│   │   ├── components.ts
│   │   ├── config.ts
│   │   └── index.ts
│   └── index.ts                # Public API exports
├── dist/                       # Built framework files
├── examples/                   # Example apps
│   └── hello-world/            # Current src/index.ts moves here
└── package.json                # Publishable to npm
```

#### Framework API Design

```typescript
// Public API exports (src/index.ts)
export { Psionic } from './core/Psionic'
export { defineComponent } from './core/Component'
export { html, css, js } from './core/templates'
export { createRouter } from './core/Router'
export { HypermediaService, RelayService } from './core/services'
export type { PsionicConfig, Component, Route } from './types'

// Usage in user's app
import { Psionic, defineComponent, html } from '@openagentsinc/psionic'

const app = new Psionic({
  port: 3000,
  relays: ['ws://localhost:8080']
})

const HomePage = defineComponent({
  name: 'HomePage',
  render: () => html`
    <h1>Welcome to Psionic</h1>
    <button hx-get="/about" hx-target="#content">Learn More</button>
  `
})
```

### 2. CLI Package: `@openagentsinc/create-psionic`

A separate package for scaffolding new Psionic applications.

```
packages/create-psionic/
├── src/
│   ├── index.ts               # CLI entry point
│   ├── prompts.ts            # Interactive setup
│   ├── scaffold.ts           # Project generation
│   ├── templates.ts          # Template management
│   └── utils.ts
├── templates/                 # Starter templates
│   ├── basic/
│   │   ├── src/
│   │   │   ├── components/
│   │   │   ├── pages/
│   │   │   │   └── index.ts
│   │   │   └── app.ts
│   │   ├── public/
│   │   ├── package.json
│   │   ├── tsconfig.json
│   │   ├── psionic.config.ts
│   │   └── README.md
│   ├── with-effect/          # Full Effect.js integration
│   ├── with-agents/          # OpenAgents SDK integration  
│   ├── with-nostr/           # Nostr relay setup
│   └── full-stack/           # Complete example with all features
├── tests/
└── package.json
```

#### CLI Usage

```bash
# Primary usage
bunx @openagentsinc/create-psionic my-app

# With options
bunx @openagentsinc/create-psionic my-app --template with-agents

# Interactive mode (default)
bunx @openagentsinc/create-psionic my-app
? Choose a template: (Use arrow keys)
❯ basic - Minimal Psionic app
  with-effect - Effect.js service architecture
  with-agents - OpenAgents SDK integration
  with-nostr - Nostr relay configuration
  full-stack - Everything included
```

## User Project Structure

What gets created when someone runs `create-psionic`:

```
my-psionic-app/
├── src/
│   ├── components/            # User's components
│   │   ├── Layout.ts
│   │   └── AgentCard.ts
│   ├── pages/                 # Route handlers
│   │   ├── index.ts          # Home page
│   │   ├── agents.ts         # /agents route
│   │   └── api/              # API routes
│   │       └── inference.ts
│   ├── services/             # Effect services
│   │   └── Database.ts
│   ├── app.ts                # App configuration
│   └── index.ts              # Entry point
├── public/                   # Static assets
│   ├── styles.css
│   └── favicon.ico
├── psionic.config.ts         # Framework configuration
├── package.json
├── tsconfig.json
├── .env.example
└── README.md
```

### Example Configuration File

```typescript
// psionic.config.ts
import { defineConfig } from '@openagentsinc/psionic'

export default defineConfig({
  server: {
    port: 3002,
    hostname: 'localhost'
  },
  
  hypermedia: {
    // HTMX configuration
    htmx: {
      version: '1.9.x',
      extensions: ['ws', 'preload']
    }
  },
  
  relays: {
    // WebSocket relay configuration
    default: 'ws://localhost:8080',
    pool: [
      'wss://relay.damus.io',
      'wss://relay.nostr.band'
    ]
  },
  
  integrations: {
    // Optional integrations
    effect: true,
    openagents: {
      sdk: true,
      nostr: true
    }
  }
})
```

## Migration Path

### Phase 1: Current Development (Now)
- Keep current structure for rapid prototyping
- Build core features in `src/`
- Document API design decisions

### Phase 2: Framework Extraction
1. Create `src/core/` with framework code
2. Move example to `examples/hello-world/`
3. Define public API exports
4. Add build process for npm distribution

### Phase 3: CLI Development
1. Create separate `create-psionic` package
2. Build template system
3. Add interactive prompts
4. Test scaffolding process

### Phase 4: Publishing
1. Publish `@openagentsinc/psionic` to npm
2. Publish `@openagentsinc/create-psionic` to npm
3. Update documentation
4. Create getting started guide

## Technical Considerations

### Build Process
- Framework needs proper ESM/CJS builds
- Types must be generated and included
- Client-side JavaScript needs bundling
- CSS needs processing/bundling

### Dependencies
- Framework: Minimal deps (elysia, effect)
- CLI: Scaffolding tools (prompts, fs-extra, etc.)
- Templates: Show best practices

### Versioning Strategy
- Framework follows semver strictly
- CLI can iterate faster
- Templates pinned to framework versions

## Why This Architecture?

1. **Separation of Concerns**: Framework code vs scaffolding logic
2. **Independent Evolution**: CLI and framework can update separately  
3. **Smaller Install Size**: Users only get what they need
4. **Template Flexibility**: Easy to add new starters
5. **Standard Pattern**: Follows Next.js, Vite, etc.

## Open Questions

1. Should templates live in the CLI package or separate repo?
2. How do we handle Bun vs Node.js in templates?
3. Should we provide a migration tool for updates?
4. How do we handle CSS/styling in the framework?
5. What's the plugin/extension story?

## Next Steps

1. Continue building features in current structure
2. Identify clear boundaries between framework and app code
3. Design plugin system for extensibility
4. Create proof-of-concept for template system
5. Test with real applications before finalizing