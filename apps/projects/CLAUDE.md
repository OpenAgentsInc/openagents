# OpenAgents Website Development Guide

This is a React Router v7 (formerly known as Remix) app using a Cloudflare Workers template.

## Build & Run Commands
- Start dev server: `pnpm dev`
- Build for production: `pnpm build`
- Preview production build: `pnpm preview`
- Run typechecks: `pnpm t` or `pnpm typecheck`
- Deploy to Cloudflare: `pnpm deploy`

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

## Routing
- Routes are defined in `app/routes.ts` using React Router v7's route configuration
- Route components should be placed in `app/routes/` directory
- Use the `index()` function for index routes (e.g., `index("routes/home.tsx")`)
- Use the `route()` function for path routes (e.g., `route("spawn", "routes/spawn.tsx")`)
- Each route component should have a matching type file in `routes/+types/` directory
- Page components should export a default component and optional loader/action functions
- Use nested routes for layouts (parent routes act as layout components)
- Follow RRv7 conventions for data loading with `loader` and mutations with `action`

## Cloudflare Workers
- Remember this app uses Cloudflare Workers for deployment
- Run `wrangler types` before typecheck for proper type generation
- Test locally with `wrangler dev` before deploying
- Use `wrangler versions upload` for preview URLs