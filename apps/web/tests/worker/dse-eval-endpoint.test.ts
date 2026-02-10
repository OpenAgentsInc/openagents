import { createExecutionContext, env, waitOnExecutionContext } from "cloudflare:test";
import { describe, expect, it, vi } from "vitest";

import { Hash } from "@openagentsinc/dse";

import type { WorkerEnv } from "../../src/effuse-host/env";

declare module "cloudflare:test" {
  interface ProvidedEnv extends WorkerEnv {}
}

const state = vi.hoisted(() => ({
  // Seeded by the test at runtime.
  chunkText: "" as string,
  blobId: "" as string,

  listExamplesCalls: [] as any[],
  getArtifactCalls: [] as any[],
  putArtifactCalls: [] as any[],
  putEvalReportCalls: [] as any[],
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
      // dse.examples.listExamples
      if (isRecord(args) && typeof args.signatureId === "string" && typeof args.limit === "number") {
        state.listExamplesCalls.push({ ref, args });
        return {
          ok: true,
          examples: [
            {
              signatureId: args.signatureId,
              exampleId: "ex1",
              inputJson: {
                question: "What did we decide?",
                threadChunks: [{ id: state.blobId, hash: state.blobId, size: state.chunkText.length }],
              },
              expectedJson: { summary: "- We agreed to ship Phase 7 judge evals." },
              split: "holdout",
              tags: ["seed"],
              source: "test",
              meta: {
                blobs: [{ id: state.blobId, text: state.chunkText }],
              },
              createdAtMs: 1,
              updatedAtMs: 1,
            },
          ],
        };
      }

      // dse.artifacts.getArtifact
      if (isRecord(args) && typeof args.signatureId === "string" && typeof args.compiled_id === "string") {
        state.getArtifactCalls.push({ ref, args });
        return {
          ok: true,
          artifact: {
            format: "openagents.dse.compiled_artifact",
            formatVersion: 1,
            signatureId: args.signatureId,
            compiled_id: args.compiled_id,
            createdAt: "2026-02-10T00:00:00.000Z",
            hashes: {
              inputSchemaHash: "sha256:in",
              outputSchemaHash: "sha256:out",
              promptIrHash: "sha256:prompt",
              paramsHash: "sha256:params",
            },
            params: {
              paramsVersion: 1,
              strategy: { id: "direct.v1" },
              decode: { mode: "strict_json", maxRepairs: 0 },
              budgets: { maxTimeMs: 10_000, maxLmCalls: 10, maxOutputChars: 60_000 },
            },
            eval: { evalVersion: 1, kind: "unscored" },
            optimizer: { id: "test" },
            provenance: {},
          },
        };
      }

      throw new Error(`Unexpected Convex query in tests: ${String(ref?.name ?? ref)}`);
    });

  const mutation = (ref: any, args: any) =>
    Effect.sync(() => {
      // dse.evalReports.putReport
      if (isRecord(args) && typeof args.signatureId === "string" && typeof args.evalHash === "string") {
        state.putEvalReportCalls.push({ ref, args });
        return { ok: true, existed: false };
      }

      // dse.artifacts.putArtifact (pinned judge artifacts)
      if (
        isRecord(args) &&
        typeof args.signatureId === "string" &&
        typeof args.compiled_id === "string" &&
        "json" in args
      ) {
        state.putArtifactCalls.push({ ref, args });
        return { ok: true, existed: false };
      }

      throw new Error(`Unexpected Convex mutation in tests: ${String(ref?.name ?? ref)}`);
    });

  const action = (_ref: any, _args: any) => Effect.fail(new Error("convex.action not used in tests"));
  const subscribeQuery = (_ref: any, _args: any) =>
    Stream.fail(new Error("convex.subscribeQuery not used in Worker tests"));

  const connectionState = () => Effect.succeed(null);

  const ConvexServiceLive = Layer.succeed(actual.ConvexService, { query, mutation, action, subscribeQuery, connectionState });
  return { ...actual, ConvexServiceLive };
});

const { default: worker } = await import("../../src/effuse-host/worker");

describe("apps/web worker DSE eval endpoint (Phase 7)", () => {
  it("evaluates recap with judge reward and stores an eval report including pinned judge info", async () => {
    state.listExamplesCalls.length = 0;
    state.getArtifactCalls.length = 0;
    state.putArtifactCalls.length = 0;
    state.putEvalReportCalls.length = 0;

    state.chunkText = "User: please ship judge evals.\nAssistant: ok.";
    state.blobId = await Hash.sha256IdFromString(state.chunkText);

    const ORIGIN = "http://example.com";
    const request = new Request(`${ORIGIN}/api/dse/eval`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        signatureId: "@openagents/autopilot/canary/RecapThread.v1",
        compiled_id: "sha256:recap",
        split: "holdout",
        includeExampleDetails: true,
      }),
    });

    const envWithAi = Object.assign(Object.create(env as any), {
      AI: {
        run: async (_model: any, req: any) => {
          const all = JSON.stringify(req?.messages ?? []);
          const isJudge = all.includes("Task: score predSummary against expectedSummary");
          if (isJudge) {
            return {
              choices: [{ message: { content: "{\"score\":1,\"notes\":\"ok\"}" } }],
              usage: { prompt_tokens: 1, completion_tokens: 1 },
            };
          }
          return {
            choices: [{ message: { content: "{\"summary\":\"- We agreed to ship Phase 7 judge evals.\"}" } }],
            usage: { prompt_tokens: 1, completion_tokens: 1 },
          };
        },
      },
    }) as any;

    const ctx = createExecutionContext();
    const res = await worker.fetch(request, envWithAi, ctx);
    await waitOnExecutionContext(ctx);

    expect(res.status).toBe(200);
    const j = (await res.json()) as any;
    expect(j.ok).toBe(true);
    expect(j.signatureId).toBe("@openagents/autopilot/canary/RecapThread.v1");
    expect(j.compiled_id).toBe("sha256:recap");
    expect(j.split).toBe("holdout");
    expect(typeof j.evalHash).toBe("string");

    // Pinned judge artifact should be stored (replayability).
    expect(state.putArtifactCalls.length).toBeGreaterThanOrEqual(1);

    // Eval report stored in Convex should include judge metric details.
    expect(state.putEvalReportCalls.length).toBe(1);
    const storedJson = state.putEvalReportCalls[0]?.args?.json as any;
    expect(storedJson?.format).toBe("openagents.dse.eval_report");
    expect(Array.isArray(storedJson?.examples)).toBe(true);
    const ex0 = storedJson.examples?.[0];
    const judgeSignal = (ex0?.signals ?? []).find((s: any) => s?.metric?.kind === "judge");
    expect(judgeSignal?.metric?.judge?.signatureId).toBe("@openagents/autopilot/judge/ThreadSummaryQuality.v1");
    expect(typeof judgeSignal?.metric?.judge?.compiled_id).toBe("string");
  });
});
