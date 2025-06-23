# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is an OpenAgents Effect monorepo for building Bitcoin-powered digital agents. The repository follows a clean architecture with packages and apps:

### Packages (Libraries)
- **`@openagentsinc/sdk`** - Bitcoin-powered digital agents SDK
- **`@openagentsinc/nostr`** - Effect-based Nostr protocol implementation
- **`@openagentsinc/cli`** - Command-line interface demo (placeholder for future development)
- **`@openagentsinc/ui`** - Custom Tailwind theme with Basecoat CSS design system
- **`@openagentsinc/ai`** - AI provider abstraction
- **`@openagentsinc/psionic`** - Hypermedia web framework with built-in component explorer

### Apps (User-facing applications)
- **`@openagentsinc/openagents.com`** - Main website built with Psionic

## 🚨 CRITICAL: MANDATORY DOCUMENTATION BEFORE CODING 🚨

**STOP! Before writing ANY code related to the following systems, you MUST read the relevant guides:**

### Effect, Streaming, or Async Operations
**MANDATORY READING**: `/docs/guides/effect-architecture-guide.md`
- Required if: Working with ANY Effect code, async operations, services, layers, or error handling
- Why: Effect patterns are complex and mixing Promises/Effects WILL break the system

### AI Providers (Cloudflare, OpenRouter, Ollama, etc.)
**MANDATORY READING**: `/docs/guides/ai-provider-integration.md`
- Required if: Adding providers, modifying AI endpoints, working with chat/completion APIs
- Why: All providers MUST follow consistent patterns for frontend compatibility

### Streaming, SSE, or Real-time Data
**MANDATORY READING**: `/docs/guides/streaming-architecture.md`
- Required if: Working with streaming responses, chat streams, or any SSE implementation
- Why: Layer provision for streams is the #1 source of bugs - one mistake breaks everything

### Quick Effect Patterns
**MANDATORY READING**: `/docs/guides/effect-quick-reference.md`
- Required if: You need quick lookup for common patterns
- Why: Shows correct patterns vs common mistakes

### Language Models & Model Configuration
**MANDATORY READING**: `/docs/guides/language-model-integration.md`
- Required if: Adding new language models, configuring model lists, working with model selection UI
- Why: Models require specific configuration in multiple places and must maintain UI compatibility

**⚠️ FAILURE TO READ THESE GUIDES WILL RESULT IN:**
- "Service not found" errors that are difficult to debug
- Streaming responses that hang forever
- Broken chat functionality
- Hours of debugging that could be avoided

**These guides were written specifically for coding agents after extensive debugging sessions. They contain critical information about non-obvious requirements like providing ALL layers before Stream.toReadableStreamEffect(). Skip them at your own peril.**

## Essential Commands

### Development Workflow
```bash
# Install dependencies
pnpm i

# Set up git hooks for quality checks
pnpm setup-hooks

# Type checking across all packages
pnpm check

# Build all packages (required before testing)
pnpm build

# Run tests
pnpm test

# Lint code
pnpm lint

# Clean build artifacts
pnpm clean
```

### Package-specific Commands
```bash
# Generate Effect package exports (run after adding new files)
# IMPORTANT: Do NOT run codegen on @openagentsinc/ui package!
pnpm --filter=@openagentsinc/nostr codegen
pnpm --filter=@openagentsinc/cli codegen

# Build individual packages
pnpm --filter=@openagentsinc/sdk build
pnpm --filter=@openagentsinc/ui build
```

### Testing
```bash
# Run tests with coverage
pnpm coverage

# Run tests for specific package
pnpm --filter=@openagentsinc/sdk test
```

## Architecture Patterns

**📚 BEFORE READING THIS SECTION**: If you plan to implement anything based on these patterns, you MUST first read the comprehensive guides in `/docs/guides/`:
- `effect-architecture-guide.md` - Complete Effect patterns and Psionic framework
- `ai-provider-integration.md` - AI provider implementation details
- `streaming-architecture.md` - Critical streaming implementation patterns
- `effect-quick-reference.md` - Common patterns and anti-patterns

### Effect Service Architecture
- **SDK Package**: Core SDK with Agent, Lightning, Nostr, Compute, and Inference namespaces
- **Nostr Package**: Effect-based Nostr protocol implementation with NIP support
- **CLI Package**: Command-line interface demo

