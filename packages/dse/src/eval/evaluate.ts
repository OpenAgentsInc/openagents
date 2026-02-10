import { Effect, Schema } from "effect";

import type { DseCompiledArtifactV1, EvalSummaryV1 } from "../compiledArtifact.js";
import { sha256IdFromCanonicalJson } from "../hashes.js";
import type { DseSignature } from "../signature.js";
import type { PredictEnv } from "../runtime/predict.js";
import { make as makePredict } from "../runtime/predict.js";
import { layerInMemory as layerPolicyInMemory, PolicyRegistryService } from "../runtime/policyRegistry.js";
import { makeInMemory as makeInMemoryReceiptRecorder, type PredictReceiptV1 } from "../runtime/receipt.js";

import { EvalCacheService, evalCacheKeyId, type EvalCacheKeyV1 } from "./cache.js";
import {
  datasetHash,
  filter as filterDataset,
  sample as sampleDataset,
  selectedExampleIdsHash,
  type Dataset,
  type DatasetFilter,
  type SamplePlan
} from "./dataset.js";
import {
  computeReward,
  type RewardBundle,
  type RewardSignalArgs,
  type RewardSignalReportV1
} from "./reward.js";

export class EvalError extends Schema.TaggedError<EvalError>()("EvalError", {
  message: Schema.String,
  cause: Schema.optional(Schema.Defect)
}) {}

export type EvalExampleResultV1 = {
  readonly exampleId: string;
  readonly signatureId: string;
  readonly compiled_id: string;
  readonly datasetHash: string;
  readonly rewardId: string;
  readonly rewardVersion: number;
  readonly reward: number;
  readonly signals: ReadonlyArray<RewardSignalReportV1>;
  readonly predictMeta?:
    | {
        readonly strategyId?: string | undefined;
        readonly durationMs?: number | undefined;
        readonly contextPressure?: unknown | undefined;
        readonly budgetUsage?: unknown | undefined;
      }
    | undefined;
  readonly outputHash?: string | undefined;
  readonly error?: { readonly errorName: string; readonly message: string } | undefined;
};

export type EvalResultV1 = {
  readonly summary: EvalSummaryV1;
  readonly examples?: ReadonlyArray<EvalExampleResultV1> | undefined;
};

export type EvaluateOptions<I, O, Y> = {
  readonly signature: DseSignature<I, O>;
  readonly artifact: DseCompiledArtifactV1;
  readonly dataset: Dataset<I, Y>;
  readonly reward: RewardBundle<I, O, Y>;
  readonly filter?: DatasetFilter | undefined;
  readonly sample?: SamplePlan | undefined;
  readonly includeExampleDetails?: boolean | undefined;
};

export type EvalEnv = Exclude<PredictEnv, PolicyRegistryService> | EvalCacheService;

function predictErrorSummary(error: unknown): { readonly errorName: string; readonly message: string } {
  if (error && typeof error === "object") {
    const name = (error as any)._tag ?? (error as any).name;
    const message = (error as any).message;
    if (typeof message === "string") {
      return { errorName: typeof name === "string" ? name : "PredictError", message };
    }
  }
  return { errorName: "PredictError", message: String(error) };
}

function toolFailuresFromMeta(meta: unknown): number {
  if (!meta || typeof meta !== "object") return 0;
  const n = (meta as any).toolFailures;
  if (typeof n !== "number" || !Number.isFinite(n)) return 0;
  return Math.max(0, Math.floor(n));
}

