import { Effect, Layer, Schema } from "effect";

import {
  BlobStore,
  Budget,
  Hashes,
  CompileJob,
  CompiledArtifact,
  Eval,
  EvalCache,
  EvalDataset,
  Lm,
  Predict,
  Receipt,
  TraceMining,
  VarSpace,
} from "@openagentsinc/dse";

import { signatures as dseCatalogSignatures } from "../../../autopilot-worker/src/dseCatalog";

import { api } from "../../convex/_generated/api";
import { AuthService } from "../effect/auth";
import { ConvexService } from "../effect/convex";
import { RequestContextService, makeServerRequestContext } from "../effect/requestContext";
import { TelemetryService } from "../effect/telemetry";

import { layerDsePredictEnvForAutopilotRun, makeDseLmClientWithOpenRouterPrimary } from "./dse";
import { DSE_OPS_ADMIN_SUBJECT, isDseAdminSecretAuthorized, withDseAdminSecretServices } from "./dseAdminSecret";
import { collectDatasetBlobsFromExamples, seedBlobStoreFromDatasetBlobs } from "./dseDatasetBlobs";
import { compileJobForSignature, convexDatasetIdForExamples } from "./dseJobs";
import { PINNED_DSE_ARTIFACTS } from "./dsePinnedArtifacts";
import { OA_REQUEST_ID_HEADER, formatRequestIdLogToken } from "./requestId";
import type { WorkerEnv } from "./env";
import { getWorkerRuntime } from "./runtime";
import type {
  DseGetActiveResult,
  DseGetArtifactResult,
  DseGetReportResult,
  DseListExamplesResult,
  DseStopCanaryResult,
} from "./convexTypes";
import type { DseSignature } from "@openagentsinc/dse";

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

const findSignatureById = (signatureId: string): DseSignature<unknown, unknown> | null => {
  for (const sig of Object.values(dseCatalogSignatures)) {
    if (sig && typeof sig === "object" && "id" in sig && String(sig.id) === signatureId)
      return sig as DseSignature<unknown, unknown>;
  }
  return null;
};

const compileEnv = Layer.mergeAll(
  BlobStore.layerInMemory(),
  Budget.layerInMemory(),
  Receipt.layerNoop(),
  EvalCache.layerInMemory(),
  VarSpace.layerInMemory(),
);

const decodeCompileReportHoldoutReward = (reportJson: unknown): number => {
  const report = reportJson as { report?: { holdoutReward?: number } };
  const holdout = report?.report?.holdoutReward;
  const n = typeof holdout === "number" ? holdout : NaN;
  if (!Number.isFinite(n)) throw new Error("invalid_compile_report");
  return n;
};

const normalizeTagArray = (raw: unknown, maxItems: number): Array<string> => {
  if (!Array.isArray(raw)) return [];
  const out: string[] = [];
  for (const v of raw) {
    if (typeof v !== "string") continue;
    const s = v.trim();
    if (!s) continue;
    out.push(s.slice(0, 120));
    if (out.length >= maxItems) break;
  }
  return out;
};

const parseJsonlRows = (jsonl: string, maxLines: number): Array<unknown> => {
  const lines = jsonl
    .split("\n")
    .map((l) => l.trim())
    .filter((l) => l.length > 0);
  if (lines.length > maxLines) throw new Error("invalid_input");

  const out: unknown[] = [];
  for (const line of lines) {
    out.push(JSON.parse(line));
  }
  return out;
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

type TraceExportBody = {
  readonly receiptId?: unknown;
  readonly exampleId?: unknown;
  readonly split?: unknown;
  readonly tags?: unknown;
  readonly dryRun?: unknown;
};

type EvalRunBody = {
  readonly signatureId?: unknown;
  readonly compiled_id?: unknown;
  readonly split?: unknown;
  readonly limit?: unknown;
  readonly includeExampleDetails?: unknown;
  readonly opsRunId?: unknown;
};

type ExamplesImportBody = {
  readonly signatureId?: unknown;
  /** JSONL string of rows: { exampleId, inputJson, expectedJson, split?, tags?, meta? } */
  readonly jsonl?: unknown;
  /** Alternative to jsonl: direct JSON array of rows. */
  readonly examples?: unknown;
  /** Optional ops run id; when provided, import emits dseOpsRunEvents via /dseOpsRuns.appendEvent. */
  readonly opsRunId?: unknown;
  /** Optional source string attached to imported examples. */
  readonly source?: unknown;
  /** Optional extra tags appended to each row. */
  readonly tagsAppend?: unknown;
};

type OpsRunStartBody = {
  readonly runId?: unknown;
  readonly commitSha?: unknown;
  readonly baseUrl?: unknown;
  readonly signatureIds?: unknown;
  readonly notes?: unknown;
  readonly links?: unknown;
};

type OpsRunEventBody = {
  readonly runId?: unknown;
  readonly level?: unknown;
  readonly phase?: unknown;
  readonly message?: unknown;
  readonly json?: unknown;
  readonly tsMs?: unknown;
};

type OpsRunFinishBody = {
  readonly runId?: unknown;
  readonly status?: unknown;
  readonly summaryJson?: unknown;
};

type ExercisePredictBody = {
  readonly signatureId?: unknown;
  readonly threadId?: unknown;
  readonly count?: unknown;
  readonly split?: unknown;
  readonly limit?: unknown;
};

type DseAuthMode = "session_only" | "session_or_admin_secret" | "admin_secret_only";

const runAuthedDseAdmin = async <A>(
  request: Request,
  env: WorkerEnv,
  program: Effect.Effect<A, unknown, ConvexService | AuthService | TelemetryService | RequestContextService>,
  authMode: DseAuthMode = "session_only",
): Promise<Response> => {
  const requestId = request.headers.get(OA_REQUEST_ID_HEADER) ?? "missing";
  const { runtime, config } = getWorkerRuntime(env);

  const adminSecretOk = isDseAdminSecretAuthorized(request, env);
  if (authMode === "admin_secret_only" && !adminSecretOk) {
    return json({ ok: false, error: "unauthorized" }, { status: 401, headers: { "cache-control": "no-store" } });
  }

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
    (authMode !== "session_only" && adminSecretOk ? withDseAdminSecretServices(env, config.convexUrl, program) : program).pipe(
      Effect.provideService(RequestContextService, makeServerRequestContext(request)),
      Effect.provideService(TelemetryService, requestTelemetry),
    ),
  );

  if (exit._tag === "Failure") {
    const msg = String(exit._tag === "Failure" ? exit.cause : "dse_admin_failed");
    const status = msg.includes("unauthorized") ? 401 : msg.includes("invalid_input") ? 400 : 500;
    console.error(`[dse.admin] ${formatRequestIdLogToken(requestId)}`, msg);
    return json({ ok: false, error: msg }, { status, headers: { "cache-control": "no-store" } });
  }

  return json(exit.value, { status: 200, headers: { "cache-control": "no-store" } });
};