### Key Patterns Used
- **Schema-first development**: API contracts defined with `@effect/schema`
- **Effect Services**: Dependency injection with `Effect.Service` and `Layer`
- **Tagged errors**: Type-safe error handling with branded error types
- **NIP-06 compliance**: Deterministic key derivation for agent identities

### Package Dependencies
```
sdk → nostr (NIP-06 key derivation)
cli → ai (AI features)
ui → (standalone, Tailwind + Basecoat CSS)
psionic → (standalone, web framework)
openagents.com → psionic, sdk, nostr (main website)
```

## Build System

### Multi-format Build Process
Each package builds in this order:
1. **ESM**: TypeScript compilation (`build-esm`)
2. **Annotation**: Pure call annotations for tree-shaking (`build-annotate`)
3. **CJS**: Babel transformation for CommonJS (`build-cjs`)
4. **Packaging**: Effect build utils final packaging (`pack-v2`)

### TypeScript Configuration
- **Composite builds** with project references for incremental compilation
- **Effect Language Service** integration for enhanced IntelliSense
- **Strict TypeScript** settings with Effect-specific configurations

## Development Guidelines

### Package Script Execution
**IMPORTANT**: Always use `pnpm run <script>` instead of `pnpm <script>` to avoid conflicts with pnpm's built-in commands:
- ✅ `pnpm run deploy` (runs package script)
- ❌ `pnpm deploy` (pnpm built-in command, will fail)
- ✅ `pnpm run dev` (runs package script)
- ✅ `pnpm run build` (runs package script)

### Adding New Features
1. **SDK-first**: Define new agent capabilities in SDK package
2. **Generate exports**: Run `pnpm codegen` after adding new files
3. **Implement services**: Add Effect services with proper layers
4. **Demo integration**: Update Pylon demo to showcase new features

### Testing Strategy
- Tests use `@effect/vitest` for Effect-specific utilities
- Build packages before running tests (tests run against compiled output)
- Placeholder tests exist - implement comprehensive test coverage

### Code Organization
- **SDK**: Agent lifecycle, Lightning integration, Nostr communication
- **Nostr**: NIPs implementation, key derivation, protocol handling
- **CLI**: Command definitions, user interface (demo package)

### Effect Specific Notes
- Use `Effect.gen` for readable async code composition
- Leverage `Layer` for application wiring and dependency management
- Define services with proper interfaces for testability
- Use `Schema` for runtime validation and type generation

## Important Notes

- **Commander Repository**: The Commander repository is located at `/Users/christopherdavid/code/commander` - do not clone it
- **Development Servers**: Never start development servers with `pnpm dev` or similar commands - the user will handle this
- **Git Hooks**: NEVER use `--no-verify` when pushing. Always fix all linting and test errors before pushing

### Critical: UI Package Codegen
The UI package (`@openagentsinc/ui`) does NOT use Effect codegen because:
- It contains both `.ts` and `.tsx` files (React components)
- React components (`.tsx`) should NOT be processed by Effect codegen
- Exports are manually managed in `packages/ui/src/web/index.ts`
- **DO NOT** add any `effect` configuration to the UI package.json
- The package.json should have NO `effect` field at all

If you see CI errors about Effect build-utils failing on UI package:
1. Remove any `effect` field from packages/ui/package.json
2. Delete any generated `packages/ui/src/index.ts` file if it exists
3. The UI package codegen script is overridden to just echo a message

## Common Development Tasks

### Adding New SDK Features
1. Define new namespace or methods in SDK
2. Add proper TypeScript types and branded types
3. Implement Effect-based services if needed
4. Update Pylon demo to showcase feature
5. Run build and test across packages

### Package Management
- All dependencies locked to exact versions (no ranges)
- Use `pnpm add` to add dependencies
- Patched dependencies require exact version overrides in root `package.json`

### Release Process
- Uses Changesets for version management
- Automated build validation before publishing
- Packages build independently but share common configuration

## Component Explorer

Psionic includes a built-in component library explorer for systematic UI development.

### Configuration
```typescript
const app = createPsionicApp({
  name: 'MyApp',
  // Component explorer configuration
  componentsDir: 'stories',          // Default: "stories"
  componentsPath: '/components',     // Default: "/components"
  enableComponents: true,            // Default: true
  componentExplorerOptions: {
    styles: customStyles,            // CSS to include
    navigation: navComponent,        // Navigation HTML
    baseClass: 'my-theme'           // Root CSS class
  }
})
```

### Creating Stories
Create `.story.ts` files in your stories directory:

