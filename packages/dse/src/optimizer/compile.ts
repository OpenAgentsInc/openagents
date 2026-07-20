import { Effect, Schema as S } from "effect";

import {
  makeCandidateArtifact,
  type CandidateArtifact,
  type CompiledProgram,
  type DatasetRevision,
  type DatasetSplit,
  type DseSignature,
  type DseTimestamp,
  type EvaluationReport,
  type Metric,
  type SearchPlan,
} from "../contract/index.js";
import { type PredictDeps } from "../runtime/predict.js";
import { DseModel } from "../runtime/model.js";
import { EvaluationError, evaluateCandidate } from "./evaluate.js";
import { generateCandidates, type CandidateKnobs } from "./search.js";

/**
 * The offline compile job.
 *
 * Compile validates the split, generates a bounded deterministic candidate set,
 * enforces the rollout budget, scores every candidate on VALIDATION, selects the
 * winner (best score, stable-hash tie-break), and scores only the winner on
 * HOLDOUT. It emits an immutable winner artifact plus the validation and holdout
 * reports. Compile never promotes; promotion is a separate independently-reviewed
 * decision.
 */

export class CompileError extends S.TaggedErrorClass<CompileError>()("dse/CompileError", {
  reason: S.Literals(["missing_holdout", "contaminated", "budget_exceeded", "no_candidates"]),
  detail: S.String,
}) {}

export interface CompileResult {
  readonly searchPlan: SearchPlan;
  readonly winner: CandidateArtifact;
  readonly validationReport: EvaluationReport;
  readonly holdoutReport: EvaluationReport;
  readonly evaluatedCount: number;
}

export interface CompileArgs<I, O> {
  readonly signature: DseSignature<I, O>;
  readonly base: CompiledProgram;
  readonly knobs: CandidateKnobs;
  readonly searchPlan: SearchPlan;
  readonly revision: DatasetRevision;
  readonly split: DatasetSplit;
  readonly metric: Metric<O>;
  readonly producedAt: typeof DseTimestamp.Type;
  readonly deps: PredictDeps;
}

interface ScoredCandidate {
  readonly artifact: CandidateArtifact;
  readonly validationReport: EvaluationReport;
}

const betterThan = (left: ScoredCandidate, right: ScoredCandidate): boolean => {
  const leftScore = left.validationReport.aggregateScore;
  const rightScore = right.validationReport.aggregateScore;
  if (leftScore !== rightScore) return leftScore > rightScore;
  // Stable, reproducible tie-break by content-addressed identity.
  return left.artifact.digest < right.artifact.digest;
};

export const compileSignature = <I, O>(
  args: CompileArgs<I, O>,
): Effect.Effect<CompileResult, CompileError | EvaluationError, DseModel> =>
  Effect.gen(function* () {
    // Fail closed on a contaminated or absent holdout even if a caller bypassed
    // the split builder.
    if (args.split.holdout.length === 0) {
      return yield* new CompileError({ reason: "missing_holdout", detail: "holdout is empty" });
    }
    const holdoutSet = new Set<string>(args.split.holdout);
    for (const id of [...args.split.train, ...args.split.validation]) {
      if (holdoutSet.has(id)) {
        return yield* new CompileError({ reason: "contaminated", detail: id });
      }
    }

    const budget = args.searchPlan.budget;
    const programs = generateCandidates({
      algorithm: args.searchPlan.algorithm,
      base: args.base,
      knobs: args.knobs,
      cap: Math.min(args.searchPlan.candidateCap, budget.maxCandidates),
    });
    if (programs.length === 0) {
      return yield* new CompileError({ reason: "no_candidates", detail: "empty candidate set" });
    }

    const rollouts = programs.length * args.split.validation.length;
    if (rollouts > budget.maxRollouts) {
      return yield* new CompileError({
        reason: "budget_exceeded",
        detail: `${rollouts} rollouts exceed ${budget.maxRollouts}`,
      });
    }

    let best: ScoredCandidate | null = null;
    for (const program of programs) {
      const artifact = makeCandidateArtifact({
        signatureId: args.signature.signatureId,
        datasetRevisionId: args.revision.revisionId,
        searchPlan: args.searchPlan,
        program,
        producedAt: args.producedAt,
      });
      const validationReport = yield* evaluateCandidate({
        signature: args.signature,
        program,
        candidateId: artifact.candidateId,
        metric: args.metric,
        revision: args.revision,
        splitName: "validation",
        splitIds: args.split.validation,
        deps: args.deps,
      });
      const scored: ScoredCandidate = { artifact, validationReport };
      if (best === null || betterThan(scored, best)) best = scored;
    }

    if (best === null) {
      return yield* new CompileError({ reason: "no_candidates", detail: "no candidate evaluated" });
    }
    const winner = best;
    const holdoutReport = yield* evaluateCandidate({
      signature: args.signature,
      program: winner.artifact.program,
      candidateId: winner.artifact.candidateId,
      metric: args.metric,
      revision: args.revision,
      splitName: "holdout",
      splitIds: args.split.holdout,
      deps: args.deps,
    });

    return {
      searchPlan: args.searchPlan,
      winner: winner.artifact,
      validationReport: winner.validationReport,
      holdoutReport,
      evaluatedCount: programs.length,
    };
  });
