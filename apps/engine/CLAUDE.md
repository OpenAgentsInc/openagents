# Engine Development Guide

## Project Context
- This is the Agent Engine within a monorepo - focus ONLY on `/apps/engine`
- Purpose: Local web-based autonomous agent for processing GitHub issues
- Uses Effect framework for functional programming patterns

## Build & Run Commands
- Build and run in one step: `pnpm start` or `pnpm github`
- Run with auto-reload: `pnpm watch`
- Build only: `pnpm build-esm`
- Run after build: `pnpm dev`
- Run typechecks: `pnpm check` - ALWAYS run before finishing work
- Run linter: `pnpm lint` or `pnpm lint-fix`
- Run all verification: `pnpm verify` (typecheck + lint + test)
- Run tests: `pnpm test:run` (non-watch mode)
- Run single test: `pnpm test -- --run "path/to/test.test.ts"`
  - Note: IMPORTANT to do it like this, not `pnpm test {filepath}` because that will trigger watch mode
- Run tests with coverage: `pnpm coverage`

## Code Style Guidelines
- **TypeScript**: Strict mode with full type safety
- **Imports**: Use Effect imports with proper namespacing
- **Formatting**: 2-space indentation, 120 line width, no semicolons
- **Naming**: Underscore prefix for unused variables (e.g., `_unused`)
- **Types**: NEVER use `any` type under any circumstances. Always use proper typings, even if it requires extra work
- **Error Handling**: Use Effect's error handling patterns with proper boundaries
- **Arrays**: Use generic array types (e.g., `Array<string>`)
- **Quotes**: Always use double quotes for strings

## Testing Requirements
- Write unit tests for all core functionality
- Create GitHub API mocks for testing
- Test all error handling scenarios
- Ensure all tests and typechecks pass before committing: `pnpm verify`

## Special Rules
- **NEVER use `any` type**: The codebase must have perfect types throughout. Look at the TypeScript definitions to find the correct type instead of using `any` as a shortcut
- Always use Effect's functional patterns for handling asynchronous code and errors
