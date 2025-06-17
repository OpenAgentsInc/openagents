# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is an OpenAgents Effect monorepo for building Bitcoin-powered digital agents. The repository follows a clean architecture with packages and apps:

### Packages (Libraries)
- **`@openagentsinc/sdk`** - Bitcoin-powered digital agents SDK
- **`@openagentsinc/nostr`** - Effect-based Nostr protocol implementation
- **`@openagentsinc/cli`** - Command-line interface client
- **`@openagentsinc/ui`** - Shared UI components (React/Tailwind)
- **`@openagentsinc/ai`** - AI provider abstraction
- **`@openagentsinc/psionic`** - Hypermedia web framework
- **`@openagentsinc/storybook`** - Component development and documentation

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
- **CLI Package**: Command-line interface with AI features

### Key Patterns Used
- **Schema-first development**: API contracts defined with `@effect/schema`
- **Effect Services**: Dependency injection with `Effect.Service` and `Layer`
- **Tagged errors**: Type-safe error handling with branded error types
- **NIP-06 compliance**: Deterministic key derivation for agent identities

### Package Dependencies
```
sdk → nostr (NIP-06 key derivation)
cli → ai (AI features)
ui → (standalone, React components)
psionic → (standalone, web framework)
storybook → (standalone, component docs)
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
- **CLI**: Command definitions, AI integrations, user interface

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
