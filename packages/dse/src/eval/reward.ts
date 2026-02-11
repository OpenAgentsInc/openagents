import { Effect, Schema } from "effect";

import type { Metric, MetricReportV1 } from "./metric.js";
import { evaluateMetric, type MetricEnv } from "./metric.js";
import { BlobStoreService } from "../runtime/blobStore.js";
import type { PredictReceiptV1 } from "../runtime/receipt.js";

const messageFromUnknown = (value: unknown): string => {
  if (value && typeof value === "object") {
    const record = value as Record<string, unknown>;
    if (typeof record.message === "string") return record.message;
  }
  return String(value);
};

export type RewardSignalReportV1 = {
  readonly signalId: string;
  readonly weight: number;
  readonly score: number;
  readonly notes?: string | undefined;
  readonly metric?: MetricReportV1 | undefined;
};

export class RewardError extends Schema.TaggedError<RewardError>()("RewardError", {
  message: Schema.String,
  cause: Schema.optional(Schema.Defect)
}) {}

export type RewardSignal<I, O, Y> = {
  readonly signalId: string;
  readonly weight: number;
  readonly evaluate: (args: RewardSignalArgs<I, O, Y>) => Effect.Effect<
    RewardSignalReportV1,
    RewardError,
    MetricEnv
  >;
};

export type RewardSignalArgs<I, O, Y> = {
  readonly input: I;
  readonly expected: Y;
  readonly pred: O | null;
  readonly predictReceipt?: PredictReceiptV1 | undefined;
  readonly predictError?: { readonly errorName: string; readonly message: string } | undefined;
  readonly toolFailures?: number | undefined;
};

export type RewardBundle<I, O, Y> = {
  readonly rewardId: string;
  readonly rewardVersion: number;
  readonly signals: ReadonlyArray<RewardSignal<I, O, Y>>;
};

export function makeBundle<I, O, Y>(options: {
  readonly rewardId: string;
  readonly rewardVersion: number;
  readonly signals: ReadonlyArray<RewardSignal<I, O, Y>>;
}): RewardBundle<I, O, Y> {
  return {
    rewardId: options.rewardId,
    rewardVersion: options.rewardVersion,
    signals: options.signals
  };
}

export function computeReward(
  reports: ReadonlyArray<RewardSignalReportV1>
): { readonly reward: number; readonly totalWeight: number } {
  const weights = reports.map((r) => (Number.isFinite(r.weight) ? r.weight : 0));
  const totalWeight = weights.reduce((a, b) => a + b, 0);
  if (totalWeight <= 0) return { reward: 0, totalWeight: 0 };

  const weighted =
    reports.reduce((acc, r) => acc + clamp01(r.score) * r.weight, 0) / totalWeight;
  return { reward: clamp01(weighted), totalWeight };
}

export function signalFormatValidity<I, O, Y>(options?: {
  readonly signalId?: string;
  readonly weight?: number;
}): RewardSignal<I, O, Y> {
  const signalId = options?.signalId ?? "format_validity.v1";
  const weight = options?.weight ?? 0.3;
  return {
    signalId,
    weight,
    evaluate: (args) =>
      Effect.succeed({
        signalId,
        weight,
        score: args.pred ? 1 : 0,
        ...(args.pred ? {} : { notes: args.predictError?.message ?? "Predict failed" })
      })
  };
}

export function signalMetric<I, O, Y>(metric: Metric<I, O, Y>, options?: {
  readonly weight?: number;
  readonly signalId?: string;
}): RewardSignal<I, O, Y> {
  const signalId = options?.signalId ?? `${metric.metricId}.signal.v1`;
  const weight = options?.weight ?? 0.6;
  return {
    signalId,
    weight,
    evaluate: (args) =>
      Effect.gen(function* () {
        if (!args.pred) {
          return {
            signalId,
            weight,
            score: 0,
            notes: "No prediction to score"
          } satisfies RewardSignalReportV1;
        }

        const report = yield* evaluateMetric(metric, {
          input: args.input,
          pred: args.pred,
          expected: args.expected
        });

        return {
          signalId,
          weight,
          score: report.score,
          metric: report,
          ...(report.notes ? { notes: report.notes } : {})
        } satisfies RewardSignalReportV1;
      }).pipe(
        Effect.catchAll((cause) =>
          Effect.fail(
            RewardError.make({
              message: "Metric signal failed",
              cause
            })
          )
        )
      )
  };
}

export function signalToolFailurePenalty<I, O, Y>(options?: {
  readonly signalId?: string;
  readonly weight?: number;
  readonly maxFailures?: number;
}): RewardSignal<I, O, Y> {
  const signalId = options?.signalId ?? "tool_failures.v1";
  const weight = options?.weight ?? 0.1;
  const maxFailures = Math.max(1, Math.floor(options?.maxFailures ?? 1));
  return {
    signalId,
    weight,
    evaluate: (args) => {
      const failures = Math.max(0, Math.floor(args.toolFailures ?? 0));
      const score = 1 - Math.min(1, failures / maxFailures);
      return Effect.succeed({
        signalId,
        weight,
        score,
        ...(failures > 0 ? { notes: `toolFailures=${failures}` } : {})
      });
    }
  };
}

