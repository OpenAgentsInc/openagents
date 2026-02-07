import { Effect, Schema } from "effect";

import type { DseCompiledArtifactV1 } from "../compiledArtifact.js";
import * as CompiledArtifact from "../compiledArtifact.js";
import type { DseParams } from "../params.js";
import { emptyParamsV1 } from "../params.js";
import type { DseSignature } from "../signature.js";
import {
  paramsHash,
  promptIrHash,
  schemaJsonHash,
  sha256IdFromCanonicalJson,
} from "../hashes.js";

import type { EvalEnv } from "../eval/evaluate.js";
import type { Dataset } from "../eval/dataset.js";
import { filter as filterDataset, datasetHash as datasetHashEffect } from "../eval/dataset.js";
import type { RewardBundle } from "../eval/reward.js";
import { evaluate } from "../eval/evaluate.js";

import type { CompileJobSpecV1, CompileOptimizerV1, CompileSearchSpaceV1 } from "./job.js";
import { compileJobHash } from "./job.js";

export class CompileError extends Schema.TaggedError<CompileError>()("CompileError", {
  message: Schema.String,
  cause: Schema.optional(Schema.Defect)
}) {}

export type CandidateScoreV1 = {
  readonly compiled_id: string;
  readonly params: DseParams;
  readonly reward: number;
};

export type CompileRunReportV1 = {
  readonly jobHash: string;
  readonly datasetHash: string;
  readonly evaluatedCandidates: ReadonlyArray<CandidateScoreV1>;
  readonly best: CandidateScoreV1;
  readonly trainReward: number;
  readonly holdoutReward: number;
};

export type CompileResultV1 = {
  readonly artifact: DseCompiledArtifactV1;
  readonly report: CompileRunReportV1;
};

export type CompileOptions<I, O, Y> = {
  readonly signature: DseSignature<I, O>;
  readonly baseParams?: DseParams | undefined;
  readonly dataset: Dataset<I, Y>;
  readonly reward: RewardBundle<I, O, Y>;
  readonly searchSpace: CompileSearchSpaceV1;
  readonly optimizer: CompileOptimizerV1;
  readonly provenance?: {
    readonly compilerVersion?: string | undefined;
    readonly gitSha?: string | undefined;
  } | undefined;
};

function sortUniq(ids: ReadonlyArray<string>): ReadonlyArray<string> {
  const cleaned = ids.map((id) => id.trim()).filter((id) => id.length > 0);
  cleaned.sort();
  return cleaned.filter((id, i) => (i === 0 ? true : id !== cleaned[i - 1]));
}

function mergeParams(base: DseParams, patch: Partial<DseParams>): DseParams {
  // DseParams is shallow with nested objects. We merge the known nested objects.
  const next: any = { ...base };
  for (const [k, v] of Object.entries(patch)) {
    if (v === undefined) continue;
    if (k === "instruction" || k === "fewShot" || k === "model" || k === "decode" || k === "tools") {
      next[k] = { ...(base as any)[k], ...(v as any) };
    } else {
      next[k] = v;
    }
  }
  return next as DseParams;
}

function pickBest(candidates: ReadonlyArray<CandidateScoreV1>): CandidateScoreV1 {
  if (candidates.length === 0) {
    throw new Error("No candidates were evaluated");
  }
  const EPS = 1e-12;
  let best = candidates[0]!;
  for (let i = 1; i < candidates.length; i++) {
    const c = candidates[i]!;
    const dr = c.reward - best.reward;
    if (dr > EPS) {
      best = c;
      continue;
    }
    if (Math.abs(dr) <= EPS) {
      // Deterministic tie-breaker.
      if (c.compiled_id.localeCompare(best.compiled_id) < 0) best = c;
    }
  }
  return best;
}

function hasSplit(dataset: Dataset<any, any>, split: string): boolean {
  return dataset.examples.some((e) => e.split === split);
}

function defaultTrainHoldoutSplits<I, Y>(dataset: Dataset<I, Y>): {
  readonly train: Dataset<I, Y>;
  readonly holdout: Dataset<I, Y>;
} {
  const hasAnySplit = dataset.examples.some((e) => e.split != null);
  if (!hasAnySplit) return { train: dataset, holdout: dataset };
  const train = hasSplit(dataset, "train")
    ? filterDataset(dataset, { split: "train" })
    : dataset;
  const holdout = hasSplit(dataset, "holdout")
    ? filterDataset(dataset, { split: "holdout" })
    : train;
  return { train, holdout };
}

