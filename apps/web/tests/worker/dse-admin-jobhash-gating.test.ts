import { createExecutionContext, env, waitOnExecutionContext } from "cloudflare:test";
import { describe, expect, it, vi } from "vitest";

import { Effect } from "effect";

import { CompileJob, EvalDataset } from "@openagentsinc/dse";

import {
  SELECT_TOOL_SIGNATURE_ID,
  compileJobForSignature,
  convexDatasetIdForExamples,
} from "../../src/effuse-host/dseJobs";

import type { WorkerEnv } from "../../src/effuse-host/env";

declare module "cloudflare:test" {
  interface ProvidedEnv extends WorkerEnv {}
}

const state = vi.hoisted(() => ({
  expectedJobHash: "" as string,
  listExamplesCalls: [] as any[],
  getReportCalls: [] as any[],
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
              exampleId: "train1",
              inputJson: { message: "What can you do?", blueprintHint: { userHandle: "Ada", agentName: "Autopilot" } },
              expectedJson: { action: "none" },
              split: "train",
              tags: ["seed"],
              source: "test",
              createdAtMs: 1,
              updatedAtMs: 1,
            },
            {
              signatureId: args.signatureId,
              exampleId: "holdout1",
              inputJson: { message: "Hi", blueprintHint: { userHandle: "Ada", agentName: "Autopilot" } },
              expectedJson: { action: "none" },
              split: "holdout",
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
        if (args.jobHash !== state.expectedJobHash) return { ok: true, report: null };
        return {
          ok: true,
          report: {
            signatureId: args.signatureId,
            jobHash: args.jobHash,
            datasetId: convexDatasetIdForExamples(args.signatureId),
            datasetHash: args.datasetHash,
            compiled_id: "compiled_other",
            json: {
              format: "openagents.dse.compile_report",
              formatVersion: 1,
              job: { signatureId: args.signatureId, datasetId: convexDatasetIdForExamples(args.signatureId) },
              report: { holdoutReward: 0.5 },
            },
            createdAtMs: 123,
          },
        };
      }

      throw new Error(`Unexpected Convex query in tests: ${String(ref?.name ?? ref)}`);
    });

  const mutation = (_ref: any, _args: any) => Effect.fail(new Error("convex.mutation not used in tests"));
  const action = (_ref: any, _args: any) => Effect.fail(new Error("convex.action not used in tests"));
  const subscribeQuery = (_ref: any, _args: any) =>
    Stream.fail(new Error("convex.subscribeQuery not used in Worker tests"));

  const ConvexServiceLive = Layer.succeed(actual.ConvexService, { query, mutation, action, subscribeQuery });
  return { ...actual, ConvexServiceLive };
});

const { default: worker } = await import("../../src/effuse-host/worker");

