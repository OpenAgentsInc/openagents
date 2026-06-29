# Long-running inference on Cloudflare — response strategies

*2026-06-22. Reference for how the Khala / inference gateway must handle
completions that take tens of seconds to minutes, so we never hit edge timeouts.
Written after a `524` postmortem (below).*

## The 524 postmortem

A `POST /v1/chat/completions` for `openagents/khala-code` with the full
"build a single-file three.js crossy-road game" prompt returned
`{"error":"provider_error","reason":"fireworks responded 524"}`. Root cause: the
request was **non-streaming** (`stream:false`) via a blocking `curl`. The Worker
awaited the entire multi-minute Fireworks completion before responding; the
Cloudflare edge gives up on an origin that produces no bytes for ~100s and
returns **524**. `curl` was not the problem — **synchronous, non-streaming
buffering** was. A short prompt on the same model succeeded with a full metered
receipt; only the long single-shot generation timed out.

## Rule of thumb

- **Interactive call → stream (SSE).** Default for anything a human/agent waits on.
- **Detached / minutes-long / agentic → async batch job (Queue/DO).** Never a
  single blocking request.
- **Live UI (cockpit, Verse) → Durable Object + WebSocket.**
- Never hold a single synchronous HTTP request open for work that can exceed
  ~60–90s.

## Strategy 1 — Streaming SSE (the default)

Stream tokens as they are generated. First byte in ~1s; every chunk resets the
idle timer, so a 3-minute generation never trips the edge timeout. This is how
every major inference API serves long generations.

**We have it, and as of #6035 it truly passes through.** `src/inference/
chat-completions-routes.ts` supports `stream: true` (the
`if (inferenceRequest.stream)` branch, OpenAI SSE `data: …\n\n` framing). The
fix for the game generation is `"stream": true` and a client that consumes SSE
(`curl --no-buffer`, or the SDK's stream mode).

**Caveat that bit us (#6035 / refs #6027):** the original streaming
implementation was NOT a true pass-through. The Fireworks adapter `stream`
returned a fully-materialized `ReadonlyArray<InferenceStreamChunk>` (it read the
WHOLE upstream SSE body before returning a single chunk), and the route then
serialized all chunks into one string `Response`. So even with `stream:true`,
the edge saw no bytes until the entire multi-minute generation completed — still
a 524. On top of that, OpenAI-compatible providers omit the `usage` object from
streamed responses unless you send `stream_options.include_usage`, so a short
streamed prompt failed receipt-first with `fireworks stream missing terminal
usage frame`. #6035 fixes both: the adapter now opts in to the terminal usage
frame and exposes an incremental `streamSse` source over the upstream
`response.body`; the route pumps each frame to the client as it arrives (no
server-side buffering), so every chunk resets the edge idle-timer and a 3-minute
generation never 524s. Metering still settles receipt-first from the terminal
usage frame after the upstream closes. (A non-streaming `stream:false`
crossy-road call still 524s — that is fundamental synchronous buffering on the
edge; the fix is to stream, or use Strategy 2 for detached runs.)

Caveat: the `openagents` receipt block + verification verdict are emitted at the
**end** of the stream (a terminal SSE event), since verification runs on the full
output. Clients render tokens live, then attach the receipt/verdict on stream
close.

## Strategy 2 — Async batch jobs (detached / long / agentic)

Submit → get a job id (`202`) → a **Queue** consumer or **Durable Object** runs
the work *off the request path* (no edge timeout exists there) → persist a
**dereferenceable receipt** → client polls a status/receipt route (or receives a
webhook). Right for Autopilot coding runs and any work that legitimately takes
minutes, or that should survive client disconnects.

**Rails we already have:** `src/inference/batch-job-routes.ts`
(`handleBatchJobsSubmit` / `handleBatchJobStatusRead` / `handleBatchJobReceiptRead`),
the `inference_batch_jobs` D1 table (migration `0217`), the public receipt route
`/api/public/inference/batch-job-receipts/:receiptRef`, and Cloudflare **Queues**
(`openagents-autopilot-runner-events`, `openagents-adjutant-enrichment-jobs`) +
**Durable Objects** already bound in `wrangler.jsonc`. **Gap:** the Queue/DO
consumer that actually executes a submitted batch job against the gateway +
writes the receipt. Cloudflare **Workflows** (durable, multi-step, retried) is
the purpose-built primitive if a job has stages.

## Strategy 3 — Durable Object + hibernatable WebSocket (live UIs)

A Region/session DO owns the long generation, survives its whole duration, and
fans tokens + world-events to many subscribers over a hibernatable WebSocket.
This is the engine the Verse already runs on (`apps/openagents-world` Region DO,
`packages/world-client`). Use it when you also want multiplayer or the in-world
"watch the energy flow" visualization, not just a single client stream.

## What to wire (tracked as issues, EPIC #6017)

1. **Stream interactive khala calls.** Rewire the M8 head-to-head runner and the
   Autopilot cockpit to `stream:true` + SSE; make streaming the default for
   interactive paths. (Immediate 524 fix.)
2. **Batch-job consumer.** Wire a Queue (or DO/Workflow) consumer that executes
   submitted `inference_batch_jobs` against the gateway and writes the
   dereferenceable receipt, so detached/minutes-long runs never touch the edge
   timeout. Surface submit + poll on the OpenAI-compatible surface.

## One line

Interactive → stream; detached/long → batch job (Queue/DO); live UI → DO +
WebSocket. The pieces exist in-repo; the 524 was using none of them.
