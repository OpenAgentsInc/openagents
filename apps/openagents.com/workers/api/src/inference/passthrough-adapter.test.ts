import { Cause, Effect, Exit, Option, Redacted } from "effect";
import { describe, expect, test } from "vitest";

import {
  type PassthroughAdapterConfig,
  type PassthroughFetch,
  makePassthroughAdapter,
} from "./passthrough-adapter";
import {
  InferenceAdapterError,
  type InferenceRequest,
  type InferenceToolCallDelta,
} from "./provider-adapter";

const run = <A>(
  effect: Effect.Effect<A, InferenceAdapterError>,
): Promise<Exit.Exit<A, InferenceAdapterError>> => Effect.runPromiseExit(effect);

const successValue = <A>(exit: Exit.Exit<A, InferenceAdapterError>): A => {
  if (!Exit.isSuccess(exit)) {
    throw new Error(`expected success, got: ${String(exit.cause)}`);
  }
  return exit.value;
};

const failureError = (exit: Exit.Exit<unknown, InferenceAdapterError>): InferenceAdapterError => {
  expect(Exit.isFailure(exit)).toBe(true);
  const failure = Exit.isFailure(exit) ? Cause.findErrorOption(exit.cause) : Option.none();
  if (Option.isNone(failure)) {
    throw new Error("expected a typed InferenceAdapterError failure");
  }
  expect(failure.value).toBeInstanceOf(InferenceAdapterError);
  return failure.value;
};

// Capture the last fetch call so we can assert request mapping.
type Captured = { url: string; init: RequestInit };

const fetchReturning =
  (status: number, body: unknown, captured?: Array<Captured>): PassthroughFetch =>
  async (url, init) => {
    captured?.push({ url, init });
    return new Response(JSON.stringify(body), {
      headers: { "content-type": "application/json" },
      status,
    });
  };

const fetchRawBody =
  (status: number, rawBody: string, captured?: Array<Captured>): PassthroughFetch =>
  async (url, init) => {
    captured?.push({ url, init });
    return new Response(rawBody, { status });
  };

const openAiConfig = (
  fetch: PassthroughFetch,
  overrides: Partial<PassthroughAdapterConfig> = {},
): PassthroughAdapterConfig => ({
  apiKey: Redacted.make("sk-openai-test"),
  baseUrl: "https://api.openai.com",
  fetch,
  id: "passthrough-openai",
  wireFormat: "openai",
  ...overrides,
});

const anthropicConfig = (
  fetch: PassthroughFetch,
  overrides: Partial<PassthroughAdapterConfig> = {},
): PassthroughAdapterConfig => ({
  apiKey: Redacted.make("sk-ant-test"),
  baseUrl: "https://api.anthropic.com",
  fetch,
  id: "passthrough-anthropic",
  wireFormat: "anthropic",
  ...overrides,
});

const request = (overrides: Partial<InferenceRequest> = {}): InferenceRequest => ({
  messages: [{ content: "hello world", role: "user" }],
  model: "gpt-test",
  passthroughParams: {},
  stream: false,
  ...overrides,
});

const openAiPayload = {
  choices: [
    {
      finish_reason: "stop",
      index: 0,
      message: { content: "hi there", role: "assistant" },
    },
  ],
  model: "gpt-test-served",
  usage: {
    completion_tokens: 7,
    prompt_tokens: 12,
    prompt_tokens_details: { cached_tokens: 4 },
    total_tokens: 19,
  },
};

const readFileTool = {
  function: {
    description: "Read a file from the project",
    name: "read_file",
    parameters: {
      properties: { path: { type: "string" } },
      required: ["path"],
      type: "object",
    },
  },
  type: "function",
};

// The exact upstream shape `gpt-5.6-luna` returns for a tool-calling turn:
// `content: null`, the answer entirely in `tool_calls`, `finish_reason:
// "tool_calls"` (captured against api.openai.com, 2026-07-31).
const openAiToolCallPayload = {
  choices: [
    {
      finish_reason: "tool_calls",
      index: 0,
      message: {
        content: null,
        role: "assistant",
        tool_calls: [
          {
            function: { arguments: '{"path":"src/main.rs"}', name: "read_file" },
            id: "call_luna_1",
            type: "function",
          },
        ],
      },
    },
  ],
  model: "gpt-5.6-luna",
  usage: { completion_tokens: 18, prompt_tokens: 141, total_tokens: 159 },
};

const openAiTwoToolCallsPayload = {
  choices: [
    {
      finish_reason: "tool_calls",
      index: 0,
      message: {
        content: null,
        role: "assistant",
        tool_calls: [
          {
            function: { arguments: '{"path":"a.rs"}', name: "read_file" },
            id: "call_luna_1",
            type: "function",
          },
          {
            function: { arguments: '{"path":"src"}', name: "list_directory" },
            id: "call_luna_2",
            type: "function",
          },
        ],
      },
    },
  ],
  model: "gpt-5.6-luna",
  usage: { completion_tokens: 32, prompt_tokens: 141, total_tokens: 173 },
};

const openAiReasoningPayload = {
  choices: [
    {
      finish_reason: "stop",
      index: 0,
      message: { content: "391", role: "assistant" },
    },
  ],
  model: "gpt-5.6-luna",
  usage: {
    completion_tokens: 96,
    completion_tokens_details: { reasoning_tokens: 92 },
    prompt_tokens: 20,
    total_tokens: 116,
  },
};

