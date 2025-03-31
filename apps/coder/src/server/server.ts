// apps/coder/src/server/server.ts
import { Hono } from 'hono';
import { logger } from 'hono/logger'; // Optional but helpful for debugging

// Define environment types if needed, otherwise use simple Hono<any>
// type Env = { /* ... bindings ... */ };
// const app = new Hono<Env>();
const app = new Hono();

app.use('*', logger()); // Log all requests

// --- Define API Routes Here ---
app.get('/api/ping', (c) => {
    console.log('[Server] Received /api/ping request');
    return c.json({ message: 'pong' });
});

// Placeholder for chat route
app.post('/api/chat', async (c) => {
    console.log('[Server] Received /api/chat request (placeholder)');
    // TODO: Implement actual chat logic using ported MCP client
    const body = await c.req.json();
    console.log('[Server] Chat request body:', body);
    // Simulate a streaming response for now if needed, or just a simple JSON response
    return c.json({ reply: "Chat endpoint received message.", history: body.messages }, 200);
    // Example streaming (requires Vercel AI SDK Hono adapter later):
    // const stream = /* ... AI SDK stream ... */
    // return streamToResponse(stream, c)
});

// --- End API Routes ---

export default app;