import { describe, expect, test } from "bun:test";
import * as S from "effect/Schema";
import { Effect } from "effect";
import { toolToOpenRouterDefinition, runOpenRouterChat, type LogLevel, type OpenRouterLogger } from "./openrouter.js";

describe("openrouter mapping", () => {
  test("converts Effect schema tool to OpenRouter function definition", () => {
    const tool = {
      name: "read",
      label: "read",
      description: "Read a file",
      schema: S.Struct({ path: S.String }),
      execute: () => null as any,
    };

    const def: any = toolToOpenRouterDefinition(tool);

    expect(def.type).toBe("function");
    expect(def.function?.name).toBe("read");
    expect(def.function?.description).toBe("Read a file");
    expect(def.function?.parameters).toHaveProperty("properties.path");
  });
});

describe("openrouter client", () => {
  const originalFetch = globalThis.fetch;
  const envApiKey = process.env.OPENROUTER_API_KEY;

  const setApiKey = () => {
    process.env.OPENROUTER_API_KEY = "test-key";
  };

  const resetEnv = () => {
    if (envApiKey === undefined) {
      delete process.env.OPENROUTER_API_KEY;
    } else {
      process.env.OPENROUTER_API_KEY = envApiKey;
    }
  };

  const makeMemoryLogger = (level: LogLevel): { logger: OpenRouterLogger; logs: string[] } => {
    const logs: string[] = [];
    const push = (prefix: string) => (msg: string) => logs.push(`${prefix}:${msg}`);
    return {
      logs,
      logger: {
        level,
        debug: push("debug"),
        info: push("info"),
        warn: push("warn"),
        error: push("error"),
      },
    };
  };

  test("aborts request on timeout and logs warning", async () => {
    setApiKey();
    const { logger, logs } = makeMemoryLogger("debug");

    globalThis.fetch = ((_url: string, init?: RequestInit) =>
      new Promise((_resolve, reject) => {
        init?.signal?.addEventListener("abort", () => reject(new Error("aborted")));
      })) as any;

    const promise = Effect.runPromise(
      runOpenRouterChat({
        messages: [{ role: "user", content: "hi" }],
        timeoutMs: 10,
        retry: { attempts: 1 },
        logger,
      }),
    );

    await expect(promise).rejects.toThrow(/timed out/i);
    expect(logs.some((msg) => msg.includes("warn:[OpenRouter] Request timed out"))).toBe(true);

    globalThis.fetch = originalFetch;
    resetEnv();
  });

  test("uses leveled logging (info is emitted when enabled)", async () => {
    setApiKey();
    const { logger, logs } = makeMemoryLogger("info");

    globalThis.fetch = (async () =>
      ({
        ok: true,
        json: async () => ({
          id: "resp-123",
          choices: [{ message: { content: "hello", tool_calls: [] } }],
        }),
      })) as any;

    await Effect.runPromise(
      runOpenRouterChat({
        messages: [{ role: "user", content: "hi" }],
        retry: { attempts: 1 },
        logger,
        logLevel: "info",
      }),
    );

    expect(logs.some((msg) => msg.startsWith("info:[OpenRouter] Response received"))).toBe(true);
    expect(logs.some((msg) => msg.startsWith("warn:"))).toBe(false);

    globalThis.fetch = originalFetch;
    resetEnv();
  });

  test("applies per-request baseUrl/apiKey/header overrides", async () => {
    setApiKey();
    let receivedUrl = "";
    let receivedAuth: string | undefined;
    let receivedHeader: string | undefined;

    globalThis.fetch = (async (url: string, init?: RequestInit) => {
      receivedUrl = url;
      const hdrs = init?.headers as Record<string, string> | undefined;
      receivedAuth = hdrs?.Authorization;
      receivedHeader = hdrs?.["X-Test-Header"];
      return {
        ok: true,
        json: async () => ({
          id: "resp-override",
          choices: [{ message: { content: "hello", tool_calls: [] } }],
        }),
      } as any;
    }) as any;

    await Effect.runPromise(
      runOpenRouterChat({
        baseUrl: "https://example.test/api",
        apiKey: "override-key",
        headers: { "X-Test-Header": "present" },
        messages: [{ role: "user", content: "hi" }],
        retry: { attempts: 1 },
      }),
    );

    expect(receivedUrl).toContain("https://example.test/api/chat/completions");
    expect(receivedAuth).toBe("Bearer override-key");
    expect(receivedHeader).toBe("present");

    globalThis.fetch = originalFetch;
    resetEnv();
  });
});
