# OpenAgents Development Guide

## Build & Run Commands
- Start dev server: `yarn start` or `expo start`
- Run on Android: `yarn android` 
- Run on iOS: `yarn ios`
- Run on web: `yarn web`
- Run typechecks: `yarn t` or `yarn workspace <workspace-name> t`

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
- **ALWAYS run typechecks** before committing: `yarn t` to check all workspaces, or `yarn workspace <workspace-name> t` for specific package
- Fix all type errors before submitting changes
- React Native has specific styling types - ensure styles follow platform constraints
- Web-specific styles (like `calc()`, `sticky`, etc.) must be handled with platform-specific code

## Special Rules
- Cloudflare Workers: See `.cursor/rules/cloudflare.mdc` for specific guidelines