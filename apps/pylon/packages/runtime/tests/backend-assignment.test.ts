import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  APPLE_FM_DEFAULT_MODEL_ID,
  PROBE_APPLE_FM_BACKEND_CAPABILITY,
  PROBE_GEMINI_BACKEND_CAPABILITY,
  PROBE_PSIONIC_QWEN_BACKEND_CAPABILITY,
  runProbeBackendAssignment,
  type ProbeRunAssignment,
  type ProbeRunnerAssignmentProof,
  type ProbeRunnerIdentity,
} from "../src";

const assignment = (): ProbeRunAssignment => ({
  assignmentId: "assignment_apple_fm_1",
  runnerSessionId: "runner_session_1",
  goal: "Summarize this local repo.",
  backend: {
    kind: "apple_fm_bridge",
    profile: "apple-fm-local",
  },
});

const geminiAssignment = (): ProbeRunAssignment => ({
  assignmentId: "assignment_gemini_1",
  runnerSessionId: "runner_session_1",
  goal: "Summarize this local repo through Gemini.",
  backend: {
    kind: "gemini_api",
    backendProfileId: "gemini-api",
  },
});

const psionicAssignment = (): ProbeRunAssignment => ({
  assignmentId: "assignment_psionic_1",
  runnerSessionId: "runner_session_1",
  goal: "Summarize this local repo through Psionic Qwen.",
  backend: {
    kind: "psionic_qwen35",
    backendProfileId: "psionic-qwen35-local",
  },
  blueprint: {
    registryVersionRef: "blueprint_registry.test",
    backendCapabilityRefs: [PROBE_PSIONIC_QWEN_BACKEND_CAPABILITY],
  },
});

const runner = (capabilities = ["probe.run", PROBE_APPLE_FM_BACKEND_CAPABILITY]): ProbeRunnerIdentity => ({
  runnerId: "runner_local_1",
  kind: "local",
  linkedSubject: "user_1",
  linkedAt: "2026-06-07T00:00:00.000Z",
  capabilities,
});

const proof = (): ProbeRunnerAssignmentProof => ({
  runnerId: "runner_local_1",
  assignmentId: "assignment_apple_fm_1",
  runnerSessionId: "runner_session_1",
  issuedAt: "2026-06-07T00:00:00.000Z",
  nonce: "nonce_apple_fm_1",
  proofKind: "test",
});

