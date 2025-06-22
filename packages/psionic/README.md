# @openagentsinc/psionic

A hypermedia web framework built on Effect, providing a simple yet powerful API for building server-rendered applications with built-in component exploration, documentation systems, and seamless API integration.

## Features

- 🚀 **Effect-based HTTP Server** - Built on Effect platform for type-safe, composable web applications
- 🎨 **Component Explorer** - Built-in component library viewer for systematic UI development
- 📚 **Documentation System** - Automatic markdown documentation serving
- 🔌 **API Integration** - Easy mounting of REST APIs with Elysia compatibility
- 🎯 **Route Parameters** - Express-style route parameters (`:param`)
- 📁 **Static File Serving** - Efficient static asset serving with Bun
- 🔄 **Hot Reloading** - Fast development with Bun's built-in hot reload
- 🎭 **Hypermedia-first** - Server-rendered HTML as the primary interface

## Installation

```bash
pnpm add @openagentsinc/psionic
```

## Quick Start

```typescript
import { createPsionicApp } from '@openagentsinc/psionic'

const app = createPsionicApp({
  name: 'My App',
  port: 3000
})

// Simple route
app.get('/', () => {
  return '<h1>Welcome to Psionic!</h1>'
})

// JSON API
app.get('/api/status', () => {
  return { status: 'ok', timestamp: new Date().toISOString() }
})

// Route parameters
app.get('/users/:id', (context) => {
  return `<h1>User ${context.params.id}</h1>`
})

app.start()
```

## Core API

### Creating an Application

```typescript
const app = createPsionicApp({
  name: 'My App',                    // Application name (shown in console)
  port: 3000,                        // Port to listen on
  host: 'localhost',                 // Hostname to bind to
  staticDir: './public',             // Static files directory
  componentsDir: './stories',        // Component stories directory
  componentsPath: '/components',     // URL path for component explorer
  componentExplorerOptions: {
    styles: customCSS,               // Custom styles for explorer
    navigation: navHTML,             // Custom navigation HTML
    baseClass: 'my-theme'           // Base CSS class
  }
})
```

### HTTP Methods

Psionic supports all standard HTTP methods:

```typescript
app.get('/users', () => { /* ... */ })
app.post('/users', async (context) => {
  const body = await context.request.json()
  return { created: body }
})
app.put('/users/:id', (context) => { /* ... */ })
app.delete('/users/:id', (context) => { /* ... */ })
app.patch('/users/:id', (context) => { /* ... */ })
```

### Route Context

Every route handler receives a context object:

```typescript
interface RouteContext {
  request: Request      // Standard Web Request object
  params: Record<string, string>  // Route parameters
}

app.get('/posts/:category/:id', (context) => {
  console.log(context.params.category)  // e.g., "tech"
  console.log(context.params.id)        // e.g., "123"
  console.log(context.request.url)      // Full URL
  console.log(context.request.headers)  // Request headers
})
```

### Response Types

Psionic automatically handles different return types:

```typescript
// HTML response (Content-Type: text/html)
app.get('/page', () => '<h1>HTML Page</h1>')

// JSON response (Content-Type: application/json)
app.get('/api/data', () => ({ message: 'Hello', data: [1, 2, 3] }))

// Plain text response
app.get('/text', () => 'Plain text response')

// Custom Response object
app.get('/custom', () => {
  return new Response('Custom response', {
    status: 201,
    headers: { 'X-Custom': 'Header' }
  })
})
```

### Static File Serving

Serve static files from a directory:

```typescript
app.static('/public', { path: './static' })

// Now /public/style.css serves ./static/style.css
```

### Component Explorer

Enable a built-in component library explorer:

```typescript
app.components('./stories', {
  path: '/components',    // URL path (default: /components)
  styles: customCSS,      // Custom styles
  navigation: navHTML,    // Custom navigation
  baseClass: 'theme'      // Base CSS class
})

// Create story files in ./stories directory:
// Button.story.ts
export const title = "Button"
export const component = "Button"

export const Default = {
  name: "Default Button",
  html: `<button class="btn">Click me</button>`,
  description: "Basic button component"
}

export const Primary = {
  name: "Primary Button", 
  html: `<button class="btn btn-primary">Primary</button>`
}
```

### Documentation System

Serve markdown documentation:

```typescript
app.docs('./docs', {
  path: '/docs'    // URL path (default: /docs)
})

// Now /docs/guide.md serves ./docs/guide.md as HTML
```

### API Mounting

Mount external API routers (with Elysia compatibility):

```typescript
// Mount an API router
app.api(apiRouter, { prefix: '/api/v1' })

// Legacy Elysia compatibility
app.elysia.use(elysiaPlugin)
app.elysia.get('/legacy', handler)
app.elysia.group('/api', (api) => {
  api.get('/users', () => [])
  api.post('/users', () => {})
})
```

### WebSocket Support (Coming Soon)

```typescript
app.websocket('/ws', {
  open(ws) { /* ... */ },
  message(ws, message) { /* ... */ },
  close(ws) { /* ... */ }
})
```

## Advanced Features

### Custom HTML Documents

Use the built-in document helpers:

```typescript
import { document, html, css } from '@openagentsinc/psionic'

app.get('/', () => {
  return document({
    title: 'My Page',
    styles: css`
      body { 
        background: #000; 
        color: #fff; 
      }
    `,
    body: html`
      <h1>Welcome!</h1>
      <p>This is a Psionic app.</p>
    `
  })
})
```

### Type-Safe Route Handlers

With TypeScript, get full type safety:

```typescript
interface User {
  id: string
  name: string
  email: string
}

app.get('/api/users/:id', async (context): Promise<User> => {
  const userId = context.params.id
  // TypeScript knows userId is a string
  return {
    id: userId,
    name: `User ${userId}`,
    email: `user${userId}@example.com`
  }
})
```

### Error Handling

Errors are automatically caught and returned as appropriate responses:

```typescript
app.get('/api/data', async () => {
  // If this throws, Psionic returns a 500 error
  const data = await fetchSomeData()
  return data
})
```

## Architecture

Psionic is built on:

- **Effect** - Functional programming library for TypeScript
- **@effect/platform** - Effect's HTTP server platform
- **Bun** - Fast JavaScript runtime and bundler
- **Hypermedia** - Server-rendered HTML as the primary interface

### Effect Integration

Under the hood, Psionic uses Effect's powerful HTTP router:

```typescript
// Routes are compiled to Effect HttpRouter
const router = HttpRouter.empty.pipe(
  HttpRouter.get("/", handler),
  HttpRouter.post("/api/data", handler)
)

// Server runs on Effect platform
const HttpLive = HttpServer.serve(router).pipe(
  Layer.provide(BunHttpServer.layer({ port }))
)
```

## Development

### Running Examples

```bash
# Run the example app
cd apps/example-psionic
bun run dev
```

### Building

```bash
pnpm --filter=@openagentsinc/psionic build
```

### Testing

```bash
pnpm --filter=@openagentsinc/psionic test
```

## Migration from Elysia

If you're migrating from the previous Elysia-based version:

1. Routes work the same way
2. Use `app.elysia` for legacy compatibility
3. Plugins can be adapted using the API mounting feature
4. WebSocket support is coming soon

## Future Roadmap

- [ ] WebSocket support with Effect
- [ ] Server-sent events (SSE)
- [ ] Built-in HTMX integration
- [ ] Live reloading in development
- [ ] Type-safe route generation
- [ ] OpenAPI schema generation
- [ ] Built-in authentication helpers

## License

MIT