const anthropicPayload = {
  content: [
    { text: "hi ", type: "text" },
    { text: "there", type: "text" },
    { input: {}, name: "noop", type: "tool_use" },
  ],
  model: "claude-test-served",
  stop_reason: "end_turn",
  usage: { cache_read_input_tokens: 5, input_tokens: 20, output_tokens: 8 },
};

describe("passthrough adapter — OpenAI wire format", () => {
  test("maps the request to OpenAI Chat Completions and extracts usage", async () => {
    const captured: Array<Captured> = [];
    const adapter = makePassthroughAdapter(
      openAiConfig(fetchReturning(200, openAiPayload, captured)),
    );

    const exit = await run(
      adapter.complete(
        request({
          messages: [
            { content: "be terse", role: "system" },
            { content: "hello world", role: "user" },
          ],
          passthroughParams: { max_tokens: 256, temperature: 0.3, top_p: 0.9 },
        }),
      ),
    );
    const result = successValue(exit);

    // Endpoint + auth header mapping.
    expect(captured[0]?.url).toBe("https://api.openai.com/v1/chat/completions");
    const headers = captured[0]?.init.headers as Record<string, string>;
    expect(headers.authorization).toBe("Bearer sk-openai-test");

    // Body mapping: model, messages (system preserved), allow-listed params.
    const body = JSON.parse(String(captured[0]?.init.body));
    expect(body.model).toBe("gpt-test");
    expect(body.max_tokens).toBe(256);
    expect(body.temperature).toBe(0.3);
    expect(body.top_p).toBe(0.9);
    expect(body.messages).toEqual([
      { content: "be terse", role: "system" },
      { content: "hello world", role: "user" },
    ]);

    // Receipt-first usage, including the cached-input dimension.
    expect(result.content).toBe("hi there");
    expect(result.finishReason).toBe("stop");
    expect(result.servedModel).toBe("gpt-test-served");
    expect(result.usage.promptTokens).toBe(12);
    expect(result.usage.completionTokens).toBe(7);
    expect(result.usage.totalTokens).toBe(19);
    expect(result.usage.cachedPromptTokens).toBe(4);
  });

  test("falls back to default max_tokens and omits unset params", async () => {
    const captured: Array<Captured> = [];
    const adapter = makePassthroughAdapter(
      openAiConfig(fetchReturning(200, openAiPayload, captured), {
        defaultMaxTokens: 2_048,
      }),
    );

    await run(adapter.complete(request()));

    const body = JSON.parse(String(captured[0]?.init.body));
    expect(body.max_tokens).toBe(2_048);
    expect("temperature" in body).toBe(false);
    expect("seed" in body).toBe(false);
  });
});

describe("passthrough adapter — Anthropic wire format", () => {
  test("maps the request to Anthropic Messages, splitting out system", async () => {
    const captured: Array<Captured> = [];
    const adapter = makePassthroughAdapter(
      anthropicConfig(fetchReturning(200, anthropicPayload, captured)),
    );

    const exit = await run(
      adapter.complete(
        request({
          messages: [
            { content: "be terse", role: "system" },
            { content: "hello", role: "user" },
            { content: "hi", role: "assistant" },
            { content: "more", role: "user" },
          ],
          model: "claude-test",
          passthroughParams: { max_tokens: 512, top_k: 5 },
        }),
      ),
    );
    const result = successValue(exit);

    // Endpoint + Anthropic auth/version headers.
    expect(captured[0]?.url).toBe("https://api.anthropic.com/v1/messages");
    const headers = captured[0]?.init.headers as Record<string, string>;
    expect(headers["x-api-key"]).toBe("sk-ant-test");
    expect(headers["anthropic-version"]).toBe("2023-06-01");
    expect(headers.authorization).toBeUndefined();

    // system is hoisted out of messages; only user/assistant turns remain.
    const body = JSON.parse(String(captured[0]?.init.body));
    expect(body.model).toBe("claude-test");
    expect(body.max_tokens).toBe(512);
    expect(body.top_k).toBe(5);
    expect(body.system).toBe("be terse");
    expect(body.messages).toEqual([
      { content: "hello", role: "user" },
      { content: "hi", role: "assistant" },
      { content: "more", role: "user" },
    ]);

    // Text blocks concatenated; tool_use ignored. Usage from input/output.
    expect(result.content).toBe("hi there");
    expect(result.finishReason).toBe("stop");
    expect(result.servedModel).toBe("claude-test-served");
    expect(result.usage.promptTokens).toBe(20);
    expect(result.usage.completionTokens).toBe(8);
    expect(result.usage.totalTokens).toBe(28);
    expect(result.usage.cachedPromptTokens).toBe(5);
  });

  test("maps Anthropic stop_reason max_tokens to finish_reason length", async () => {
    const adapter = makePassthroughAdapter(
      anthropicConfig(fetchReturning(200, { ...anthropicPayload, stop_reason: "max_tokens" })),
    );
    const exit = await run(adapter.complete(request({ model: "claude-test" })));
    expect(successValue(exit).finishReason).toBe("length");
  });
});

