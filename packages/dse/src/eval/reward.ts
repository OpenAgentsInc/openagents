import { Effect, Schema } from "effect";

import type { Metric, MetricReportV1 } from "./metric.js";
import { evaluateMetric, type MetricEnv } from "./metric.js";

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

function clamp01(n: number): number {
  if (!Number.isFinite(n)) return 0;
  if (n <= 0) return 0;
  if (n >= 1) return 1;
  return n;
}