```typescript
// Button.story.ts
export const title = "Button"
export const component = "Button"

export const Default = {
  name: "Default Button",
  html: `<button class="btn">Click me</button>`,
  description: "Basic button component"
}

export const Primary = {
  name: "Primary Button", 
  html: `<button class="btn btn-primary">Primary</button>`
}
```

### Features
- **Zero Dependencies**: Built into Psionic, no external packages
- **Theme Integration**: Inherits your app's styling and theme system
- **Simple Format**: HTML-based stories without complex abstractions
- **Auto-discovery**: Automatically finds all `.story.ts` files
- **Hot Navigation**: Accessible at `/components` by default

## Browser Automation & Testing (Autotest Package)

The `@openagentsinc/autotest` package provides comprehensive browser automation and visual testing capabilities for Claude Code. See the full documentation at [docs/autotest.md](docs/autotest.md).

### Key Features
- **Server Lifecycle Management**: Start, monitor, and stop development servers programmatically
- **Test Orchestration**: Automated testing of multiple routes with comprehensive monitoring
- **Screenshot Capture**: Visual verification and regression testing
- **Error Detection**: Console messages, network requests, and page errors monitoring

### Common Commands

#### Quick Screenshot Capture
```bash
# Navigate to the autotest package
cd packages/autotest

# Capture a screenshot of any URL
bun run src/cli.ts '{"url":"http://localhost:3000","fullPage":true}'
```

#### Full Test Orchestration
```bash
# Test OpenAgents.com with default configuration
bun src/orchestrate.ts --default

# Test with custom configuration
bun src/orchestrate.ts "$(cat test-config.json)"
```

### Common Workflow for Claude Code
1. **Start orchestrated testing**: `bun packages/autotest/src/orchestrate.ts --default`
2. **Review test results**: `cat packages/autotest/test-report.json`
3. **View screenshots**: `Read: packages/autotest/.autotest/screenshots/screenshot-*.png`
4. **Analyze errors**: Check console messages, network requests, and page errors in report

### Integration Notes
- Server processes use daemon fibers to stay alive during testing
- Automatic port finding prevents conflicts
- Screenshots saved to `.autotest/screenshots/` (gitignored)
- Test reports include comprehensive monitoring data
- Ready state detection via configurable regex patterns

For detailed usage, configuration options, and troubleshooting, see [docs/autotest.md](docs/autotest.md).

## UI and Styling

### UI Package
The `@openagentsinc/ui` package provides:
- **Custom Tailwind theme** with OpenAgents color palette (black, offblack, darkgray, etc.)
- **Basecoat CSS** integration for modern component styling
- **Multiple themes**: Zinc (default), Catppuccin, Gruvbox, Nord
- **Berkeley Mono** font as the primary monospace font

### Component Patterns
Components in `apps/openagents.com/src/components/` use:
- **Template literals** with Psionic's `html` tag
- **CSS-in-JS** with Psionic's `css` tag for scoped styles
- **CSS variables** for theming (--text, --offblack, --darkgray, etc.)
- **Tailwind classes** where appropriate

Example component pattern:
```typescript
import { html, css } from "@openagentsinc/psionic"

export function myComponent({ title }: { title: string }) {
  return html`
    <div class="my-component">
      <h2>${title}</h2>
    </div>
    
    <style>
      .my-component {
        background: var(--offblack);
        color: var(--text);
        padding: 1rem;
      }
    </style>
  `
}
```

## Database Migrations

**CRITICAL**: This project uses PlanetScale MySQL with Drizzle ORM. Database schema issues can block development.

### Schema Management

- **Single Source of Truth**: `packages/relay/src/schema.ts` defines all database tables
- **Migration Scripts**: Use `packages/relay/scripts/run-migration.ts` for schema changes  
- **Configuration**: `packages/relay/drizzle.config.ts` connects to PlanetScale

### When Schema and Database Don't Match

**Symptoms**:
- Error: "Unknown column 'X' in 'field list'"
- API endpoints returning 500 errors
- Database queries failing

**Solution Process**:

1. **Check Schema vs Database**:
   ```bash
   cd packages/relay
   bun scripts/run-migration.ts  # Shows current vs expected columns
   ```

2. **Add Missing Columns**:
   - Update migration script with new columns
   - Run migration to add missing fields
   - Rebuild relay package

3. **Common Missing Columns**:
   - `creator_pubkey VARCHAR(64) NOT NULL` - For channel ownership
   - `message_count BIGINT DEFAULT 0` - For channel stats  
   - `updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP` - For record tracking

