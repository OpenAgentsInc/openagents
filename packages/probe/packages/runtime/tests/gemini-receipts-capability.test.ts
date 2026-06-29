import { describe, expect, test } from "bun:test";
import { Effect, Schema as S } from "effect";
import {
  GeminiBackendAvailabilityReceipt,
  PROBE_GEMINI_BACKEND_CAPABILITY,
  defineProbeLlmTool,
  makeGeminiAvailabilityReceipt,
  makeGeminiClient,
  makeProbeLlmRequest,
  reportGeminiBackendCapability,
  type ProbeRunnerIdentity,
} from "../src";

const runner = (): ProbeRunnerIdentity => ({
  runnerId: "runner_gemini_1",
  kind: "pylon",
  linkedSubject: "provider_1",
  linkedAt: "2026-06-08T00:00:00.000Z",
  capabilities: ["probe.run", PROBE_GEMINI_BACKEND_CAPABILITY],
});

const sse = (...events: ReadonlyArray<unknown>): string =>
  `${events.map((event) => `data: ${JSON.stringify(event)}\n`).join("\n")}data: [DONE]\n\n`;

describe("Gemini receipts and capability reporting", () => {
  test("availability receipts redact URL credentials and API keys", async () => {
    const receipt = makeGeminiAvailabilityReceipt({
      profileId: "gemini-api",
      model: "gemini-3.5-flash",
      baseUrl: "https://user:password@generativelanguage.googleapis.com/v1beta?key=secret",
      ready: true,
      apiKeySource: "GOOGLE_GENERATIVE_AI_API_KEY",
      observedAt: "2026-06-08T00:00:00.000Z",
    });

    const decoded = await Effect.runPromise(S.decodeUnknownEffect(GeminiBackendAvailabilityReceipt)(receipt));

    expect(decoded.baseUrl).toBe("https://generativelanguage.googleapis.com/v1beta");
    expect(decoded.apiKeyRedacted).toBe(true);
    expect(JSON.stringify(decoded)).not.toContain("password");
    expect(JSON.stringify(decoded)).not.toContain("secret");
  });

  test("Gemini completion emits transcript and native tool-call receipts", async () => {
    const responses = [
      sse({
        candidates: [
          {
            content: {
              role: "model",
              parts: [{ functionCall: { name: "lookup", args: { query: "weather" } } }],
            },
            finishReason: "STOP",
          },
        ],
      }),
      sse({
        candidates: [{ content: { role: "model", parts: [{ text: "done" }] }, finishReason: "STOP" }],
        usageMetadata: { promptTokenCount: 4, candidatesTokenCount: 1, totalTokenCount: 5 },
      }),
    ];
    const client = await Effect.runPromise(
      makeGeminiClient({
        apiKey: "test-key",
        now: new Date("2026-06-08T00:00:00.000Z"),
        fetch: async () =>
          new Response(
            responses.shift() ?? "",
            { status: 200 },
          ),
      }),
    );
    const result = await Effect.runPromise(
      client.complete({
        request: makeProbeLlmRequest({
          model: { provider: "google", model: "gemini-3.5-flash" },
          prompt: "Use lookup.",
        }),
        tools: {
          lookup: defineProbeLlmTool({
            name: "lookup",
            description: "Lookup data.",
            inputSchema: { type: "object" },
            execute: () => Effect.succeed({ ok: true }),
          }),
        },
      }),
    );

    expect(result.receipt).toMatchObject({
      kind: "probe_backend_transcript",
      backendKind: "gemini_api",
      roundTrips: 2,
      contentRedacted: true,
    });
    expect(result.toolReceipts).toEqual([
      {
        kind: "probe_backend_tool_call",
        backendKind: "gemini_api",
        profileId: "gemini-api",
        model: "gemini-3.5-flash",
        toolCallId: "tool_0",
        toolName: "lookup",
        status: "success",
        observedAt: "2026-06-08T00:00:00.000Z",
        contentRedacted: true,
      },
    ]);
    expect(JSON.stringify(result.receipt)).not.toContain("Use lookup");
    expect(JSON.stringify(result.toolReceipts)).not.toContain("weather");
    expect(JSON.stringify(result)).not.toContain("test-key");
  });

  test("reports Gemini backend capability when API key is available", async () => {
    const report = await Effect.runPromise(
      reportGeminiBackendCapability({
        runner: runner(),
        apiKey: "test-key",
        trustedBackendBaseUrl: "https://user:password@generativelanguage.googleapis.com/v1beta?key=secret",
        now: new Date("2026-06-08T00:00:00.000Z"),
      }),
    );

    expect(report.available).toBe(true);
    expect(report.status).toBe("ready");
    expect(report.backendKind).toBe("gemini_api");
    expect(report.capability).toBe(PROBE_GEMINI_BACKEND_CAPABILITY);
    expect(report.advertisedCapabilities).toEqual([PROBE_GEMINI_BACKEND_CAPABILITY]);
    expect(report.support).toEqual({ sseStreaming: true, nativeToolCalls: true, toolCallbacks: false });
    expect(report.baseUrl).toBe("https://generativelanguage.googleapis.com/v1beta");
    expect(JSON.stringify(report)).not.toContain("test-key");
    expect(JSON.stringify(report)).not.toContain("password");
    expect(JSON.stringify(report)).not.toContain("secret");
  });

  test("does not advertise Gemini capability when API key is missing", async () => {
    const report = await Effect.runPromise(
      reportGeminiBackendCapability({
        runner: runner(),
        env: {},
        now: new Date("2026-06-08T00:00:00.000Z"),
      }),
    );

    expect(report.available).toBe(false);
    expect(report.status).toBe("unavailable");
    expect(report.advertisedCapabilities).toEqual([]);
    expect(report.unavailableReason).toBe("missing_credential");
    expect(report.receipt).toMatchObject({
      kind: "probe_backend_availability",
      backendKind: "gemini_api",
      ready: false,
      apiKeyRedacted: true,
      contentRedacted: true,
    });
  });
});
