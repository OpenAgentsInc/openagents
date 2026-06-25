Here is the planning memo based on our reading of the GTM push doc, the inference gateway implementation, and all test files.

---

## OpenCode API Compatibility — Planning Memo

### 1. Content Arrays

**Current state:** The gateway reads `record['content']` through `decodeOpenAiContent` which handles string, `null`, and `Array<{type: 'text', text: string}>` (joining text parts with `\n\n`). Only `text` content parts are supported — image/vision `image_url` parts, `input_audio`, or multimodal content are rejected as `undefined` → `invalid_request` (400).

**What OpenCode sends:** OpenCode uses the AI SDK which sends `content` as a string for text-only turns, and as an array of `{type: 'text', text: string}` when combining system + user. It does not send images currently.

**Gap:** None for OpenCode's current usage. The gateway's content parsing already handles both string and text-array forms.

**Recommended tests:**
- String content
- Text-part array (multi-part text)
- Mixed null-content-with-tool_calls (the OpenCode pattern for assistant tool-call messages)
- Rejection of unsupported content parts (image_url → 400)

---

### 2. Tool Calls (`tool_calls`)

**Current state — INBOUND:** `openai-chat-compat.ts` `inferenceToolCallsFromUnknown` parses the OpenAI `tool_calls` array on assistant messages, extracting `id`, `type: 'function'`, `function.name`, and `function.arguments`. These are threaded through `InferenceMessage.toolCalls` and preserved in the normalized `InferenceRequest.messages`.

**Current state — OUTBOUND (non-streaming):** `openAiResponse` attaches `tool_calls` to the `message` object when `result.toolCalls` is non-empty. The `InferenceResult` type carries `toolCalls?: ReadonlyArray<InferenceToolCall>`.

**Current state — OUTBOUND (streaming):** `openAiChunkDelta` forwards `tool_calls` as `InferenceToolCallDelta` arrays in each SSE delta frame. The true pass-through path (`streamSse`) also forwards `event.toolCallDeltas` directly from the upstream.

**Current state — tool result messages:** `decodeMessage` handles `role: 'tool'` messages with `tool_call_id` and `name`, preserving the full round-trip.

**Gap:** The adapter implementations (Hydralisk, Fireworks) must produce `toolCalls` on `InferenceResult` for non-streaming and `toolCallDeltas` on `InferenceStreamEvent` for streaming. The stub-echo adapter in tests returns no tool calls — there are no end-to-end integration tests that exercise a real adapter producing tool calls and verifying the full round-trip through the gateway SSE path.

**Recommended tests:**
- Non-streaming: adapter returns `toolCalls`, verify OpenAI shape in response
- Streaming (buffered): adapter returns chunks with `toolCallDeltas`, verify `tool_calls` in SSE `chat.completion.chunk` deltas
- Streaming (true pass-through): upstream `streamSse` emits `toolCallDeltas`, verify forwarded in client SSE
- Round-trip: send assistant tool_calls + tool results → verify they reach the adapter → verify response preserves new tool_calls
- Multi-turn tool conversation (tool call → tool result → next tool call)
- Empty/null content with tool calls (OpenCode pattern)
- Tool call arguments with JSON containing special characters

---

### 3. Streaming SSE

**Current state:** Three code paths in `chat-completions-routes.ts`:

1. **True pass-through (`streamSse`):** `makePassThroughResponseStream`, lines 1340-1618. Pumps upstream `InferenceStreamSource` frame-by-frame into a `ReadableStream<Uint8Array>`. Each frame is serialized as:
   ```
   data: {"choices":[{"delta":{...},"finish_reason":null,"index":0}],"created":...,"id":"...","model":"...","object":"chat.completion.chunk"}\n\n
   ```
   Terminal frame carries `openagents` disclosure. Stream ends with `data: [DONE]\n\n`.

2. **Buffered stream (fallback):** Lines 2099-2324. Materializes entire `InferenceStreamChunk[]` array, then serializes all frames plus terminal. Used when adapter has no `streamSse` or when component channel is active.

3. **Non-streaming:** Lines 2327-2501. Single JSON response.