### Migration Commands

```bash
# Generate migration (reference only)
cd packages/relay && pnpm db:generate

# Run custom migration (PREFERRED)
cd packages/relay && bun scripts/run-migration.ts

# Rebuild after migration
pnpm --filter=@openagentsinc/relay build

# Test database connectivity
curl http://localhost:3003/api/channels/list
```

### Emergency Fixes

If database queries fail:

1. Check error message for missing column name
2. Add column to `scripts/run-migration.ts`:
   ```typescript
   { name: 'missing_column', sql: 'ADD COLUMN missing_column VARCHAR(64) NOT NULL DEFAULT ""' }
   ```
3. Run migration script
4. Rebuild and test

**NEVER** modify production database manually - always use migration scripts.

See [DATABASE_MIGRATION_GUIDE.md](DATABASE_MIGRATION_GUIDE.md) for complete documentation.

## FORBIDDEN DEVELOPMENT PATTERNS

### ❌ NEVER Create "Simpler Mocks" 

**ABSOLUTELY FORBIDDEN**: Replacing complex service architecture with "simpler" implementations.

**Examples of BANNED thought processes**:
- "Let me fix this by replacing the complex service usage with a simpler mock implementation"
- "I'll just remove the Effect services and use basic mocks instead"
- "This is too complex, let me simplify it"

**Why This is FORBIDDEN**:
- The Effect service architecture is **intentionally designed** for type safety and dependency injection
- Complex layers exist to ensure proper service composition and error handling
- "Simpler" implementations break the architectural integrity and type safety guarantees

**REQUIRED Approach Instead**:
- **READ THE GUIDES FIRST**: `/docs/guides/effect-architecture-guide.md`
- **Figure out how to make all services work together properly**
- **Fix TypeScript errors by providing proper service layers, not by removing them**
- **Understand and respect the existing Effect architecture**
- **When service integration is complex, that complexity serves a purpose**

**Correct Mindset**:
- "How do I properly provide all required services in the Layer?"
- "What Effect dependencies am I missing and how do I provide them?"
- "How does the existing architecture expect this to be wired together?"

### ❌ NEVER Mix Promises and Effects

**ABSOLUTELY FORBIDDEN**: Using `Effect.runPromise` inside route handlers or mixing async/await with Effect.

**Example of FORBIDDEN code**:
```typescript
// ❌ NEVER DO THIS
export async function handler(ctx) {
  const result = await Effect.runPromise(myEffect)  // Creates isolated context!
  return result
}
```

**READ**: `/docs/guides/effect-architecture-guide.md` for correct patterns

### ❌ NEVER Convert Streams Without Layers

**ABSOLUTELY FORBIDDEN**: Using `Stream.toReadableStreamEffect` without providing ALL required layers.

**Example of FORBIDDEN code**:
```typescript
// ❌ THIS WILL CAUSE "Service not found" ERRORS
const readable = yield* Stream.toReadableStreamEffect(stream)
```

**REQUIRED**:
```typescript
// ✅ ALWAYS provide ALL layers
const readable = yield* Stream.toReadableStreamEffect(stream).pipe(
  Effect.provide(Layer.merge(
    BunHttpPlatform.layer,
    FetchHttpClient.layer,
    YourServiceLayer
  ))
)
```

**READ**: `/docs/guides/streaming-architecture.md` for complete understanding

**Never take shortcuts. Never simplify. Always respect the architectural decisions that were made for production systems.**

## Final Reminder: Architecture Guides Are NOT Optional

If you've made it this far and are about to start coding, ask yourself:

1. **Are you working with Effects, async operations, or services?**  
   → You MUST have read `/docs/guides/effect-architecture-guide.md`

2. **Are you working with AI providers or chat endpoints?**  
   → You MUST have read `/docs/guides/ai-provider-integration.md`

3. **Are you working with streaming or SSE?**  
   → You MUST have read `/docs/guides/streaming-architecture.md`

4. **Are you unsure about any Effect pattern?**  
   → You MUST have read `/docs/guides/effect-quick-reference.md`

**These guides exist because previous agents spent HOURS debugging issues that are clearly documented.** The guides contain non-obvious requirements, critical patterns, and common pitfalls that you WILL encounter.

**The #1 cause of wasted debugging time is agents who think they can figure it out without reading the guides first.**

Don't be that agent. Read the guides. Your future self will thank you.