export function evaluate<I, O, Y>(
  options: EvaluateOptions<I, O, Y>
): Effect.Effect<EvalResultV1, EvalError, EvalEnv> {
  return Effect.gen(function* () {
    const cache = yield* EvalCacheService;

    const dataset0 = options.filter
      ? filterDataset(options.dataset, options.filter)
      : options.dataset;

    const { dataset: dataset1, selectedExampleIds } = options.sample
      ? sampleDataset(dataset0, options.sample)
      : { dataset: dataset0, selectedExampleIds: dataset0.examples.map((e) => e.exampleId) };

    const dHash = yield* datasetHash(dataset1);
    const selectionHash = yield* selectedExampleIdsHash(selectedExampleIds);

    const signatureId = options.signature.id;
    const compiled_id = options.artifact.compiled_id;
    const rewardId = options.reward.rewardId;
    const rewardVersion = options.reward.rewardVersion;

    const policy = layerPolicyInMemory({
      activeBySignatureId: { [signatureId]: compiled_id },
      artifacts: [options.artifact]
    });

    const predict = makePredict(options.signature);

    const evalOne = (example: (typeof dataset1.examples)[number]) =>
      Effect.gen(function* () {
        const key: EvalCacheKeyV1 = {
          signatureId,
          compiled_id,
          datasetHash: dHash,
          metricId: rewardId,
          metricVersion: rewardVersion,
          exampleId: example.exampleId
        };

        const keyId = yield* evalCacheKeyId(key);

        const cached = (yield* cache.get(keyId)) as EvalExampleResultV1 | null;
        if (cached && cached.exampleId === example.exampleId) {
          return cached;
        }

        const receipts0 = makeInMemoryReceiptRecorder();
        const predictedEither = yield* Effect.either(
          predict(example.input).pipe(Effect.provide(policy), Effect.provide(receipts0.layer))
        );

        const predictReceipt = (receipts0.getReceipts()[0] ?? null) as PredictReceiptV1 | null;

        const pred: O | null =
          predictedEither._tag === "Right" ? predictedEither.right : null;
        const predictError =
          predictedEither._tag === "Left"
            ? predictErrorSummary(predictedEither.left)
            : undefined;

        const toolFailures = toolFailuresFromMeta(example.meta);

        const signalArgs: RewardSignalArgs<I, O, Y> = {
          input: example.input,
          expected: example.expected,
          pred,
          ...(predictReceipt ? { predictReceipt } : {}),
          ...(predictError ? { predictError } : {}),
          ...(toolFailures ? { toolFailures } : {})
        };

        const signalReports = yield* Effect.forEach(
          options.reward.signals,
          (signal) =>
            signal.evaluate(signalArgs).pipe(
              Effect.either,
              Effect.map((either) => {
                if (either._tag === "Right") return either.right;
                return {
                  signalId: signal.signalId,
                  weight: signal.weight,
                  score: 0,
                  notes:
                    either.left && typeof either.left === "object"
                      ? (either.left as any).message ?? "Signal failed"
                      : "Signal failed"
                } satisfies RewardSignalReportV1;
              })
            ),
          { concurrency: 1 }
        );

        const { reward } = computeReward(signalReports);

        const outputHash =
          pred == null
            ? undefined
            : yield* sha256IdFromCanonicalJson(
                Schema.encodeSync(options.signature.output)(pred)
              );

        const result: EvalExampleResultV1 = {
          exampleId: example.exampleId,
          signatureId,
          compiled_id,
          datasetHash: dHash,
          rewardId,
          rewardVersion,
          reward,
          signals: signalReports,
          ...(predictReceipt
            ? {
                predictMeta: {
                  strategyId: predictReceipt.strategyId,
                  durationMs: predictReceipt.timing?.durationMs,
                  contextPressure: predictReceipt.contextPressure,
                  budgetUsage: predictReceipt.budget?.usage
                }
              }
            : {}),
          ...(outputHash ? { outputHash } : {}),
          ...(predictError ? { error: predictError } : {})
        };

        yield* cache.set(keyId, result as unknown);
        return result;
      }).pipe(
        Effect.catchAll((cause) =>
          Effect.fail(
            EvalError.make({
              message: `Failed to evaluate exampleId=${example.exampleId}`,
              cause
            })
          )
        )
      );

    const results = yield* Effect.forEach(dataset1.examples, evalOne, {
      concurrency: 1
    });

    const n = results.length;
    const meanReward =
      n === 0
        ? 0
        : results.reduce((acc, r) => acc + r.reward, 0) / Math.max(1, n);

    const summary: EvalSummaryV1 = {
      evalVersion: 1,
      kind: "scored",
      reward: meanReward,
      datasetId: dataset1.datasetId,
      datasetHash: dHash,
      metricId: rewardId,
      metricVersion: rewardVersion,
      n,
      selectedExampleIdsHash: selectionHash,
      ...(options.sample ? { seed: options.sample.seed } : {})
    };

    return {
      summary,
      ...(options.includeExampleDetails ? { examples: results } : {})
    };
  }).pipe(
    Effect.catchAll((cause) =>
      Effect.fail(
        EvalError.make({
          message: "Evaluation failed",
          cause
        })
      )
    )
  );
}
