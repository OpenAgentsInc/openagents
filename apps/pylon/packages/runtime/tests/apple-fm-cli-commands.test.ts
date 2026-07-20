import { describe, expect, test } from "vite-plus/test";
import { Effect } from "effect";
import { APPLE_FM_DEFAULT_MODEL_ID, runProbeCli } from "../src";

/** A fake bridge `fetch` for the CLI deps: ready health + a fixed completion. */
function fakeBridgeFetch(completionText: string): typeof fetch {
  return (async (input: RequestInfo | URL) => {
    const url = new URL(String(input));
    if (url.pathname === "/health") {
      return Response.json({ ready: true, modelId: APPLE_FM_DEFAULT_MODEL_ID, platform: "fake-apple-silicon", version: "test" });
    }
    if (url.pathname === "/v1/chat/completions") {
      return Response.json({
        id: "fake_completion_1",
        model: APPLE_FM_DEFAULT_MODEL_ID,
        choices: [{ index: 0, message: { role: "assistant", content: completionText }, finishReason: "stop" }],
        usage: { truth: "estimated", promptTokens: 2, completionTokens: 3, totalTokens: 5 },
      });
    }
    return new Response("not found", { status: 404 });
  }) as unknown as typeof fetch;
}

describe("Apple FM runnable CLI commands", () => {
  test("apple-fm health --json reports typed readiness without inference", async () => {
    const seen: string[] = [];
    const result = await Effect.runPromise(
      runProbeCli(["apple-fm", "health", "--base-url", "http://127.0.0.1:11439", "--json"], {
        env: {},
        fetch: (async (input: RequestInfo | URL) => {
          seen.push(new URL(String(input)).pathname);
          return Response.json({ ready: true, modelId: APPLE_FM_DEFAULT_MODEL_ID, platform: "fake-apple-silicon", version: "test" });
        }) as unknown as typeof fetch,
      }),
    );
    expect(result.exitCode).toBe(0);
    const out = JSON.parse(result.stdout);
    expect(out.command).toBe("apple-fm.health");
    expect(out.ready).toBe(true);
    expect(out.status).toBe("ready");
    expect(seen).toEqual(["/health"]); // no inference endpoint hit
    expect(result.stdout).toContain('"contentRedacted": true');
  });

  test("apple-fm infer returns real completion text and honest usage truth (--json)", async () => {
    const result = await Effect.runPromise(
      runProbeCli(["apple-fm", "infer", "--base-url", "http://127.0.0.1:11439", "--prompt", "hi", "--json"], {
        env: {},
        fetch: fakeBridgeFetch("infer ok"),
      }),
    );
    expect(result.exitCode).toBe(0);
    const out = JSON.parse(result.stdout);
    expect(out.command).toBe("apple-fm.infer");
    expect(out.text).toBe("infer ok");
    expect(out.usage.truth).toBe("estimated");
    expect(out.receipt.contentRedacted).toBe(true);
  });

  test("apple-fm infer prints text and usage in the default human format", async () => {
    const result = await Effect.runPromise(
      runProbeCli(["apple-fm", "infer", "--base-url", "http://127.0.0.1:11439", "--prompt", "hi"], {
        env: {},
        fetch: fakeBridgeFetch("infer ok"),
      }),
    );
    expect(result.exitCode).toBe(0);
    expect(result.stdout).toContain("Apple FM inference");
    expect(result.stdout).toContain("text: infer ok");
    expect(result.stdout).toContain("truth=estimated");
  });

  test("apple-fm session streams a bounded session turn via /v1/sessions + SSE (--json)", async () => {
    // Mirror the real bridge's session endpoints: create returns a session id,
    // the stream returns SSE snapshot + completed frames.
    const sessionFetch = (async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = new URL(String(input));
      if (url.pathname === "/v1/sessions" && (init?.method ?? "GET") === "POST" && !url.pathname.endsWith("/stream")) {
        return Response.json({ session: { id: "apple_fm_session_test" } });
      }
      if (url.pathname === "/v1/sessions/apple_fm_session_test/responses/stream") {
        const body =
          `event: snapshot\ndata: {"content":"session ok","finishReason":"stop","output":"session ok","sequence":0}\n\n` +
          `event: completed\ndata: {"content":"session ok","model":"apple-foundation-model","output":"session ok","usage":{"completionTokens":2,"promptTokens":3,"totalTokens":5,"truth":"estimated"}}\n\n`;
        return new Response(body, { status: 200, headers: { "content-type": "text/event-stream" } });
      }
      return new Response("not found", { status: 404 });
    }) as unknown as typeof fetch;

    const result = await Effect.runPromise(
      runProbeCli(["apple-fm", "session", "--base-url", "http://127.0.0.1:11439", "--prompt", "hi", "--json"], {
        env: {},
        fetch: sessionFetch,
      }),
    );
    expect(result.exitCode).toBe(0);
    const out = JSON.parse(result.stdout);
    expect(out.command).toBe("apple-fm.session");
    expect(out.text).toBe("session ok");
    expect(out.usage.truth).toBe("estimated");
    expect(Array.isArray(out.events)).toBe(true);
  });

  test("apple-fm session --stream reconstructs the progressive snapshot deltas", async () => {
    const streamFetch = (async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = new URL(String(input));
      if (url.pathname === "/v1/sessions" && (init?.method ?? "GET") === "POST") {
        return Response.json({ session: { id: "apple_fm_session_test" } });
      }
      if (url.pathname === "/v1/sessions/apple_fm_session_test/responses/stream") {
        // Three progressive (cumulative) snapshots, then completed.
        const body =
          `event: snapshot\ndata: {"content":"Red","output":"Red","sequence":0}\n\n` +
          `event: snapshot\ndata: {"content":"Red Green","output":"Red Green","sequence":1}\n\n` +
          `event: snapshot\ndata: {"content":"Red Green Blue","finishReason":"stop","output":"Red Green Blue","sequence":2}\n\n` +
          `event: completed\ndata: {"content":"Red Green Blue","model":"apple-foundation-model","output":"Red Green Blue","usage":{"completionTokens":3,"promptTokens":2,"totalTokens":5,"truth":"estimated"}}\n\n`;
        return new Response(body, { status: 200, headers: { "content-type": "text/event-stream" } });
      }
      return new Response("not found", { status: 404 });
    }) as unknown as typeof fetch;

    const json = await Effect.runPromise(
      runProbeCli(["apple-fm", "session", "--base-url", "http://127.0.0.1:11439", "--prompt", "colors", "--json"], { env: {}, fetch: streamFetch }),
    );
    const out = JSON.parse(json.stdout);
    expect(out.snapshots).toBe(3);
    expect(out.usage.truth).toBe("estimated");

    const streamed = await Effect.runPromise(
      runProbeCli(["apple-fm", "session", "--base-url", "http://127.0.0.1:11439", "--prompt", "colors", "--stream"], { env: {}, fetch: streamFetch }),
    );
    expect(streamed.stdout).toContain("snapshots: 3");
    // Deltas reconstruct the final text ("Red" + " Green" + " Blue").
    expect(streamed.stdout).toContain("stream: Red Green Blue");
  });

  test("apple-fm infer surfaces a not-ready bridge as a typed failure, not a fake success", async () => {
    const result = await Effect.runPromise(
      runProbeCli(["apple-fm", "infer", "--base-url", "http://127.0.0.1:11439", "--prompt", "hi", "--json"], {
        env: {},
        fetch: (async (input: RequestInfo | URL) => {
          const url = new URL(String(input));
          if (url.pathname === "/v1/chat/completions") {
            return Response.json({ error: "apple_fm_not_ready", message: "not ready", unavailableReason: "model_unavailable" }, { status: 503 });
          }
          return new Response("not found", { status: 404 });
        }) as unknown as typeof fetch,
      }),
    );
    expect(result.exitCode).toBe(1);
    const out = JSON.parse(result.stdout);
    expect(out.state).toBe("failed");
    expect(out.command).toBe("apple-fm.infer");
  });

  test("apple-fm usage lists the runnable inference and tool commands", async () => {
    const result = await Effect.runPromise(runProbeCli(["apple-fm", "bogus-command"], { env: {} }));
    expect(result.exitCode).toBe(1);
    expect(result.stdout).toContain("apple-fm infer");
    expect(result.stdout).toContain("apple-fm session");
    expect(result.stdout).toContain("apple-fm tool");
    expect(result.stdout).toContain("--auto-launch");
  });
});