const runAuthedDseAdminRaw = async (
  request: Request,
  env: WorkerEnv,
  program: Effect.Effect<Response, unknown, ConvexService | AuthService | TelemetryService | RequestContextService>,
  authMode: DseAuthMode = "session_only",
): Promise<Response> => {
  const requestId = request.headers.get(OA_REQUEST_ID_HEADER) ?? "missing";
  const { runtime, config } = getWorkerRuntime(env);

  const adminSecretOk = isDseAdminSecretAuthorized(request, env);
  if (authMode === "admin_secret_only" && !adminSecretOk) {
    return json({ ok: false, error: "unauthorized" }, { status: 401, headers: { "cache-control": "no-store" } });
  }

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
    (authMode !== "session_only" && adminSecretOk ? withDseAdminSecretServices(env, config.convexUrl, program) : program).pipe(
      Effect.provideService(RequestContextService, makeServerRequestContext(request)),
      Effect.provideService(TelemetryService, requestTelemetry),
    ),
  );

  if (exit._tag === "Failure") {
    const msg = String(exit._tag === "Failure" ? exit.cause : "dse_admin_failed");
    const status = msg.includes("unauthorized") ? 401 : msg.includes("invalid_input") ? 400 : 500;
    console.error(`[dse.admin] ${formatRequestIdLogToken(requestId)}`, msg);
    return json({ ok: false, error: msg }, { status, headers: { "cache-control": "no-store" } });
  }

  return exit.value;
};

