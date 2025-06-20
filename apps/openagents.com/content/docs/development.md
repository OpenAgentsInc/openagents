---
title: Development Guide
date: 2024-12-17
summary: Contributing to OpenAgents - setup, workflow, and best practices
category: guide
order: 5
---

# Development Guide

This guide covers everything you need to contribute to OpenAgents, from initial setup to submitting pull requests.

## Prerequisites

- **Node.js 18+**: Required for ES modules support
- **pnpm**: Efficient package manager for monorepos
- **Bun**: For running Psionic applications
- **Git**: Version control
- **Ollama**: For AI inference features

### Installing Prerequisites

```bash
# Install pnpm
npm install -g pnpm

# Install Bun
curl -fsSL https://bun.sh/install | bash

# Install Ollama (macOS)
brew install ollama

# Install Ollama (Linux)
curl -fsSL https://ollama.com/install.sh | sh
```

## Initial Setup

### 1. Clone the Repository

```bash
git clone https://github.com/OpenAgentsInc/openagents.git
cd openagents
```

### 2. Install Dependencies

```bash
pnpm i
```

### 3. Set Up Git Hooks

```bash
pnpm setup-hooks
```

This installs pre-commit hooks that:
- Run type checking
- Run linting
- Prevent commits with errors

### 4. Build All Packages

```bash
pnpm build
```

> **Important**: Always build before running tests, as tests run against compiled output.

## Development Workflow

### Essential Commands

```bash
# Type checking across all packages
pnpm check

# Run all tests
pnpm test

# Lint code
pnpm lint

# Clean build artifacts
pnpm clean

# Build specific package
pnpm --filter=@openagentsinc/sdk build

# Run tests for specific package
pnpm --filter=@openagentsinc/sdk test
```

### Package Development

When working on a specific package:

```bash
# Navigate to package
cd packages/sdk

# Build the package
pnpm build

# Run tests
pnpm test

# Generate Effect exports (not for UI package!)
pnpm codegen
```

### Running Applications

#### OpenAgents.com Website

```bash
cd apps/openagents.com
bun dev
# Visit http://localhost:3000
```

## Code Style Guidelines

### TypeScript

- **Strict mode**: All packages use strict TypeScript
- **Explicit types**: Prefer explicit over inferred types for public APIs
- **Effect patterns**: Use Effect.gen for async operations
- **Branded types**: Use branded types for domain concepts

```typescript
// Good: Branded types
type Satoshis = number & { readonly brand: unique symbol }

// Good: Effect.gen pattern
const program = Effect.gen(function*() {
  const service = yield* MyService
  return yield* service.doSomething()
})

// Good: Explicit return types
export function calculate(input: number): Effect.Effect<Result, CalculationError> {
  // ...
}
```

### File Organization

```
package-name/
├── src/
│   ├── index.ts       # Public exports
│   ├── services/      # Effect services
│   ├── schemas/       # Data schemas
│   └── utils/         # Helper functions
├── test/
│   └── *.test.ts      # Test files
├── package.json
├── tsconfig.json
└── README.md
```

### Naming Conventions

- **Files**: kebab-case (`my-service.ts`)
- **Classes/Types**: PascalCase (`MyService`, `AgentIdentity`)
- **Functions/Variables**: camelCase (`createAgent`, `isValid`)
- **Constants**: UPPER_SNAKE_CASE (`MAX_RETRIES`)
- **Effect Services**: PascalCase with "Service" suffix (`DatabaseService`)

## Adding New Features

### 1. Create a Design Document

For significant features, create a design doc in `/docs`:

```markdown
# Feature: Agent Persistence

## Overview
Brief description of the feature

## Goals
- What we're trying to achieve
- Success criteria

## Design
Technical approach and architecture

## Implementation Plan
1. Step-by-step tasks
2. Dependencies
3. Testing strategy
```

### 2. Write Tests First

```typescript
// test/agent-persistence.test.ts
import { describe, it, expect } from "vitest"
import { Effect } from "effect"

describe("Agent Persistence", () => {
  it("should save agent state", () => {
    const result = Effect.runSync(saveAgent(testAgent))
    expect(result).toMatchObject({ success: true })
  })
})
```

### 3. Implement with Effect

```typescript
// src/services/persistence.ts
export class PersistenceService extends Effect.Service<PersistenceService>()(
  "app/PersistenceService",
  {
    effect: Effect.gen(function*() {
      const storage = yield* StorageService
      
      return {
        save: (agent: AgentIdentity) =>
          Effect.tryPromise({
            try: () => storage.put(agent.id, agent),
            catch: (error) => new PersistenceError({ error })
          })
      }
    })
  }
) {}
```

### 4. Update Exports

```typescript
// src/index.ts
export * from "./services/persistence.js"
```

Then run codegen:

```bash
pnpm codegen
```

## Testing

### Test Structure

```typescript
import { describe, it, expect } from "vitest"
import { Effect, Layer } from "effect"

describe("Feature Name", () => {
  // Setup test layer
  const TestLayer = Layer.succeed(
    ConfigService,
    { apiUrl: "http://test.local" }
  )
  
  it("should handle success case", async () => {
    const program = myFunction("input").pipe(
      Effect.provide(TestLayer)
    )
    
    const result = await Effect.runPromise(program)
    expect(result).toBe("expected output")
  })
  
  it("should handle error case", async () => {
    const program = myFunction("bad input").pipe(
      Effect.provide(TestLayer)
    )
    
    const result = await Effect.runPromiseExit(program)
    expect(result._tag).toBe("Failure")
  })
})
```

