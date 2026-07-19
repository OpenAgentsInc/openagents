import {
  compareStrings,
  type ObligationState,
  type SurfaceObligation,
  type SurfaceRow,
} from "./schema.ts";

/**
 * AR-1 (issue #9057): grade assurance obligations over the AR-0 inventory.
 *
 * The grade uses the assurance-spec vocabulary and keeps the four coverage
 * facts independent: `mapped`, `designed`, `observed`, `accepted`. This packet
 * never emits `observed` or `accepted` — those require a passing, source-bound
 * sweep receipt (AR-3 #9059) or owner acceptance. `inconclusive` is the default
 * for a real coverage gap; `out-of-scope` is a typed disposition for surfaces
 * intentionally excluded from grading. A designed oracle is not a passing
 * observation, and this grader must never present it as one.
 */

/** Unverified reasons that mean "intentionally excluded from grading". */
const OUT_OF_SCOPE_REASONS = new Set([
  "config-only",
  "reference-only",
  "historical",
  "retirement-source",
  "third-party",
]);

/** Oracle types that constitute an authored, executable design signal. */
const DESIGN_ORACLE_TYPES = new Set([
  "test",
  "behavior-contract",
  "assurance-obligation",
  "ste-check",
  "drift-oracle",
]);

export const gradeSurface = (
  surface: SurfaceRow,
  outOfScope: Record<string, string>,
): SurfaceObligation => {
  const disposition = outOfScope[surface.id];
  if (disposition !== undefined) {
    return { state: "out-of-scope", note: disposition };
  }

  if (surface.unverified) {
    if (OUT_OF_SCOPE_REASONS.has(surface.unverified.reason)) {
      return { state: "out-of-scope", note: `unverified: ${surface.unverified.reason}` };
    }
    return {
      state: "inconclusive",
      note: `no oracle authored (${surface.unverified.reason}); real coverage gap`,
    };
  }

  const designSignals = surface.oracles
    .filter((oracle) => DESIGN_ORACLE_TYPES.has(oracle.type))
    .map((oracle) => oracle.type);
  if (designSignals.length > 0) {
    const unique = [...new Set(designSignals)].sort(compareStrings);
    return {
      state: "designed",
      note: `executable oracle(s) authored: ${unique.join(", ")}; observation pending AR-3 sweep`,
    };
  }

  // Only a mapping-level oracle (e.g. product-spec validation) exists.
  return {
    state: "mapped",
    note: "mapped to intent/validation only; behavior oracle not yet designed",
  };
};

/** Grade every surface, returning a new array with the `obligation` field set. */
export const gradeSurfaces = (
  surfaces: ReadonlyArray<SurfaceRow>,
  outOfScope: Record<string, string>,
): ReadonlyArray<SurfaceRow> =>
  surfaces.map((surface) => ({ ...surface, obligation: gradeSurface(surface, outOfScope) }));

/** Top-level program area for the coverage report. */
export const programArea = (owningPath: string): string => {
  if (owningPath.startsWith("apps/openagents.com/workers/")) return "openagents.com/workers";
  if (owningPath.startsWith("apps/openagents.com/")) return "openagents.com/*";
  if (owningPath === "apps/openagents.com") return "openagents.com";
  if (owningPath.startsWith("apps/")) return `apps/${owningPath.split("/")[1]}`;
  if (owningPath.startsWith("packages/")) return "packages";
  if (owningPath.startsWith("crates/")) return "crates";
  if (owningPath.startsWith("specs/")) return "specs";
  if (owningPath.startsWith("docs/")) return "docs";
  if (owningPath.startsWith("types/")) return "types";
  return owningPath.split("/")[0] ?? owningPath;
};

export type AreaCoverage = {
  readonly area: string;
  readonly total: number;
  readonly byState: Record<ObligationState, number>;
};

const emptyStates = (): Record<ObligationState, number> => ({
  mapped: 0,
  designed: 0,
  observed: 0,
  accepted: 0,
  inconclusive: 0,
  "out-of-scope": 0,
});

/**
 * Program-area coverage report keeping mapped / designed / observed / accepted
 * as four independent facts. No blended score — that is structurally excluded.
 */
export const coverageByArea = (
  surfaces: ReadonlyArray<SurfaceRow>,
): ReadonlyArray<AreaCoverage> => {
  const areas = new Map<string, Record<ObligationState, number>>();
  for (const surface of surfaces) {
    const area = programArea(surface.owningPath);
    const states = areas.get(area) ?? emptyStates();
    const state = surface.obligation?.state ?? "inconclusive";
    states[state] += 1;
    areas.set(area, states);
  }
  return [...areas.entries()]
    .map(([area, byState]) => ({
      area,
      total: Object.values(byState).reduce((sum, count) => sum + count, 0),
      byState,
    }))
    .sort((a, b) => compareStrings(a.area, b.area));
};