// Regression suite for the hosted Omega Luna lane. Every one of these
// assertions maps to a body shape VERIFIED against the real upstream API on
// 2026-07-31: the rejected shapes returned HTTP 400 and the accepted shape
// returned HTTP 200. The production defect was that `max_tokens` was always
// sent, so the first upstream call 400-ed and the turn dead-ended.
const lunaBody = (captured: Array<Captured>): Record<string, unknown> =>
  JSON.parse(String(captured[0]?.init.body)) as Record<string, unknown>;

describe("passthrough adapter — OpenAI restricted reasoning models", () => {
  test("sends max_completion_tokens and never max_tokens for gpt-5.6-luna", async () => {
    const captured: Array<Captured> = [];
    const adapter = makePassthroughAdapter(
      openAiConfig(fetchReturning(200, openAiPayload, captured)),
    );

    const exit = await run(
      adapter.complete(
        request({
          model: "gpt-5.6-luna",
          passthroughParams: { max_tokens: 512 },
        }),
      ),
    );

    expect(Exit.isSuccess(exit)).toBe(true);
    const body = lunaBody(captured);
    // Upstream 400s on `max_tokens` for this family — it must be absent.
    expect(body["max_tokens"]).toBeUndefined();
    // The caller's output budget is carried over, not dropped.
    expect(body["max_completion_tokens"]).toBe(512);
  });

  test("falls back to the configured default output budget", async () => {
    const captured: Array<Captured> = [];
    const adapter = makePassthroughAdapter(
      openAiConfig(fetchReturning(200, openAiPayload, captured), {
        defaultMaxTokens: 777,
      }),
    );

    await run(adapter.complete(request({ model: "gpt-5.6-luna" })));

    const body = lunaBody(captured);
    expect(body["max_tokens"]).toBeUndefined();
    expect(body["max_completion_tokens"]).toBe(777);
  });

  test("an explicit max_completion_tokens wins over max_tokens", async () => {
    const captured: Array<Captured> = [];
    const adapter = makePassthroughAdapter(
      openAiConfig(fetchReturning(200, openAiPayload, captured)),
    );

    await run(
      adapter.complete(
        request({
          model: "gpt-5.6-luna",
          passthroughParams: { max_completion_tokens: 64, max_tokens: 512 },
        }),
      ),
    );

    expect(lunaBody(captured)["max_completion_tokens"]).toBe(64);
  });

  test("drops sampling params this family rejects with a hard 400", async () => {
    const captured: Array<Captured> = [];
    const adapter = makePassthroughAdapter(
      openAiConfig(fetchReturning(200, openAiPayload, captured)),
    );

    await run(
      adapter.complete(
        request({
          model: "gpt-5.6-luna",
          passthroughParams: {
            frequency_penalty: 0.5,
            presence_penalty: 0.5,
            stop: ["XYZ"],
            temperature: 0.7,
            top_p: 0.9,
          },
        }),
      ),
    );

    const body = lunaBody(captured);
    for (const rejected of [
      "frequency_penalty",
      "presence_penalty",
      "stop",
      "temperature",
      "top_p",
    ]) {
      expect(body[rejected]).toBeUndefined();
    }
  });

  test("still forwards the params this family accepts", async () => {
    const captured: Array<Captured> = [];
    const adapter = makePassthroughAdapter(
      openAiConfig(fetchReturning(200, openAiPayload, captured)),
    );

    await run(
      adapter.complete(
        request({
          model: "gpt-5.6-luna",
          passthroughParams: { seed: 42, tool_choice: "auto" },
        }),
      ),
    );

    const body = lunaBody(captured);
    expect(body["seed"]).toBe(42);
    expect(body["tool_choice"]).toBe("auto");
  });

  test("defaults reasoning_effort to 'none' when function tools are present", async () => {
    const captured: Array<Captured> = [];
    const adapter = makePassthroughAdapter(
      openAiConfig(fetchReturning(200, openAiPayload, captured)),
    );

    await run(
      adapter.complete(
        request({
          model: "gpt-5.6-luna",
          passthroughParams: {
            tools: [{ function: { name: "f" }, type: "function" }],
          },
        }),
      ),
    );

    // Upstream: "Function tools with reasoning_effort are not supported ...
    // set reasoning_effort to 'none'."
    expect(lunaBody(captured)["reasoning_effort"]).toBe("none");
  });

  test("a caller-supplied reasoning_effort wins over the tool default", async () => {
    const captured: Array<Captured> = [];
    const adapter = makePassthroughAdapter(
      openAiConfig(fetchReturning(200, openAiPayload, captured)),
    );

    await run(
      adapter.complete(
        request({
          model: "gpt-5.6-luna",
          passthroughParams: {
            reasoning_effort: "low",
            tools: [{ function: { name: "f" }, type: "function" }],
          },
        }),
      ),
    );

    expect(lunaBody(captured)["reasoning_effort"]).toBe("low");
  });

  test("omits reasoning_effort when no tools are sent", async () => {
    const captured: Array<Captured> = [];
    const adapter = makePassthroughAdapter(
      openAiConfig(fetchReturning(200, openAiPayload, captured)),
    );

    await run(adapter.complete(request({ model: "gpt-5.6-luna" })));

    expect(lunaBody(captured)["reasoning_effort"]).toBeUndefined();
  });

  test("the restricted profile is an exact-id match, not a prefix rule", async () => {
    const captured: Array<Captured> = [];
    const adapter = makePassthroughAdapter(
      openAiConfig(fetchReturning(200, openAiPayload, captured)),
    );

    await run(
      adapter.complete(
        request({
          model: "gpt-4o-mini",
          passthroughParams: { max_tokens: 256, temperature: 0.7 },
        }),
      ),
    );

    // An unrelated OpenAI model keeps the legacy shape untouched.
    const body = lunaBody(captured);
    expect(body["max_tokens"]).toBe(256);
    expect(body["temperature"]).toBe(0.7);
    expect(body["max_completion_tokens"]).toBeUndefined();
  });

  test("matches the restricted id case- and whitespace-insensitively", async () => {
    const captured: Array<Captured> = [];
    const adapter = makePassthroughAdapter(
      openAiConfig(fetchReturning(200, openAiPayload, captured)),
    );

    await run(
      adapter.complete(
        request({
          model: "  GPT-5.6-Luna ",
          passthroughParams: { max_tokens: 32 },
        }),
      ),
    );

    const body = lunaBody(captured);
    expect(body["max_tokens"]).toBeUndefined();
    expect(body["max_completion_tokens"]).toBe(32);
  });
});

