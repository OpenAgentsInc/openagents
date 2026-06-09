import { describe, expect, test } from "bun:test";
import { Effect, Schema as S } from "effect";
import {
  AppleFmChatCompletionResponse,
  AppleFmHealthResponse,
  APPLE_FM_DEFAULT_BASE_URL,
  APPLE_FM_DEFAULT_MODEL_ID,
  makeAppleFmAvailabilityReceipt,
  makeAppleFmTranscriptReceipt,
  resolveAppleFmBackendProfile,
  type AppleFmUsageMeasurement,
} from "../../index";

describe("Apple FM backend contract", () => {
  test("resolves the Apple FM local profile with the retained env override order", async () => {
    const explicit = await Effect.runPromise(
      resolveAppleFmBackendProfile({
        explicitBaseUrl: "http://127.0.0.1:11439",
        env: {
          PROBE_APPLE_FM_BASE_URL: "http://127.0.0.1:11438",
          OPENAGENTS_APPLE_FM_BASE_URL: "http://127.0.0.1:11437",
        },
      }),
    );
    expect(explicit.baseUrl).toBe("http://127.0.0.1:11439");
    expect(explicit.baseUrlSource).toBe("explicit");

    const probeEnv = await Effect.runPromise(
      resolveAppleFmBackendProfile({
        env: {
          PROBE_APPLE_FM_BASE_URL: "http://127.0.0.1:11438",
          OPENAGENTS_APPLE_FM_BASE_URL: "http://127.0.0.1:11437",
        },
      }),
    );
    expect(probeEnv.baseUrl).toBe("http://127.0.0.1:11438");
    expect(probeEnv.baseUrlSource).toBe("PROBE_APPLE_FM_BASE_URL");

    const openAgentsEnv = await Effect.runPromise(
      resolveAppleFmBackendProfile({
        env: {
          OPENAGENTS_APPLE_FM_BASE_URL: "http://127.0.0.1:11437",
        },
      }),
    );
    expect(openAgentsEnv.baseUrl).toBe("http://127.0.0.1:11437");
    expect(openAgentsEnv.baseUrlSource).toBe("OPENAGENTS_APPLE_FM_BASE_URL");

    const fallback = await Effect.runPromise(resolveAppleFmBackendProfile());
    expect(fallback.baseUrl).toBe(APPLE_FM_DEFAULT_BASE_URL);
    expect(fallback.baseUrlSource).toBe("default");
  });

  test("decodes CI-safe fake bridge health and completion responses", async () => {
    const server = Bun.serve({
      port: 0,
      fetch: async (request) => {
        const url = new URL(request.url);

        if (request.method === "GET" && url.pathname === "/health") {
          return Response.json({
            ready: true,
            modelId: APPLE_FM_DEFAULT_MODEL_ID,
            platform: "fake-apple-silicon",
            version: "test",
          });
        }

        if (request.method === "POST" && url.pathname === "/v1/chat/completions") {
          return Response.json({
            id: "fake_completion_1",
            model: APPLE_FM_DEFAULT_MODEL_ID,
            choices: [
              {
                index: 0,
                message: {
                  role: "assistant",
                  content: "fake Apple FM response",
                },
                finishReason: "stop",
              },
            ],
            usage: {
              truth: "estimated",
              promptTokens: 3,
              completionTokens: 4,
              totalTokens: 7,
            },
          });
        }

        return new Response("not found", { status: 404 });
      },
    });

    try {
      const baseUrl = `http://127.0.0.1:${server.port}`;
      const healthResponse = await fetch(new URL("/health", baseUrl));
      const health = await Effect.runPromise(S.decodeUnknownEffect(AppleFmHealthResponse)(await healthResponse.json()));

      expect(health.ready).toBe(true);
      expect(health.modelId).toBe(APPLE_FM_DEFAULT_MODEL_ID);

      const completionResponse = await fetch(new URL("/v1/chat/completions", baseUrl), {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          model: APPLE_FM_DEFAULT_MODEL_ID,
          messages: [{ role: "user", content: "smoke" }],
        }),
      });
      const completion = await Effect.runPromise(
        S.decodeUnknownEffect(AppleFmChatCompletionResponse)(await completionResponse.json()),
      );

      expect(completion.choices[0]?.message.content).toBe("fake Apple FM response");
      expect(completion.usage?.truth).toBe("estimated");
    } finally {
      server.stop(true);
    }
  });

  test("records availability and transcript receipts without raw URL credentials", () => {
    const usage: AppleFmUsageMeasurement = {
      truth: "unknown",
    };

    const availability = makeAppleFmAvailabilityReceipt({
      profileId: "apple-fm-local",
      model: APPLE_FM_DEFAULT_MODEL_ID,
      baseUrl: "http://user:secret@127.0.0.1:11435/?token=hidden",
      ready: false,
      unavailableReason: "unsupported_hardware",
      observedAt: "2026-06-07T00:00:00.000Z",
    });
    const transcript = makeAppleFmTranscriptReceipt({
      profileId: "apple-fm-local",
      model: APPLE_FM_DEFAULT_MODEL_ID,
      usage,
      observedAt: "2026-06-07T00:00:00.000Z",
    });

    expect(availability.contentRedacted).toBe(true);
    expect(availability.baseUrl).toBe("http://127.0.0.1:11435");
    expect(JSON.stringify(availability)).not.toContain("secret");
    expect(JSON.stringify(availability)).not.toContain("hidden");
    expect(transcript.usage?.truth).toBe("unknown");
  });
});
