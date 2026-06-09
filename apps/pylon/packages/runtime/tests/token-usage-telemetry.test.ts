import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  makeAppleFmProbeTokenUsageEvent,
  makeGeminiProbeTokenUsageEvent,
  makeOmegaTokenUsageTelemetryClient,
  makeProbeAssignmentTokenUsageSourceRefs,
  makeProbeTokenUsageTelemetryClientFromEnv,
  makeStaticProbeTokenUsageTelemetryClient,
  recordProbeTokenUsageEvent,
  validateProbeTokenUsageEvent,
  type GeminiCompleteResult,
  type ProbeRunAssignment,
} from "../src";
import { makeProbeLlmUsage } from "../src/llm";

const observedAt = "2026-06-08T12:00:00.000Z";

const geminiResult = (usage = makeProbeLlmUsage({
  cacheReadInputTokens: 3,
  cacheWriteInputTokens: 4,
  inputTokens: 10,
  outputTokens: 7,
  reasoningTokens: 2,
  totalTokens: 17,
  providerMetadata: {
    google: {
      promptTokenCount: 10,
      candidatesTokenCount: 5,
      thoughtsTokenCount: 2,
      totalTokenCount: 17,
    },
  },
})): GeminiCompleteResult => ({
  events: [],
  finalRequest: {
    generation: {},
    messages: [],
    model: { provider: "google", model: "gemini-3.5-flash" },
    providerOptions: {},
    system: [],
    toolChoice: undefined,
    tools: [],
  },
  profile: {
    attachMode: "direct_api",
    auth: "api_key",
    baseUrl: "https://generativelanguage.googleapis.com/v1beta",
    baseUrlSource: "default",
    defaultBaseUrl: "https://generativelanguage.googleapis.com/v1beta",
    id: "gemini-api",
    kind: "gemini_api",
    model: "gemini-3.5-flash",
    readinessPath: "",
    streamMode: "sse",
  },
  receipt: {
    backendKind: "gemini_api",
    contentRedacted: true,
    kind: "probe_backend_transcript",
    model: "gemini-3.5-flash",
    observedAt,
    profileId: "gemini-api",
    roundTrips: 1,
    usage,
  },
  roundTrips: 1,
  text: "redacted result text",
  toolReceipts: [],
});