describe("passthrough adapter — error mapping", () => {
  test("maps a 429 to a retryable adapter error", async () => {
    const adapter = makePassthroughAdapter(
      openAiConfig(fetchReturning(429, { error: "slow down" })),
    );
    const exit = await run(adapter.complete(request()));
    const error = failureError(exit);
    expect(error.adapterId).toBe("passthrough-openai");
    expect(error.reason).toContain("retryable");
    expect(error.reason).toContain("429");
  });

  test("maps a 503 to a retryable adapter error", async () => {
    const adapter = makePassthroughAdapter(
      anthropicConfig(fetchReturning(503, { error: "overloaded" })),
    );
    const exit = await run(adapter.complete(request({ model: "claude-test" })));
    const error = failureError(exit);
    expect(error.reason).toContain("retryable");
    expect(error.reason).toContain("503");
  });

  test("maps a 400 to a non-retryable adapter error", async () => {
    const adapter = makePassthroughAdapter(
      openAiConfig(fetchReturning(400, { error: "bad model" })),
    );
    const exit = await run(adapter.complete(request()));
    const error = failureError(exit);
    expect(error.reason).toContain("400");
    expect(error.reason).not.toContain("retryable");
  });

  // Diagnosability regression: an opaque "partner rejected request (400)" hid
  // an unsupported-parameter rejection for three debugging sessions.
  test("surfaces the partner error message on a non-retryable rejection", async () => {
    const adapter = makePassthroughAdapter(
      openAiConfig(
        fetchReturning(400, {
          error: {
            code: "unsupported_parameter",
            message:
              "Unsupported parameter: 'max_tokens' is not supported with this model. Use 'max_completion_tokens' instead.",
            param: "max_tokens",
            type: "invalid_request_error",
          },
        }),
      ),
    );
    const exit = await run(adapter.complete(request()));
    const error = failureError(exit);
    expect(error.reason).toContain("400");
    expect(error.reason).toContain("unsupported_parameter");
    expect(error.reason).toContain("max_completion_tokens");
  });

  test("bounds an overlong partner error message", async () => {
    const adapter = makePassthroughAdapter(
      openAiConfig(fetchReturning(400, { error: { message: "x".repeat(5_000) } })),
    );
    const exit = await run(adapter.complete(request()));
    const error = failureError(exit);
    expect(error.reason.length).toBeLessThan(300);
  });

  test("keeps the plain rejection reason when the partner sends no error object", async () => {
    const adapter = makePassthroughAdapter(
      openAiConfig(fetchReturning(400, { error: "bad model" })),
    );
    const exit = await run(adapter.complete(request()));
    expect(failureError(exit).reason).toBe("partner rejected request (400)");
  });

  test("maps a transport throw to a retryable adapter error", async () => {
    const throwingFetch: PassthroughFetch = async () => {
      throw new TypeError("network down");
    };
    const adapter = makePassthroughAdapter(openAiConfig(throwingFetch));
    const exit = await run(adapter.complete(request()));
    const error = failureError(exit);
    expect(error.reason).toContain("retryable");
    expect(error.reason).toContain("transport");
  });

  test("maps a non-JSON body to an adapter error", async () => {
    const adapter = makePassthroughAdapter(openAiConfig(fetchRawBody(200, "<html>nope</html>")));
    const exit = await run(adapter.complete(request()));
    const error = failureError(exit);
    expect(error.reason).toContain("non-JSON");
  });
});

