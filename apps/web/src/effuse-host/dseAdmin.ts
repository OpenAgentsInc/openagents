import { Effect, Layer, Schema } from "effect";

import {
  BlobStore,
  Budget,
  CanonicalJson,
  CompileJob,
  CompiledArtifact,
  Eval,
  EvalCache,
  EvalDataset,
  EvalMetric,
  EvalReward,
  Lm,
  Receipt,
} from "@openagentsinc/dse";

import { signatures as dseCatalogSignatures } from "../../../autopilot-worker/src/dseCatalog";

import { api } from "../../convex/_generated/api";
import { AuthService } from "../effect/auth";
import { ConvexService } from "../effect/convex";
import { RequestContextService, makeServerRequestContext } from "../effect/requestContext";
import { TelemetryService } from "../effect/telemetry";

import { makeDseLmClientWithOpenRouterPrimary } from "./dse";
import { OA_REQUEST_ID_HEADER, formatRequestIdLogToken } from "./requestId";
import type { WorkerEnv } from "./env";
import { getWorkerRuntime } from "./runtime";

const MODEL_ID_CF = "@cf/openai/gpt-oss-120b";
const PRIMARY_MODEL_OPENROUTER = "moonshotai/kimi-k2.5";

const json = (body: unknown, init?: ResponseInit): Response =>
  new Response(JSON.stringify(body), {
    ...init,
    headers: {
      "content-type": "application/json; charset=utf-8",
      ...(init?.headers ?? {}),
    },
  });

const readJson = async (request: Request): Promise<any> => {
  try {
    return await request.json();
  } catch {
    return null;
  }
};

const mapStoredSplitToDseSplit = (split: unknown): EvalDataset.DatasetSplit | undefined => {
  if (split === "train") return "train";
  if (split === "holdout") return "holdout";
  // Legacy name: Stage 4 used "dev".
  if (split === "dev") return "holdout";
  if (split === "test") return "test";
  return undefined;
};

const findSignatureById = (signatureId: string) => {
  for (const sig of Object.values(dseCatalogSignatures) as any[]) {
    if (sig && typeof sig === "object" && String((sig as any).id ?? "") === signatureId) return sig as any;
  }
  return null;
};

const rewardExactJsonMatch = () => {
  const metric = EvalMetric.deterministic<any, any>({
    metricId: "exact_json_match.v1",
    metricVersion: 1,
    score: (pred, expected) => (CanonicalJson.canonicalJson(pred) === CanonicalJson.canonicalJson(expected) ? 1 : 0),
    notes: (pred, expected) =>
      CanonicalJson.canonicalJson(pred) === CanonicalJson.canonicalJson(expected) ? undefined : "mismatch",
  });

  return EvalReward.makeBundle({
    rewardId: "reward_exact_json_match.v1",
    rewardVersion: 1,
    signals: [
      EvalReward.signalFormatValidity({ weight: 0.2 }),
      EvalReward.signalMetric(metric, { weight: 0.8, signalId: "exact_json_match.signal.v1" }),
    ],
  });
};

const compileJobSpecForSignature = (input: { readonly signatureId: string; readonly datasetId: string }) => {
  const reward = rewardExactJsonMatch();
  const searchSpace: CompileJob.CompileSearchSpaceV1 = {};
  const optimizer: CompileJob.CompileOptimizerV1 = { id: "instruction_grid.v1" };

  const jobSpec: CompileJob.CompileJobSpecV1 = {
    format: "openagents.dse.compile_job",
    formatVersion: 1,
    signatureId: input.signatureId,
    datasetId: input.datasetId,
    metricId: reward.rewardId,
    searchSpace,
    optimizer,
  };

  return { jobSpec, reward };
};

const compileEnv = Layer.mergeAll(
  BlobStore.layerInMemory(),
  Budget.layerInMemory(),
  Receipt.layerNoop(),
  EvalCache.layerInMemory(),
);

const decodeCompileReportHoldoutReward = (reportJson: unknown): number => {
  const holdout = (reportJson as any)?.report?.holdoutReward;
  const n = typeof holdout === "number" ? holdout : NaN;
  if (!Number.isFinite(n)) throw new Error("invalid_compile_report");
  return n;
};

