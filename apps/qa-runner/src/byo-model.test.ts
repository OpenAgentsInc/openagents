// BYO-model config tests (pure, no network, no OpenAgents login).
//
// Proves the OSS credential path: model + base URL + key come from flags/env
// ONLY, with no OpenAgents secrets-dir discovery and no agent-token lookup.

import { describe, expect, test } from "bun:test";
import {
  ByoModelConfigError,
  DEFAULT_QA_BASE_URL,
  DEFAULT_QA_MODEL,
  FREE_KHALA_KEY_URL,
  QA_CLIENT_HEADER,
  QA_DEMAND_KIND_HEADER,
  QA_DEMAND_SOURCE_HEADER,
  makeByoChatClient,
  resolveByoModelConfig,
} from "./byo-model";

describe("resolveByoModelConfig (BYO, no OpenAgents login)", () => {
  test("flags resolve a full OpenAI-compatible endpoint", () => {
    const config = resolveByoModelConfig({
      flags: { model: "gpt-4o-mini", baseUrl: "https://api.openai.com/v1/", apiKey: "sk-test" },
      env: {},
    });
    expect(config.model).toBe("gpt-4o-mini");
    expect(config.baseUrl).toBe("https://api.openai.com/v1"); // trailing slash stripped
    expect(config.apiKey).toBe("sk-test");
    expect(config.keySource).toBe("--api-key flag");
  });

  test("flags win over env; env QA_* and OPENAI_* both accepted", () => {
    const config = resolveByoModelConfig({
      flags: { model: "from-flag" },
      env: { QA_MODEL: "ignored", OPENAI_BASE_URL: "http://localhost:8080/v1", OPENAI_API_KEY: "k" },
    });
    expect(config.model).toBe("from-flag");
    expect(config.baseUrl).toBe("http://localhost:8080/v1");
    expect(config.apiKey).toBe("k");
    expect(config.keySource).toBe("OPENAI_API_KEY env");
  });

  test("QA_* env names take precedence over OPENAI_* env names", () => {
    const config = resolveByoModelConfig({
      env: {
        QA_MODEL: "qa-model",
        OPENAI_MODEL: "openai-model",
        QA_BASE_URL: "https://qa/v1",
        OPENAI_BASE_URL: "https://openai/v1",
        QA_API_KEY: "qa-key",
        OPENAI_API_KEY: "openai-key",
      },
    });
    expect(config.model).toBe("qa-model");
    expect(config.baseUrl).toBe("https://qa/v1");
    expect(config.keySource).toBe("QA_API_KEY env");
  });

  test("defaults to Khala model/base while still requiring a key", () => {
    const config = resolveByoModelConfig({ env: { QA_API_KEY: "oa_agent_test" } });
    expect(config.model).toBe(DEFAULT_QA_MODEL);
    expect(config.baseUrl).toBe(DEFAULT_QA_BASE_URL);
    expect(config.keySource).toBe("QA_API_KEY env");
    expect(config.demandKind).toBe("internal");
    expect(config.demandSource).toBe("qa-runner");
  });

  test("missing key is an honest error that points to the free Khala key endpoint", () => {
    expect(() => resolveByoModelConfig({ env: {} })).toThrow(ByoModelConfigError);
    expect(() => resolveByoModelConfig({ env: {} })).toThrow(FREE_KHALA_KEY_URL);
  });

  test("--allow-keyless permits a keyless local server", () => {
    const config = resolveByoModelConfig({
      flags: { model: "local-model", baseUrl: "http://localhost:8080/v1" },
      env: {},
      allowKeyless: true,
    });
    expect(config.apiKey).toBe("");
    expect(config.keySource).toBe("none (keyless)");
  });

  test("does NOT read any OpenAgents-specific env var or secrets dir", () => {
    // Even with an OpenAgents agent token present in env, BYO ignores it: the
    // ONLY accepted key vars are QA_API_KEY / OPENAI_API_KEY / --api-key.
    expect(() =>
      resolveByoModelConfig({
        flags: { model: "m", baseUrl: "https://x/v1" },
        env: { OPENAGENTS_API_KEY: "oa-secret", OPENAGENTS_AGENT_TOKEN: "tok" },
      }),
    ).toThrow(/API key/);
  });
});

describe("makeByoChatClient (fetch-based OpenAI-compatible client)", () => {
  test("posts to <base>/chat/completions with the model and returns content", async () => {
    let seenUrl = "";
    let seenAuth: string | null = "MISSING";
    const fakeFetch = (async (url: string | URL | Request, init?: RequestInit) => {
      seenUrl = String(url);
      seenAuth = new Headers(init?.headers).get("authorization");
      return new Response(JSON.stringify({ choices: [{ message: { content: "hello" } }] }), {
        status: 200,
        headers: { "content-type": "application/json" },
      });
    }) as unknown as typeof fetch;

    const client = makeByoChatClient(
      {
        apiKey: "sk-1",
        baseUrl: "https://api.example/v1",
        demandKind: "external",
        demandSource: "third-party",
        keySource: "x",
        model: "m",
      },
      { fetchImpl: fakeFetch },
    );
    const out = await client.complete([{ role: "user", content: "hi" }]);
    expect(out).toBe("hello");
    expect(seenUrl).toBe("https://api.example/v1/chat/completions");
    expect(seenAuth).toBe("Bearer sk-1");
  });

  test("omits the Authorization header entirely for a keyless endpoint", async () => {
    let seenAuth: string | null = "MISSING";
    const fakeFetch = (async (_url: string | URL | Request, init?: RequestInit) => {
      seenAuth = new Headers(init?.headers).get("authorization");
      return new Response(JSON.stringify({ choices: [{ message: { content: "" } }] }), { status: 200 });
    }) as unknown as typeof fetch;
    const client = makeByoChatClient(
      {
        apiKey: "",
        baseUrl: "http://localhost:8080/v1",
        demandKind: "unlabeled",
        demandSource: "local",
        keySource: "none",
        model: "local",
      },
      { fetchImpl: fakeFetch },
    );
    await client.complete([{ role: "user", content: "hi" }]);
    expect(seenAuth).toBeNull();
  });

  test("tags OpenAgents Khala traffic as internal QA demand without sending secrets", async () => {
    let seenHeaders = new Headers();
    const fakeFetch = (async (_url: string | URL | Request, init?: RequestInit) => {
      seenHeaders = new Headers(init?.headers);
      return new Response(JSON.stringify({ choices: [{ message: { content: "ok" } }] }), {
        status: 200,
        headers: { "content-type": "application/json" },
      });
    }) as unknown as typeof fetch;

    const config = resolveByoModelConfig({
      env: { QA_API_KEY: "oa_agent_test", QA_DEMAND_SOURCE: "qa-dogfood" },
    });
    const client = makeByoChatClient(config, { fetchImpl: fakeFetch });
    await client.complete([{ role: "user", content: "hi" }]);

    expect(seenHeaders.get(QA_CLIENT_HEADER)).toBe("qa-runner");
    expect(seenHeaders.get(QA_DEMAND_KIND_HEADER)).toBe("internal");
    expect(seenHeaders.get(QA_DEMAND_SOURCE_HEADER)).toBe("qa-dogfood");
    expect(seenHeaders.get(QA_DEMAND_SOURCE_HEADER)).not.toContain("oa_agent");
  });
});
