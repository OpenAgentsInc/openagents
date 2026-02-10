import { createExecutionContext, env, waitOnExecutionContext } from "cloudflare:test";
import { describe, expect, it, vi } from "vitest";

import type { WorkerEnv } from "../../src/effuse-host/env";

declare module "cloudflare:test" {
  interface ProvidedEnv extends WorkerEnv {}
}

const state = vi.hoisted(() => ({
  listExamplesCalls: [] as any[],
  getReportCalls: [] as any[],
  putArtifactCalls: [] as any[],
  putReportCalls: [] as any[],
  storedReport: null as any,
}));

vi.mock("@workos/authkit-session", async (importOriginal) => {
  const actual = await importOriginal<any>();
  return {
    ...actual,
    createAuthService: () => ({
      withAuth: async (_request: Request) => ({
        auth: {
          user: { id: "admin-1", email: "admin@example.com", firstName: "A", lastName: "D" },
          sessionId: "sess-1",
          accessToken: "token-1",
        },
        refreshedSessionData: undefined,
      }),
      saveSession: async (_auth: unknown, _sessionData: string) => ({ headers: {} as Record<string, string> }),
    }),
  };
});

vi.mock("../../src/effect/convex", async (importOriginal) => {
  const actual = await importOriginal<any>();
  const { Effect, Layer, Stream } = await import("effect");

  const isRecord = (u: unknown): u is Record<string, any> => Boolean(u) && typeof u === "object";

  const query = (ref: any, args: any) =>
    Effect.sync(() => {
      if (isRecord(args) && typeof args.signatureId === "string" && typeof args.limit === "number") {
        state.listExamplesCalls.push({ ref, args });
        return {
          ok: true,
          examples: [
            {
              signatureId: args.signatureId,
              exampleId: "ex1",
              inputJson: {
                message: "What can you do?",
                blueprintHint: { userHandle: "Ada", agentName: "Autopilot" },
              },
              expectedJson: { action: "none" },
              split: "train",
              tags: ["seed"],
              source: "test",
              createdAtMs: 1,
              updatedAtMs: 1,
            },
            {
              signatureId: args.signatureId,
              exampleId: "ex2",
              inputJson: {
                message: "Hi",
                blueprintHint: { userHandle: "Ada", agentName: "Autopilot" },
              },
              expectedJson: { action: "none" },
              split: "train",
              tags: ["seed"],
              source: "test",
              createdAtMs: 2,
              updatedAtMs: 2,
            },
          ],
        };
      }

      if (
        isRecord(args) &&
        typeof args.signatureId === "string" &&
        typeof args.jobHash === "string" &&
        typeof args.datasetHash === "string"
      ) {
        state.getReportCalls.push({ ref, args });
        if (
          state.storedReport &&
          state.storedReport.signatureId === args.signatureId &&
          state.storedReport.jobHash === args.jobHash &&
          state.storedReport.datasetHash === args.datasetHash
        ) {
          return { ok: true, report: state.storedReport };
        }
        return { ok: true, report: null };
      }

      throw new Error(`Unexpected Convex query in tests: ${String(ref?.name ?? ref)}`);
    });

  const mutation = (ref: any, args: any) =>
    Effect.sync(() => {
      if (
        isRecord(args) &&
        typeof args.signatureId === "string" &&
        typeof args.compiled_id === "string" &&
        "json" in args &&
        !("jobHash" in args)
      ) {
        state.putArtifactCalls.push({ ref, args });
        return { ok: true, existed: false };
      }

      if (
        isRecord(args) &&
        typeof args.signatureId === "string" &&
        typeof args.jobHash === "string" &&
        typeof args.datasetHash === "string" &&
        typeof args.compiled_id === "string"
      ) {
        state.putReportCalls.push({ ref, args });
        state.storedReport = {
          signatureId: args.signatureId,
          jobHash: args.jobHash,
          datasetId: args.datasetId,
          datasetHash: args.datasetHash,
          compiled_id: args.compiled_id,
          json: args.json,
          createdAtMs: 123,
        };
        return { ok: true, existed: false };
      }

      throw new Error(`Unexpected Convex mutation in tests: ${String(ref?.name ?? ref)}`);
    });

  const action = (_ref: any, _args: any) => Effect.fail(new Error("convex.action not used in tests"));
  const subscribeQuery = (_ref: any, _args: any) =>
    Stream.fail(new Error("convex.subscribeQuery not used in Worker tests"));

  const ConvexServiceLive = Layer.succeed(actual.ConvexService, { query, mutation, action, subscribeQuery });
  return { ...actual, ConvexServiceLive };
});