function instructionGrid(
  base: DseParams,
  searchSpace: CompileSearchSpaceV1
): ReadonlyArray<DseParams> {
  const variants = searchSpace.instructionVariants;
  if (!variants || variants.length === 0) return [base];

  const sorted = [...variants].sort((a, b) => a.id.localeCompare(b.id));
  return sorted.map((v) =>
    mergeParams(base, { instruction: { text: v.text } })
  );
}

function greedyFewShotForward(
  base: DseParams,
  searchSpace: CompileSearchSpaceV1
): { readonly initial: DseParams; readonly candidateIds: ReadonlyArray<string>; readonly kMax: number } {
  const fs = searchSpace.fewShot;
  if (!fs) {
    return { initial: base, candidateIds: [], kMax: 0 };
  }
  const candidateIds = sortUniq(fs.candidateExampleIds);
  const kMax = Math.max(0, Math.floor(fs.kMax));

  const selected0 = sortUniq(base.fewShot?.exampleIds ?? []);
  const initial =
    selected0.length > 0
      ? mergeParams(base, { fewShot: { exampleIds: selected0, k: selected0.length } })
      : base;

  return { initial, candidateIds, kMax };
}

export function compile<I, O, Y>(
  options: CompileOptions<I, O, Y>
): Effect.Effect<CompileResultV1, CompileError, EvalEnv> {
  return Effect.gen(function* () {
    const baseParams = options.baseParams ?? options.signature.defaults.params ?? emptyParamsV1;

    const { train, holdout } = defaultTrainHoldoutSplits(options.dataset);

    const dHash = yield* datasetHashEffect(options.dataset);
    const job: CompileJobSpecV1 = {
      format: "openagents.dse.compile_job",
      formatVersion: 1,
      signatureId: options.signature.id,
      datasetId: options.dataset.datasetId,
      metricId: options.reward.rewardId,
      searchSpace: options.searchSpace,
      optimizer: options.optimizer
    };

    const jobHash = yield* compileJobHash(job);

    const [inputSchemaHash, outputSchemaHash, promptHash] = yield* Effect.all([
      schemaJsonHash(options.signature.input),
      schemaJsonHash(options.signature.output),
      promptIrHash(options.signature.prompt)
    ]);

    const evaluateParams = (params: DseParams, dataset: Dataset<I, Y>) =>
      Effect.gen(function* () {
        const compiled_id = yield* paramsHash(params);
        const artifact: DseCompiledArtifactV1 = {
          format: "openagents.dse.compiled_artifact",
          formatVersion: 1,
          signatureId: options.signature.id,
          compiled_id,
          createdAt: new Date().toISOString(),
          hashes: {
            inputSchemaHash,
            outputSchemaHash,
            promptIrHash: promptHash,
            paramsHash: compiled_id
          },
          params: params as any,
          eval: { evalVersion: 1, kind: "unscored" },
          optimizer: { id: "compile_candidate" },
          provenance: {}
        };

        const evalRes = yield* evaluate({
          signature: options.signature,
          artifact,
          dataset,
          reward: options.reward
        });

        return {
          compiled_id,
          params,
          reward: evalRes.summary.reward ?? 0,
          evalSummary: evalRes.summary
        };
      });

    const runInstructionGrid = () =>
      Effect.gen(function* () {
        const paramSets = instructionGrid(baseParams, options.searchSpace);
        const scored = yield* Effect.forEach(
          paramSets,
          (p) =>
            evaluateParams(p, train).pipe(
              Effect.map((r) => ({ compiled_id: r.compiled_id, params: r.params, reward: r.reward } satisfies CandidateScoreV1))
            ),
          { concurrency: 1 }
        );
        const best = pickBest(scored);
        return { scored, best };
      });

    const runGreedyFewShot = (start: CandidateScoreV1) =>
      Effect.gen(function* () {
        const { initial, candidateIds, kMax } = greedyFewShotForward(start.params, options.searchSpace);
        if (kMax === 0 || candidateIds.length === 0) {
          return { scored: [] as CandidateScoreV1[], best: start };
        }

        // Ensure we start from a scored state (so "no improvement" can be decided deterministically).
        let current = yield* evaluateParams(initial, train).pipe(
          Effect.map((r) => ({ compiled_id: r.compiled_id, params: r.params, reward: r.reward } satisfies CandidateScoreV1))
        );

        const evaluated: Array<CandidateScoreV1> = [];
        evaluated.push(current);

        let selected = sortUniq(current.params.fewShot?.exampleIds ?? []);
        let remaining = candidateIds.filter((id) => !selected.includes(id));

        const EPS = 1e-12;
        while (selected.length < kMax && remaining.length > 0) {
          const stepCandidates = yield* Effect.forEach(
            remaining,
            (id) => {
              const nextSelected = [...selected, id];
              const nextParams = mergeParams(current.params, {
                fewShot: { exampleIds: nextSelected, k: nextSelected.length }
              });
              return evaluateParams(nextParams, train).pipe(
                Effect.map((r) => ({ compiled_id: r.compiled_id, params: r.params, reward: r.reward } satisfies CandidateScoreV1))
              );
            },
            { concurrency: 1 }
          );

          for (const c of stepCandidates) evaluated.push(c);

          const bestStep = pickBest(stepCandidates);
          if (bestStep.reward - current.reward <= EPS) {
            break;
          }

          current = bestStep;
          selected = sortUniq(current.params.fewShot?.exampleIds ?? []);
          remaining = remaining.filter((id) => !selected.includes(id));
        }

        // Return only the candidates evaluated in this phase (excluding the incoming start param).
        return { scored: evaluated, best: current };
      });

    const optimizerId = options.optimizer.id;
    let evaluatedCandidates: Array<CandidateScoreV1> = [];

    let best: CandidateScoreV1;
    if (optimizerId === "instruction_grid.v1") {
      const r = yield* runInstructionGrid();
      evaluatedCandidates = [...r.scored];
      best = r.best;
    } else if (optimizerId === "fewshot_greedy_forward.v1") {
      // Start from base (as "start") so we score it first.
      const baseScore = yield* evaluateParams(baseParams, train).pipe(
        Effect.map((r) => ({ compiled_id: r.compiled_id, params: r.params, reward: r.reward } satisfies CandidateScoreV1))
      );
      const r = yield* runGreedyFewShot(baseScore);
      evaluatedCandidates = [baseScore, ...r.scored];
      best = r.best;
    } else {
      // Joint: instruction grid, then greedy few-shot from the best instruction.
      const r1 = yield* runInstructionGrid();
      const r2 = yield* runGreedyFewShot(r1.best);
      evaluatedCandidates = [...r1.scored, ...r2.scored];
      best = r2.best;
    }

    const trainEval = yield* evaluateParams(best.params, train);
    const holdoutEval = yield* evaluateParams(best.params, holdout);

    const paramsHashValue = yield* paramsHash(best.params);

    const searchSpaceHash = yield* sha256IdFromCanonicalJson(options.searchSpace);

    const artifact: DseCompiledArtifactV1 = {
      format: "openagents.dse.compiled_artifact",
      formatVersion: 1,
      signatureId: options.signature.id,
      compiled_id: paramsHashValue,
      createdAt: new Date().toISOString(),
      hashes: {
        inputSchemaHash,
        outputSchemaHash,
        promptIrHash: promptHash,
        paramsHash: paramsHashValue
      },
      params: best.params as any,
      eval: holdoutEval.evalSummary,
      optimizer: {
        id: options.optimizer.id,
        config: {
          ...(options.optimizer.config ? { config: options.optimizer.config } : {}),
          trainReward: trainEval.reward,
          holdoutReward: holdoutEval.reward,
          candidates: evaluatedCandidates.length
        },
        iterations: evaluatedCandidates.length
      },
      provenance: {
        ...(options.provenance?.compilerVersion
          ? { compilerVersion: options.provenance.compilerVersion }
          : {}),
        ...(options.provenance?.gitSha ? { gitSha: options.provenance.gitSha } : {}),
        datasetId: options.dataset.datasetId,
        datasetHash: dHash,
        metricId: options.reward.rewardId,
        searchSpaceHash
      }
    };

    // Validate artifact schema to avoid emitting invalid artifacts from optimizers.
    const validated = yield* Effect.try({
      try: () =>
        Schema.decodeUnknownSync(CompiledArtifact.DseCompiledArtifactV1Schema)(artifact),
      catch: (cause) =>
        CompileError.make({
          message: "Compiled artifact failed validation",
          cause
        })
    });

    return {
      artifact: validated,
      report: {
        jobHash,
        datasetHash: dHash,
        evaluatedCandidates,
        best,
        trainReward: trainEval.reward,
        holdoutReward: holdoutEval.reward
      }
    };
  }).pipe(
    Effect.catchAll((cause) =>
      Effect.fail(
        CompileError.make({
          message: "Compile failed",
          cause
        })
      )
    )
  );
}