export function signalEvidenceQuoteInBlobStore<I, O, Y>(options: {
  readonly signalId: string;
  readonly weight: number;
  readonly extractEvidence: (args: { readonly input: I; readonly pred: O; readonly expected: Y }) =>
    | { readonly blobId: string; readonly quote: string }
    | null;
  readonly maxQuoteChars?: number | undefined;
}): RewardSignal<I, O, Y> {
  const maxQuoteChars = Math.max(1, Math.floor(options.maxQuoteChars ?? 4000));
  return {
    signalId: options.signalId,
    weight: options.weight,
    evaluate: (args) =>
      Effect.gen(function* () {
        if (!args.pred) {
          return {
            signalId: options.signalId,
            weight: options.weight,
            score: 0,
            notes: "No prediction to score"
          } satisfies RewardSignalReportV1;
        }

        const ev = options.extractEvidence({
          input: args.input,
          pred: args.pred,
          expected: args.expected
        });
        if (!ev) {
          return {
            signalId: options.signalId,
            weight: options.weight,
            score: 0,
            notes: "Missing evidence"
          } satisfies RewardSignalReportV1;
        }

        const blobId = String(ev.blobId ?? "").trim();
        const quote0 = String(ev.quote ?? "");
        const quote = quote0.length > maxQuoteChars ? quote0.slice(0, maxQuoteChars) : quote0;
        if (!blobId) {
          return {
            signalId: options.signalId,
            weight: options.weight,
            score: 0,
            notes: "Evidence blobId is empty"
          } satisfies RewardSignalReportV1;
        }
        if (quote.trim().length === 0) {
          return {
            signalId: options.signalId,
            weight: options.weight,
            score: 0,
            notes: "Evidence quote is empty"
          } satisfies RewardSignalReportV1;
        }

        const blobs = yield* BlobStoreService;
        const attempt = yield* Effect.either(blobs.getText(blobId));
        if (attempt._tag === "Left") {
          const msg = messageFromUnknown(attempt.left);
          return {
            signalId: options.signalId,
            weight: options.weight,
            score: 0,
            notes: msg
          } satisfies RewardSignalReportV1;
        }

        const text = attempt.right;
        if (text == null) {
          return {
            signalId: options.signalId,
            weight: options.weight,
            score: 0,
            notes: `Missing blob (blobId=${blobId})`
          } satisfies RewardSignalReportV1;
        }

        const ok = text.includes(quote);
        return {
          signalId: options.signalId,
          weight: options.weight,
          score: ok ? 1 : 0,
          ...(ok ? {} : { notes: "Quote not found in blob" })
        } satisfies RewardSignalReportV1;
      })
  };
}

export function signalPredictCostPenalty<I, O, Y>(options?: {
  readonly signalId?: string;
  readonly weight?: number;
  readonly targetDurationMs?: number;
  readonly targetLmCalls?: number;
  readonly targetToolCalls?: number;
}): RewardSignal<I, O, Y> {
  const signalId = options?.signalId ?? "predict_cost.v1";
  const weight = options?.weight ?? 0.1;
  const targetDurationMs = Math.max(1, Math.floor(options?.targetDurationMs ?? 800));
  const targetLmCalls = Math.max(1, Math.floor(options?.targetLmCalls ?? 1));
  const targetToolCalls = Math.max(0, Math.floor(options?.targetToolCalls ?? 0));

  return {
    signalId,
    weight,
    evaluate: (args) =>
      Effect.sync(() => {
        const receipt = args.predictReceipt;
        if (!receipt) {
          return {
            signalId,
            weight,
            score: 0,
            notes: "Missing predictReceipt"
          } satisfies RewardSignalReportV1;
        }

        const usage = receipt.budget?.usage;
        const durationMs = receipt.timing?.durationMs ?? usage?.elapsedMs ?? 0;
        const lmCalls = usage?.lmCalls ?? 0;
        const toolCalls = usage?.toolCalls ?? 0;

        const durScore = durationMs <= 0 ? 1 : clamp01(targetDurationMs / durationMs);
        const lmScore = lmCalls <= 0 ? 1 : clamp01(targetLmCalls / lmCalls);
        const toolScore = toolCalls <= 0 ? 1 : clamp01((targetToolCalls + 1) / (toolCalls + 1));

        const score = clamp01(durScore * lmScore * toolScore);
        const notes = `durationMs=${durationMs} lmCalls=${lmCalls} toolCalls=${toolCalls}`;
        return { signalId, weight, score, notes } satisfies RewardSignalReportV1;
      })
  };
}

function clamp01(n: number): number {
  if (!Number.isFinite(n)) return 0;
  if (n <= 0) return 0;
  if (n >= 1) return 1;
  return n;
}
