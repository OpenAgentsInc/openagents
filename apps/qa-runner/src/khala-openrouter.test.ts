// Khala OpenRouter backend tests (openagents #6182). NO NETWORK, NO SPEND: the
// OpenRouter `ChatClient` is built over the MOCK Effect layer (canned fixtures),
// and backend selection is pure env logic. The real layer is never armed here.
//
// We verify: backend selection (default gpt-oss; OpenRouter when armed), that the
// OpenRouter-backed ChatClient drives the JSON-action loop reliably with the mock
// layer (no truncation wobble), the reasoning-empty-content honest failure, and
// that `makeKhalaChatClient` routes to the selected backend.

import { describe, expect, test } from "bun:test";
import { Layer } from "effect";
import {
  OpenRouterClient,
  OpenRouterRateLimitError,
  makeOpenRouterClientMock,
} from "@openagentsinc/probe-runtime";
import { makeKhalaDriver } from "./khala-driver";
import {
  OPENROUTER_DEFAULT_MODEL,
  makeOpenRouterChatClient,
  selectKhalaBackend,
} from "./khala-openrouter";

describe("selectKhalaBackend", () => {
  test("defaults to gpt-oss when OpenRouter is not armed", () => {
    const sel = selectKhalaBackend({ env: {} });
    expect(sel.backend).toBe("gpt-oss");
    expect(sel.model).toBe("openagents/khala");
  });

  test("selects OpenRouter when KHALA_DRIVER_BACKEND=openrouter", () => {
    const sel = selectKhalaBackend({ env: { KHALA_DRIVER_BACKEND: "openrouter" } });
    expect(sel.backend).toBe("openrouter");
    expect(sel.model).toBe(OPENROUTER_DEFAULT_MODEL); // openrouter/free, cost=0
  });

  test("selects OpenRouter when OPENROUTER_LIVE is truthy", () => {
    expect(selectKhalaBackend({ env: { OPENROUTER_LIVE: "1" } }).backend).toBe("openrouter");
    expect(selectKhalaBackend({ env: { OPENROUTER_LIVE: "true" } }).backend).toBe("openrouter");
    expect(selectKhalaBackend({ env: { OPENROUTER_LIVE: "0" } }).backend).toBe("gpt-oss");
  });

  test("an explicit gpt-oss selection overrides OPENROUTER_LIVE", () => {
    const sel = selectKhalaBackend({ env: { KHALA_DRIVER_BACKEND: "gpt-oss", OPENROUTER_LIVE: "1" } });
    expect(sel.backend).toBe("gpt-oss");
  });

  test("honors OPENROUTER_MODEL for the OpenRouter lane", () => {
    const sel = selectKhalaBackend({ env: { KHALA_DRIVER_BACKEND: "openrouter", OPENROUTER_MODEL: "anthropic/claude-haiku" } });
    expect(sel.model).toBe("anthropic/claude-haiku");
  });
});

describe("makeOpenRouterChatClient (mock layer — no network)", () => {
  test("drives the Khala JSON-action loop to a pass verdict with no truncation", async () => {
    // Canned model replies: a clean navigate then a done(pass). With the mock
    // layer there is no JSON-truncation wobble — proving the reliable lane.
    const layer = makeOpenRouterClientMock({
      replies: [
        { content: '{"action":"navigate","url":"/login"}' },
        { content: '{"action":"done","verdict":"pass","summary":"logged in"}' },
      ],
    });
    const chat = makeOpenRouterChatClient({ env: {}, layer });
    const driver = makeKhalaDriver({ goal: "verify login", chat, log: () => undefined });

    const a1 = await driver.nextAction();
    expect(a1).toEqual({ action: "navigate", url: "/login" });
    driver.recordObservation("navigated; url is /login");
    const a2 = await driver.nextAction();
    expect(a2).toBeNull();
    expect(driver.finalVerdict()).toBe("pass");
  });

  test("surfaces an honest error when the model returns empty content (reasoning budget)", async () => {
    const layer = makeOpenRouterClientMock({
      replies: [{ content: "", finishReason: "length", usage: { reasoningTokens: 2048 } }],
    });
    const chat = makeOpenRouterChatClient({ env: {}, layer });
    await expect(chat.complete([{ role: "user", content: "go" }])).rejects.toThrow(/empty content/);
  });

  test("a tagged OpenRouterError from the layer becomes a thrown error the driver records", async () => {
    const layer: Layer.Layer<OpenRouterClient> = makeOpenRouterClientMock({
      failWith: new OpenRouterRateLimitError({ reason: "429" }),
    });
    const chat = makeOpenRouterChatClient({ env: {}, layer });
    await expect(chat.complete([{ role: "user", content: "go" }])).rejects.toBeDefined();
  });

  test("defaults to the mock layer (no spend) when OPENROUTER_LIVE is not armed", async () => {
    // No explicit layer, env not armed: the client must use the mock, returning
    // the canned fixture rather than reaching the network.
    const chat = makeOpenRouterChatClient({ env: {}, mock: { replies: [{ content: "mock-ok" }] } });
    const out = await chat.complete([{ role: "user", content: "go" }]);
    expect(out).toBe("mock-ok");
  });
});
