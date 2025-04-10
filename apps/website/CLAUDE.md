# OpenAgents Website Development Guide

## Build & Run Commands
- Start dev server: `yarn dev`
- Build for production: `yarn build`
- Preview production build: `yarn preview`
- Run typechecks: `yarn t` or `yarn typecheck`
- Deploy to Cloudflare: `yarn deploy`

## Code Style Guidelines
- **TypeScript**: Use strict mode with verbatimModuleSyntax
- **Imports**: Group by React/libraries first, then components, then utilities
- **Formatting**: Use consistent spacing and indentation (2 spaces)
- **Components**: Use functional components with React hooks
- **Naming**: PascalCase for components/types, camelCase for functions/variables
- **Types**: Always define explicit types (avoid `any`)
- **Paths**: Use aliased imports (`@/*` or `~/*` for app directory)
- **Error Handling**: Use try/catch with appropriate error logging

## UI Components
- Always use shadcn/ui components from `app/components/ui/`
- If a needed UI component doesn't exist, ask the user to add it first
- Use the `ThemeProvider` for theme management
- Use the `useTheme` hook to access/modify theme settings
- Prefer utility functions from `app/lib/utils.ts` (e.g., `cn()` for class merging)

## Cloudflare Workers
- Remember this app uses Cloudflare Workers for deployment
- Run `wrangler types` before typecheck for proper type generation
- Test locally with `wrangler dev` before deploying
- Use `wrangler versions upload` for preview URLs