describe("Probe backend assignment routing", () => {
  test("runs an Apple FM assignment without provider auth materialization", async () => {
    const seenPaths: string[] = [];
    let tokenUsageEvent: unknown;
    const result = await Effect.runPromise(
      runProbeBackendAssignment({
        runner: runner(),
        proof: proof(),
        assignment: assignment(),
        trustedBackendBaseUrl: "http://127.0.0.1:11439",
        fetch: async (input, init) => {
          const url = new URL(String(input));
          seenPaths.push(`${init?.method ?? "GET"} ${url.pathname}`);

          if (url.pathname === "/api/stats/token-usage/events") {
            tokenUsageEvent = JSON.parse(String(init?.body));
            return Response.json({ ok: true });
          }

          if (url.pathname === "/health") {
            return Response.json({
              ready: true,
              modelId: APPLE_FM_DEFAULT_MODEL_ID,
            });
          }

          const body = JSON.parse(String(init?.body));
          expect(body.messages[0].content).toBe("Summarize this local repo.");
          return Response.json({
            model: APPLE_FM_DEFAULT_MODEL_ID,
            choices: [
              {
                message: {
                  role: "assistant",
                  content: "local summary",
                },
                finish_reason: "stop",
              },
            ],
            usage: {
              truth: "exact",
              promptTokens: 4,
              completionTokens: 2,
              totalTokens: 6,
            },
          });
        },
        now: new Date("2026-06-07T00:00:00.000Z"),
      }),
    );

    expect(result.authRequired).toBe(false);
    expect(result.backendKind).toBe("apple_fm_bridge");
    expect(result.completion.text).toBe("local summary");
    expect(result.completion.usage.truth).toBe("exact");
    expect(result.events.map((event) => event.kind)).toEqual([
      "probe_backend_run_started",
      "probe_backend_run_finished",
    ]);
    expect(seenPaths).toEqual(["GET /health", "POST /v1/chat/completions", "POST /api/stats/token-usage/events"]);
    expect(tokenUsageEvent).toMatchObject({
      producerSystem: "probe",
      provider: "apple_fm",
      sourceRefs: {
        runRef: "probe.assignment.assignment_apple_fm_1",
        sessionRef: "probe.runner_session.runner_session_1",
      },
      sourceRoute: "probe_local_model",
      tokenCounts: {
        inputTokens: 4,
        outputTokens: 2,
        totalTokens: 6,
      },
      usageTruth: "exact",
    });
  });

  test("rejects Apple FM assignment when runner lacks backend capability", async () => {
    await expect(
      Effect.runPromise(
        runProbeBackendAssignment({
          runner: runner(["probe.run"]),
          proof: proof(),
          assignment: assignment(),
          fetch: async () => Response.json({ ready: true, modelId: APPLE_FM_DEFAULT_MODEL_ID }),
        }),
      ),
    ).rejects.toMatchObject({ _tag: "ProbeRunnerAuthorizationError" });
  });

  test("rejects Apple FM assignment when live health is not ready", async () => {
    await expect(
      Effect.runPromise(
        runProbeBackendAssignment({
          runner: runner(),
          proof: proof(),
          assignment: assignment(),
          fetch: async () =>
            Response.json({
              ready: false,
              modelId: APPLE_FM_DEFAULT_MODEL_ID,
              unavailableReason: "model_unavailable",
              message: "model is not admitted on this host",
            }),
          now: new Date("2026-06-07T00:00:00.000Z"),
        }),
      ),
    ).rejects.toMatchObject({
      _tag: "ProbeBackendAssignmentError",
      receipt: {
        kind: "probe_backend_availability",
        ready: false,
        unavailableReason: "model_unavailable",
      },
      events: [
        { kind: "probe_backend_run_started" },
        { kind: "probe_backend_run_failed" },
      ],
    });
  });

  test("runs a Gemini assignment through the direct API backend", async () => {
    const seenUrls: string[] = [];
    const result = await Effect.runPromise(
      runProbeBackendAssignment({
        runner: runner(["probe.run", PROBE_GEMINI_BACKEND_CAPABILITY]),
        proof: { ...proof(), assignmentId: "assignment_gemini_1" },
        assignment: geminiAssignment(),
        env: { GEMINI_API_KEY: "test-gemini-key" },
        fetch: async (input, init) => {
          seenUrls.push(String(input));
          expect(new Headers(init?.headers).get("x-goog-api-key")).toBe("test-gemini-key");
          const body = JSON.parse(String(init?.body));
          expect(body.contents[0].parts[0].text).toBe("Summarize this local repo through Gemini.");

          return new Response(
            [
              "data: {\"candidates\":[{\"content\":{\"role\":\"model\",\"parts\":[{\"text\":\"gemini summary\"}]},\"finishReason\":\"STOP\"}]}",
              "data: [DONE]",
              "",
            ].join("\n"),
            { status: 200 },
          );
        },
        now: new Date("2026-06-08T00:00:00.000Z"),
      }),
    );

    expect(result.authRequired).toBe(false);
    expect(result.backendKind).toBe("gemini_api");
    expect(result.profileId).toBe("gemini-api");
    expect(result.completion.text).toBe("gemini summary");
    expect(result.events.map((event) => event.kind)).toEqual([
      "probe_backend_run_started",
      "probe_backend_run_finished",
    ]);
    expect(result.events[0].backendKind).toBe("gemini_api");
    expect(JSON.stringify(result.events)).not.toContain("test-gemini-key");
    expect(seenUrls[0]).toContain("/v1beta/models/gemini-3.5-flash:streamGenerateContent");
  });

  test("rejects Gemini assignment when runner lacks Gemini backend capability", async () => {
    await expect(
      Effect.runPromise(
        runProbeBackendAssignment({
          runner: runner(["probe.run", PROBE_APPLE_FM_BACKEND_CAPABILITY]),
          proof: { ...proof(), assignmentId: "assignment_gemini_1" },
          assignment: geminiAssignment(),
          env: { GEMINI_API_KEY: "test-gemini-key" },
          fetch: async () => Response.json({}),
        }),
      ),
    ).rejects.toMatchObject({ _tag: "ProbeRunnerAuthorizationError" });
  });

  test("runs a Psionic Qwen assignment through the attach-only backend", async () => {
    const seenPaths: string[] = [];
    const result = await Effect.runPromise(
      runProbeBackendAssignment({
        runner: runner(["probe.run", PROBE_PSIONIC_QWEN_BACKEND_CAPABILITY]),
        proof: { ...proof(), assignmentId: "assignment_psionic_1" },
        assignment: psionicAssignment(),
        trustedBackendBaseUrl: "http://127.0.0.1:18080",
        fetch: async (input, init) => {
          const url = new URL(String(input));
          seenPaths.push(`${init?.method ?? "GET"} ${url.pathname}`);

          if (url.pathname === "/health") {
            return Response.json({
              ready: true,
              execution_engine: "psionic",
              supported_endpoints: ["/v1/chat/completions", "/v1/responses"],
            });
          }

          if (url.pathname === "/v1/models") {
            return Response.json({
              data: [
                {
                  id: "qwen3.5-0.8b",
                  artifact_digest: "afb707b6b8fac6e475acc42bc8380fc0b8d2e0e4190be5a969fbf62fcc897db5",
                },
                {
                  id: "qwen3.5-2b",
                  artifact_manifest_ref: "artifact.psionic.qwen35.2b.q8_0.manifest",
                },
              ],
            });
          }

          const body = JSON.parse(String(init?.body));
          expect(body.model).toBe("qwen3.5-2b");
          expect(body.messages[0].content).toBe("Summarize this local repo through Psionic Qwen.");

          return Response.json({
            choices: [
              {
                message: {
                  role: "assistant",
                  content: "psionic summary",
                },
                finish_reason: "stop",
              },
            ],
            usage: {
              prompt_tokens: 8,
              completion_tokens: 2,
              total_tokens: 10,
            },
          });
        },
        now: new Date("2026-06-10T00:00:00.000Z"),
      }),
    );

    expect(result.authRequired).toBe(false);
    expect(result.backendKind).toBe("psionic_qwen35");
    expect(result.profileId).toBe("psionic-qwen35-local");
    expect(result.completion.text).toBe("psionic summary");
    expect(result.events.map((event) => event.kind)).toEqual([
      "probe_backend_run_started",
      "probe_backend_run_finished",
    ]);
    expect(result.events[1].receipt).toMatchObject({
      kind: "probe_backend_transcript",
      backendKind: "psionic_qwen35",
      contentRedacted: true,
    });
    expect(seenPaths).toEqual(["GET /health", "GET /v1/models", "POST /v1/chat/completions"]);
  });

  test("refuses Psionic Qwen assignment with typed connector state when unattached", async () => {
    await expect(
      Effect.runPromise(
        runProbeBackendAssignment({
          runner: runner(["probe.run", PROBE_PSIONIC_QWEN_BACKEND_CAPABILITY]),
          proof: { ...proof(), assignmentId: "assignment_psionic_1" },
          assignment: psionicAssignment(),
          fetch: async () => {
            throw new Error("connection refused");
          },
          now: new Date("2026-06-10T00:00:00.000Z"),
        }),
      ),
    ).rejects.toMatchObject({
      _tag: "ProbeBackendAssignmentError",
      receipt: {
        kind: "probe_backend_availability",
        backendKind: "psionic_qwen35",
        ready: false,
        status: "unreachable",
        blockerRefs: ["blocker.psionic_qwen35.health_unreachable"],
        contentRedacted: true,
      },
      events: [
        { kind: "probe_backend_run_started", backendKind: "psionic_qwen35" },
        { kind: "probe_backend_run_failed", backendKind: "psionic_qwen35" },
      ],
    });
  });
});