describe("passthrough adapter — streaming", () => {
  test("forces a non-streamed partner call and emits content + terminal usage frames", async () => {
    const captured: Array<Captured> = [];
    const adapter = makePassthroughAdapter(
      openAiConfig(fetchReturning(200, openAiPayload, captured)),
    );

    const exit = await run(adapter.stream(request({ stream: true })));
    const chunks = successValue(exit);

    // The partner is always asked for a non-streamed response so we settle
    // metering from real usage rather than reconstructing it from deltas.
    const body = JSON.parse(String(captured[0]?.init.body));
    expect(body.stream).toBe(false);

    expect(chunks).toHaveLength(2);
    expect(chunks[0]?.contentDelta).toBe("hi there");
    expect(chunks[0]?.usage).toBeUndefined();
    expect(chunks[1]?.contentDelta).toBe("");
    expect(chunks[1]?.finishReason).toBe("stop");
    expect(chunks[1]?.usage?.totalTokens).toBe(19);
    expect(chunks[1]?.usage?.completionTokens).toBe(7);
  });

  test("propagates a partner failure through stream as a retryable error", async () => {
    const adapter = makePassthroughAdapter(openAiConfig(fetchReturning(500, { error: "boom" })));
    const exit = await run(adapter.stream(request({ stream: true })));
    expect(Exit.isFailure(exit)).toBe(true);
  });

  // REGRESSION — the hosted GPT-5.6 Luna lane rendered NOTHING (omega#160).
  //
  // A coding client always sends tools. `gpt-5.6-luna` answers such a turn with
  // `content: null` and the whole answer in `tool_calls` (verified upstream,
  // 2026-07-31). The streamed projection emitted content only, so the entire
  // turn became two empty deltas: HTTP 200, real usage, no error anywhere, and
  // a client that showed a spinner and then nothing. A stream that carries a
  // tool call must say so.
  test("carries the assistant's tool calls through the streamed frames", async () => {
    const adapter = makePassthroughAdapter(
      openAiConfig(fetchReturning(200, openAiToolCallPayload)),
    );

    const chunks = successValue(
      await run(
        adapter.stream(
          request({
            model: "gpt-5.6-luna",
            passthroughParams: { tools: [readFileTool] },
            stream: true,
          }),
        ),
      ),
    );

    // The whole answer IS the tool call, so the turn must not be silent.
    expect(chunks[0]?.toolCallDeltas).toEqual([
      {
        function: { arguments: '{"path":"src/main.rs"}', name: "read_file" },
        id: "call_luna_1",
        index: 0,
        type: "function",
      },
    ]);
    expect(chunks[0]?.contentDelta).toBe("");
    expect(chunks[1]?.finishReason).toBe("tool_calls");
    // The streamed frames and the non-streamed result agree about what the
    // assistant asked for; they are two projections of one partner response.
    const completed = successValue(
      await run(
        adapter.complete(
          request({ model: "gpt-5.6-luna", passthroughParams: { tools: [readFileTool] } }),
        ),
      ),
    );
    expect(completed.toolCalls?.[0]?.id).toBe("call_luna_1");
  });

  test("indexes several tool calls so an accumulating client keeps them apart", async () => {
    const adapter = makePassthroughAdapter(
      openAiConfig(fetchReturning(200, openAiTwoToolCallsPayload)),
    );

    const chunks = successValue(await run(adapter.stream(request({ stream: true }))));

    expect(chunks[0]?.toolCallDeltas?.map(delta => delta.index)).toEqual([0, 1]);
    expect(chunks[0]?.toolCallDeltas?.map(delta => delta.function?.name)).toEqual([
      "read_file",
      "list_directory",
    ]);
  });

  // A reasoning model still answers in `content` over Chat Completions
  // (verified upstream against `gpt-5.6-luna`, 2026-07-31: the visible answer
  // is in `message.content` and `reasoning_tokens` is reported separately in
  // usage). Visible assistant text must survive the streamed projection with
  // the reasoning accounting intact.
  test("a reasoning-model turn yields visible assistant text", async () => {
    const adapter = makePassthroughAdapter(
      openAiConfig(fetchReturning(200, openAiReasoningPayload)),
    );

    const chunks = successValue(
      await run(adapter.stream(request({ model: "gpt-5.6-luna", stream: true }))),
    );

    expect(chunks[0]?.contentDelta).toBe("391");
    expect(chunks[0]?.toolCallDeltas).toBeUndefined();
    expect(chunks[1]?.finishReason).toBe("stop");
    expect(chunks[1]?.usage?.completionTokens).toBe(96);
  });

  test("a text-only turn carries no tool-call deltas at all", async () => {
    const adapter = makePassthroughAdapter(openAiConfig(fetchReturning(200, openAiPayload)));

    const chunks = successValue(await run(adapter.stream(request({ stream: true }))));

    expect(chunks[0]?.contentDelta).toBe("hi there");
    expect(chunks[0]?.toolCallDeltas).toBeUndefined();
    expect(chunks[1]?.toolCallDeltas).toBeUndefined();
  });
});

// --- incremental pass-through stream (streamSse) --------------------------
//
// REGRESSION — the hosted `gpt-5.6-luna` lane did not stream: the whole answer
// appeared at once. The buffered `stream` above is why. It asks the partner for
// a NON-streamed body, waits for the entire completion, and then emits exactly
// two frames — so however fast the partner produced tokens, the caller saw one
// block of text. These tests assert the opposite property directly: MANY frames
// arrive, in order, as separate deltas.

// An SSE Response over a real ReadableStream that splits every line in HALF
// across two enqueues, so a frame is only parseable once both reads land. A
// parser that reads whole lines out of one buffered blob cannot pass this.
const chunkedSseResponse = (frames: ReadonlyArray<unknown>): Response => {
  const encoder = new TextEncoder();
  const lines = [
    ...frames.map(frame => `data: ${JSON.stringify(frame)}\n\n`),
    "data: [DONE]\n\n",
  ];
  let index = 0;
  const stream = new ReadableStream<Uint8Array>({
    pull(controller) {
      if (index >= lines.length) {
        controller.close();
        return;
      }
      const line = lines[index]!;
      const mid = Math.floor(line.length / 2);
      controller.enqueue(encoder.encode(line.slice(0, mid)));
      controller.enqueue(encoder.encode(line.slice(mid)));
      index += 1;
    },
  });
  return new Response(stream, {
    headers: { "content-type": "text/event-stream" },
    status: 200,
  });
};

