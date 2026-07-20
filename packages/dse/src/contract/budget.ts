import { Schema as S } from "effect";

/**
 * Runtime budgets and the search plan.
 *
 * A `ResourceBudget` bounds an offline compile: rollout count, wall-clock, worker
 * concurrency, output size, memory, and thermal headroom. The optimizer is
 * required to stop at the admitted limit. A `SearchPlan` records the exact,
 * honestly-named search algorithm and its bounds, so a repeated compile has a
 * reviewable deterministic plan.
 */

export const RESOURCE_BUDGET_SCHEMA_LITERAL = "openagents.dse.resource_budget.v1" as const;
export const SEARCH_PLAN_SCHEMA_LITERAL = "openagents.dse.search_plan.v1" as const;

/** The default candidate cap the deleted DSE compiler used; kept as the ceiling. */
export const DEFAULT_CANDIDATE_CAP = 128 as const;
export const MAX_CANDIDATE_CAP = 500 as const;

export const ThermalLevel = S.Literals(["nominal", "fair", "serious", "critical"]);
export type ThermalLevel = typeof ThermalLevel.Type;

export const ResourceBudget = S.Struct({
  schema: S.Literal(RESOURCE_BUDGET_SCHEMA_LITERAL),
  maxCandidates: S.Number.check(
    S.isInt(),
    S.isGreaterThanOrEqualTo(1),
    S.isLessThanOrEqualTo(MAX_CANDIDATE_CAP),
  ),
  maxRollouts: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(1)),
  maxWallClockMs: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(1)),
  maxConcurrency: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(1), S.isLessThanOrEqualTo(64)),
  maxOutputChars: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(1)),
  maxMemoryBytes: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(1)),
  maxThermalLevel: ThermalLevel,
});
export type ResourceBudget = typeof ResourceBudget.Type;

/** The honest, exact search-algorithm names ported from the terminal DSE package. */
export const SearchAlgorithm = S.Literals([
  "instruction_grid.v1",
  "fewshot_greedy_forward.v1",
  "joint_instruction_grid_then_fewshot_greedy_forward.v1",
  "knobs_grid.v1",
  "knobs_grid_refine.v1",
]);
export type SearchAlgorithm = typeof SearchAlgorithm.Type;

export const SearchPlan = S.Struct({
  schema: S.Literal(SEARCH_PLAN_SCHEMA_LITERAL),
  algorithm: SearchAlgorithm,
  candidateCap: S.Number.check(
    S.isInt(),
    S.isGreaterThanOrEqualTo(1),
    S.isLessThanOrEqualTo(MAX_CANDIDATE_CAP),
  ),
  seed: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
  budget: ResourceBudget,
});
export type SearchPlan = typeof SearchPlan.Type;

const decodeBudget = S.decodeUnknownSync(ResourceBudget);
const decodePlan = S.decodeUnknownSync(SearchPlan);

/** A conservative default budget for a bounded local offline compile. */
export const defaultResourceBudget = (
  overrides: Partial<Omit<ResourceBudget, "schema">> = {},
): ResourceBudget =>
  decodeBudget({
    schema: RESOURCE_BUDGET_SCHEMA_LITERAL,
    maxCandidates: DEFAULT_CANDIDATE_CAP,
    maxRollouts: 100_000,
    maxWallClockMs: 600_000,
    maxConcurrency: 4,
    maxOutputChars: 8_000,
    maxMemoryBytes: 512 * 1024 * 1024,
    maxThermalLevel: "serious",
    ...overrides,
  });

export const makeSearchPlan = (args: {
  readonly algorithm: SearchAlgorithm;
  readonly candidateCap?: number;
  readonly seed?: number;
  readonly budget?: ResourceBudget;
}): SearchPlan => {
  const budget = args.budget ?? defaultResourceBudget();
  const candidateCap = Math.min(args.candidateCap ?? DEFAULT_CANDIDATE_CAP, budget.maxCandidates);
  return decodePlan({
    schema: SEARCH_PLAN_SCHEMA_LITERAL,
    algorithm: args.algorithm,
    candidateCap,
    seed: args.seed ?? 0,
    budget,
  });
};