describe("Probe token usage telemetry", () => {
  test("normalizes Gemini usage and produces stable idempotency keys", () => {
    const first = makeGeminiProbeTokenUsageEvent({
      agentSurface: "cli",
      command: "backend.gemini.smoke",
      result: geminiResult(),
      sourceRefs: { anonymizedSourceRef: "probe.cli.backend.gemini.smoke.gemini-api" },
    });
    const second = makeGeminiProbeTokenUsageEvent({
      agentSurface: "cli",
      command: "backend.gemini.smoke",
      result: geminiResult(),
      sourceRefs: { anonymizedSourceRef: "probe.cli.backend.gemini.smoke.gemini-api" },
    });

    expect(first.sourceRoute).toBe("probe_direct_provider");
    expect(first.provider).toBe("google_gemini");
    expect(first.usageTruth).toBe("exact");
    expect(first.tokenCounts).toEqual({
      cacheReadTokens: 3,
      cacheWrite1hTokens: 0,
      cacheWrite5mTokens: 4,
      inputTokens: 10,
      outputTokens: 7,
      reasoningTokens: 2,
      totalTokens: 17,
    });
    expect(first.idempotencyKey).toBe(second.idempotencyKey);
    expect(first.eventId).toBe(second.eventId);
    expect(JSON.stringify(first)).not.toContain("redacted result text");
  });

  test("preserves Apple FM usage truth when mapping local model counts", () => {
    const event = makeAppleFmProbeTokenUsageEvent({
      agentSurface: "cli",
      command: "apple-fm.smoke",
      observedAt,
      profile: {
        attachMode: "attach_existing",
        auth: "none",
        baseUrl: "http://127.0.0.1:11435",
        baseUrlSource: "default",
        defaultBaseUrl: "http://127.0.0.1:11435",
        id: "apple-fm-local",
        kind: "apple_fm_bridge",
        model: "apple-foundation-model",
        readinessPath: "/health",
        streamMode: "snapshot",
      },
      usage: {
        completionTokens: 5,
        promptTokens: 2,
        totalTokens: 7,
        truth: "estimated",
      },
    });

    expect(event.sourceRoute).toBe("probe_local_model");
    expect(event.provider).toBe("apple_fm");
    expect(event.usageTruth).toBe("estimated");
    expect(event.tokenCounts).toMatchObject({
      inputTokens: 2,
      outputTokens: 5,
      totalTokens: 7,
    });
  });

  test("hashes assignment repository scope and rejects private telemetry material", async () => {
    const assignment: ProbeRunAssignment = {
      assignmentId: "assignment_repo_1",
      runnerSessionId: "runner_session_1",
      goal: "private goal stays out of telemetry",
      repo: {
        branch: "customer/private-branch",
        path: "/Users/chris/private/customer-repo",
      },
    };
    const sourceRefs = makeProbeAssignmentTokenUsageSourceRefs(assignment);

    expect(sourceRefs.repositoryRef).toMatch(/^repo\.sha256\.[a-f0-9]{32}$/);
    expect(JSON.stringify(sourceRefs)).not.toContain("/Users/chris");
    expect(JSON.stringify(sourceRefs)).not.toContain("customer-repo");

    const event = makeGeminiProbeTokenUsageEvent({
      agentSurface: "managed_assignment",
      result: geminiResult(),
      sourceRefs,
    });

    await expect(Effect.runPromise(validateProbeTokenUsageEvent(event))).resolves.toBeUndefined();
    await expect(
      Effect.runPromise(
        validateProbeTokenUsageEvent({
          ...event,
          safeMetadata: {
            prompt: "raw prompt must not leave Probe",
          },
        }),
      ),
    ).rejects.toMatchObject({ _tag: "ProbeTokenUsageTelemetryUnsafe" });
  });

  test("posts redacted events to Omega token usage ingestion", async () => {
    const event = makeGeminiProbeTokenUsageEvent({
      actor: { userId: "user_chris" },
      agentSurface: "cli",
      command: "backend.gemini.smoke",
      privacy: { leaderboardEligible: false, privacyOptOut: true },
      result: geminiResult(),
    });
    const seen: { authorization?: string; body?: unknown; url?: string } = {};
    const client = makeOmegaTokenUsageTelemetryClient({
      baseUrl: "https://openagents.com",
      bearerToken: "oa_agent_test",
      fetch: async (input, init) => {
        seen.url = String(input);
        seen.authorization = new Headers(init?.headers).get("authorization") ?? "";
        seen.body = JSON.parse(String(init?.body));

        return Response.json({ ok: true }, { status: 201 });
      },
    });

    await Effect.runPromise(recordProbeTokenUsageEvent(client, event));

    expect(seen.url).toBe("https://openagents.com/api/stats/token-usage/events");
    expect(seen.authorization).toBe("Bearer oa_agent_test");
    expect(seen.body).toMatchObject({
      actor: { userId: "user_chris" },
      privacy: { leaderboardEligible: false, privacyOptOut: true },
      producerSystem: "probe",
      sourceRoute: "probe_direct_provider",
    });
    expect(JSON.stringify(seen.body)).not.toContain("raw prompt");
    expect(JSON.stringify(seen.body)).not.toContain("redacted result text");
    expect(JSON.stringify(seen.body)).not.toContain("oa_agent_test");
  });

  test("supports local no-op and explicit send opt-out modes", async () => {
    const event = makeGeminiProbeTokenUsageEvent({
      agentSurface: "cli",
      result: geminiResult(),
    });
    const noEnvTelemetry = makeStaticProbeTokenUsageTelemetryClient();

    await Effect.runPromise(recordProbeTokenUsageEvent(noEnvTelemetry.client, event));
    expect(noEnvTelemetry.events).toHaveLength(1);

    const disabled = makeProbeTokenUsageTelemetryClientFromEnv({
      env: {
        PROBE_OMEGA_BASE_URL: "https://openagents.com",
        PROBE_TOKEN_USAGE_OPT_OUT: "true",
      },
      fetch: async () => {
        throw new Error("disabled telemetry should not fetch");
      },
    });

    await expect(Effect.runPromise(recordProbeTokenUsageEvent(disabled, event))).resolves.toBeUndefined();
  });
});
