import { Effect, Layer, Schema } from "effect";

import {
  BlobStore,
  Budget,
  Compile,
  CompileJob,
  CompiledArtifact,
  EvalCache,
  EvalDataset,
  Lm,
  Receipt,
  VarSpace,
} from "@openagentsinc/dse";

import { signatures as dseCatalogSignatures } from "../../../autopilot-worker/src/dseCatalog";

import { api } from "../../convex/_generated/api";
import { AuthService } from "../effect/auth";
import { ConvexService } from "../effect/convex";
import { RequestContextService, makeServerRequestContext } from "../effect/requestContext";
import { TelemetryService } from "../effect/telemetry";

import { makeDseLmClientWithOpenRouterPrimary } from "./dse";
import { isDseAdminSecretAuthorized, withDseAdminSecretServices } from "./dseAdminSecret";
import { collectDatasetBlobsFromExamples, seedBlobStoreFromDatasetBlobs } from "./dseDatasetBlobs";
import { RECAP_THREAD_SIGNATURE_ID, SUMMARIZE_THREAD_SIGNATURE_ID, compileJobForSignature, convexDatasetIdForExamples } from "./dseJobs";
import { PINNED_DSE_ARTIFACTS } from "./dsePinnedArtifacts";
import { OA_REQUEST_ID_HEADER, formatRequestIdLogToken } from "./requestId";
import type { WorkerEnv } from "./env";
import { getWorkerRuntime } from "./runtime";
import type { DseGetReportResult, DseListExamplesResult } from "./convexTypes";
import type { DseSignature } from "@openagentsinc/dse";

const MODEL_ID_CF = "@cf/openai/gpt-oss-20b";
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

type CompileBody = {
  readonly signatureId?: unknown;
};

