import { makeReferenceAdapter } from "@openagentsinc/agent-harness-contract";
import { Effect, Exit, Stream } from "effect";
import { describe, expect, test } from "vite-plus/test";

import {
  makeOpenAgentsCloudHarnessEnvironment,
  startHarnessInEnvironment,
} from "./contract.js";
import { makeOpenAgentsCloudHarnessEnvironmentRunner } from "./openagents-cloud-runner.js";
import { runOpenAgentsCloudHarness } from "./run-openagents-cloud-harness.js";

const source = {
  lane: "managed_cloud",
  adapterKind: "openagents_native",
  surface: "server",
} as const;

const sessionProjection = (input: {
  readonly id: string;
  readonly state: "queued" | "running" | "completed" | "failed" | "cancelled";
  readonly placement_ref?: string | null;
  readonly agent_computer_state?: string;
  readonly artifact_ref?: string | null;
}) => ({
  object: "cloud.coding_session",
  product_object: "agent.computer_session",
  id: input.id,
  lane: "cloud-gcp",
  adapter: "codex",
  repo_ref: "OpenAgentsInc/openagents",
  repo_trust_tier: "private",
  timeout_seconds: 1800,
  state: input.state,
  placement_ref: input.placement_ref ?? "placement.cloud-coding.test",
  lease_refs: ["lease.test.01"],
  work_context_ref: "workctx.test.01",
  agent_computer_ref: "agentcomputer.test.01",
  agent_computer_state: input.agent_computer_state ?? "active",
  lifecycle_receipt_refs: [],
  resource_usage_receipt_refs: [],
  artifact_ref: input.artifact_ref ?? null,
  created_at: "2026-07-24T00:00:00.000Z",
});

const jsonResponse = (status: number, body: unknown): Response =>
  new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json" },
  });

