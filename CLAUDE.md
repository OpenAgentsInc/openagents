# OpenAgents Development Guide

## Build & Run Commands
- Start dev server: `yarn start` or `expo start`
- Run on Android: `yarn android` 
- Run on iOS: `yarn ios`
- Run on web: `yarn web`

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

## Special Rules
- Cloudflare Workers: See `.cursor/rules/cloudflare.mdc` for specific guidelines