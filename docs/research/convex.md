# Convex Integration with Bun, Vanilla JS/HTMX, and Effect TypeScript

## Bun runtime compatibility is excellent

**Convex officially supports Bun** for client-side operations with full functionality. The integration is seamless - you can use `bun install convex` and `bunx convex dev` without any issues. The key architectural insight is that Convex backend functions run on their own optimized runtime (similar to Cloudflare Workers), while your Bun application handles the client side perfectly.

```typescript
// Initialize Convex client in Bun
import { ConvexHttpClient } from "convex/browser";
const httpClient = new ConvexHttpClient(process.env.CONVEX_URL);

// Real-time subscriptions work perfectly with Bun
import { ConvexClient } from "convex/browser";
const client = new ConvexClient(process.env.CONVEX_URL);
const unsubscribe = client.onUpdate(api.messages.list, {}, (messages) =>
  console.log(messages)
);
```

## Vanilla JS + HTMX integration requires a server bridge

Since HTMX operates through server interactions, you'll need to create endpoints that bridge between HTMX requests and Convex operations. Here's a complete implementation pattern:

```javascript
// Server-side bridge (using Elysia/Bun)
import { Elysia } from 'elysia';
import { ConvexClient } from "convex/browser";

const app = new Elysia();
const convexClient = new ConvexClient(process.env.CONVEX_URL);

// SSE endpoint for real-time updates
app.get('/convex-stream', ({ set }) => {
  set.headers['content-type'] = 'text/event-stream';

  return new ReadableStream({
    start(controller) {
      const unsubscribe = convexClient.onUpdate(
        api.messages.list,
        {},
        (messages) => {
          const html = messages.map(msg =>
            `<div class="message">${msg.content}</div>`
          ).join('');

          controller.enqueue(`event: messageUpdate\n`);
          controller.enqueue(`data: ${html}\n\n`);
        }
      );

      return () => unsubscribe();
    }
  });
});

// Mutation endpoint
app.post('/api/convex/messages', async ({ body }) => {
  const { content } = body;
  await convexClient.mutation(api.messages.send, { content });
  return `<div class="message">${content}</div>`;
});
```

```html
<!-- Frontend HTML with HTMX -->
<div hx-ext="sse"
     sse-connect="/convex-stream"
     sse-swap="messageUpdate">
  <div id="messages-container">
    <!-- Messages update here via SSE -->
  </div>
</div>

<form hx-post="/api/convex/messages"
      hx-target="#messages-container"
      hx-swap="beforeend">
  <input name="content" type="text" required>
  <button type="submit">Send</button>
</form>
```

## Effect TypeScript integration is well-supported

There's an existing library `@maple/convex-effect` that provides Effect wrappers, or you can create custom integration patterns:

```typescript
// Using the existing library
import { createServerApi } from "@maple/convex-effect"
import type { DataModel } from "./_generated/dataModel"

export const { query, mutation, action } = createServerApi<DataModel>()

// Custom Effect service implementation
import { Effect, Context, Layer } from "effect"

// Define service interface
export interface ConvexService {
  readonly query: <T>(
    name: string,
    args?: Record<string, any>
  ) => Effect.Effect<T, ConvexQueryError | ConvexConnectionError>

  readonly mutation: <T>(
    name: string,
    args?: Record<string, any>
  ) => Effect.Effect<T, ConvexMutationError | ConvexConnectionError>
}

export const ConvexService = Context.GenericTag<ConvexService>("ConvexService")

// Implementation with error handling and retry
const make = Effect.gen(function* () {
  const convexUrl = yield* Config.string("CONVEX_URL")
  const client = new ConvexHttpClient(convexUrl)

  return ConvexService.of({
    query: (name, args) =>
      Effect.tryPromise({
        try: () => client.query(name as any, args),
        catch: (error) => new ConvexQueryError(name, error)
      }),

    mutation: (name, args) =>
      Effect.tryPromise({
        try: () => client.mutation(name as any, args),
        catch: (error) => new ConvexMutationError(name, error)
      })
  })
})

// Usage with retry and error handling
const program = Effect.gen(function* () {
  const convex = yield* ConvexService

  const result = yield* pipe(
    convex.query("users:list"),
    Effect.retry(Schedule.exponential("100 millis")),
    Effect.catchTags({
      ConvexConnectionError: (error) =>
        Effect.logError(`Connection failed: ${error.message}`)
    })
  )

  return result
})
```

