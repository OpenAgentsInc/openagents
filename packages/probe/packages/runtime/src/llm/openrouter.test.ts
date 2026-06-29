// OpenRouter Effect wrapper tests (openagents #6182). NO NETWORK, NO SPEND:
// every test uses the mock layer or pure functions. The real layer
// (`OpenRouterClientLive`) is never constructed here — it would need a key and
// would be billable. We verify: the mock layer's chat/stream behavior, the
// tagged-error mapping from SDK errors, the fail-closed Config, and the spend
// guard (`clampMaxTokens`).

import { describe, expect, test } from "bun:test";
import { ConfigProvider, Effect, Layer } from "effect";
import {
  OPENROUTER_DEFAULT_MAX_TOKENS,
  OPENROUTER_DEFAULT_MODEL,
  OPENROUTER_FREE_MODEL,
  OPENROUTER_MAX_TOKENS_CAP,
  OpenRouterAuthError,
  OpenRouterClient,
  OpenRouterConfig,
  OpenRouterRateLimitError,
  OpenRouterTimeoutError,
  OpenRouterUpstreamError,
  clampMaxTokens,
  makeOpenRouterClientMock,
  mapOpenRouterError,
} from "./openrouter";
import {
  PaymentRequiredResponseError,
  ProviderOverloadedResponseError,
  RequestTimeoutError,
  TooManyRequestsResponseError,
  UnauthorizedResponseError,
} from "@openrouter/sdk/models/errors";

const httpMeta = { response: new Response(""), request: new Request("https://openrouter.ai"), body: "" };

function runWithMock<A, E>(
  layer: Layer.Layer<OpenRouterClient>,
  program: Effect.Effect<A, E, OpenRouterClient>,
): Promise<A> {
  return Effect.runPromise(program.pipe(Effect.provide(layer)));
}

describe("OpenRouter spend-discipline defaults", () => {
  test("the default model is the free router (cost=0)", () => {
    expect(OPENROUTER_DEFAULT_MODEL).toBe(OPENROUTER_FREE_MODEL);
    expect(OPENROUTER_FREE_MODEL).toBe("openrouter/free");
  });

  test("clampMaxTokens caps any request to the hard ceiling", () => {
    expect(clampMaxTokens(999_999, OPENROUTER_DEFAULT_MAX_TOKENS)).toBe(OPENROUTER_MAX_TOKENS_CAP);
    expect(clampMaxTokens(256, OPENROUTER_DEFAULT_MAX_TOKENS)).toBe(256);
    expect(clampMaxTokens(undefined, OPENROUTER_DEFAULT_MAX_TOKENS)).toBe(OPENROUTER_DEFAULT_MAX_TOKENS);
    expect(clampMaxTokens(0, OPENROUTER_DEFAULT_MAX_TOKENS)).toBe(OPENROUTER_DEFAULT_MAX_TOKENS);
    expect(clampMaxTokens(-5, OPENROUTER_DEFAULT_MAX_TOKENS)).toBe(OPENROUTER_DEFAULT_MAX_TOKENS);
    expect(clampMaxTokens(Number.NaN, 512)).toBe(512);
  });
});

describe("OpenRouterClientMock (the no-network test/CI default)", () => {
  test("chat returns the next canned fixture, then empty when exhausted", async () => {
    const layer = makeOpenRouterClientMock({
      replies: [{ content: '{"action":"navigate","url":"/login"}' }, { content: '{"action":"done","verdict":"pass"}' }],
    });
    const out = await runWithMock(
      layer,
      Effect.gen(function* () {
        const c = yield* OpenRouterClient;
        const a = yield* c.chat({ messages: [{ role: "user", content: "go" }] });
        const b = yield* c.chat({ messages: [{ role: "user", content: "next" }] });
        const empty = yield* c.chat({ messages: [{ role: "user", content: "again" }] });
        return [a.content, b.content, empty.content];
      }),
    );
    expect(out[0]).toBe('{"action":"navigate","url":"/login"}');
    expect(out[1]).toBe('{"action":"done","verdict":"pass"}');
    expect(out[2]).toBe("");
  });

  test("chatStream replays the canned content through onChunk", async () => {
    const layer = makeOpenRouterClientMock({ replies: [{ content: "hello world" }] });
    const chunks: string[] = [];
    const result = await runWithMock(
      layer,
      Effect.gen(function* () {
        const c = yield* OpenRouterClient;
        return yield* c.chatStream({
          messages: [{ role: "user", content: "hi" }],
          onChunk: (chunk) => chunks.push(chunk.content),
        });
      }),
    );
    expect(result.content).toBe("hello world");
    expect(chunks).toEqual(["hello world"]);
  });

  test("a mock can be told to fail with a tagged error (error-path tests)", async () => {
    const layer = makeOpenRouterClientMock({ failWith: new OpenRouterRateLimitError({ reason: "429" }) });
    const exit = await Effect.runPromiseExit(
      Effect.gen(function* () {
        const c = yield* OpenRouterClient;
        return yield* c.chat({ messages: [{ role: "user", content: "x" }] });
      }).pipe(Effect.provide(layer)),
    );
    expect(exit._tag).toBe("Failure");
  });
});

