import { Effect, Schema } from "effect";

import type { DseCompiledArtifactV1 } from "../compiledArtifact.js";
import type { DseSignature } from "../signature.js";
import type { PredictEnv, PredictError } from "../runtime/predict.js";
import { make as makePredict } from "../runtime/predict.js";
import { layerInMemory as layerPolicyInMemory, PolicyRegistryService } from "../runtime/policyRegistry.js";

export type MetricReportV1 = {
  readonly metricId: string;
  readonly metricVersion: number;
  readonly kind: "deterministic" | "judge";
  readonly score: number;
  readonly notes?: string | undefined;
  readonly judge?: {
    readonly signatureId: string;
    readonly compiled_id: string;
  } | undefined;
};

export class MetricError extends Schema.TaggedError<MetricError>()("MetricError", {
  message: Schema.String,
  cause: Schema.optional(Schema.Defect)
}) {}

export type DeterministicMetric<O, Y> = {
  readonly metricId: string;
  readonly metricVersion: number;
  readonly kind: "deterministic";
  readonly score: (pred: O, expected: Y) => number;
  readonly notes?: (pred: O, expected: Y) => string | undefined;
};

export function deterministic<O, Y>(options: {
  readonly metricId: string;
  readonly metricVersion: number;
  readonly score: (pred: O, expected: Y) => number;
  readonly notes?: (pred: O, expected: Y) => string | undefined;
}): DeterministicMetric<O, Y> {
  return {
    metricId: options.metricId,
    metricVersion: options.metricVersion,
    kind: "deterministic",
    score: options.score,
    ...(options.notes ? { notes: options.notes } : {})
  };
}

export type JudgeMetric<I, O, Y, JIn, JOut> = {
  readonly metricId: string;
  readonly metricVersion: number;
  readonly kind: "judge";
  readonly judge: {
    readonly signature: DseSignature<JIn, JOut>;
    readonly artifact: DseCompiledArtifactV1;
  };
  readonly buildJudgeInput: (args: {
    readonly input: I;
    readonly pred: O;
    readonly expected: Y;
  }) => JIn;
  readonly scoreFromJudgeOutput: (out: JOut) => number;
  readonly notesFromJudgeOutput?: (out: JOut) => string | undefined;
};

export function judge<I, O, Y, JIn, JOut>(options: {
  readonly metricId: string;
  readonly metricVersion: number;
  readonly judgeSignature: DseSignature<JIn, JOut>;
  readonly judgeArtifact: DseCompiledArtifactV1;
  readonly buildJudgeInput: (args: { readonly input: I; readonly pred: O; readonly expected: Y }) => JIn;
  readonly scoreFromJudgeOutput: (out: JOut) => number;
  readonly notesFromJudgeOutput?: (out: JOut) => string | undefined;
}): JudgeMetric<I, O, Y, JIn, JOut> {
  return {
    metricId: options.metricId,
    metricVersion: options.metricVersion,
    kind: "judge",
    judge: { signature: options.judgeSignature, artifact: options.judgeArtifact },
    buildJudgeInput: options.buildJudgeInput,
    scoreFromJudgeOutput: options.scoreFromJudgeOutput,
    ...(options.notesFromJudgeOutput ? { notesFromJudgeOutput: options.notesFromJudgeOutput } : {})
  };
}

export type Metric<I, O, Y> =
  | DeterministicMetric<O, Y>
  | JudgeMetric<I, O, Y, any, any>;

export type MetricEnv = Exclude<PredictEnv, PolicyRegistryService>;

export function evaluateMetric<I, O, Y>(
  metric: Metric<I, O, Y>,
  args: { readonly input: I; readonly pred: O; readonly expected: Y }
): Effect.Effect<MetricReportV1, MetricError, MetricEnv> {
  if (metric.kind === "deterministic") {
    const score = clamp01(metric.score(args.pred, args.expected));
    return Effect.succeed({
      metricId: metric.metricId,
      metricVersion: metric.metricVersion,
      kind: "deterministic",
      score,
      ...(metric.notes ? { notes: metric.notes(args.pred, args.expected) } : {})
    });
  }

  // Judge metrics require LM access; we implement them as an Effect that expects
  // Predict's environment (LM client, blob store, receipts) and provides its own
  // pinned policy registry.
  const judgeSig = metric.judge.signature;
  const judgeArtifact = metric.judge.artifact;
  const predict = makePredict(judgeSig);
  const policy = layerPolicyInMemory({
    activeBySignatureId: { [judgeSig.id]: judgeArtifact.compiled_id },
    artifacts: [judgeArtifact]
  });

  const judgeInput = metric.buildJudgeInput(args);
  return predict(judgeInput).pipe(
    Effect.provide(policy),
    Effect.map((out) => {
      const score = clamp01(metric.scoreFromJudgeOutput(out));
      return {
        metricId: metric.metricId,
        metricVersion: metric.metricVersion,
        kind: "judge",
        score,
        ...(metric.notesFromJudgeOutput
          ? { notes: metric.notesFromJudgeOutput(out) }
          : {}),
        judge: {
          signatureId: judgeSig.id,
          compiled_id: judgeArtifact.compiled_id
        }
      } satisfies MetricReportV1;
    }),
    Effect.catchAll((cause: PredictError) =>
      Effect.fail(
        MetricError.make({
          message: "Judge metric prediction failed",
          cause
        })
      )
    )
  );
}

function clamp01(n: number): number {
  if (!Number.isFinite(n)) return 0;
  if (n <= 0) return 0;
  if (n >= 1) return 1;
  return n;
}
