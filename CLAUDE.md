# OpenAgents Development Guide

## Build & Run Commands
- Start dev server: `pnpm start` or `expo start`
- Run on Android: `pnpm android` 
- Run on iOS: `pnpm ios`
- Run on web: `pnpm web`
- Run typechecks: `pnpm t` or `pnpm --filter <package-name> t`

## Code Style Guidelines
- **TypeScript**: Use strict mode for all new code
- **Imports**: Group imports with React/Expo first, then components, then utilities
- **Formatting**: 2-space indentation, trailing commas in multi-line objects
- **Components**: Use functional components with hooks
- **Naming**: PascalCase for components, camelCase for variables/functions
- **Types**: Use explicit typing rather than `any` wherever possible
- **Error Handling**: Use try/catch with specific error types

## Project Structure
- Keep related files (components, styles, tests) together
- Use relative imports for related files, absolute for distant imports

## Validation Requirements
- **ALWAYS run typechecks** before committing: `pnpm t` to check all workspaces, or `pnpm --filter <package-name> t` for specific package
- Fix all type errors before submitting changes
- React Native has specific styling types - ensure styles follow platform constraints
- Web-specific styles (like `calc()`, `sticky`, etc.) must be handled with platform-specific code

## Special Rules
- Cloudflare Workers: See `.cursor/rules/cloudflare.mdc` for specific guidelines