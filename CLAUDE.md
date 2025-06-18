# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is an OpenAgents Effect monorepo for building Bitcoin-powered digital agents. The repository follows a clean architecture with packages and apps:

### Packages (Libraries)
- **`@openagentsinc/sdk`** - Bitcoin-powered digital agents SDK
- **`@openagentsinc/nostr`** - Effect-based Nostr protocol implementation
- **`@openagentsinc/cli`** - Command-line interface demo (placeholder for future development)
- **`@openagentsinc/ui`** - WebTUI CSS library
- **`@openagentsinc/ai`** - AI provider abstraction
- **`@openagentsinc/psionic`** - Hypermedia web framework with built-in component explorer

### Apps (User-facing applications)
- **`@openagentsinc/openagents.com`** - Main website built with Psionic
- **`@openagentsinc/pylon`** - SDK demo application
- **`@openagentsinc/playground`** - UI component testing playground

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
ui → (standalone, WebTUI CSS)
psionic → (standalone, web framework)
pylon → sdk (demo app)
playground → ui (component testing)
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
