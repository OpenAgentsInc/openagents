# Overnight Development Guide

## Build & Test Commands
- Run typecheck: `yarn t` or `yarn check`
- Run ESLint: `yarn lint` or `yarn lint-fix` to auto-fix issues
- Run tests: `yarn test` or `yarn test <pattern>` for specific tests
- Run single test file: `yarn test test/path/to/file.test.ts`
- Build package: `yarn build`
- Run program: `yarn dev` or `tsx ./src/Program.ts`

## Code Style Guidelines
- **TypeScript**: Strict mode with explicit types. Avoid `any`
- **Formatting**: 2-space indentation, double quotes, no semicolons, no trailing commas
- **Imports**: Use `import * as Module from "module"` pattern for Effect modules
- **Naming**: PascalCase for types/interfaces, camelCase for variables/functions
- **Components**: Follow Effect pattern for functional composition
- **Error Handling**: Use Effect's error handling capabilities (`Effect.try`, `Effect.catch`)
- **Testing**: Use `@effect/vitest` for writing tests

## Project Structure
- `/src`: Source code
- `/test`: Test files (follow `.test.ts` naming convention)
- `/docs`: Documentation and project history
- `/build`: Compiled output (not committed)

## Validation Requirements
- Always run typechecks before committing: `yarn t`
- Fix all lint errors: `yarn lint-fix`
- Ensure tests pass: `yarn test`