const { default: worker } = await import("../../src/effuse-host/worker");

describe("apps/web worker DSE compile endpoint (Stage 5)", () => {
  it("runs Compile.compile, stores artifact + compile report, and is idempotent by (jobHash,datasetHash)", async () => {
    state.listExamplesCalls.length = 0;
    state.getReportCalls.length = 0;
    state.putArtifactCalls.length = 0;
    state.putReportCalls.length = 0;
    state.storedReport = null;

    const ORIGIN = "http://example.com";

    const makeRequest = () =>
      new Request(`${ORIGIN}/api/dse/compile`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ signatureId: "@openagents/autopilot/blueprint/SelectTool.v1" }),
      });

    const envWithAi = Object.assign(Object.create(env as any), {
      AI: {
        run: async () => ({
          choices: [{ message: { content: "{\"action\":\"none\"}" } }],
          usage: { prompt_tokens: 1, completion_tokens: 1 },
        }),
      },
    }) as any;

    // First run: writes artifact + report.
    const ctx1 = createExecutionContext();
    const res1 = await worker.fetch(makeRequest(), envWithAi, ctx1);
    await waitOnExecutionContext(ctx1);
    expect(res1.status).toBe(200);
    const j1 = (await res1.json()) as any;
    expect(j1.ok).toBe(true);
    expect(j1.existed).toBe(false);
    expect(typeof j1.jobHash).toBe("string");
    expect(typeof j1.datasetHash).toBe("string");
    expect(typeof j1.compiled_id).toBe("string");

    expect(state.putArtifactCalls.length).toBe(1);
    expect(state.putReportCalls.length).toBe(1);

    // Phase 4: compile must be non-trivial for SelectTool (multiple instruction variants).
    const storedJob = state.putReportCalls[0]?.args?.json?.job as any;
    expect(storedJob?.signatureId).toBe("@openagents/autopilot/blueprint/SelectTool.v1");
    expect(Array.isArray(storedJob?.searchSpace?.instructionVariants)).toBe(true);
    expect(storedJob.searchSpace.instructionVariants.length).toBeGreaterThanOrEqual(2);

    const storedReport = state.putReportCalls[0]?.args?.json?.report as any;
    expect(Array.isArray(storedReport?.evaluatedCandidates)).toBe(true);
    expect(storedReport.evaluatedCandidates.length).toBeGreaterThanOrEqual(2);

    // Artifact compiled_id should match paramsHash (compile invariant).
    const storedArtifact = state.putArtifactCalls[0]?.args?.json as any;
    expect(storedArtifact.compiled_id).toBe(storedArtifact.hashes?.paramsHash);

    // Second run: served from cached report (no writes).
    const ctx2 = createExecutionContext();
    const res2 = await worker.fetch(makeRequest(), envWithAi, ctx2);
    await waitOnExecutionContext(ctx2);
    expect(res2.status).toBe(200);
    const j2 = (await res2.json()) as any;
    expect(j2.ok).toBe(true);
    expect(j2.existed).toBe(true);

    expect(j2.jobHash).toBe(j1.jobHash);
    expect(j2.datasetHash).toBe(j1.datasetHash);
    expect(j2.compiled_id).toBe(j1.compiled_id);

    expect(state.putArtifactCalls.length).toBe(1);
    expect(state.putReportCalls.length).toBe(1);
  });
});