const fetchStreaming =
  (response: () => Response, captured?: Array<Captured>): PassthroughFetch =>
  async (url, init) => {
    captured?.push({ url, init });
    return response();
  };

const drainSource = async (
  adapter: ReturnType<typeof makePassthroughAdapter>,
  input: InferenceRequest,
) => {
  const source = successValue(await run(adapter.streamSse!(input)));
  const deltas: Array<string> = [];
  const reasoningDeltas: Array<string> = [];
  const toolCallDeltas: Array<InferenceToolCallDelta> = [];
  let frameCount = 0;
  for await (const event of source.frames) {
    frameCount += 1;
    if (event.contentDelta !== "") {
      deltas.push(event.contentDelta);
    }
    if (event.reasoningDelta !== undefined) {
      reasoningDeltas.push(event.reasoningDelta);
    }
    if (event.toolCallDeltas !== undefined) {
      toolCallDeltas.push(...event.toolCallDeltas);
    }
  }
  return { deltas, frameCount, reasoningDeltas, source, toolCallDeltas };
};

describe("passthrough adapter — incremental SSE pass-through", () => {
  test("exposes streamSse for the OpenAI wire format", () => {
    const adapter = makePassthroughAdapter(openAiConfig(fetchReturning(200, openAiPayload)));
    expect(typeof adapter.streamSse).toBe("function");
  });

  // Anthropic Messages streams a different event vocabulary this parser does not
  // speak, so the lane must fall back to the buffered path rather than silently
  // dropping every frame.
  test("omits streamSse for the Anthropic wire format", () => {
    const adapter = makePassthroughAdapter(anthropicConfig(fetchReturning(200, anthropicPayload)));
    expect(adapter.streamSse).toBeUndefined();
  });

  test("yields each content delta as its own frame instead of one block", async () => {
    const captured: Array<Captured> = [];
    const adapter = makePassthroughAdapter(
      openAiConfig(
        fetchStreaming(
          () =>
            chunkedSseResponse([
              { choices: [{ delta: { content: "Hel" }, index: 0 }] },
              { choices: [{ delta: { content: "lo" }, index: 0 }] },
              {
                choices: [{ delta: {}, finish_reason: "stop", index: 0 }],
                model: "gpt-5.6-luna",
                usage: {
                  completion_tokens: 2,
                  prompt_tokens: 7,
                  total_tokens: 9,
                },
              },
            ]),
          captured,
        ),
      ),
    );

    const drained = await drainSource(
      adapter,
      request({ model: "gpt-5.6-luna", stream: true }),
    );

    // THE POINT OF THIS CHANGE: two SEPARATE deltas, not one "Hello".
    expect(drained.deltas).toEqual(["Hel", "lo"]);
    expect(drained.deltas.length).toBeGreaterThan(1);

    // The partner is asked to stream, and to stream over SSE.
    const body = JSON.parse(String(captured[0]?.init.body));
    expect(body.stream).toBe(true);
    const headers = captured[0]?.init.headers as Record<string, string>;
    expect(headers.accept).toBe("text/event-stream");

    // Receipt-first: usage is opted into and surfaced from the terminal frame.
    expect(body.stream_options).toEqual({ include_usage: true });
    const terminal = drained.source.terminal();
    expect(terminal.finishReason).toBe("stop");
    expect(terminal.servedModel).toBe("gpt-5.6-luna");
    expect(terminal.usage).toEqual({
      completionTokens: 2,
      promptTokens: 7,
      totalTokens: 9,
    });
  });

  test("carries the cached-input dimension from a terminal usage frame", async () => {
    const adapter = makePassthroughAdapter(
      openAiConfig(
        fetchStreaming(() =>
          chunkedSseResponse([
            { choices: [{ delta: { content: "ok" }, index: 0 }] },
            {
              choices: [{ delta: {}, finish_reason: "stop", index: 0 }],
              usage: {
                completion_tokens: 3,
                prompt_tokens: 11,
                prompt_tokens_details: { cached_tokens: 4 },
                total_tokens: 14,
              },
            },
          ]),
        ),
      ),
    );

    const drained = await drainSource(adapter, request({ stream: true }));
    expect(drained.source.terminal().usage?.cachedPromptTokens).toBe(4);
  });

  // Receipt-first: a stream without real counts must NOT be settled on an
  // estimate reconstructed from the deltas.
  test("leaves terminal usage undefined when the partner sends none", async () => {
    const adapter = makePassthroughAdapter(
      openAiConfig(
        fetchStreaming(() =>
          chunkedSseResponse([
            { choices: [{ delta: { content: "ok" }, index: 0 }] },
            { choices: [{ delta: {}, finish_reason: "stop", index: 0 }] },
          ]),
        ),
      ),
    );

    const drained = await drainSource(adapter, request({ stream: true }));
    expect(drained.deltas).toEqual(["ok"]);
    expect(drained.source.terminal().usage).toBeUndefined();
    expect(drained.source.terminal().finishReason).toBe("stop");
  });

  // A coding client always sends tools, and `gpt-5.6-luna` streams a tool call's
  // arguments as many partial JSON fragments. They must arrive as SEPARATE
  // frames, verbatim: assembling them here would restore the all-at-once
  // behavior this change exists to remove.
  test("forwards tool-call argument fragments verbatim, one frame at a time", async () => {
    const adapter = makePassthroughAdapter(
      openAiConfig(
        fetchStreaming(() =>
          chunkedSseResponse([
            {
              choices: [
                {
                  delta: {
                    tool_calls: [
                      {
                        function: { arguments: "", name: "read_file" },
                        id: "call_luna_1",
                        index: 0,
                        type: "function",
                      },
                    ],
                  },
                  index: 0,
                },
              ],
            },
            {
              choices: [
                {
                  delta: {
                    tool_calls: [{ function: { arguments: '{"pa' }, index: 0 }],
                  },
                  index: 0,
                },
              ],
            },
            {
              choices: [
                {
                  delta: {
                    tool_calls: [
                      { function: { arguments: 'th":"src/main.rs"}' }, index: 0 },
                    ],
                  },
                  index: 0,
                },
              ],
            },
            {
              choices: [{ delta: {}, finish_reason: "tool_calls", index: 0 }],
              usage: { completion_tokens: 18, prompt_tokens: 141, total_tokens: 159 },
            },
          ]),
        ),
      ),
    );

    const drained = await drainSource(
      adapter,
      request({
        model: "gpt-5.6-luna",
        passthroughParams: { tools: [readFileTool] },
        stream: true,
      }),
    );

    // Three separate tool-call frames, arguments NOT pre-concatenated.
    expect(drained.toolCallDeltas).toHaveLength(3);
    expect(drained.toolCallDeltas.map(delta => delta.function?.arguments)).toEqual([
      "",
      '{"pa',
      'th":"src/main.rs"}',
    ]);
    expect(drained.toolCallDeltas[0]).toEqual({
      function: { arguments: "", name: "read_file" },
      id: "call_luna_1",
      index: 0,
      type: "function",
    });
    // Every fragment stays on index 0 so an accumulating client rebuilds one call.
    expect(drained.toolCallDeltas.every(delta => delta.index === 0)).toBe(true);
    expect(drained.source.terminal().finishReason).toBe("tool_calls");
    expect(drained.source.terminal().usage?.totalTokens).toBe(159);
  });

  test("keeps parallel tool calls apart by index across frames", async () => {
    const adapter = makePassthroughAdapter(
      openAiConfig(
        fetchStreaming(() =>
          chunkedSseResponse([
            {
              choices: [
                {
                  delta: {
                    tool_calls: [
                      {
                        function: { arguments: "", name: "read_file" },
                        id: "call_luna_1",
                        index: 0,
                        type: "function",
                      },
                      {
                        function: { arguments: "", name: "list_directory" },
                        id: "call_luna_2",
                        index: 1,
                        type: "function",
                      },
                    ],
                  },
                  index: 0,
                },
              ],
            },
            {
              choices: [
                {
                  delta: {
                    tool_calls: [{ function: { arguments: '{"path":"a.rs"}' }, index: 0 }],
                  },
                  index: 0,
                },
              ],
            },
          ]),
        ),
      ),
    );

    const drained = await drainSource(adapter, request({ stream: true }));
    expect(drained.toolCallDeltas.map(delta => delta.index)).toEqual([0, 1, 0]);
  });

  test("streams provider-labeled reasoning on its own channel", async () => {
    const adapter = makePassthroughAdapter(
      openAiConfig(
        fetchStreaming(() =>
          chunkedSseResponse([
            { choices: [{ delta: { reasoning_content: "think" }, index: 0 }] },
            { choices: [{ delta: { content: "391" }, index: 0 }] },
          ]),
        ),
      ),
    );

    const drained = await drainSource(adapter, request({ model: "gpt-5.6-luna", stream: true }));
    expect(drained.reasoningDeltas).toEqual(["think"]);
    expect(drained.deltas).toEqual(["391"]);
  });

  test("ignores the [DONE] sentinel and blank lines", async () => {
    const adapter = makePassthroughAdapter(
      openAiConfig(
        fetchStreaming(() =>
          chunkedSseResponse([
            { choices: [{ delta: { content: "a" }, index: 0 }] },
            { choices: [{ delta: { content: "b" }, index: 0 }] },
          ]),
        ),
      ),
    );

    const drained = await drainSource(adapter, request({ stream: true }));
    expect(drained.frameCount).toBe(2);
    expect(drained.deltas).toEqual(["a", "b"]);
  });

  // The last frame of a stream may arrive without a trailing newline.
  test("flushes a trailing frame that has no final newline", async () => {
    const adapter = makePassthroughAdapter(
      openAiConfig(
        fetchStreaming(
          () =>
            new Response(
              new ReadableStream<Uint8Array>({
                start(controller) {
                  const encoder = new TextEncoder();
                  controller.enqueue(
                    encoder.encode(
                      'data: {"choices":[{"delta":{"content":"first"},"index":0}]}\n',
                    ),
                  );
                  controller.enqueue(
                    encoder.encode(
                      'data: {"choices":[{"delta":{"content":"last"},"index":0}]}',
                    ),
                  );
                  controller.close();
                },
              }),
              { headers: { "content-type": "text/event-stream" }, status: 200 },
            ),
        ),
      ),
    );

    const drained = await drainSource(adapter, request({ stream: true }));
    expect(drained.deltas).toEqual(["first", "last"]);
  });

  test("surfaces a partner 429 as a retryable connect-time failure", async () => {
    const adapter = makePassthroughAdapter(openAiConfig(fetchReturning(429, { error: "slow" })));
    const exit = await run(adapter.streamSse!(request({ stream: true })));
    const error = failureError(exit);
    expect(error.reason).toContain("retryable");
    expect(error.reason).toContain("429");
  });

  test("surfaces a partner rejection detail before any byte is emitted", async () => {
    const adapter = makePassthroughAdapter(
      openAiConfig(
        fetchReturning(400, {
          error: { code: "unsupported_parameter", message: "no streaming here" },
        }),
      ),
    );
    const exit = await run(adapter.streamSse!(request({ stream: true })));
    const error = failureError(exit);
    expect(error.reason).toContain("400");
    expect(error.reason).toContain("unsupported_parameter");
    expect(error.reason).not.toContain("retryable");
  });

  test("fails when the partner returns no response body", async () => {
    const adapter = makePassthroughAdapter(
      openAiConfig(fetchStreaming(() => new Response(null, { status: 200 }))),
    );
    const exit = await run(adapter.streamSse!(request({ stream: true })));
    expect(failureError(exit).reason).toContain("no response body");
  });

  // The restricted reasoning profile still applies on the streamed path: the
  // body that 400-ed the whole lane must not come back through this door.
  test("keeps the restricted reasoning body shape on the streamed path", async () => {
    const captured: Array<Captured> = [];
    const adapter = makePassthroughAdapter(
      openAiConfig(
        fetchStreaming(
          () => chunkedSseResponse([{ choices: [{ delta: { content: "ok" }, index: 0 }] }]),
          captured,
        ),
      ),
    );

    await drainSource(
      adapter,
      request({
        model: "gpt-5.6-luna",
        passthroughParams: { max_tokens: 512, temperature: 0.7 },
        stream: true,
      }),
    );

    const body = lunaBody(captured);
    expect(body["max_tokens"]).toBeUndefined();
    expect(body["max_completion_tokens"]).toBe(512);
    expect(body["temperature"]).toBeUndefined();
  });
});

