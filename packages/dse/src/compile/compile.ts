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
import { evaluate, type EvalExampleResultV1 } from "../eval/evaluate.js";

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

const asRecord = (value: unknown): Record<string, unknown> | null =>
  value && typeof value === "object" ? (value as Record<string, unknown>) : null;

function sortUniq(ids: ReadonlyArray<string>): ReadonlyArray<string> {
  const cleaned = ids.map((id) => id.trim()).filter((id) => id.length > 0);
  cleaned.sort();
  return cleaned.filter((id, i) => (i === 0 ? true : id !== cleaned[i - 1]));
}

function mergeParams(base: DseParams, patch: Partial<DseParams>): DseParams {
  return {
    ...base,
    ...(patch.paramsVersion !== undefined ? { paramsVersion: patch.paramsVersion } : {}),
    ...(patch.strategy !== undefined
      ? { strategy: { ...(base.strategy ?? {}), ...patch.strategy } }
      : {}),
    ...(patch.instruction !== undefined
      ? { instruction: { ...(base.instruction ?? {}), ...patch.instruction } }
      : {}),
    ...(patch.fewShot !== undefined
      ? { fewShot: { ...(base.fewShot ?? {}), ...patch.fewShot } }
      : {}),
    ...(patch.model !== undefined
      ? { model: { ...(base.model ?? {}), ...patch.model } }
      : {}),
    ...(patch.modelRoles !== undefined
      ? { modelRoles: { ...(base.modelRoles ?? {}), ...patch.modelRoles } }
      : {}),
    ...(patch.decode !== undefined
      ? { decode: { ...(base.decode ?? {}), ...patch.decode } }
      : {}),
    ...(patch.tools !== undefined
      ? { tools: { ...(base.tools ?? {}), ...patch.tools } }
      : {}),
    ...(patch.rlmLite !== undefined
      ? { rlmLite: { ...(base.rlmLite ?? {}), ...patch.rlmLite } }
      : {}),
    ...(patch.budgets !== undefined
      ? { budgets: { ...(base.budgets ?? {}), ...patch.budgets } }
      : {})
  };
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

function strategyGrid(base: DseParams, searchSpace: CompileSearchSpaceV1): ReadonlyArray<DseParams> {
  const variants = searchSpace.strategyVariants;
  if (!variants || variants.length === 0) return [base];
  const sorted = [...variants].sort((a, b) => a.id.localeCompare(b.id));
  return sorted.map((v) => mergeParams(base, { strategy: { id: v.strategyId } }));
}

function rlmControllerInstructionGrid(base: DseParams, searchSpace: CompileSearchSpaceV1): ReadonlyArray<DseParams> {
  const variants = searchSpace.rlmControllerInstructionVariants;
  if (!variants || variants.length === 0) return [base];
  const sorted = [...variants].sort((a, b) => a.id.localeCompare(b.id));
  return sorted.map((v) =>
    mergeParams(base, { rlmLite: { controllerInstructions: v.text } })
  );
}

function rlmChunkingPolicyGrid(base: DseParams, searchSpace: CompileSearchSpaceV1): ReadonlyArray<DseParams> {
  const variants = searchSpace.rlmChunkingPolicyVariants;
  if (!variants || variants.length === 0) return [base];
  const sorted = [...variants].sort((a, b) => a.id.localeCompare(b.id));
  return sorted.map((v) =>
    mergeParams(base, {
      rlmLite: {
        chunkDefaults: {
          chunkChars: v.chunkChars,
          ...(typeof v.overlapChars === "number" ? { overlapChars: v.overlapChars } : {}),
          ...(typeof v.maxChunks === "number" ? { maxChunks: v.maxChunks } : {})
        }
      }
    })
  );
}

function rlmSubRoleGrid(base: DseParams, searchSpace: CompileSearchSpaceV1): ReadonlyArray<DseParams> {
  const variants = searchSpace.rlmSubRoleVariants;
  if (!variants || variants.length === 0) return [base];
  const sorted = [...variants].sort((a, b) => a.id.localeCompare(b.id));
  return sorted.map((v) => mergeParams(base, { rlmLite: { subRole: v.subRole } }));
}

function budgetProfileGrid(base: DseParams, searchSpace: CompileSearchSpaceV1): ReadonlyArray<DseParams> {
  const profiles = searchSpace.budgetProfiles;
  if (!profiles || profiles.length === 0) return [base];
  const sorted = [...profiles].sort((a, b) => a.id.localeCompare(b.id));
  return sorted.map((p) => {
    const merged = { ...(base.budgets ?? {}), ...(p.budgets ?? {}) };
    return mergeParams(base, { budgets: merged });
  });
}

type EvaluatedParams = {
  readonly compiled_id: string;
  readonly params: DseParams;
  readonly reward: number;
  readonly evalSummary: CompiledArtifact.EvalSummaryV1;
  readonly examples?: ReadonlyArray<EvalExampleResultV1> | undefined;
};

const maxCandidatesFromOptimizerConfig = (config: unknown): number => {
  const record = asRecord(config);
  const raw = record?.maxCandidates;
  if (typeof raw !== "number" || !Number.isFinite(raw)) return 128;
  return Math.max(1, Math.min(500, Math.floor(raw)));
};

function knobsGrid(base: DseParams, searchSpace: CompileSearchSpaceV1, options?: { readonly maxCandidates?: number }): ReadonlyArray<DseParams> {
  const maxCandidates = Math.max(1, Math.floor(options?.maxCandidates ?? 128));

  // Deterministic staged grid expansion to limit combinatorial blowups.
  let seeds: Array<DseParams> = [base];

  const expand = (fn: (p: DseParams) => ReadonlyArray<DseParams>) => {
    const next: Array<DseParams> = [];
    for (const s of seeds) {
      for (const v of fn(s)) {
        next.push(v);
        if (next.length >= maxCandidates) break;
      }
      if (next.length >= maxCandidates) break;
    }
    seeds = next.length > 0 ? next : seeds;
  };

  expand((p) => strategyGrid(p, searchSpace));
  expand((p) => instructionGrid(p, searchSpace));
  expand((p) => rlmControllerInstructionGrid(p, searchSpace));
  expand((p) => rlmChunkingPolicyGrid(p, searchSpace));
  expand((p) => rlmSubRoleGrid(p, searchSpace));
  expand((p) => budgetProfileGrid(p, searchSpace));

  // Always include the base params (so "no-op" is a candidate) and de-dupe deterministically.
  const uniq: Array<DseParams> = [];
  const seen = new Set<string>();
  for (const p of [base, ...seeds]) {
    const key = JSON.stringify(p);
    if (seen.has(key)) continue;
    seen.add(key);
    uniq.push(p);
  }
  return uniq.slice(0, maxCandidates);
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

    const evaluateParams = (
      params: DseParams,
      dataset: Dataset<I, Y>,
      evalOptions?: { readonly includeExampleDetails?: boolean | undefined }
    ) =>
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
          params,
          eval: { evalVersion: 1, kind: "unscored" },
          optimizer: { id: "compile_candidate" },
          provenance: {}
        };

        const evalRes = yield* evaluate({
          signature: options.signature,
          artifact,
          dataset,
          reward: options.reward,
          includeExampleDetails: evalOptions?.includeExampleDetails ?? false
        });

        return {
          compiled_id,
          params,
          reward: evalRes.summary.reward ?? 0,
          evalSummary: evalRes.summary,
          ...(evalRes.examples ? { examples: evalRes.examples } : {})
        } satisfies EvaluatedParams;
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

    const runKnobsGrid = () =>
      Effect.gen(function* () {
        const maxCandidates = maxCandidatesFromOptimizerConfig(
          options.optimizer.config
        );

        const paramSets = knobsGrid(baseParams, options.searchSpace, { maxCandidates });
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
    } else if (optimizerId === "joint_instruction_grid_then_fewshot_greedy_forward.v1") {
      // Joint: instruction grid, then greedy few-shot from the best instruction.
      const r1 = yield* runInstructionGrid();
      const r2 = yield* runGreedyFewShot(r1.best);
      evaluatedCandidates = [...r1.scored, ...r2.scored];
      best = r2.best;
    } else if (optimizerId === "knobs_grid.v1" || optimizerId === "knobs_grid_refine.v1") {
      const r = yield* runKnobsGrid();
      evaluatedCandidates = [...r.scored];
      best = r.best;

      if (optimizerId === "knobs_grid_refine.v1") {
        const details = yield* evaluateParams(best.params, train, { includeExampleDetails: true });
        const examples = details.examples ?? [];
        const failures = examples.filter((example) => example.reward < 1);

        const hasBudgetExceeded = failures.some((example) =>
          (example.error?.errorName ?? "").includes("BudgetExceeded")
        );
        const hasDecodeError = failures.some((example) =>
          (example.error?.errorName ?? "").includes("OutputDecode")
        );
        const hasEvidenceFail = failures.some((e) =>
          e.signals.some(
            (signal) =>
              signal.signalId === "evidence_quote_in_blob.v1" && signal.score <= 0
          )
        );

        const patches: Array<Partial<DseParams>> = [];

        if (hasDecodeError) {
          patches.push({
            rlmLite: {
              controllerInstructions:
                "Critical: Output MUST be valid JSON matching the RLM Action schema. No markdown, no commentary, no arrays. If stuck, choose Search/Preview/Chunk, then Final."
            }
          });
        }

        if (hasEvidenceFail) {
          patches.push({
            rlmLite: {
              controllerInstructions:
                "Evidence rule: when producing Final, ensure evidence.quote is an exact substring from the cited blob. If you cannot find a quote, answer must be \"unknown\" and quote must be empty."
            }
          });
        }

        if (hasBudgetExceeded) {
          const b0 = best.params.budgets ?? {};
          patches.push({
            budgets: {
              ...b0,
              maxTimeMs: Math.max(0, Math.floor((b0.maxTimeMs ?? 15000) * 2)),
              maxLmCalls: Math.max(0, Math.floor((b0.maxLmCalls ?? 20) * 2)),
              maxRlmIterations: Math.max(0, Math.floor((b0.maxRlmIterations ?? 6) + 6)),
              maxSubLmCalls: Math.max(0, Math.floor((b0.maxSubLmCalls ?? 10) + 10)),
              maxOutputChars: Math.max(0, Math.floor((b0.maxOutputChars ?? 120000) * 2))
            }
          });
        }

        if (patches.length > 0) {
          const refined = yield* Effect.forEach(
            patches,
            (patch) =>
              evaluateParams(mergeParams(best.params, patch), train).pipe(
                Effect.map((r) => ({ compiled_id: r.compiled_id, params: r.params, reward: r.reward } satisfies CandidateScoreV1))
              ),
            { concurrency: 1 }
          );
          evaluatedCandidates.push(...refined);
          best = pickBest([best, ...refined]);
        }
      }
    } else {
      return yield* Effect.fail(
        CompileError.make({ message: `Unknown optimizer id: ${String(optimizerId)}` })
      );
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
      params: best.params,
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