type PromoteBody = {
  readonly signatureId?: unknown;
  readonly compiled_id?: unknown;
  readonly minHoldoutDelta?: unknown;
  readonly requireHoldout?: unknown;
};

type CanaryStartBody = {
  readonly signatureId?: unknown;
  readonly canary_compiled_id?: unknown;
  readonly rolloutPct?: unknown;
  readonly minHoldoutDelta?: unknown;
  readonly requireHoldout?: unknown;
  readonly minSamples?: unknown;
  readonly maxErrorRate?: unknown;
  readonly reason?: unknown;
};

type CanaryStopBody = {
  readonly signatureId?: unknown;
  readonly reason?: unknown;
};

const runAuthedDseAdmin = async <A>(
  request: Request,
  env: WorkerEnv,
  program: Effect.Effect<A, unknown, ConvexService | AuthService | TelemetryService | RequestContextService>,
): Promise<Response> => {
  const requestId = request.headers.get(OA_REQUEST_ID_HEADER) ?? "missing";
  const { runtime } = getWorkerRuntime(env);

  const telemetryBase = runtime.runSync(
    Effect.gen(function* () {
      return yield* TelemetryService;
    }),
  );
  const requestTelemetry = telemetryBase.withFields({
    requestId,
    method: request.method,
    pathname: new URL(request.url).pathname,
  });

  const exit = await runtime.runPromiseExit(
    program.pipe(
      Effect.provideService(RequestContextService, makeServerRequestContext(request)),
      Effect.provideService(TelemetryService, requestTelemetry),
    ),
  );

  if (exit._tag === "Failure") {
    const msg = String((exit as any)?.cause ?? "dse_admin_failed");
    const status = msg.includes("unauthorized") ? 401 : msg.includes("invalid_input") ? 400 : 500;
    console.error(`[dse.admin] ${formatRequestIdLogToken(requestId)}`, msg);
    return json({ ok: false, error: msg }, { status, headers: { "cache-control": "no-store" } });
  }

  return json(exit.value, { status: 200, headers: { "cache-control": "no-store" } });
};

