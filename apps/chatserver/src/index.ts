import { Hono } from 'hono';
import { stream } from 'hono/streaming';
import { streamText } from "ai";
import { createWorkersAI } from "workers-ai-provider";

interface Env {
  AI: typeof AI;
}

const app = new Hono<{ Bindings: Env }>();

app.get('/', c => c.text('200'));

app.post('/', async c => {
  const workersai = createWorkersAI({ binding: c.env.AI });
  const result = streamText({
    model: workersai("@cf/meta/llama-3.3-70b-instruct-fp8-fast"),
    prompt: 'Write one paragraph speculating about collaborative, multiplayer, asynchronous AI agents for coding and more',
  });

  // Mark the response as a v1 data stream:
  c.header('X-Vercel-AI-Data-Stream', 'v1');
  c.header('Content-Type', 'text/plain; charset=utf-8');

  return stream(c, stream => stream.pipe(result.toDataStream()));
});

export default app;