export const handleDseAdminRequest = async (
  request: Request,
  env: WorkerEnv,
  _ctx: ExecutionContext,
): Promise<Response | null> => {
  const url = new URL(request.url);
  if (!url.pathname.startsWith("/api/dse/")) return null;

  if (url.pathname === "/api/dse/eval") {
    if (request.method !== "POST") return new Response("Method not allowed", { status: 405 });
    if (!env.AI) return json({ ok: false, error: "ai_unbound" }, { status: 500, headers: { "cache-control": "no-store" } });
    const aiBinding = env.AI;

    const body = (await readJson(request)) as EvalRunBody | null;
    const signatureId = typeof body?.signatureId === "string" ? body.signatureId : "";
    const compiled_id = typeof body?.compiled_id === "string" ? body.compiled_id : "";

    const splitRaw = typeof body?.split === "string" ? body.split : "holdout";
    const split =
      splitRaw === "train" || splitRaw === "dev" || splitRaw === "holdout" || splitRaw === "test" ? splitRaw : "holdout";

    const limitRaw = typeof body?.limit === "number" ? body.limit : 500;
    const limit = Number.isFinite(limitRaw) ? Math.max(1, Math.min(500, Math.floor(limitRaw))) : 500;

    const includeExampleDetails = body?.includeExampleDetails === undefined ? true : Boolean(body.includeExampleDetails);

    const opsRunId = typeof body?.opsRunId === "string" ? body.opsRunId.trim() : "";

    if (!signatureId || !compiled_id) {
      return json(
        { ok: false, error: "invalid_input", message: "Expected body: { signatureId: string, compiled_id: string, split?: string }" },
        { status: 400, headers: { "cache-control": "no-store" } },
      );
    }

    const signature = findSignatureById(signatureId);
    if (!signature) {
      return json({ ok: false, error: "unknown_signature" }, { status: 404, headers: { "cache-control": "no-store" } });
    }

    const program = Effect.gen(function* () {
      const telemetry = yield* TelemetryService;
      const t = telemetry.withNamespace("dse.eval");

      const auth = yield* AuthService;
      const session = yield* auth.getSession();
      if (!session.userId) return yield* Effect.fail(new Error("unauthorized"));

      const convex = yield* ConvexService;

      // Ensure pinned artifacts (notably judges) are present in Convex for replayability.
      for (const art of PINNED_DSE_ARTIFACTS) {
        const encoded = Schema.encodeSync(CompiledArtifact.DseCompiledArtifactV1Schema)(art);
        yield* convex.mutation(api.dse.artifacts.putArtifact, {
          signatureId: art.signatureId,
          compiled_id: art.compiled_id,
          json: encoded,
        });
      }

      const exRes = yield* convex.query(api.dse.examples.listExamples, { signatureId, limit });
      const listResult = exRes as DseListExamplesResult;
      const raw = Array.isArray(listResult?.examples) ? listResult.examples : [];
      const datasetBlobs = collectDatasetBlobsFromExamples(raw);

      const decodeInput = Schema.decodeUnknownSync(signature.input);
      const decodeOutput = Schema.decodeUnknownSync(signature.output);

      const examples = raw.map((r) => ({
        exampleId: String(r?.exampleId ?? ""),
        input: decodeInput(r?.inputJson),
        expected: decodeOutput(r?.expectedJson),
        split: mapStoredSplitToDseSplit(r?.split),
        tags: Array.isArray(r?.tags) ? (r.tags as unknown[]).filter((tt) => typeof tt === "string") : undefined,
        meta: (r as any)?.meta,
      }));

      const datasetId = convexDatasetIdForExamples(signatureId);
      const dataset = yield* EvalDataset.make({ datasetId, examples });

      const datasetFiltered = EvalDataset.filter(dataset, { split: mapStoredSplitToDseSplit(split) ?? split });
      const datasetHash = yield* EvalDataset.datasetHash(datasetFiltered);

      const { reward } = compileJobForSignature({ signatureId, datasetId });

      const artifactRes = yield* convex.query(api.dse.artifacts.getArtifact, { signatureId, compiled_id });
      const artifactResult = artifactRes as DseGetArtifactResult;
      const rawArtifact = artifactResult?.artifact ?? null;
      if (!rawArtifact) return yield* Effect.fail(new Error("artifact_not_found"));

      const artifact = yield* Effect.try({
        try: () => Schema.decodeUnknownSync(CompiledArtifact.DseCompiledArtifactV1Schema)(rawArtifact),
        catch: () => new Error("invalid_artifact"),
      });

      const dseLmClient = makeDseLmClientWithOpenRouterPrimary({
        env: { OPENROUTER_API_KEY: env.OPENROUTER_API_KEY, AI: aiBinding },
        defaultModelIdCf: MODEL_ID_CF,
        primaryModelOpenRouter: PRIMARY_MODEL_OPENROUTER,
      });

      if (opsRunId) {
        yield* convex
          .mutation(api.dse.opsRuns.appendEvent, {
            runId: opsRunId,
            level: "info",
            phase: "phase7.eval.start",
            message: `eval started signatureId=${signatureId} compiled_id=${compiled_id} split=${split}`,
            json: { signatureId, compiled_id, split, datasetExamples: datasetFiltered.examples.length },
          })
          .pipe(Effect.catchAll(() => Effect.void));
      }

      const evalRes = yield* Effect.gen(function* () {
        if (datasetBlobs.length > 0) {
          yield* seedBlobStoreFromDatasetBlobs({ blobs: datasetBlobs });
        }
        return yield* Eval.evaluate({
          signature,
          artifact,
          dataset: datasetFiltered,
          reward,
          includeExampleDetails,
        });
      }).pipe(Effect.provideService(Lm.LmClientService, dseLmClient), Effect.provide(compileEnv));

      const evalHash = yield* Hashes.sha256IdFromCanonicalJson({
        signatureId,
        compiled_id,
        datasetHash: evalRes.summary.datasetHash ?? datasetHash,
        rewardId: evalRes.summary.metricId ?? reward.rewardId,
        rewardVersion: evalRes.summary.metricVersion ?? reward.rewardVersion,
        split,
        selectedExampleIdsHash: evalRes.summary.selectedExampleIdsHash ?? "",
      });

      const stored = yield* convex.mutation(api.dse.evalReports.putReport, {
        signatureId,
        evalHash,
        compiled_id,
        datasetId,
        datasetHash: evalRes.summary.datasetHash ?? datasetHash,
        rewardId: reward.rewardId,
        rewardVersion: reward.rewardVersion,
        split,
        selectedExampleIdsHash: evalRes.summary.selectedExampleIdsHash,
        n: evalRes.summary.n,
        json: {
          format: "openagents.dse.eval_report",
          formatVersion: 1,
          summary: evalRes.summary,
          ...(includeExampleDetails ? { examples: evalRes.examples ?? [] } : {}),
        },
      });

      yield* t.event("eval.finished", {
        signatureId,
        compiled_id,
        split,
        datasetHash: evalRes.summary.datasetHash ?? datasetHash,
        evalHash,
        existed: Boolean((stored as any)?.existed),
      });

      if (opsRunId) {
        yield* convex
          .mutation(api.dse.opsRuns.appendEvent, {
            runId: opsRunId,
            level: "info",
            phase: "phase7.eval.finish",
            message: `eval finished signatureId=${signatureId} compiled_id=${compiled_id} evalHash=${evalHash}`,
            json: { signatureId, compiled_id, split, evalHash, existed: Boolean((stored as any)?.existed) },
          })
          .pipe(Effect.catchAll(() => Effect.void));
      }

      return {
        ok: true as const,
        signatureId,
        compiled_id,
        split,
        evalHash,
        existed: Boolean((stored as any)?.existed),
      };
    });

    return runAuthedDseAdmin(request, env, program, "session_or_admin_secret");
  }

  if (url.pathname === "/api/dse/canary/status") {
    if (request.method !== "GET") return new Response("Method not allowed", { status: 405 });

    const signatureId = String(url.searchParams.get("signatureId") ?? "").trim();
    if (!signatureId) {
      return json({ ok: false, error: "invalid_input" }, { status: 400, headers: { "cache-control": "no-store" } });
    }

    const program = Effect.gen(function* () {
      const convex = yield* ConvexService;
      return yield* convex.query(api.dse.canary.getCanary, { signatureId });
    });

    return runAuthedDseAdmin(request, env, program, "admin_secret_only");
  }

  if (url.pathname === "/api/dse/exercise/thread/ensure") {
    if (request.method !== "POST") return new Response("Method not allowed", { status: 405 });

    // Body ignored (reserved for future options).
    await readJson(request);

    const program = Effect.gen(function* () {
      const convex = yield* ConvexService;
      const res = yield* convex.mutation(api.autopilot.threads.ensureOwnedThread, {});
      const threadId = String((res as any)?.threadId ?? "");
      if (!threadId) return yield* Effect.fail(new Error("ensure_thread_failed"));
      return { ok: true as const, threadId };
    });

    return runAuthedDseAdmin(request, env, program, "admin_secret_only");
  }

  if (url.pathname === "/api/dse/exercise/predict") {
    if (request.method !== "POST") return new Response("Method not allowed", { status: 405 });
    if (!env.AI) return json({ ok: false, error: "ai_unbound" }, { status: 500, headers: { "cache-control": "no-store" } });
    const aiBinding = env.AI;

    const body = (await readJson(request)) as ExercisePredictBody | null;
    const signatureId = typeof body?.signatureId === "string" ? body.signatureId : "";
    const threadIdRaw = typeof body?.threadId === "string" ? body.threadId : "";
    const splitRaw = typeof body?.split === "string" ? body.split : "train";
    const split = splitRaw === "train" || splitRaw === "dev" || splitRaw === "holdout" || splitRaw === "test" ? splitRaw : "train";
    const countRaw = typeof body?.count === "number" ? body.count : 20;
    const count = Number.isFinite(countRaw) ? Math.max(1, Math.min(200, Math.floor(countRaw))) : 20;
    const limitRaw = typeof body?.limit === "number" ? body.limit : 200;
    const limit = Number.isFinite(limitRaw) ? Math.max(1, Math.min(500, Math.floor(limitRaw))) : 200;

    if (!signatureId) {
      return json({ ok: false, error: "invalid_input" }, { status: 400, headers: { "cache-control": "no-store" } });
    }

    const signature = findSignatureById(signatureId);
    if (!signature) {
      return json({ ok: false, error: "unknown_signature" }, { status: 404, headers: { "cache-control": "no-store" } });
    }

    const program = Effect.gen(function* () {
      const telemetry = yield* TelemetryService;
      const t = telemetry.withNamespace("dse.exercise");

      const auth = yield* AuthService;
      const session = yield* auth.getSession();
      if (!session.userId) return yield* Effect.fail(new Error("unauthorized"));

      const convex = yield* ConvexService;

      const threadId =
        threadIdRaw && threadIdRaw.trim().length > 0
          ? threadIdRaw.trim()
          : String(((yield* convex.mutation(api.autopilot.threads.ensureOwnedThread, {})) as any)?.threadId ?? "");
      if (!threadId) return yield* Effect.fail(new Error("missing_thread"));

      const exRes = yield* convex.query(api.dse.examples.listExamples, { signatureId, split, limit });
      const listResult = exRes as DseListExamplesResult;
      const raw = Array.isArray(listResult?.examples) ? listResult.examples : [];
      if (raw.length === 0) return yield* Effect.fail(new Error("no_examples"));

      const decodeInput = Schema.decodeUnknownSync(signature.input);
      const exampleInputs: Array<any> = raw.map((r) => decodeInput((r as any)?.inputJson));

      const dseLmClient = makeDseLmClientWithOpenRouterPrimary({
        env: { OPENROUTER_API_KEY: env.OPENROUTER_API_KEY, AI: aiBinding },
        defaultModelIdCf: MODEL_ID_CF,
        primaryModelOpenRouter: PRIMARY_MODEL_OPENROUTER,
      });

      type DseReceiptShape = { receiptId?: string; compiled_id?: string; result?: { _tag?: string } };
      const receiptIds: string[] = [];
      let lastCompiledId: string | undefined;
      const onReceipt = (r: unknown) => {
        const rr = r as DseReceiptShape;
        if (typeof rr?.receiptId === "string" && rr.receiptId.length > 0 && receiptIds.length < 50) {
          receiptIds.push(rr.receiptId);
        }
        if (typeof rr?.compiled_id === "string" && rr.compiled_id.length > 0) lastCompiledId = rr.compiled_id;
      };

      const runId = `dse_exercise_${crypto.randomUUID()}`;
      const dseEnv = layerDsePredictEnvForAutopilotRun({ threadId, runId, onReceipt });
      const predict = Predict.make(signature);

      let okCount = 0;
      let errorCount = 0;

      for (let i = 0; i < count; i++) {
        const input = exampleInputs[i % exampleInputs.length];
        const exit = yield* Effect.exit(predict(input).pipe(Effect.provideService(Lm.LmClientService, dseLmClient), Effect.provide(dseEnv)));
        if (exit._tag === "Success") okCount++;
        else errorCount++;
      }

      yield* t.event("exercise.finished", {
        signatureId,
        threadId,
        runId,
        count,
        okCount,
        errorCount,
        ...(lastCompiledId ? { compiled_id: lastCompiledId } : {}),
      });

      return {
        ok: true as const,
        signatureId,
        threadId,
        runId,
        count,
        okCount,
        errorCount,
        receiptIds,
        ...(lastCompiledId ? { compiled_id: lastCompiledId } : {}),
      };
    });

    return runAuthedDseAdmin(request, env, program, "admin_secret_only");
  }

  if (url.pathname === "/api/dse/ops/run/start") {
    if (request.method !== "POST") return new Response("Method not allowed", { status: 405 });

    const body = (await readJson(request)) as OpsRunStartBody | null;
    const runIdRaw = typeof body?.runId === "string" ? body.runId.trim() : "";
    const runId = runIdRaw.length > 0 ? runIdRaw : `opsrun_${crypto.randomUUID()}`;
    const commitSha = typeof body?.commitSha === "string" ? body.commitSha : undefined;
    const baseUrl = typeof body?.baseUrl === "string" ? body.baseUrl : undefined;
    const signatureIds = Array.isArray(body?.signatureIds) ? body?.signatureIds : undefined;
    const notes = typeof body?.notes === "string" ? body.notes : undefined;
    const links = body && "links" in body ? (body as any).links : undefined;

    const program = Effect.gen(function* () {
      const convex = yield* ConvexService;
      return yield* convex.mutation(api.dse.opsRuns.startRun, {
        runId,
        ...(commitSha ? { commitSha } : {}),
        ...(baseUrl ? { baseUrl } : {}),
        ...(signatureIds ? { signatureIds } : {}),
        ...(notes ? { notes } : {}),
        ...(links === undefined ? {} : { links }),
      });
    });

    return runAuthedDseAdmin(request, env, program, "admin_secret_only");
  }

  if (url.pathname === "/api/dse/ops/run/event") {
    if (request.method !== "POST") return new Response("Method not allowed", { status: 405 });

    const body = (await readJson(request)) as OpsRunEventBody | null;
    const runId = typeof body?.runId === "string" ? body.runId : "";
    const levelRaw = body?.level;
    const level = levelRaw === "warn" || levelRaw === "error" ? levelRaw : ("info" as const);
    const phase = typeof body?.phase === "string" ? body.phase : undefined;
    const message = typeof body?.message === "string" ? body.message : "";
    const json = body && "json" in body ? (body as any).json : undefined;
    const tsMs = typeof body?.tsMs === "number" ? body.tsMs : undefined;

    const program = Effect.gen(function* () {
      const convex = yield* ConvexService;
      return yield* convex.mutation(api.dse.opsRuns.appendEvent, {
        runId,
        level,
        ...(phase ? { phase } : {}),
        message,
        ...(json === undefined ? {} : { json }),
        ...(tsMs === undefined ? {} : { tsMs }),
      });
    });

    return runAuthedDseAdmin(request, env, program, "admin_secret_only");
  }

  if (url.pathname === "/api/dse/ops/run/finish") {
    if (request.method !== "POST") return new Response("Method not allowed", { status: 405 });

    const body = (await readJson(request)) as OpsRunFinishBody | null;
    const runId = typeof body?.runId === "string" ? body.runId : "";
    const statusRaw = body?.status;
    const status = statusRaw === "failed" ? ("failed" as const) : ("finished" as const);
    const summaryJson = body && "summaryJson" in body ? (body as any).summaryJson : undefined;

    const program = Effect.gen(function* () {
      const convex = yield* ConvexService;
      return yield* convex.mutation(api.dse.opsRuns.finishRun, {
        runId,
        status,
        ...(summaryJson === undefined ? {} : { summaryJson }),
      });
    });

    return runAuthedDseAdmin(request, env, program, "admin_secret_only");
  }

  if (url.pathname === "/api/dse/examples/import") {
    if (request.method !== "POST") return new Response("Method not allowed", { status: 405 });

    // Fail fast: do not parse/validate bodies unless authorized.
    if (!isDseAdminSecretAuthorized(request, env)) {
      return json({ ok: false, error: "unauthorized" }, { status: 401, headers: { "cache-control": "no-store" } });
    }

    const body = (await readJson(request)) as ExamplesImportBody | null;
    const signatureId = typeof body?.signatureId === "string" ? body.signatureId : "";

    if (!signatureId) {
      return json(
        { ok: false, error: "invalid_input", message: "Expected body: { signatureId: string, jsonl?: string, examples?: any[] }" },
        { status: 400, headers: { "cache-control": "no-store" } },
      );
    }

    // Fail closed: only allow known signatures.
    if (!findSignatureById(signatureId)) {
      return json({ ok: false, error: "unknown_signature" }, { status: 404, headers: { "cache-control": "no-store" } });
    }

    const opsRunId = typeof body?.opsRunId === "string" ? body.opsRunId.trim() : "";
    const sourceRaw = typeof body?.source === "string" ? body.source.trim() : "";
    const source = sourceRaw.length > 0 ? sourceRaw.slice(0, 300) : "import:/api/dse/examples/import";
    const tagsAppend = normalizeTagArray(body?.tagsAppend, 20);

    let rows: Array<unknown> = [];
    if (typeof body?.jsonl === "string") {
      const jsonl = body.jsonl;
      if (jsonl.length > 400_000) {
        return json({ ok: false, error: "invalid_input", message: "jsonl too large" }, { status: 400, headers: { "cache-control": "no-store" } });
      }
      try {
        rows = parseJsonlRows(jsonl, 500);
      } catch {
        return json({ ok: false, error: "invalid_input", message: "invalid jsonl" }, { status: 400, headers: { "cache-control": "no-store" } });
      }
    } else if (Array.isArray(body?.examples)) {
      rows = body.examples as Array<unknown>;
    } else {
      return json(
        { ok: false, error: "invalid_input", message: "Expected body.jsonl (string) or body.examples (array)" },
        { status: 400, headers: { "cache-control": "no-store" } },
      );
    }

    if (rows.length === 0) {
      return json({ ok: false, error: "invalid_input", message: "No examples provided" }, { status: 400, headers: { "cache-control": "no-store" } });
    }
    if (rows.length > 500) {
      return json({ ok: false, error: "invalid_input", message: "Too many examples (max 500)" }, { status: 400, headers: { "cache-control": "no-store" } });
    }

    const program = Effect.gen(function* () {
      const convex = yield* ConvexService;

      // Optional ops-run trace: emit start/finish events so the overnight loop can link dataset ingestion.
      if (opsRunId) {
        yield* convex
          .mutation(api.dse.opsRuns.appendEvent, {
            runId: opsRunId,
            level: "info",
            phase: "phase3.dataset_import.start",
            message: `dataset import started signatureId=${signatureId} rows=${rows.length}`,
            json: { signatureId, rows: rows.length, source },
          })
          .pipe(Effect.catchAll(() => Effect.void));
      }

      let inserted = 0;
      let updated = 0;

      for (const r of rows) {
        const row = r as any;
        const exampleId = typeof row?.exampleId === "string" ? row.exampleId.trim() : "";
        if (!exampleId) return yield* Effect.fail(new Error("invalid_input"));
        if (exampleId.length > 200) return yield* Effect.fail(new Error("invalid_input"));

        const splitRaw = typeof row?.split === "string" ? row.split : undefined;
        const split =
          splitRaw === "train" || splitRaw === "dev" || splitRaw === "holdout" || splitRaw === "test"
            ? (splitRaw as "train" | "dev" | "holdout" | "test")
            : undefined;

        const tagsRow = normalizeTagArray(row?.tags, 50);
        const mergedTags = Array.from(new Set([...tagsRow, ...tagsAppend])).slice(0, 50);

        const res = yield* convex.mutation(api.dse.examples.putExample, {
          signatureId,
          exampleId,
          inputJson: row?.inputJson ?? null,
          expectedJson: row?.expectedJson ?? null,
          ...(split ? { split } : {}),
          ...(mergedTags.length ? { tags: mergedTags } : {}),
          source,
          // Optional metadata (for example: blob texts used to seed BlobStore during eval/compile).
          ...(row?.meta !== undefined ? { meta: row.meta } : {}),
        });

        const existed = Boolean((res as any)?.existed);
        if (existed) updated++;
        else inserted++;
      }

      if (opsRunId) {
        yield* convex
          .mutation(api.dse.opsRuns.appendEvent, {
            runId: opsRunId,
            level: "info",
            phase: "phase3.dataset_import.finish",
            message: `dataset import finished signatureId=${signatureId} inserted=${inserted} updated=${updated} total=${rows.length}`,
            json: { signatureId, inserted, updated, total: rows.length, source },
          })
          .pipe(Effect.catchAll(() => Effect.void));
      }

      return { ok: true as const, signatureId, inserted, updated, total: rows.length };
    });

    return runAuthedDseAdmin(request, env, program, "admin_secret_only");
  }

  if (url.pathname === "/api/dse/receipts/list") {
    if (request.method !== "GET") return new Response("Method not allowed", { status: 405 });

    const signatureId = (url.searchParams.get("signatureId") ?? "").trim();
    const limitRaw = url.searchParams.get("limit");
    const limit =
      typeof limitRaw === "string" && limitRaw.trim().length > 0
        ? Math.max(0, Math.min(200, Math.floor(Number(limitRaw))))
        : 50;

    const requireRlmTraceRaw = (url.searchParams.get("requireRlmTrace") ?? "").trim().toLowerCase();
    const requireRlmTrace = requireRlmTraceRaw === "1" || requireRlmTraceRaw === "true" || requireRlmTraceRaw === "yes";

    const resultTagRaw = (url.searchParams.get("resultTag") ?? "").trim();
    const resultTag = resultTagRaw === "Ok" || resultTagRaw === "Error" ? (resultTagRaw as "Ok" | "Error") : null;

    const strategyIdRaw = (url.searchParams.get("strategyId") ?? "").trim();
    const strategyId = strategyIdRaw.length > 0 ? strategyIdRaw : null;

    if (!signatureId) {
      return json({ ok: false, error: "invalid_input" }, { status: 400, headers: { "cache-control": "no-store" } });
    }

    // Fail closed: only allow known signatures.
    if (!findSignatureById(signatureId)) {
      return json({ ok: false, error: "unknown_signature" }, { status: 404, headers: { "cache-control": "no-store" } });
    }

    const program = Effect.gen(function* () {
      const auth = yield* AuthService;
      const session = yield* auth.getSession();
      if (!session.userId) return yield* Effect.fail(new Error("unauthorized"));
      if (session.userId !== DSE_OPS_ADMIN_SUBJECT) return yield* Effect.fail(new Error("unauthorized"));

      const convex = yield* ConvexService;
      const res = yield* convex.query(api.dse.receipts.listPredictReceiptsBySignatureIdAdmin, { signatureId, limit });
      const raw = Array.isArray((res as any)?.receipts) ? ((res as any).receipts as Array<any>) : [];

      const receipts = raw.filter((r) => {
        if (requireRlmTrace && !(typeof r?.rlmTraceBlobId === "string" && r.rlmTraceBlobId.length > 0)) return false;
        if (resultTag && String(r?.resultTag ?? "") !== resultTag) return false;
        if (strategyId && String(r?.strategyId ?? "") !== strategyId) return false;
        return true;
      });

      return { ok: true as const, receipts };
    });

    // Allow headless ops usage via admin-secret mode.
    return runAuthedDseAdmin(request, env, program, "session_or_admin_secret");
  }

  if (url.pathname.startsWith("/api/dse/receipt/")) {
    if (request.method !== "GET") return new Response("Method not allowed", { status: 405 });

    const receiptId = url.pathname.slice("/api/dse/receipt/".length).trim();
    if (!receiptId) {
      return json({ ok: false, error: "invalid_input" }, { status: 400, headers: { "cache-control": "no-store" } });
    }

    const program = Effect.gen(function* () {
      const auth = yield* AuthService;
      const session = yield* auth.getSession();
      if (!session.userId) return yield* Effect.fail(new Error("unauthorized"));

      const convex = yield* ConvexService;
      const isOpsAdmin = session.userId === "user_dse_admin";
      const recRes = yield* convex.query(
        isOpsAdmin ? api.dse.receipts.getPredictReceiptByReceiptIdAdmin : api.dse.receipts.getPredictReceiptByReceiptId,
        { receiptId },
      );
      const receiptRow = (recRes as any)?.receipt ?? null;
      if (!receiptRow) return { ok: true as const, receipt: null };

      const receiptJson = receiptRow?.json ?? null;
      const receipt = yield* Effect.try({
        try: () => Schema.decodeUnknownSync(Receipt.PredictReceiptV1Schema)(receiptJson),
        catch: () => new Error("invalid_receipt"),
      });

      return {
        ok: true as const,
        receipt,
        threadId: String(receiptRow?.threadId ?? ""),
        runId: String(receiptRow?.runId ?? ""),
        createdAtMs: Number(receiptRow?.createdAtMs ?? 0),
      };
    });

    // Allow headless ops usage via admin-secret mode (and ops-admin session bypass thread access).
    return runAuthedDseAdmin(request, env, program, "session_or_admin_secret");
  }

  if (url.pathname.startsWith("/api/dse/blob/")) {
    if (request.method !== "GET") return new Response("Method not allowed", { status: 405 });

    const rest = url.pathname.slice("/api/dse/blob/".length);
    const [receiptId, blobId] = rest.split("/").filter((p) => p.length > 0);

    if (!receiptId || !blobId) {
      return json({ ok: false, error: "invalid_input" }, { status: 400, headers: { "cache-control": "no-store" } });
    }

    const program = Effect.gen(function* () {
      const auth = yield* AuthService;
      const session = yield* auth.getSession();
      if (!session.userId) return yield* Effect.fail(new Error("unauthorized"));

      const convex = yield* ConvexService;

      const isOpsAdmin = session.userId === "user_dse_admin";
      const recRes = yield* convex.query(
        isOpsAdmin ? api.dse.receipts.getPredictReceiptByReceiptIdAdmin : api.dse.receipts.getPredictReceiptByReceiptId,
        { receiptId },
      );
      const receiptRow = (recRes as any)?.receipt ?? null;
      if (!receiptRow) return yield* Effect.fail(new Error("receipt_not_found"));

      const threadId = String(receiptRow?.threadId ?? "");
      const runId = String(receiptRow?.runId ?? "");
      if (!threadId || !runId) return yield* Effect.fail(new Error("receipt_missing_scope"));

      const blobRes = yield* convex.query(isOpsAdmin ? api.dse.blobs.getTextAdmin : api.dse.blobs.getText, { threadId, runId, blobId });
      const text = (blobRes as any)?.text ?? null;
      if (typeof text !== "string" || text.length === 0) return yield* Effect.fail(new Error("blob_not_found"));

      return new Response(text, {
        status: 200,
        headers: {
          "content-type": "text/plain; charset=utf-8",
          "cache-control": "no-store",
        },
      });
    });

    // Allow headless ops usage via admin-secret mode (and ops-admin session bypass thread access).
    return runAuthedDseAdminRaw(request, env, program, "session_or_admin_secret");
  }

  if (url.pathname === "/api/dse/trace/export") {
    if (request.method !== "POST") return new Response("Method not allowed", { status: 405 });

    const adminSecretOk = isDseAdminSecretAuthorized(request, env);

    const body = (await readJson(request)) as TraceExportBody | null;
    const receiptId = typeof body?.receiptId === "string" ? body.receiptId : "";
    const exampleIdOverride = typeof body?.exampleId === "string" ? body.exampleId : null;
    const splitRaw = typeof body?.split === "string" ? body.split : null;
    const tagsRaw = Array.isArray(body?.tags) ? body?.tags : null;
    const dryRun = body?.dryRun === undefined ? false : Boolean(body.dryRun);

    const tags =
      tagsRaw
        ?.map((t) => (typeof t === "string" ? t.trim() : ""))
        .filter((t) => t.length > 0)
        .slice(0, 50) ?? [];

    const split =
      splitRaw === "train" || splitRaw === "dev" || splitRaw === "holdout" || splitRaw === "test"
        ? (splitRaw as "train" | "dev" | "holdout" | "test")
        : undefined;

    if (!receiptId) {
      return json({ ok: false, error: "invalid_input" }, { status: 400, headers: { "cache-control": "no-store" } });
    }

    const program = Effect.gen(function* () {
      const telemetry = yield* TelemetryService;
      const t = telemetry.withNamespace("dse.trace_export");

      const auth = yield* AuthService;
      const session = yield* auth.getSession();
      if (!session.userId) return yield* Effect.fail(new Error("unauthorized"));

      const convex = yield* ConvexService;

      const recRes = adminSecretOk
        ? yield* convex.query(api.dse.receipts.getPredictReceiptByReceiptIdAdmin, { receiptId })
        : yield* convex.query(api.dse.receipts.getPredictReceiptByReceiptId, { receiptId });
      const receiptRow = (recRes as any)?.receipt ?? null;
      if (!receiptRow) return yield* Effect.fail(new Error("receipt_not_found"));

      const threadId = String(receiptRow?.threadId ?? "");
      const runId = String(receiptRow?.runId ?? "");
      const receiptJson = receiptRow?.json ?? null;

      const receipt = yield* Effect.try({
        try: () => Schema.decodeUnknownSync(Receipt.PredictReceiptV1Schema)(receiptJson),
        catch: () => new Error("invalid_receipt"),
      });

      const blobId = receipt.rlmTrace?.blob?.id;
      if (!blobId) return yield* Effect.fail(new Error("missing_rlm_trace"));

      const blobRes = adminSecretOk
        ? yield* convex.query(api.dse.blobs.getTextAdmin, { threadId, runId, blobId })
        : yield* convex.query(api.dse.blobs.getText, { threadId, runId, blobId });
      const traceText = (blobRes as any)?.text;
      if (typeof traceText !== "string" || traceText.length === 0) {
        return yield* Effect.fail(new Error("trace_blob_not_found"));
      }

      const candidate = yield* TraceMining.candidateExampleFromRlmTrace({
        receipt,
        traceText,
      });

      const exampleId = exampleIdOverride ?? candidate.exampleId;

      const mergedTags = Array.from(
        new Set([...(candidate.tags ?? []), ...tags].map((s) => String(s).trim()).filter((s) => s.length > 0)),
      ).slice(0, 50);

      const meta = {
        kind: "openagents.trace_export.v1",
        receiptId: receipt.receiptId,
        signatureId: candidate.signatureId,
        threadId,
        runId,
        strategyId: receipt.strategyId ?? null,
        compiled_id: receipt.compiled_id ?? null,
        rlmTrace: {
          blobId,
          eventCount:
            typeof receipt.rlmTrace?.eventCount === "number" && Number.isFinite(receipt.rlmTrace.eventCount)
              ? Math.max(0, Math.floor(receipt.rlmTrace.eventCount))
              : null,
        },
      };

      if (dryRun) {
        yield* t.event("trace_export.dry_run", { signatureId: candidate.signatureId, exampleId, receiptId });
        return { ok: true as const, dryRun: true as const, signatureId: candidate.signatureId, exampleId, candidate, meta };
      }

      const putRes = yield* convex.mutation(api.dse.examples.putExample, {
        signatureId: candidate.signatureId,
        exampleId,
        inputJson: candidate.inputJson,
        expectedJson: candidate.expectedJson,
        ...(split ? { split } : {}),
        ...(mergedTags.length ? { tags: mergedTags } : {}),
        ...(candidate.source ? { source: candidate.source } : {}),
        meta,
      });

      yield* t.event("trace_export.ok", { signatureId: candidate.signatureId, exampleId, receiptId });

      return {
        ok: true as const,
        dryRun: false as const,
        existed: Boolean((putRes as any)?.existed),
        signatureId: candidate.signatureId,
        exampleId,
      };
    });

    return runAuthedDseAdmin(request, env, program, "session_or_admin_secret");
  }

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

      const exRes = yield* convex.query(api.dse.examples.listExamples, { signatureId, limit: 500 });
      const listResult = exRes as DseListExamplesResult;
      const raw = Array.isArray(listResult?.examples) ? listResult.examples : [];
      const datasetBlobs = collectDatasetBlobsFromExamples(raw);

      const decodeInput = Schema.decodeUnknownSync(signature.input);
      const decodeOutput = Schema.decodeUnknownSync(signature.output);

      const examples = raw.map((r) => ({
        exampleId: String(r?.exampleId ?? ""),
        input: decodeInput(r?.inputJson),
        expected: decodeOutput(r?.expectedJson),
        split: mapStoredSplitToDseSplit(r?.split),
        tags: Array.isArray(r?.tags) ? (r.tags as unknown[]).filter((t) => typeof t === "string") : undefined,
        meta: (r as any)?.meta,
      }));

      const datasetId = convexDatasetIdForExamples(signatureId);
      const dataset = yield* EvalDataset.make({ datasetId, examples });
      const datasetHash = yield* EvalDataset.datasetHash(dataset);

      const holdout = EvalDataset.filter(dataset, { split: "holdout" });
      if (requireHoldout && holdout.examples.length === 0) {
        return yield* Effect.fail(new Error("holdout_required"));
      }

      const { jobSpec, reward } = compileJobForSignature({ signatureId, datasetId });
      const jobHash = yield* CompileJob.compileJobHash(jobSpec);

      const reportRes = yield* convex.query(api.dse.compileReports.getReport, { signatureId, jobHash, datasetHash });
      const reportResult = reportRes as DseGetReportResult;
      const report = reportResult?.report ?? null;
      if (!report) return yield* Effect.fail(new Error("compile_report_not_found"));
      if (String(report.compiled_id ?? "") !== compiled_id) return yield* Effect.fail(new Error("compiled_id_mismatch"));

      const candidateHoldoutReward = yield* Effect.try({
        try: () => decodeCompileReportHoldoutReward(report.json),
        catch: () => new Error("invalid_compile_report"),
      });

      const activeRes = yield* convex.query(api.dse.active.getActive, { signatureId });
      const activeResult = activeRes as DseGetActiveResult;
      const control_compiled_id = typeof activeResult?.compiled_id === "string" ? activeResult.compiled_id : null;

      let baselineHoldoutReward = 0;
      if (control_compiled_id) {
        const baseArtifactRes = yield* convex.query(api.dse.artifacts.getArtifact, {
          signatureId,
          compiled_id: control_compiled_id,
        });
        const baseArtifactResult = baseArtifactRes as DseGetArtifactResult;
        const baseRaw = baseArtifactResult?.artifact ?? null;
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
        const evalRes = yield* Effect.gen(function* () {
          if (datasetBlobs.length > 0) {
            yield* seedBlobStoreFromDatasetBlobs({ blobs: datasetBlobs });
          }
          return yield* Eval.evaluate({
            signature,
            artifact: baseArtifact,
            dataset: holdout,
            reward,
          });
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
      });

      // Canary config becomes stale after a promotion; clear it best-effort.
      yield* convex.mutation(api.dse.canary.stopCanary, { signatureId, reason: "promoted" }).pipe(
        Effect.catchAll(() => Effect.void),
      );

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

    return runAuthedDseAdmin(request, env, program, "session_or_admin_secret");
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

      const exRes = yield* convex.query(api.dse.examples.listExamples, { signatureId, limit: 500 });
      const listResult = exRes as DseListExamplesResult;
      const raw = Array.isArray(listResult?.examples) ? listResult.examples : [];
      const datasetBlobs = collectDatasetBlobsFromExamples(raw);

      const decodeInput = Schema.decodeUnknownSync(signature.input);
      const decodeOutput = Schema.decodeUnknownSync(signature.output);

      const examples = raw.map((r) => ({
        exampleId: String(r?.exampleId ?? ""),
        input: decodeInput(r?.inputJson),
        expected: decodeOutput(r?.expectedJson),
        split: mapStoredSplitToDseSplit(r?.split),
        meta: (r as any)?.meta,
      }));

      const datasetId = convexDatasetIdForExamples(signatureId);
      const dataset = yield* EvalDataset.make({ datasetId, examples });
      const datasetHash = yield* EvalDataset.datasetHash(dataset);

      const holdout = EvalDataset.filter(dataset, { split: "holdout" });
      if (requireHoldout && holdout.examples.length === 0) {
        return yield* Effect.fail(new Error("holdout_required"));
      }

      const { jobSpec, reward } = compileJobForSignature({ signatureId, datasetId });
      const jobHash = yield* CompileJob.compileJobHash(jobSpec);

      const reportRes = yield* convex.query(api.dse.compileReports.getReport, { signatureId, jobHash, datasetHash });
      const reportResult = reportRes as DseGetReportResult;
      const report = reportResult?.report ?? null;
      if (!report) return yield* Effect.fail(new Error("compile_report_not_found"));
      if (String(report.compiled_id ?? "") !== canary_compiled_id) return yield* Effect.fail(new Error("compiled_id_mismatch"));

      const candidateHoldoutReward = yield* Effect.try({
        try: () => decodeCompileReportHoldoutReward(report.json),
        catch: () => new Error("invalid_compile_report"),
      });

      const activeRes = yield* convex.query(api.dse.active.getActive, { signatureId });
      const activeResult = activeRes as DseGetActiveResult;
      const control_compiled_id = typeof activeResult?.compiled_id === "string" ? activeResult.compiled_id : null;
      if (!control_compiled_id) return yield* Effect.fail(new Error("control_missing"));

      const baseArtifactRes = yield* convex.query(api.dse.artifacts.getArtifact, {
        signatureId,
        compiled_id: control_compiled_id,
      });
      const baseArtifactResult = baseArtifactRes as DseGetArtifactResult;
      const baseRaw = baseArtifactResult?.artifact ?? null;
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
      const evalRes = yield* Effect.gen(function* () {
        if (datasetBlobs.length > 0) {
          yield* seedBlobStoreFromDatasetBlobs({ blobs: datasetBlobs });
        }
        return yield* Eval.evaluate({
          signature,
          artifact: baseArtifact,
          dataset: holdout,
          reward,
        });
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
      });

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

    return runAuthedDseAdmin(request, env, program, "session_or_admin_secret");
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
      const stopped = yield* convex.mutation(api.dse.canary.stopCanary, {
        signatureId,
        ...(reason ? { reason } : {}),
      });
      const stopResult = stopped as DseStopCanaryResult;
      yield* t.event("canary.stopped", {
        signatureId,
        existed: Boolean(stopResult?.existed),
        ...(reason ? { reason } : {}),
      });

      return { ok: true, signatureId, existed: Boolean(stopResult?.existed) } as const;
    });

    return runAuthedDseAdmin(request, env, program, "session_or_admin_secret");
  }

  return null;
};
