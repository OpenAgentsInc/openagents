import { Schema as S } from "effect";

import { canonicalStringify } from "../internal/canonical.js";
import { sha256Hex } from "../internal/sha256.js";
import {
  CompiledProgram,
  type DecodePolicy,
  type ExampleId,
  type SearchAlgorithm,
} from "../contract/index.js";

/**
 * Deterministic, bounded candidate generation.
 *
 * These are the honestly-named searches ported from the terminal DSE package:
 * instruction grid, greedy few-shot pool, joint search, and knob grids. There is
 * no MIPRO, GEPA, COPRO, Pareto, Bayesian scheduler, or generic module graph —
 * those were never implemented and stay out of scope. Generation is a pure
 * function of its inputs: the base program comes first, duplicates are removed by
 * canonical bytes, the remainder is ordered by a stable hash, and the result is
 * truncated to the cap. Candidate generation never sees holdout labels.
 */

export interface CandidateKnobs {
  readonly instructions: ReadonlyArray<string>;
  readonly fewShotSets: ReadonlyArray<ReadonlyArray<typeof ExampleId.Type>>;
  readonly modelRoles: ReadonlyArray<string>;
  readonly decodePolicies: ReadonlyArray<DecodePolicy>;
}

const decodeProgram = S.decodeUnknownSync(CompiledProgram);

const variant = (
  base: CompiledProgram,
  patch: {
    readonly instruction?: string;
    readonly fewShot?: ReadonlyArray<typeof ExampleId.Type>;
    readonly modelRole?: string;
    readonly decodePolicy?: DecodePolicy;
  },
): CompiledProgram =>
  decodeProgram({
    schema: base.schema,
    signatureId: base.signatureId,
    modelRole: patch.modelRole ?? base.modelRole,
    decodePolicy: patch.decodePolicy ?? base.decodePolicy,
    promptIr: {
      ...base.promptIr,
      instruction: patch.instruction ?? base.promptIr.instruction,
      fewShotExampleIds: patch.fewShot ?? base.promptIr.fewShotExampleIds,
    },
  });

const REFINE_SUFFIX = " Return only strict JSON. Do not claim any action you did not take.";

const rawVariants = (
  algorithm: SearchAlgorithm,
  base: CompiledProgram,
  knobs: CandidateKnobs,
): ReadonlyArray<CompiledProgram> => {
  const instructions =
    knobs.instructions.length > 0 ? knobs.instructions : [base.promptIr.instruction];
  const fewShotSets =
    knobs.fewShotSets.length > 0 ? knobs.fewShotSets : [base.promptIr.fewShotExampleIds];
  const roles = knobs.modelRoles.length > 0 ? knobs.modelRoles : [base.modelRole];
  const policies = knobs.decodePolicies.length > 0 ? knobs.decodePolicies : [base.decodePolicy];

  switch (algorithm) {
    case "instruction_grid.v1":
      return instructions.map((instruction) => variant(base, { instruction }));
    case "fewshot_greedy_forward.v1":
      return fewShotSets.map((fewShot) => variant(base, { fewShot }));
    case "joint_instruction_grid_then_fewshot_greedy_forward.v1":
      return instructions.flatMap((instruction) =>
        fewShotSets.map((fewShot) => variant(base, { instruction, fewShot })),
      );
    case "knobs_grid.v1":
      return instructions.flatMap((instruction) =>
        fewShotSets.flatMap((fewShot) =>
          roles.flatMap((modelRole) =>
            policies.map((decodePolicy) =>
              variant(base, { instruction, fewShot, modelRole, decodePolicy }),
            ),
          ),
        ),
      );
    case "knobs_grid_refine.v1": {
      const gridded = instructions.flatMap((instruction) =>
        fewShotSets.flatMap((fewShot) =>
          roles.flatMap((modelRole) =>
            policies.map((decodePolicy) =>
              variant(base, { instruction, fewShot, modelRole, decodePolicy }),
            ),
          ),
        ),
      );
      const refined = instructions.map((instruction) =>
        variant(base, { instruction: instruction + REFINE_SUFFIX }),
      );
      return [...gridded, ...refined];
    }
  }
};

const canonicalKey = (program: CompiledProgram): string => canonicalStringify(program);

/**
 * Generate a deterministic, deduplicated, capped candidate list. The base
 * program is always retained first; the rest are ordered by the stable hash of
 * their canonical bytes, so the ordering does not depend on knob input order.
 */
export const generateCandidates = (args: {
  readonly algorithm: SearchAlgorithm;
  readonly base: CompiledProgram;
  readonly knobs: CandidateKnobs;
  readonly cap: number;
}): ReadonlyArray<CompiledProgram> => {
  const baseKey = canonicalKey(args.base);
  const deduped = new Map<string, CompiledProgram>();
  deduped.set(baseKey, args.base);
  for (const candidate of rawVariants(args.algorithm, args.base, args.knobs)) {
    const key = canonicalKey(candidate);
    if (!deduped.has(key)) deduped.set(key, candidate);
  }

  const rest = [...deduped.entries()]
    .filter(([key]) => key !== baseKey)
    .map(([key, program]) => ({ program, hash: sha256Hex(key) }))
    .sort((left, right) => (left.hash < right.hash ? -1 : left.hash > right.hash ? 1 : 0))
    .map((entry) => entry.program);

  return [args.base, ...rest].slice(0, Math.max(1, args.cap));
};
