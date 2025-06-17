# @openagentsinc/psionic

Sync-first hypermedia web framework for OpenAgents.

## Development

### Prerequisites

- Bun runtime installed
- Run `pnpm install` from the repository root to install dependencies

### Dependency Management

This package uses a **hybrid approach**:
- **pnpm** for all dependency management (installing, adding, removing packages)
- **Bun** for runtime execution only (running the server, tests, etc.)

To add dependencies:
```bash
# From repository root
pnpm add <package> --filter=@openagentsinc/psionic

# Never use 'bun install' or 'bun add' in this monorepo
```

### Running the development server

From the repository root:

```bash
pnpm --filter=@openagentsinc/psionic dev
```

Or from the package directory:

```bash
cd packages/psionic
pnpm run dev
```

The server will start on http://localhost:3002

### Available endpoints

- `GET /` - Minimal Psionic landing page (black background, centered text)

### Hot reload

The development server supports hot reload. Make changes to `src/index.ts` and the server will automatically restart.

## Architecture

Psionic is built on:
- **Bun** - Fast JavaScript runtime
- **Elysia** - Bun-first web framework
- **Hypermedia** - Server-rendered HTML as the primary interface

## Next Steps

This is a basic scaffold. The full implementation (as described in issue #935) will include:
- Effect service integration
- WebSocket event streaming
- Server-rendered components
- HTMX integration
- Living AI interfaces