export const handleDseAdminRequest = async (
  request: Request,
  env: WorkerEnv,
  _ctx: ExecutionContext,
): Promise<Response | null> => {
  const url = new URL(request.url);
  if (!url.pathname.startsWith("/api/dse/")) return null;

  if (url.pathname === "/api/dse/promote") {
    if (request.method !== "POST") return new Response("Method not allowed", { status: 405 });
    if (!env.AI) return json({ ok: false, error: "ai_unbound" }, { status: 500, headers: { "cache-control": "no-store" } });
    const aiBinding = env.AI;

    const body = (await readJson(request)) as PromoteBody | null;
    const signatureId = typeof body?.signatureId === "string" ? body.signatureId : "";
    const compiled_id = typeof body?.compiled_id === "string" ? body.compiled_id : "";
    const minHoldoutDelta = typeof body?.minHoldoutDelta === "number" ? body.minHoldoutDelta : 0.05;
    const requireHoldout = body?.requireHoldout === undefined ? true : Boolean(body.requireHoldout);

    if (!signatureId || !compiled_id) {
      return json({ ok: false, error: "invalid_input" }, { status: 400, headers: { "cache-control": "no-store" } });
    }

    const signature = findSignatureById(signatureId);
    if (!signature) {
      return json({ ok: false, error: "unknown_signature" }, { status: 404, headers: { "cache-control": "no-store" } });
    }

    const program = Effect.gen(function* () {
      const telemetry = yield* TelemetryService;
      const t = telemetry.withNamespace("dse.promote");

      const auth = yield* AuthService;
      const session = yield* auth.getSession();
      if (!session.userId) return yield* Effect.fail(new Error("unauthorized"));

      const convex = yield* ConvexService;

      const exRes = yield* convex.query(api.dse.examples.listExamples, { signatureId, limit: 500 } as any);
      const raw = Array.isArray((exRes as any)?.examples) ? ((exRes as any).examples as any[]) : [];

      const decodeInput = Schema.decodeUnknownSync((signature as any).input);
      const decodeOutput = Schema.decodeUnknownSync((signature as any).output);

      const examples = raw.map((r) => ({
        exampleId: String(r?.exampleId ?? ""),
        input: decodeInput(r?.inputJson),
        expected: decodeOutput(r?.expectedJson),
        split: mapStoredSplitToDseSplit(r?.split),
        tags: Array.isArray(r?.tags) ? (r.tags as unknown[]).filter((t) => typeof t === "string") : undefined,
      }));

      const datasetId = `convex:dseExamples:${signatureId}`;
      const dataset = yield* EvalDataset.make({ datasetId, examples });
      const datasetHash = yield* EvalDataset.datasetHash(dataset);

      const holdout = EvalDataset.filter(dataset, { split: "holdout" });
      if (requireHoldout && holdout.examples.length === 0) {
        return yield* Effect.fail(new Error("holdout_required"));
      }

      const { jobSpec, reward } = compileJobSpecForSignature({ signatureId, datasetId });
      const jobHash = yield* CompileJob.compileJobHash(jobSpec);

      const reportRes = yield* convex.query(api.dse.compileReports.getReport, { signatureId, jobHash, datasetHash } as any);
      const report = (reportRes as any)?.report ?? null;
      if (!report) return yield* Effect.fail(new Error("compile_report_not_found"));
      if (String(report.compiled_id ?? "") !== compiled_id) return yield* Effect.fail(new Error("compiled_id_mismatch"));

      const candidateHoldoutReward = yield* Effect.try({
        try: () => decodeCompileReportHoldoutReward(report.json),
        catch: () => new Error("invalid_compile_report"),
      });

      const activeRes = yield* convex.query(api.dse.active.getActive, { signatureId } as any);
      const control_compiled_id = typeof (activeRes as any)?.compiled_id === "string" ? (activeRes as any).compiled_id : null;

      let baselineHoldoutReward = 0;
      if (control_compiled_id) {
        const baseArtifactRes = yield* convex.query(api.dse.artifacts.getArtifact, { signatureId, compiled_id: control_compiled_id } as any);
        const baseRaw = (baseArtifactRes as any)?.artifact ?? null;
        if (!baseRaw) return yield* Effect.fail(new Error("control_artifact_missing"));

        const baseArtifact = yield* Effect.try({
          try: () => Schema.decodeUnknownSync(CompiledArtifact.DseCompiledArtifactV1Schema)(baseRaw),
          catch: (cause) => new Error(`invalid_control_artifact: ${String(cause)}`),
        });

        const dseLmClient = makeDseLmClientWithOpenRouterPrimary({
          env: { OPENROUTER_API_KEY: env.OPENROUTER_API_KEY, AI: aiBinding },
          defaultModelIdCf: MODEL_ID_CF,
          primaryModelOpenRouter: PRIMARY_MODEL_OPENROUTER,
        });
        const evalRes = yield* Eval.evaluate({
          signature,
          artifact: baseArtifact,
          dataset: holdout,
          reward,
        }).pipe(Effect.provideService(Lm.LmClientService, dseLmClient), Effect.provide(compileEnv));

        baselineHoldoutReward = Number(evalRes.summary.reward ?? 0);
      }

      const delta = candidateHoldoutReward - baselineHoldoutReward;
      if (!(delta >= minHoldoutDelta)) {
        yield* t.event("promote.blocked", {
          signatureId,
          compiled_id,
          baselineHoldoutReward,
          candidateHoldoutReward,
          delta,
          minHoldoutDelta,
        });
        return yield* Effect.fail(new Error("insufficient_improvement"));
      }

      yield* convex.mutation(api.dse.active.setActive, {
        signatureId,
        compiled_id,
        reason: `promote baselineHoldout=${baselineHoldoutReward.toFixed(4)} candidateHoldout=${candidateHoldoutReward.toFixed(4)} delta=${delta.toFixed(4)} minDelta=${minHoldoutDelta.toFixed(4)} jobHash=${jobHash} datasetHash=${datasetHash}`,
      } as any);

      // Canary config becomes stale after a promotion; clear it best-effort.
      yield* convex.mutation(api.dse.canary.stopCanary, { signatureId, reason: "promoted" } as any).pipe(Effect.catchAll(() => Effect.void));

      yield* t.event("promote.ok", {
        signatureId,
        from: control_compiled_id,
        to: compiled_id,
        baselineHoldoutReward,
        candidateHoldoutReward,
        delta,
        jobHash,
        datasetHash,
      });

      return {
        ok: true,
        signatureId,
        from: control_compiled_id,
        to: compiled_id,
        jobHash,
        datasetHash,
        baselineHoldoutReward,
        candidateHoldoutReward,
        delta,
      } as const;
    });

    return runAuthedDseAdmin(request, env, program);
  }

  if (url.pathname === "/api/dse/canary/start") {
    if (request.method !== "POST") return new Response("Method not allowed", { status: 405 });
    if (!env.AI) return json({ ok: false, error: "ai_unbound" }, { status: 500, headers: { "cache-control": "no-store" } });
    const aiBinding = env.AI;

    const body = (await readJson(request)) as CanaryStartBody | null;
    const signatureId = typeof body?.signatureId === "string" ? body.signatureId : "";
    const canary_compiled_id = typeof body?.canary_compiled_id === "string" ? body.canary_compiled_id : "";
    const rolloutPct = typeof body?.rolloutPct === "number" ? body.rolloutPct : NaN;
    const minHoldoutDelta = typeof body?.minHoldoutDelta === "number" ? body.minHoldoutDelta : 0.05;
    const requireHoldout = body?.requireHoldout === undefined ? true : Boolean(body.requireHoldout);
    const minSamples = typeof body?.minSamples === "number" ? body.minSamples : undefined;
    const maxErrorRate = typeof body?.maxErrorRate === "number" ? body.maxErrorRate : undefined;
    const reason = typeof body?.reason === "string" ? body.reason : undefined;

    if (!signatureId || !canary_compiled_id || !Number.isFinite(rolloutPct)) {
      return json({ ok: false, error: "invalid_input" }, { status: 400, headers: { "cache-control": "no-store" } });
    }

    const signature = findSignatureById(signatureId);
    if (!signature) {
      return json({ ok: false, error: "unknown_signature" }, { status: 404, headers: { "cache-control": "no-store" } });
    }

    const program = Effect.gen(function* () {
      const telemetry = yield* TelemetryService;
      const t = telemetry.withNamespace("dse.canary");

      const auth = yield* AuthService;
      const session = yield* auth.getSession();
      if (!session.userId) return yield* Effect.fail(new Error("unauthorized"));

      const convex = yield* ConvexService;

      const exRes = yield* convex.query(api.dse.examples.listExamples, { signatureId, limit: 500 } as any);
      const raw = Array.isArray((exRes as any)?.examples) ? ((exRes as any).examples as any[]) : [];

      const decodeInput = Schema.decodeUnknownSync((signature as any).input);
      const decodeOutput = Schema.decodeUnknownSync((signature as any).output);

      const examples = raw.map((r) => ({
        exampleId: String(r?.exampleId ?? ""),
        input: decodeInput(r?.inputJson),
        expected: decodeOutput(r?.expectedJson),
        split: mapStoredSplitToDseSplit(r?.split),
      }));

      const datasetId = `convex:dseExamples:${signatureId}`;
      const dataset = yield* EvalDataset.make({ datasetId, examples });
      const datasetHash = yield* EvalDataset.datasetHash(dataset);

      const holdout = EvalDataset.filter(dataset, { split: "holdout" });
      if (requireHoldout && holdout.examples.length === 0) {
        return yield* Effect.fail(new Error("holdout_required"));
      }

      const { jobSpec, reward } = compileJobSpecForSignature({ signatureId, datasetId });
      const jobHash = yield* CompileJob.compileJobHash(jobSpec);

      const reportRes = yield* convex.query(api.dse.compileReports.getReport, { signatureId, jobHash, datasetHash } as any);
      const report = (reportRes as any)?.report ?? null;
      if (!report) return yield* Effect.fail(new Error("compile_report_not_found"));
      if (String(report.compiled_id ?? "") !== canary_compiled_id) return yield* Effect.fail(new Error("compiled_id_mismatch"));

      const candidateHoldoutReward = yield* Effect.try({
        try: () => decodeCompileReportHoldoutReward(report.json),
        catch: () => new Error("invalid_compile_report"),
      });

      const activeRes = yield* convex.query(api.dse.active.getActive, { signatureId } as any);
      const control_compiled_id = typeof (activeRes as any)?.compiled_id === "string" ? (activeRes as any).compiled_id : null;
      if (!control_compiled_id) return yield* Effect.fail(new Error("control_missing"));

      const baseArtifactRes = yield* convex.query(api.dse.artifacts.getArtifact, { signatureId, compiled_id: control_compiled_id } as any);
      const baseRaw = (baseArtifactRes as any)?.artifact ?? null;
      if (!baseRaw) return yield* Effect.fail(new Error("control_artifact_missing"));

      const baseArtifact = yield* Effect.try({
        try: () => Schema.decodeUnknownSync(CompiledArtifact.DseCompiledArtifactV1Schema)(baseRaw),
        catch: (cause) => new Error(`invalid_control_artifact: ${String(cause)}`),
      });

      const dseLmClient = makeDseLmClientWithOpenRouterPrimary({
        env: { OPENROUTER_API_KEY: env.OPENROUTER_API_KEY, AI: aiBinding },
        defaultModelIdCf: MODEL_ID_CF,
        primaryModelOpenRouter: PRIMARY_MODEL_OPENROUTER,
      });
      const evalRes = yield* Eval.evaluate({
        signature,
        artifact: baseArtifact,
        dataset: holdout,
        reward,
      }).pipe(Effect.provideService(Lm.LmClientService, dseLmClient), Effect.provide(compileEnv));

      const baselineHoldoutReward = Number(evalRes.summary.reward ?? 0);
      const delta = candidateHoldoutReward - baselineHoldoutReward;
      if (!(delta >= minHoldoutDelta)) {
        yield* t.event("canary.blocked", {
          signatureId,
          canary_compiled_id,
          baselineHoldoutReward,
          candidateHoldoutReward,
          delta,
          minHoldoutDelta,
        });
        return yield* Effect.fail(new Error("insufficient_improvement"));
      }

      const started = yield* convex.mutation(api.dse.canary.startCanary, {
        signatureId,
        canary_compiled_id,
        rolloutPct,
        ...(minSamples !== undefined ? { minSamples } : {}),
        ...(maxErrorRate !== undefined ? { maxErrorRate } : {}),
        ...(reason ? { reason } : {}),
      } as any);

      yield* t.event("canary.started", {
        signatureId,
        control_compiled_id: started.control_compiled_id,
        canary_compiled_id,
        rolloutPct,
        baselineHoldoutReward,
        candidateHoldoutReward,
        delta,
        jobHash,
        datasetHash,
      });

      return {
        ok: true,
        signatureId,
        control_compiled_id: started.control_compiled_id,
        canary_compiled_id,
        rolloutPct,
        jobHash,
        datasetHash,
        baselineHoldoutReward,
        candidateHoldoutReward,
        delta,
      } as const;
    });

    return runAuthedDseAdmin(request, env, program);
  }

  if (url.pathname === "/api/dse/canary/stop") {
    if (request.method !== "POST") return new Response("Method not allowed", { status: 405 });

    const body = (await readJson(request)) as CanaryStopBody | null;
    const signatureId = typeof body?.signatureId === "string" ? body.signatureId : "";
    const reason = typeof body?.reason === "string" ? body.reason : undefined;

    if (!signatureId) {
      return json({ ok: false, error: "invalid_input" }, { status: 400, headers: { "cache-control": "no-store" } });
    }

    const program = Effect.gen(function* () {
      const telemetry = yield* TelemetryService;
      const t = telemetry.withNamespace("dse.canary");

      const auth = yield* AuthService;
      const session = yield* auth.getSession();
      if (!session.userId) return yield* Effect.fail(new Error("unauthorized"));

      const convex = yield* ConvexService;
      const stopped = yield* convex.mutation(api.dse.canary.stopCanary, { signatureId, ...(reason ? { reason } : {}) } as any);
      yield* t.event("canary.stopped", { signatureId, existed: Boolean((stopped as any)?.existed), ...(reason ? { reason } : {}) });

      return { ok: true, signatureId, existed: Boolean((stopped as any)?.existed) } as const;
    });

    return runAuthedDseAdmin(request, env, program);
  }

  return null;
};