## Server-side rendering with Elysia works excellently

Elysia provides native SSR support that integrates well with Convex's server-side querying capabilities:

```typescript
// Elysia SSR setup with Convex
import { Elysia } from 'elysia';
import { fetchQuery } from 'convex/nextjs';
import { renderToReadableStream } from 'react-dom/server';

const app = new Elysia()
  .get('/dashboard/:userId', async ({ params, headers }) => {
    // Authentication check
    const token = extractToken(headers);
    if (!token) {
      return new Response(null, {
        status: 302,
        headers: { Location: '/login' }
      });
    }

    // Fetch user data server-side
    const userData = await fetchQuery(
      api.users.getProfile,
      { userId: params.userId },
      { token }
    );

    // Pre-render with data
    const stream = await renderToReadableStream(
      <Dashboard initialData={userData} />,
      {
        bootstrapScripts: ['/client.js'],
        bootstrapModules: ['/hydrate.mjs']
      }
    );

    return new Response(stream, {
      headers: { 'Content-Type': 'text/html' }
    });
  });

// Client-side hydration
const serverState = window.__CONVEX_STATE__;
const convexClient = new ConvexClient(process.env.CONVEX_URL);

hydrateRoot(
  document.getElementById('root'),
  <ConvexProvider client={convexClient}>
    <App initialData={serverState} />
  </ConvexProvider>
);
```

## Key limitations and architectural considerations

**The primary limitation is architectural**: Convex backend functions cannot run on Bun runtime - they execute on Convex's own optimized runtime. This is actually beneficial as it provides:
- Zero cold starts for database operations
- Automatic scaling and optimization
- Built-in real-time synchronization

**Recommended architecture**:
```
Frontend (Bun + HTMX + Effect)
    ↓ HTTP/WebSocket
Bridge Layer (Elysia on Bun)
    ↓ Convex Client SDK
Convex Backend (Functions + Database)
```

## Implementation recommendations for OpenAgents

For your Bitcoin-powered AI agents platform, I recommend:

1. **Use Bun for all client-side code** - You get 4x faster startup and excellent performance
2. **Create an Elysia API layer** that bridges HTMX requests to Convex operations
3. **Leverage Effect for complex workflows** - Especially useful for handling Bitcoin transactions and AI agent orchestration
4. **Implement SSR for public-facing pages** using Elysia's streaming capabilities
5. **Use Convex's real-time features** for agent status updates and notifications

Here's a complete example integrating all technologies:

```typescript
// server.ts - Main Elysia server
import { Elysia } from 'elysia';
import { Effect, Layer, pipe } from 'effect';
import { ConvexService, ConvexServiceLive } from './services/convex';
import { AgentService, AgentServiceLive } from './services/agents';

const app = new Elysia()
  // HTMX endpoint for agent operations
  .post('/agents/create', async ({ body }) => {
    const program = Effect.gen(function* () {
      const agents = yield* AgentService;
      const result = yield* agents.createAgent({
        name: body.name,
        bitcoinAddress: body.bitcoinAddress,
        capabilities: body.capabilities
      });

      // Return HTMX-friendly HTML fragment
      return `
        <div class="agent-card" id="agent-${result._id}">
          <h3>${result.name}</h3>
          <p>Status: ${result.status}</p>
        </div>
      `;
    });

    const html = await Effect.runPromise(
      pipe(program, Effect.provide(AgentServiceLive))
    );

    return html;
  })

  // SSE for real-time agent updates
  .get('/agents/stream', ({ set }) => {
    set.headers['content-type'] = 'text/event-stream';

    const program = Effect.gen(function* () {
      const convex = yield* ConvexService;

      return new ReadableStream({
        start(controller) {
          const unsubscribe = convex.subscribe(
            api.agents.list,
            {},
            (agents) => {
              const html = renderAgentList(agents);
              controller.enqueue(`event: agentUpdate\n`);
              controller.enqueue(`data: ${html}\n\n`);
            }
          );

          return () => unsubscribe();
        }
      });
    });

    return Effect.runPromise(
      pipe(program, Effect.provide(ConvexServiceLive))
    );
  })
  .listen(3000);
```

This architecture provides a robust, type-safe, and performant foundation for your OpenAgents platform with excellent developer experience and real-time capabilities.