**Format compliance:** The SSE frames follow the OpenAI streaming Chat Completions format. Delta objects may be `{}` (empty), `{content: string}`, or `{tool_calls: [...]}`. The `finish_reason` is `null` for intermediate frames and `"stop"`/`"length"`/`"tool_calls"` on terminal.

**Notable detail:** The true pass-through path runs metering outside the Effect topology via `Effect.runPromise` in the flush step — a necessary bridge because `ReadableStream.start` is async but not Effect-shaped.

**Key risks for OpenCode:**
- The `data: [DONE]` terminator must be present (it is).
- Each SSE frame must be a complete JSON line (it is — single `data:` per frame).
- Tool call deltas in streaming must be forward-compatible (OpenCode processes them via AI SDK).

**Recommended tests:**
- OpenCode sends `stream: true` → receives valid SSE with content deltas
- SS evaluates frames arrive interleaved with content for tool-calling models
- Clean `data: [DONE]` termination on every stream
- Stream fault (upstream throws mid-stream) → clean SSE close, no metering
- Durable stream path: disconnect + resume by offset
- Component channel SSE format (optional `event: oa.component` events)

---

### 4. Usage Accounting

**Current state:** Receipt-first metering via `MeteringHook`. The hook receives a `MeteringContext` with real `usage` from the provider and returns a `MeteringOutcome` with `receiptRef`. Metering fires:
- Non-streaming: after dispatch, before response (line 2408)
- Buffered stream: from terminal usage frame in chunks (line 2129)
- True pass-through: via `Effect.runPromise` in terminal frame builder (line 1426)

After metering, `ServedTokensRecorder` writes a `token_usage_events` row idempotently keyed on `requestId`. The public counter at `GET /api/public/khala-tokens-served` sums this table.

**What OpenCode expects:** OpenAI-compatible `usage` in non-streaming responses (`{prompt_tokens, completion_tokens, total_tokens}`). Streaming responses carry `usage` only in the terminal SSE frame's `openagents` block (not standard OpenAI — OpenCode's AI SDK does not read it from the terminal chunk).

**Gap:** The terminal SSE frame does NOT include a standard `usage` property inside the `choices[0].delta` — usage is only on the non-standard `openagents` block. OpenCode/AI SDK does not expect `usage` from streaming chunks and derives its own estimates, so this is acceptable but worth documenting.

**Recommended tests:**
- Non-streaming: `usage` in response body matches provider usage
- Streaming: no `usage` in intermediate SSE frames (correct)
- Token counters increment on the public `/khala-tokens-served` endpoint
- Zero-token completion → no ledger row (skip condition in `served-tokens-recorder.ts:176`)
- Idempotency: same requestId → no double count
- Free tier completion still increments served-tokens counter

---

### 5. Quota / 402 Behavior

**Current state — gate ordering in `handleChatCompletions`:**

| Step | Gate | Status | Error |
|------|------|--------|-------|
| 1 | Feature flag (`enabled`) | 404 | `inference_gateway_disabled` |
| 2 | Auth | 401 | `unauthorized` |
| 3 | Fair-share rate limit | 429 | `rate_limited` |
| 4 | Body parse | 400 | `invalid_request` / `invalid_json` |
| 5 | Model validation | 400 | `model_unavailable` |
| 6 | Lane arming (serving policy) | 400 | `model_unavailable` |
| 7 | Premium access grant | 403 | `premium_model_not_allowed` |
| 8 | Balance (with bypasses) | 402 | `insufficient_credits` |
| 9 | Spend cap | 402 | `spend_cap_exceeded` |
| 10 | Dispatch → provider | 502 | `provider_error` |

**Free tier specifics:**
- 200 requests / 200,000 tokens per UTC day per free key
- Free tier bypasses balance gate (step 8)
- Over-quota → falls through to normal balance gate (402 if unfunded)
- Premium models NEVER free
- Keys minted via `POST /api/keys/free`, rate-limited per-IP-per-day

**What OpenCode expects:**
- 402 with clear `insufficient_credits` body (handled by AI SDK / OpenCode)
- 429 with `RateLimit-*` headers for rate limiting
- 401 with `WWW-Authenticate: Bearer` for auth errors
- All errors as JSON with `error` field