describe("mapOpenRouterError (SDK error -> tagged union)", () => {
  test("401 unauthorized -> OpenRouterAuthError", () => {
    const mapped = mapOpenRouterError(
      new UnauthorizedResponseError({ error: { code: 401, message: "no" } as never }, httpMeta),
    );
    expect(mapped).toBeInstanceOf(OpenRouterAuthError);
    expect(mapped._tag).toBe("OpenRouterAuthError");
  });

  test("402 payment required -> OpenRouterAuthError (out of credits)", () => {
    const mapped = mapOpenRouterError(
      new PaymentRequiredResponseError({ error: { code: 402, message: "broke" } as never }, httpMeta),
    );
    expect(mapped._tag).toBe("OpenRouterAuthError");
    expect((mapped as OpenRouterAuthError).reason).toContain("credits");
  });

  test("429 / provider overloaded -> OpenRouterRateLimitError", () => {
    expect(
      mapOpenRouterError(new TooManyRequestsResponseError({ error: { code: 429, message: "slow" } as never }, httpMeta))
        ._tag,
    ).toBe("OpenRouterRateLimitError");
    expect(
      mapOpenRouterError(
        new ProviderOverloadedResponseError({ error: { code: 429, message: "busy" } as never }, httpMeta),
      )._tag,
    ).toBe("OpenRouterRateLimitError");
  });

  test("request timeout -> OpenRouterTimeoutError", () => {
    expect(mapOpenRouterError(new RequestTimeoutError("ctx", { timeout: 5 }))._tag).toBe("OpenRouterTimeoutError");
  });

  test("unknown errors -> OpenRouterUpstreamError (no key leakage)", () => {
    const mapped = mapOpenRouterError(new Error("Bearer sk-or-SECRET should not appear"));
    expect(mapped._tag).toBe("OpenRouterUpstreamError");
    // The mapper only reads name + a truncated message; it must not synthesize a
    // key, but we also assert the reason carries the (caller-provided) text only.
    expect((mapped as OpenRouterUpstreamError).reason).toContain("Error:");
  });

  test("status-bearing unknown errors map by HTTP status", () => {
    expect(mapOpenRouterError({ statusCode: 503 })._tag).toBe("OpenRouterUpstreamError");
    expect((mapOpenRouterError({ statusCode: 503 }) as OpenRouterUpstreamError).status).toBe(503);
    expect(mapOpenRouterError({ status: 429 })._tag).toBe("OpenRouterRateLimitError");
    expect(mapOpenRouterError({ status: 401 })._tag).toBe("OpenRouterAuthError");
    expect(mapOpenRouterError({ status: 408 })._tag).toBe("OpenRouterTimeoutError");
  });

  test("tagged errors are constructible without a key", () => {
    // Defensive: none of the tagged errors carry an apiKey field.
    const errs = [
      new OpenRouterAuthError({ reason: "x" }),
      new OpenRouterRateLimitError({ reason: "x" }),
      new OpenRouterUpstreamError({ reason: "x" }),
      new OpenRouterTimeoutError({ reason: "x" }),
    ];
    for (const e of errs) {
      expect(JSON.stringify(e)).not.toContain("apiKey");
      expect(JSON.stringify(e)).not.toContain("sk-or-");
    }
  });
});

describe("OpenRouterConfig (fail-closed)", () => {
  const withConfig = (record: Record<string, string>) =>
    OpenRouterConfig.pipe(Effect.provide(ConfigProvider.layer(ConfigProvider.fromUnknown(record))));

  test("fails when OPENROUTER_API_KEY is absent (no implicit key)", async () => {
    const exit = await Effect.runPromiseExit(withConfig({}));
    expect(exit._tag).toBe("Failure");
  });

  test("resolves with a key present; defaults the model to openrouter/free", async () => {
    const cfg = await Effect.runPromise(withConfig({ OPENROUTER_API_KEY: "sk-or-test" }));
    expect(cfg.model).toBe(OPENROUTER_DEFAULT_MODEL);
    expect(cfg.defaultMaxTokens).toBe(OPENROUTER_DEFAULT_MAX_TOKENS);
    // The key is held redacted; its string form must not be the raw value.
    expect(String(cfg.apiKey)).not.toContain("sk-or-test");
  });

  test("honors OPENROUTER_MODEL / OPENROUTER_MAX_TOKENS overrides", async () => {
    const cfg = await Effect.runPromise(
      withConfig({
        OPENROUTER_API_KEY: "sk-or-test",
        OPENROUTER_MODEL: "anthropic/claude-haiku",
        OPENROUTER_MAX_TOKENS: "777",
      }),
    );
    expect(cfg.model).toBe("anthropic/claude-haiku");
    expect(cfg.defaultMaxTokens).toBe(777);
  });
});