describe("apps/web worker DSE admin gating (Phase 4)", () => {
  it("promote uses shared jobHash and rejects mismatched compiled_id", async () => {
    state.listExamplesCalls.length = 0;
    state.getReportCalls.length = 0;

    const signatureId = SELECT_TOOL_SIGNATURE_ID;
    const datasetId = convexDatasetIdForExamples(signatureId);
    const { jobSpec } = compileJobForSignature({ signatureId, datasetId });
    state.expectedJobHash = await Effect.runPromise(CompileJob.compileJobHash(jobSpec));

    const ORIGIN = "http://example.com";
    const req = new Request(`${ORIGIN}/api/dse/promote`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ signatureId, compiled_id: "compiled_requested", minHoldoutDelta: 0 }),
    });

    const envWithAi = Object.assign(Object.create(env as any), {
      AI: {
        run: async () => ({
          choices: [{ message: { content: "{\"action\":\"none\"}" } }],
          usage: { prompt_tokens: 1, completion_tokens: 1 },
        }),
      },
    }) as any;

    const ctx = createExecutionContext();
    const res = await worker.fetch(req, envWithAi, ctx);
    await waitOnExecutionContext(ctx);

    expect(res.status).toBe(500);
    const j = (await res.json()) as any;
    expect(j.ok).toBe(false);
    expect(String(j.error)).toContain("compiled_id_mismatch");

    expect(state.getReportCalls.length).toBe(1);
    expect(state.getReportCalls[0]?.args?.jobHash).toBe(state.expectedJobHash);

    // Dataset hash must include tags (must align with /api/dse/compile).
    const expectedDataset = await Effect.runPromise(
      EvalDataset.make({
        datasetId,
        examples: [
          {
            exampleId: "holdout1",
            input: { message: "Hi", blueprintHint: { userHandle: "Ada", agentName: "Autopilot" } },
            expected: { action: "none" },
            split: "holdout",
            tags: ["seed"],
          },
          {
            exampleId: "train1",
            input: { message: "What can you do?", blueprintHint: { userHandle: "Ada", agentName: "Autopilot" } },
            expected: { action: "none" },
            split: "train",
            tags: ["seed"],
          },
        ],
      }),
    );
    const expectedDatasetHash = await Effect.runPromise(EvalDataset.datasetHash(expectedDataset));
    expect(state.getReportCalls[0]?.args?.datasetHash).toBe(expectedDatasetHash);
  });

  it("canary/start uses shared jobHash and rejects mismatched canary_compiled_id", async () => {
    state.listExamplesCalls.length = 0;
    state.getReportCalls.length = 0;

    const signatureId = SELECT_TOOL_SIGNATURE_ID;
    const datasetId = convexDatasetIdForExamples(signatureId);
    const { jobSpec } = compileJobForSignature({ signatureId, datasetId });
    state.expectedJobHash = await Effect.runPromise(CompileJob.compileJobHash(jobSpec));

    const ORIGIN = "http://example.com";
    const req = new Request(`${ORIGIN}/api/dse/canary/start`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ signatureId, canary_compiled_id: "compiled_requested", rolloutPct: 10, minHoldoutDelta: 0 }),
    });

    const envWithAi = Object.assign(Object.create(env as any), {
      AI: {
        run: async () => ({
          choices: [{ message: { content: "{\"action\":\"none\"}" } }],
          usage: { prompt_tokens: 1, completion_tokens: 1 },
        }),
      },
    }) as any;

    const ctx = createExecutionContext();
    const res = await worker.fetch(req, envWithAi, ctx);
    await waitOnExecutionContext(ctx);

    expect(res.status).toBe(500);
    const j = (await res.json()) as any;
    expect(j.ok).toBe(false);
    expect(String(j.error)).toContain("compiled_id_mismatch");

    expect(state.getReportCalls.length).toBe(1);
    expect(state.getReportCalls[0]?.args?.jobHash).toBe(state.expectedJobHash);

    // Canary gating must look up the compile report by the same datasetHash as /api/dse/compile.
    // (Historically canary/start omitted tags and computed a mismatched datasetHash, causing compile_report_not_found.)
    const expectedDataset = await Effect.runPromise(
      EvalDataset.make({
        datasetId,
        examples: [
          {
            exampleId: "holdout1",
            input: { message: "Hi", blueprintHint: { userHandle: "Ada", agentName: "Autopilot" } },
            expected: { action: "none" },
            split: "holdout",
            tags: ["seed"],
          },
          {
            exampleId: "train1",
            input: { message: "What can you do?", blueprintHint: { userHandle: "Ada", agentName: "Autopilot" } },
            expected: { action: "none" },
            split: "train",
            tags: ["seed"],
          },
        ],
      }),
    );
    const expectedDatasetHash = await Effect.runPromise(EvalDataset.datasetHash(expectedDataset));
    expect(state.getReportCalls[0]?.args?.datasetHash).toBe(expectedDatasetHash);
  });

  it("keeps SelectTool compile non-trivial (instruction variants) by definition", () => {
    const signatureId = SELECT_TOOL_SIGNATURE_ID;
    const datasetId = convexDatasetIdForExamples(signatureId);
    const { jobSpec } = compileJobForSignature({ signatureId, datasetId });
    const variants = (jobSpec.searchSpace as any)?.instructionVariants;
    expect(Array.isArray(variants)).toBe(true);
    expect(variants.length).toBeGreaterThanOrEqual(2);
  });
});