const mapStoredSplitToDseSplit = (split: unknown): EvalDataset.DatasetSplit | undefined => {
  if (split === "train") return "train";
  if (split === "holdout") return "holdout";
  // Stage 4 stored "dev"; map it to DSE's "holdout" semantics for compile/eval.
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

export const handleDseCompileRequest = async (
  request: Request,
  env: WorkerEnv,
  _ctx: ExecutionContext,
): Promise<Response | null> => {
  const url = new URL(request.url);
  if (url.pathname !== "/api/dse/compile") return null;

  if (request.method !== "POST") return new Response("Method not allowed", { status: 405 });

  const requestId = request.headers.get(OA_REQUEST_ID_HEADER) ?? "missing";

  if (!env.AI) {
    return json({ ok: false, error: "ai_unbound" }, { status: 500, headers: { "cache-control": "no-store" } });
  }
  const aiBinding = env.AI;

  const body = (await readJson(request)) as CompileBody | null;
  const signatureId = typeof body?.signatureId === "string" ? body.signatureId : "";
  if (!signatureId) {
    return json(
      { ok: false, error: "invalid_input", message: "Expected body: { signatureId: string }" },
      { status: 400, headers: { "cache-control": "no-store" } },
    );
  }

  const signature = findSignatureById(signatureId);
  if (!signature) {
    return json({ ok: false, error: "unknown_signature" }, { status: 404, headers: { "cache-control": "no-store" } });
  }

  const adminSecretOk = isDseAdminSecretAuthorized(request, env);

  const { runtime, config } = getWorkerRuntime(env);

  const telemetryBase = runtime.runSync(
    Effect.gen(function* () {
      return yield* TelemetryService;
    }),
  );
  const requestTelemetry = telemetryBase.withFields({
    requestId,
    method: request.method,
    pathname: url.pathname,
  });

  const program = Effect.gen(function* () {
    const telemetry = yield* TelemetryService;
    const t = telemetry.withNamespace("dse.compile");

    const auth = yield* AuthService;
    const session = yield* auth.getSession();
    if (!session.userId) {
      return yield* Effect.fail(new Error("unauthorized"));
    }

    const convex = yield* ConvexService;

    // Phase 7: only ensure pinned judge artifacts for signatures that use judge-based rewards.
    if (signatureId === RECAP_THREAD_SIGNATURE_ID || signatureId === SUMMARIZE_THREAD_SIGNATURE_ID) {
      for (const art of PINNED_DSE_ARTIFACTS) {
        const encoded = Schema.encodeSync(CompiledArtifact.DseCompiledArtifactV1Schema)(art);
        yield* convex.mutation(api.dse.artifacts.putArtifact, {
          signatureId: art.signatureId,
          compiled_id: art.compiled_id,
          json: encoded,
        });
      }
    }

    // Load dataset examples from Convex (global store; auth required).
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
      meta: r?.meta,
    }));

    const datasetId = convexDatasetIdForExamples(signatureId);
    const dataset = yield* EvalDataset.make({ datasetId, examples });

    const datasetHash = yield* EvalDataset.datasetHash(dataset);

    const { jobSpec, reward, searchSpace, optimizer } = compileJobForSignature({ signatureId, datasetId });

    const jobHash = yield* CompileJob.compileJobHash(jobSpec);

    // If we already ran this job against this exact dataset version, return it (idempotent).
    const existing = yield* convex.query(api.dse.compileReports.getReport, { signatureId, jobHash, datasetHash });
    const reportResult = existing as DseGetReportResult;
    const existingReport = reportResult?.report ?? null;
    if (existingReport) {
      yield* t.event("compile.cached", { signatureId, jobHash, datasetHash });
      return {
        ok: true as const,
        signatureId,
        jobHash,
        datasetId,
        datasetHash,
        compiled_id: String(existingReport.compiled_id ?? ""),
        existed: true as const,
      };
    }

    yield* t.event("compile.started", { signatureId, jobHash, datasetHash, examples: dataset.examples.length });

    const dseLmClient = makeDseLmClientWithOpenRouterPrimary({
      env: { OPENROUTER_API_KEY: env.OPENROUTER_API_KEY, AI: aiBinding },
      defaultModelIdCf: MODEL_ID_CF,
      primaryModelOpenRouter: PRIMARY_MODEL_OPENROUTER,
    });

    // Compile environment: budgets + blob store on, receipts are discarded (compile is not thread-scoped).
    const compileEnv = Layer.mergeAll(
      BlobStore.layerInMemory(),
      Budget.layerInMemory(),
      Receipt.layerNoop(),
      EvalCache.layerInMemory(),
      VarSpace.layerInMemory(),
    );

    const result = yield* Effect.gen(function* () {
      // Phase 7: seed BlobStore for datasets whose inputs contain BlobRefs (e.g. recap/summarization).
      if (datasetBlobs.length > 0) {
        const seeded = yield* seedBlobStoreFromDatasetBlobs({ blobs: datasetBlobs });
        yield* t.event("compile.dataset_blobs_seeded", {
          signatureId,
          seeded: seeded.seeded,
          totalChars: seeded.totalChars,
        });
      }

      return yield* Compile.compile({
        signature,
        dataset,
        reward,
        searchSpace,
        optimizer,
      });
    }).pipe(Effect.provideService(Lm.LmClientService, dseLmClient), Effect.provide(compileEnv));

    // Sanity check: hash inputs match our precomputed ids.
    if (result.report.jobHash !== jobHash) {
      yield* Effect.fail(new Error("job_hash_mismatch"));
    }
    if (result.report.datasetHash !== datasetHash) {
      yield* Effect.fail(new Error("dataset_hash_mismatch"));
    }

    const artifactJson = Schema.encodeSync(CompiledArtifact.DseCompiledArtifactV1Schema)(result.artifact);

    yield* convex.mutation(api.dse.artifacts.putArtifact, {
      signatureId: result.artifact.signatureId,
      compiled_id: result.artifact.compiled_id,
      json: artifactJson,
    });

    yield* convex.mutation(api.dse.compileReports.putReport, {
      signatureId,
      jobHash,
      datasetId,
      datasetHash,
      compiled_id: result.artifact.compiled_id,
      json: {
        format: "openagents.dse.compile_report",
        formatVersion: 1,
        job: jobSpec,
        report: result.report,
      },
    });

    yield* t.event("compile.finished", {
      signatureId,
      jobHash,
      datasetHash,
      compiled_id: result.artifact.compiled_id,
      candidates: result.report.evaluatedCandidates.length,
    });

    return {
      ok: true as const,
      signatureId,
      jobHash,
      datasetId,
      datasetHash,
      compiled_id: result.artifact.compiled_id,
      existed: false as const,
    };
  }).pipe(
    Effect.provideService(RequestContextService, makeServerRequestContext(request)),
    Effect.provideService(TelemetryService, requestTelemetry),
  );

  const authedProgram = adminSecretOk ? withDseAdminSecretServices(env, config.convexUrl, program) : program;

  const exit = await runtime.runPromiseExit(authedProgram);
  if (exit._tag === "Failure") {
    const msg = String(exit._tag === "Failure" ? exit.cause : "compile_failed");
    const status = msg.includes("unauthorized") ? 401 : 500;
    console.error(`[dse.compile] ${formatRequestIdLogToken(requestId)}`, msg);
    return json({ ok: false, error: msg }, { status, headers: { "cache-control": "no-store" } });
  }

  return json(exit.value, { status: 200, headers: { "cache-control": "no-store" } });
};