describe("openagents_cloud HarnessEnvironmentRunner", () => {
  test("happy path: launch + poll maps to harness turn events", async () => {
    const calls: Array<{ method: string; url: string }> = [];
    let getCount = 0;
    const fetchImpl: typeof fetch = async (input, init) => {
      const url = String(input);
      const method = init?.method ?? "GET";
      calls.push({ method, url });
      if (method === "POST") {
        const body = JSON.parse(String(init?.body ?? "{}")) as {
          repoRef?: string;
          objective?: string;
        };
        expect(body.repoRef).toBe("OpenAgentsInc/openagents");
        expect(body.objective).toBe("Implement HE-01");
        return jsonResponse(
          200,
          sessionProjection({ id: "ccs_happy", state: "queued" }),
        );
      }
      getCount += 1;
      if (getCount === 1) {
        return jsonResponse(
          200,
          sessionProjection({ id: "ccs_happy", state: "running" }),
        );
      }
      return jsonResponse(
        200,
        sessionProjection({
          id: "ccs_happy",
          state: "completed",
          artifact_ref: "artifact.test.01",
          agent_computer_state: "reclaimed",
        }),
      );
    };

    const environment = makeOpenAgentsCloudHarnessEnvironment("https://openagents.com");
    const runner = makeOpenAgentsCloudHarnessEnvironmentRunner({
      bearerToken: "test-token",
      repoRef: "OpenAgentsInc/openagents",
      fetch: fetchImpl,
      pollIntervalMs: 1,
      sleep: () => Effect.void,
    });
    const harness = makeReferenceAdapter({ harnessId: "cloud-runner-test" });

    const session = await Effect.runPromise(
      startHarnessInEnvironment({
        environment,
        harness,
        options: { sessionId: "session-happy", source },
        runners: { openagentsCloud: runner },
      }),
    );

    const turn = await Effect.runPromise(
      session.promptTurn({ turnId: "turn-happy", prompt: "Implement HE-01" }),
    );
    const events = await Effect.runPromise(Stream.runCollect(turn.events));
    const done = await Effect.runPromise(turn.done);

    expect(events[0]?.kind).toBe("turn.started");
    expect(events.some((event) => event.kind === "text.delta")).toBe(true);
    expect(events.at(-1)?.kind).toBe("turn.finished");
    expect(done.finishReason).toBe("stop");
    expect(calls.some((call) => call.method === "POST")).toBe(true);
    expect(calls.some((call) => call.method === "GET")).toBe(true);
  });

  test("typed launch failure classes surface as HarnessTurnError", async () => {
    const cases = [
      { status: 401, body: { error: "unauthorized" }, failureClass: "unauthorized" },
      {
        status: 402,
        body: { error: "insufficient_credit", reason: "insufficient_credit" },
        failureClass: "insufficient_credit",
      },
      {
        status: 429,
        body: { error: "rate_limited" },
        failureClass: "rate_limited",
      },
      {
        status: 404,
        body: { error: "cloud_coding_sessions_disabled" },
        failureClass: "cloud_coding_sessions_disabled",
      },
    ] as const;

    for (const testCase of cases) {
      const runner = makeOpenAgentsCloudHarnessEnvironmentRunner({
        bearerToken: "test-token",
        repoRef: "OpenAgentsInc/openagents",
        fetch: async () => jsonResponse(testCase.status, testCase.body),
        sleep: () => Effect.void,
      });
      const harness = makeReferenceAdapter({ harnessId: "cloud-fail" });
      const session = await Effect.runPromise(
        startHarnessInEnvironment({
          environment: makeOpenAgentsCloudHarnessEnvironment("https://openagents.com"),
          harness,
          options: { sessionId: `session-${testCase.failureClass}`, source },
          runners: { openagentsCloud: runner },
        }),
      );
      const error = await Effect.runPromise(
        session
          .promptTurn({ turnId: `turn-${testCase.failureClass}`, prompt: "fail" })
          .pipe(Effect.flip),
      );
      expect(error).toMatchObject({
        _tag: "AgentHarness.TurnError",
        failureClass: testCase.failureClass,
      });
    }
  });

  test("start fails closed without bearer token or repo ref", async () => {
    const harness = makeReferenceAdapter({ harnessId: "cloud-config" });
    const missingToken = await Effect.runPromiseExit(
      startHarnessInEnvironment({
        environment: makeOpenAgentsCloudHarnessEnvironment("https://openagents.com"),
        harness,
        options: { sessionId: "session-missing-token", source },
        runners: {
          openagentsCloud: makeOpenAgentsCloudHarnessEnvironmentRunner({
            bearerToken: " ",
            repoRef: "OpenAgentsInc/openagents",
          }),
        },
      }),
    );
    expect(Exit.isFailure(missingToken)).toBe(true);

    const missingRepo = await Effect.runPromise(
      startHarnessInEnvironment({
        environment: makeOpenAgentsCloudHarnessEnvironment("https://openagents.com"),
        harness,
        options: { sessionId: "session-missing-repo", source },
        runners: {
          openagentsCloud: makeOpenAgentsCloudHarnessEnvironmentRunner({
            bearerToken: "token",
            repoRef: "",
          }),
        },
      }).pipe(Effect.flip),
    );
    expect(missingRepo).toMatchObject({
      _tag: "AgentHarness.EnvironmentError",
      failureClass: "missing_repo_ref",
    });
  });

  test("non-test caller constructs the typed environment and reaches launch", async () => {
    let sawLaunch = false;
    const result = await Effect.runPromise(
      runOpenAgentsCloudHarness({
        controlPlaneBaseUrl: "https://openagents.com/",
        bearerToken: "caller-token",
        repoRef: "OpenAgentsInc/openagents",
        objective: "Prove HE-01 production caller",
        sessionId: "session-caller",
        turnId: "turn-caller",
        fetch: async (_input, init) => {
          if ((init?.method ?? "GET") === "POST") {
            sawLaunch = true;
            return jsonResponse(
              200,
              sessionProjection({
                id: "ccs_caller",
                state: "completed",
                artifact_ref: "artifact.caller.01",
              }),
            );
          }
          return jsonResponse(404, { error: "not_found" });
        },
      }),
    );

    expect(sawLaunch).toBe(true);
    expect(result.sessionId).toBe("session-caller");
    expect(result.finishReason).toBe("stop");
    expect(result.eventKinds[0]).toBe("turn.started");
    expect(result.eventKinds.at(-1)).toBe("turn.finished");
  });

  test("keeps desktop_local as the default when no cloud runner is selected", async () => {
    const harness = makeReferenceAdapter({
      harnessId: "desktop-unchanged",
      scriptWords: ["local"],
    });
    const session = await Effect.runPromise(
      startHarnessInEnvironment({
        harness,
        options: { sessionId: "session-desktop", source: { lane: "ai_sdk_core" } },
      }),
    );
    expect(session.sessionId).toBe("session-desktop");
  });
});