**Recommended tests:**
- Zero balance + no bypass → 402 `insufficient_credits`
- Free tier within quota → 200 (bypasses 402)
- Free tier over quota (requests) → 402
- Free tier over quota (tokens) → 402
- Free tier + premium model → 403 `premium_model_not_allowed`
- 429 with RateLimit headers
- Spend cap exceeded → 402 `spend_cap_exceeded`
- Operator exemption → 200 on zero balance
- Free allowance bypass → 200 on zero balance

---

### 6. Regression Test Strategy

**Existing coverage:** 74 test files under `src/inference/`. The main file `chat-completions-routes.test.ts` is 4,072 lines covering: flag gate, auth, balance gate, bypasses (free allowance, operator exemption, free tier), malformed body, content arrays, tool call metadata, stub dispatch, metering hook invocation, funding kind threading, served tokens recording, streaming (buffered), streaming (true pass-through with gated frames), durable stream, identity guard, component channel, model_unavailable, lane arming, premium gate, spend cap, fair-share rate limit, cache-aware routing, acceptance dispatch, trace emission.

**Coverage gaps for OpenCode scenario:**
- End-to-end streaming with real tool-call deltas (the gated-frame test in `fireworks-stream-passthrough.test.ts` proves incremental pass-through but uses only content deltas, not tool call deltas)
- Multi-turn tool conversation through the gateway (multiple tool call → result rounds)
- Free tier exactly-at-quota boundary test (200th request)
- Free tier token-quota exhaustion mid-request
- Concurrent requests from same free key
- Streaming disconnect handling from OpenCode's perspective (how long before reconnect)
- SSE parsing resilience: extra whitespace, multiple `data:` lines, comments in stream

**Recommended new tests:**
1. `test('streaming with incremental tool call deltas: forwards each delta as it arrives')`
2. `test('multi-turn tool conversation: preserves tool_calls and tool_results across messages')`
3. `test('free tier: last request within quota succeeds, next request 402s')`
4. `test('free tier: token quota exhausted mid-stream causes next request 402')`
5. `test('streaming: multiple concurrent SSE consumers receive correct content')`

---

### 7. Production Smoke Coverage

**Current state:** `pylon-fabric-smoke-routes.test.ts` tests the operator-only smoke route that runs a known-answer canary through the configured fabric adapter. This verifies the adapter is live and responding.

**What a production smoke suite for OpenCode should cover:**

| Smoke | What it proves | Route |
|-------|---------------|-------|
| Canary (known-answer) | Adapter is live, returns expected output | `POST /api/operator/inference/pylon-fabric/smoke` |
| Non-streaming chat | Full gateway path (auth → model → dispatch → metering → response) | `POST /api/v1/chat/completions` |
| Streaming chat | SSE frames, `data: [DONE]`, no 524 timeout | `POST /api/v1/chat/completions?stream=true` |
| Tool calling | Model produces valid function calls | `POST /api/v1/chat/completions` (with tool definitions) |
| Free tier | Free key mint → valid free completion | `POST /api/keys/free` → `POST /api/v1/chat/completions` |
| 402 rejection | Unfunded request on non-free key → clean 402 | `POST /api/v1/chat/completions` (no key, no balance) |
| Model listing | `/v1/models` returns `openagents/khala` | `GET /api/v1/models` |
| Price quote | `/v1/quote` returns estimate | `POST /api/v1/quote` |
| Served-tokens counter | Completion increases counter | `GET /api/public/khala-tokens-served` |
| Model retrieve | `/v1/models/openagents/khala` returns metadata | `GET /api/v1/models/openagents/khala` |

---

### Summary of Priority Actions

1. **Immediate (no code needed, just test config):** Verify OpenCode can reach the endpoint with the config from the GTM doc. Test tool calling, streaming, and free tier manually.

2. **Add regression tests** for:
   - Streaming tool-call deltas (extend `fireworks-stream-passthrough.test.ts`)
   - Multi-turn tool conversation
   - Free tier boundary conditions (quota exhaustion)
   
3. **Production smoke scripts** that exercise all seven smoke types above, runnable against staging/prod with a free key.

4. **Documentation** for the GTM doc: confirm the OpenCode model key naming (the `openagents/openagents/khala` double-segment concern in §3.1). Consider accepting a shorter model key server-side so the OpenCode selector shows `openagents/khala` cleanly.
