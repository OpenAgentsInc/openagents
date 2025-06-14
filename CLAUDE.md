# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is an OpenAgents Effect monorepo demonstrating a Todo application using modern Effect patterns. The repository follows a clean architecture with these packages:

- **`@openagentsinc/domain`** - Core business logic and API contracts
- **`@openagentsinc/relay`** - Nostr relay server implementation
- **`@openagentsinc/cli`** - Command-line interface client
- **`@openagentsinc/ui`** - Shared UI components (React/Tailwind)
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
pnpm --filter=@openagentsinc/domain codegen
pnpm --filter=@openagentsinc/server codegen
pnpm --filter=@openagentsinc/cli codegen

# Build individual packages
pnpm --filter=@openagentsinc/domain build
pnpm --filter=@openagentsinc/ui build
```

### Testing
```bash
# Run tests with coverage
pnpm coverage

# Run tests for specific package
pnpm --filter=@openagentsinc/domain test
```

## Architecture Patterns

### Effect Service Architecture
- **Domain Package**: Defines API contracts using Effect Schema and HTTP API builders
- **Server Package**: Implements services using Effect Layers for dependency injection
- **CLI Package**: Uses generated HTTP clients from domain schemas

### Key Patterns Used
- **Schema-first development**: API contracts defined with `@effect/schema`
- **Effect Services**: Dependency injection with `Effect.Service` and `Layer`
- **Tagged errors**: Type-safe error handling with branded error types
- **Repository pattern**: Data access abstraction using Effect services

### Package Dependencies
```
cli → domain (API contracts)
server → domain (API contracts)
domain → (standalone)
ui → (standalone, React components)
playground → ui (component testing)
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

### Adding New Features
1. **Domain-first**: Define schemas and API contracts in domain package
2. **Generate exports**: Run `pnpm codegen` after adding new files
3. **Implement server**: Add handlers and services in server package
4. **CLI integration**: Add commands and client calls in CLI package

### Testing Strategy
- Tests use `@effect/vitest` for Effect-specific utilities
- Build packages before running tests (tests run against compiled output)
- Placeholder tests exist - implement comprehensive test coverage

### Code Organization
- **Domain**: API schemas, branded types, error definitions
- **Server**: Service implementations, repositories, HTTP handlers
- **CLI**: Command definitions, HTTP clients, user interface

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

### Adding New API Endpoints
1. Define schema in `domain/src/Api.ts`
2. Add handler in `server/src/Api.ts`
3. Update repository if data access needed
4. Add CLI command if user-facing
5. Run codegen for all packages

### Package Management
- All dependencies locked to exact versions (no ranges)
- Use `pnpm add` to add dependencies
- Patched dependencies require exact version overrides in root `package.json`

### Release Process
- Uses Changesets for version management
- Automated build validation before publishing
- Packages build independently but share common configuration
