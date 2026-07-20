import { Effect, Result, Schema as S } from "effect";

import { canonicalStringify } from "../internal/canonical.js";
import { sha256Hex } from "../internal/sha256.js";
import {
  EVALUATION_REPORT_SCHEMA_LITERAL,
  EvaluationReport,
  indexExamples,
  rewardBundle,
  type CompiledProgram,
  type DatasetRevision,
  type DatasetSplitName,
  type DseSignature,
  type DseUsageTruth,
  type ExampleId,
  type ExampleScore,
  type Metric,
} from "../contract/index.js";
import { predict, type PredictDeps } from "../runtime/predict.js";
import { DseModel } from "../runtime/model.js";

/**
 * Holdout-aware evaluation.
 *
 * The evaluator scores a compiled program over one named split by running
 * `Predict` for each example and applying the metric. It fails closed on an
 * empty split or an unknown example, and it records the honest aggregate usage
 * truth. Candidate generation never calls this on holdout; only the final winner
 * is scored on holdout, so holdout labels stay inaccessible to the search.
 */

export class EvaluationError extends S.TaggedErrorClass<EvaluationError>()("dse/EvaluationError", {
  reason: S.Literals(["empty_split", "unknown_example", "bad_reference"]),
  detail: S.String,
}) {}

const decodeReport = S.decodeUnknownSync(EvaluationReport);

const combineUsageTruth = (truths: ReadonlyArray<DseUsageTruth>): DseUsageTruth => {
  if (truths.some((truth) => truth === "unknown")) return "unknown";
  if (truths.some((truth) => truth === "estimated")) return "estimated";
  return "exact";
};

const mean = (values: ReadonlyArray<number>): number =>
  values.length === 0 ? 0 : values.reduce((sum, value) => sum + value, 0) / values.length;

export interface EvaluateArgs<I, O> {
  readonly signature: DseSignature<I, O>;
  readonly program: CompiledProgram;
  readonly candidateId: string;
  readonly metric: Metric<O>;
  readonly revision: DatasetRevision;
  readonly splitName: DatasetSplitName;
  readonly splitIds: ReadonlyArray<typeof ExampleId.Type>;
  readonly deps: PredictDeps;
}

export const evaluateCandidate = <I, O>(
  args: EvaluateArgs<I, O>,
): Effect.Effect<EvaluationReport, EvaluationError, DseModel> =>
  Effect.gen(function* () {
    if (args.splitIds.length === 0) {
      return yield* new EvaluationError({ reason: "empty_split", detail: args.splitName });
    }
    const byId = indexExamples(args.revision);
    // Decoding is pure; a Result decoder keeps the schema requirement out of the
    // Effect context so the only requirement is the injected model.
    const decodeInput = S.decodeUnknownResult(args.signature.input);
    const decodeExpected = S.decodeUnknownResult(args.signature.output);

    const scores: ExampleScore[] = [];
    const usageTruths: DseUsageTruth[] = [];

    for (const exampleId of args.splitIds) {
      const example = byId.get(exampleId);
      if (example === undefined) {
        return yield* new EvaluationError({ reason: "unknown_example", detail: exampleId });
      }

      const inputResult = decodeInput(example.input);
      if (Result.isFailure(inputResult)) {
        return yield* new EvaluationError({
          reason: "bad_reference",
          detail: `${exampleId} input`,
        });
      }
      const input = inputResult.success;
      const expectedResult = decodeExpected(example.expected);
      if (Result.isFailure(expectedResult)) {
        return yield* new EvaluationError({
          reason: "bad_reference",
          detail: `${exampleId} expected`,
        });
      }
      const expected = expectedResult.success;

      const outcome = yield* predict({
        signature: args.signature,
        candidateId: args.candidateId,
        program: args.program,
        input,
        deps: args.deps,
      }).pipe(Effect.result);

      const actual = Result.isSuccess(outcome) ? outcome.success.output : null;
      const formatValid = Result.isSuccess(outcome);
      if (Result.isSuccess(outcome)) usageTruths.push(outcome.success.receipt.usageTruth);

      const components = args.metric.score({ expected, actual, formatValid });
      const bundle = rewardBundle(components);
      scores.push({
        exampleId,
        quality: bundle.quality,
        resource: bundle.resource,
        score: bundle.score,
        components,
        formatValid,
        decodeRepaired:
          Result.isSuccess(outcome) && outcome.success.receipt.decodeOutcome === "repaired",
      });
    }

    const aggregateQuality = mean(scores.map((score) => score.quality));
    const aggregateResource = mean(scores.map((score) => score.resource));
    const aggregateScore = mean(scores.map((score) => score.score));
    const body = {
      signatureId: args.signature.signatureId,
      candidateId: args.candidateId,
      datasetRevisionId: args.revision.revisionId,
      split: args.splitName,
      metricId: args.metric.metricId,
      perExample: scores,
      aggregateQuality,
      aggregateResource,
      aggregateScore,
      usageTruth: combineUsageTruth(usageTruths),
    };
    return decodeReport({
      schema: EVALUATION_REPORT_SCHEMA_LITERAL,
      ...body,
      digest: sha256Hex(canonicalStringify(body)),
    });
  });
