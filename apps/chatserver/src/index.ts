
import { serve } from '@hono/node-server';
import { Hono } from 'hono';
import { stream } from 'hono/streaming';
import type { LanguageModelV1StreamPart } from "ai";
import { streamText, extractReasoningMiddleware, wrapLanguageModel } from "ai";
import { createWorkersAI } from "workers-ai-provider";
import { env } from "cloudflare:workers"

const app = new Hono();

app.post('/', async c => {
  const workersai = createWorkersAI({ binding: env.AI });
  const result = streamText({
    model: workersai("@cf/meta/llama-3.3-70b-instruct-fp8-fast"),
    prompt: 'Invent a new holiday and describe its traditions.',
  });

  // Mark the response as a v1 data stream:
  c.header('X-Vercel-AI-Data-Stream', 'v1');
  c.header('Content-Type', 'text/plain; charset=utf-8');

  return stream(c, stream => stream.pipe(result.toDataStream()));
});

serve({ fetch: app.fetch, port: 8080 });
