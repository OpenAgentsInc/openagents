# @openagentsinc/psionic

Sync-first hypermedia web framework for OpenAgents.

## Installation

```bash
pnpm add @openagentsinc/psionic
```

## Quick Start

```typescript
import { createPsionicApp, html, css, document } from '@openagentsinc/psionic'

const app = createPsionicApp({
  name: 'My App',
  port: 3000
})

app.route('/', () => {
  return document({
    title: 'Hello Psionic',
    body: html`<h1>Welcome to Psionic!</h1>`
  })
})

app.start()
```

## API

### `createPsionicApp(config)`

Creates a new Psionic application instance.

**Config options:**
- `name?: string` - Application name (shown in console)
- `port?: number` - Port to listen on (default: 3000)
- `catchAllRedirect?: boolean` - Redirect 404s to / (default: true)

### `app.route(path, handler)`

Define a route handler for GET requests.

### `html` and `css` template tags

Template literal tags for syntax highlighting and future processing.

### `document(options)`

Helper to create a complete HTML document.

## Development

### Running the example

```bash
pnpm --filter=@openagentsinc/psionic dev
```

This runs the hello world example on http://localhost:3002

### Building

```bash
pnpm --filter=@openagentsinc/psionic build
```

## Architecture

Psionic is built on:
- **Bun** - Fast JavaScript runtime
- **Elysia** - Bun-first web framework
- **Hypermedia** - Server-rendered HTML as the primary interface

## Future Features

- Effect service integration
- WebSocket event streaming
- Server-rendered components
- HTMX integration
- Living AI interfaces