// --- reasoning-token attribution ------------------------------------------
//
// `openAiReasoningPayload` (captured upstream from `gpt-5.6-luna`, 2026-07-31)
// has always carried `completion_tokens_details: { reasoning_tokens: 92 }`, and
// the adapter has always thrown it away: `OpenAiUsage` did not even declare the
// field. The existing streamed test asserted only `completionTokens === 96`,
// so a captured wire value sat in the repo documenting a drop that nothing
// caught. Under the OpenAI convention `reasoning_tokens` is a SUBSET of
// `completion_tokens`, so recording it moves no total and no public counter —
// it only makes the thinking share attributable in
// `token_usage_events.reasoning_tokens`.
describe("passthrough adapter reasoning-token attribution", () => {
  test("complete() maps completion_tokens_details.reasoning_tokens", async () => {
    const adapter = makePassthroughAdapter(
      openAiConfig(fetchReturning(200, openAiReasoningPayload)),
    );

    const result = successValue(await run(adapter.complete(request({ model: "gpt-5.6-luna" }))));

    expect(result.usage.reasoningTokens).toBe(92);
    expect(result.usage.completionTokens).toBe(96);
    expect(result.usage.totalTokens).toBe(116);
  });

  test("buffered stream() carries reasoning tokens on the terminal chunk", async () => {
    const adapter = makePassthroughAdapter(
      openAiConfig(fetchReturning(200, openAiReasoningPayload)),
    );

    const chunks = successValue(
      await run(adapter.stream(request({ model: "gpt-5.6-luna", stream: true }))),
    );

    expect(chunks[1]?.usage?.reasoningTokens).toBe(92);
  });

  test("streamSse() maps reasoning tokens off the terminal usage frame", async () => {
    // The streamed parser is a DIFFERENT function from the non-streaming one,
    // reading the same wire field; it is asserted separately rather than
    // assumed to follow.
    const adapter = makePassthroughAdapter(
      openAiConfig(
        fetchStreaming(() =>
          chunkedSseResponse([
            { choices: [{ delta: { content: "391" }, index: 0 }] },
            {
              choices: [{ delta: {}, finish_reason: "stop", index: 0 }],
              model: "gpt-5.6-luna",
              usage: {
                completion_tokens: 96,
                completion_tokens_details: { reasoning_tokens: 92 },
                prompt_tokens: 20,
                total_tokens: 116,
              },
            },
          ]),
        ),
      ),
    );

    const drained = await drainSource(
      adapter,
      request({ model: "gpt-5.6-luna", stream: true }),
    );

    expect(drained.source.terminal().usage?.reasoningTokens).toBe(92);
  });

  test("a partner that reports no reasoning leaves the field undefined", async () => {
    const adapter = makePassthroughAdapter(openAiConfig(fetchReturning(200, openAiPayload)));

    const result = successValue(await run(adapter.complete(request())));

    expect(result.usage.reasoningTokens).toBeUndefined();
  });
});