### Running Tests

```bash
# All tests
pnpm test

# With coverage
pnpm coverage

# Watch mode
pnpm test:watch

# Specific file
pnpm test src/agent.test.ts
```

## Creating a New Package

### 1. Copy Template

```bash
cp -r packages/template packages/my-package
cd packages/my-package
```

### 2. Update package.json

```json
{
  "name": "@openagentsinc/my-package",
  "version": "0.0.1",
  "description": "My new package",
  "type": "module",
  "sideEffects": false
}
```

### 3. Configure TypeScript

Update `tsconfig.json` references:

```json
{
  "extends": "../../tsconfig.base.json",
  "compilerOptions": {
    "outDir": "dist"
  }
}
```

### 4. Add to Workspace

Update root `pnpm-workspace.yaml` if needed.

### 5. Set Up Effect Codegen

```json
{
  "effect": {
    "includeGlobs": ["src/**/*.ts"],
    "excludeGlobs": ["src/**/*.test.ts"]
  }
}
```

## Documentation

### Code Documentation

Use JSDoc for public APIs:

```typescript
/**
 * Creates a new agent with the specified configuration.
 * 
 * @param config - Agent configuration options
 * @returns The created agent identity
 * 
 * @example
 * ```typescript
 * const agent = Agent.create({
 *   name: "My Agent",
 *   capabilities: ["chat"]
 * })
 * ```
 */
export function create(config?: AgentConfig): AgentIdentity {
  // ...
}
```

### README Files

Each package should have a comprehensive README:

```markdown
# @openagentsinc/package-name

Brief description of the package.

## Installation

\```bash
pnpm add @openagentsinc/package-name
\```

## Usage

\```typescript
import { feature } from '@openagentsinc/package-name'
\```

## API Reference

Document all public exports

## License

CC0-1.0
```

## Git Workflow

### Branch Naming

- `feature/description` - New features
- `fix/description` - Bug fixes  
- `docs/description` - Documentation
- `refactor/description` - Code refactoring
- `test/description` - Test additions

### Commit Messages

Follow conventional commits:

```bash
feat: add agent persistence layer
fix: resolve memory leak in inference stream
docs: update SDK reference with new examples
refactor: simplify effect service composition
test: add coverage for error cases
chore: update dependencies
```

### Pull Request Process

1. **Create Feature Branch**
   ```bash
   git checkout -b feature/agent-persistence
   ```

2. **Make Changes**
   - Write tests
   - Implement feature
   - Update documentation

3. **Run Checks**
   ```bash
   pnpm check
   pnpm test
   pnpm lint
   ```

4. **Commit with Message**
   ```bash
   git add .
   git commit -m "feat: add agent persistence layer"
   ```

5. **Push and Create PR**
   ```bash
   git push origin feature/agent-persistence
   ```

6. **PR Description Template**
   ```markdown
   ## Description
   Brief description of changes
   
   ## Type of Change
   - [ ] Bug fix
   - [ ] New feature
   - [ ] Breaking change
   - [ ] Documentation update
   
   ## Testing
   - [ ] Tests pass locally
   - [ ] Added new tests
   - [ ] Updated documentation
   
   ## Screenshots (if applicable)
   ```

## Debugging

### Effect Tracing

Enable Effect tracing for debugging:

```typescript
import { Effect, Logger, LogLevel } from "effect"

const program = myEffect.pipe(
  Logger.withMinimumLogLevel(LogLevel.Debug),
  Effect.withSpan("operation-name")
)
```

### Ollama Issues

```bash
# Check Ollama status
curl http://localhost:11434/api/tags

# View Ollama logs
ollama logs

# Test inference directly
curl -X POST http://localhost:11434/api/generate \
  -d '{"model": "llama3.2", "prompt": "Hello"}'
```

### Build Issues

```bash
# Clean everything
pnpm clean
rm -rf node_modules
pnpm i
pnpm build

# Check for circular dependencies
pnpm why @openagentsinc/sdk
```

## Performance Considerations

### Bundle Size

- Use tree-shaking: Mark pure functions
- Lazy load heavy dependencies
- Check bundle size with `pnpm analyze`

### Effect Best Practices

```typescript
// Good: Use Effect.all for parallel operations
const results = await Effect.all([
  fetchUser(id1),
  fetchUser(id2),
  fetchUser(id3)
], { concurrency: 3 })

// Good: Use caching for expensive operations
const getCachedData = Effect.cached(
  fetchExpensiveData,
  Duration.minutes(5)
)
```

## Security

### Never Commit Secrets

- API keys
- Private keys  
- Passwords
- Personal data

### Use Environment Variables

```typescript
// Good
const apiKey = process.env.OPENAI_API_KEY

// Bad
const apiKey = "sk-abc123..."
```

## Getting Help

### Resources

- **GitHub Issues**: Report bugs or request features
- **Discord**: Community chat (coming soon)
- **Effect Discord**: Effect.js community
- **Stack Overflow**: Tag with `openagents`

### Common Issues

See the [Troubleshooting Guide](./troubleshooting) for solutions to common problems.

---

*Thank you for contributing to OpenAgents! Your work helps build the future of autonomous AI agents.* 🚀