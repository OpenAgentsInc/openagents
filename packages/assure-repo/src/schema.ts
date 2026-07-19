import { Schema as S } from "effect";

/**
 * ASSURE-REPO surface inventory (AR-0, epic #9055 / issue #9056).
 *
 * A machine-readable inventory of every verification-bearing surface in the
 * OpenAgents monorepo. Each surface binds to its oracles (behavior contracts,
 * tests, assurance obligations, product specs, promises, smoke journeys) or
 * carries an explicit `unverified` reason. The load-bearing invariant is
 * "no silent surfaces": a row with neither an oracle ref nor an explicit
 * `unverified` reason fails validation.
 *
 * The artifact is deterministic (no wall-clock timestamps, sorted throughout)
 * so a `--check` regeneration byte-compares against the committed file; a
 * repository change that would alter the derived inventory fails the guard
 * until the artifact is regenerated. That deterministic guard is the
 * "freshness relative to main" mechanism.
 */

export const SURFACE_INVENTORY_FORMAT_VERSION = "1" as const;
export const SURFACE_INVENTORY_PATH = "docs/assure-repo/surface-inventory.v1.json" as const;
export const SURFACE_POLICY_PATH = "docs/assure-repo/surface-policy.v1.json" as const;

/** Kinds of verification-bearing surface derivable from the repository graph. */
export const SurfaceKind = S.Literals([
  "app",
  "package",
  "crate",
  "worker",
  "cli-entrypoint",
  "public-endpoint",
  "release-pipeline",
  "document",
]);
export type SurfaceKind = typeof SurfaceKind.Type;

/** Types of oracle that can prove (some dimension of) a surface. */
export const OracleType = S.Literals([
  "behavior-contract",
  "test",
  "assurance-obligation",
  "product-spec",
  "promise",
  "smoke-journey",
  "ste-check",
  "drift-oracle",
]);
export type OracleType = typeof OracleType.Type;

/**
 * A typed pointer from a surface to an oracle. The ref is an index entry, not
 * a verdict: its presence proves an oracle is *authored* for the surface, not
 * that the surface is *proven*. Obligation grading (AR-1) and the standing
 * sweep (AR-3) carry the verdict.
 */
export const OracleRef = S.Struct({
  type: OracleType,
  /** Contract id, repo-relative test path, spec path, promise id, etc. */
  ref: S.String,
});
export type OracleRef = typeof OracleRef.Type;

/**
 * Why a surface carries no oracle. An explicit reason is the repo-scale form
 * of loss accounting: the inventory says what is not observed instead of
 * implying completeness.
 */
export const UnverifiedReason = S.Literals([
  /** A real behaviour-bearing surface with no oracle authored yet. */
  "no-oracle-authored",
  /** Pure configuration/type declarations with no runtime behaviour. */
  "config-only",
  /** Tooling/scripts whose only consumer is other verified surfaces. */
  "tooling-only",
  /** Reference or vendored material, not owned behaviour. */
  "reference-only",
  /** Retained historical surface, not an active queue. */
  "historical",
  /** A surface being retired; its oracles are intentionally withdrawn. */
  "retirement-source",
  /** Generated output whose correctness is a generator-determinism concern. */
  "generated-artifact",
  /** External/third-party code included for compatibility only. */
  "third-party",
]);
export type UnverifiedReason = typeof UnverifiedReason.Type;

/**
 * Obligation grading state (AR-1, issue #9057). `designed` and `observed` are
 * never merged; `inconclusive` is the default for anything unproven. Present
 * only after AR-1 has graded the surface.
 */
export const ObligationState = S.Literals([
  "mapped",
  "designed",
  "observed",
  "accepted",
  "inconclusive",
  "out-of-scope",
]);
export type ObligationState = typeof ObligationState.Type;

export const SurfaceObligation = S.Struct({
  /** The graded state for this surface's assurance coverage. */
  state: ObligationState,
  /** Human/tool note: why this state, or the disposition for out-of-scope. */
  note: S.String,
  /** Evidence ref when observed/accepted; empty otherwise. Never a link-as-verdict. */
  evidenceRef: S.optional(S.String),
});
export type SurfaceObligation = typeof SurfaceObligation.Type;

export const UnverifiedTag = S.Struct({
  reason: UnverifiedReason,
  note: S.String,
});
export type UnverifiedTag = typeof UnverifiedTag.Type;

/** One inventoried surface. */
export const SurfaceRow = S.Struct({
  /** Stable id: `<kind>:<name-or-path>`. */
  id: S.String,
  kind: SurfaceKind,
  /** Repo-relative owning path. */
  owningPath: S.String,
  title: S.String,
  /** Whether this row was derived from a graph or hand-annotated (marked as such). */
  derivation: S.Literals(["derived", "annotated"]),
  oracles: S.Array(OracleRef),
  /** Present iff the surface has no oracle. Enforced by validation. */
  unverified: S.optional(UnverifiedTag),
  /** AR-1 grading. Absent until graded. */
  obligation: S.optional(SurfaceObligation),
});
export type SurfaceRow = typeof SurfaceRow.Type;

export const InventorySummary = S.Struct({
  totalSurfaces: S.Number,
  byKind: S.Record(S.String, S.Number),
  withOracle: S.Number,
  unverified: S.Number,
  byUnverifiedReason: S.Record(S.String, S.Number),
  byObligationState: S.Record(S.String, S.Number),
  /** Honest bounds: surface classes not yet enumerated at fine granularity. */
  coverageNotes: S.Array(S.String),
});
export type InventorySummary = typeof InventorySummary.Type;

export const SurfaceInventoryDocument = S.Struct({
  schemaVersion: S.Literal(SURFACE_INVENTORY_FORMAT_VERSION),
  repository: S.Literal("OpenAgentsInc/openagents"),
  /**
   * Content digest over the derivation inputs (workspace graph, crates,
   * contracts, specs, tracked test/doc paths). Deterministic; changes iff the
   * derived inventory changes. This is the staleness anchor.
   */
  sourceDigest: S.String,
  surfaces: S.Array(SurfaceRow),
  summary: InventorySummary,
});
export type SurfaceInventoryDocument = typeof SurfaceInventoryDocument.Type;

const decodeInventory = S.decodeUnknownSync(SurfaceInventoryDocument);

export type InventoryValidationIssue = {
  readonly kind:
    | "schema"
    | "silent_surface"
    | "oracle_and_unverified"
    | "duplicate_surface_id"
    | "unsorted_surfaces"
    | "summary_mismatch";
  readonly surfaceId?: string;
  readonly message: string;
};

export type InventoryValidation = {
  readonly ok: boolean;
  readonly issues: ReadonlyArray<InventoryValidationIssue>;
  readonly document?: SurfaceInventoryDocument;
};

/**
 * Validate a decoded (or raw) inventory. Enforces the no-silent-surface
 * invariant, mutual exclusion of oracle/unverified, id uniqueness, canonical
 * sort order, and summary agreement.
 */
export const validateSurfaceInventory = (input: unknown): InventoryValidation => {
  const issues: InventoryValidationIssue[] = [];
  let document: SurfaceInventoryDocument;
  try {
    document = decodeInventory(input);
  } catch (error) {
    return {
      ok: false,
      issues: [{ kind: "schema", message: error instanceof Error ? error.message : String(error) }],
    };
  }

  const seen = new Set<string>();
  for (const surface of document.surfaces) {
    if (seen.has(surface.id)) {
      issues.push({
        kind: "duplicate_surface_id",
        surfaceId: surface.id,
        message: `duplicate surface id ${surface.id}`,
      });
    }
    seen.add(surface.id);
    const hasOracle = surface.oracles.length > 0;
    const hasUnverified = surface.unverified !== undefined;
    if (!hasOracle && !hasUnverified) {
      issues.push({
        kind: "silent_surface",
        surfaceId: surface.id,
        message: `surface ${surface.id} has neither an oracle ref nor an explicit unverified reason`,
      });
    }
    if (hasOracle && hasUnverified) {
      issues.push({
        kind: "oracle_and_unverified",
        surfaceId: surface.id,
        message: `surface ${surface.id} is both oracle-bound and tagged unverified; a surface is one or the other`,
      });
    }
  }

  const sortedIds = [...document.surfaces].map((s) => s.id).sort(compareStrings);
  const actualIds = document.surfaces.map((s) => s.id);
  if (JSON.stringify(sortedIds) !== JSON.stringify(actualIds)) {
    issues.push({
      kind: "unsorted_surfaces",
      message: "surfaces must be sorted by id for deterministic output",
    });
  }

  const recomputed = summarize(document.surfaces);
  if (
    recomputed.totalSurfaces !== document.summary.totalSurfaces ||
    recomputed.withOracle !== document.summary.withOracle ||
    recomputed.unverified !== document.summary.unverified
  ) {
    issues.push({ kind: "summary_mismatch", message: "summary counts disagree with surfaces" });
  }

  return { ok: issues.length === 0, issues, document };
};

export const compareStrings = (left: string, right: string): number =>
  left < right ? -1 : left > right ? 1 : 0;

/** Recompute the summary block from surfaces (deterministic, sorted keys). */
export const summarize = (
  surfaces: ReadonlyArray<SurfaceRow>,
  coverageNotes: ReadonlyArray<string> = [],
): InventorySummary => {
  const byKind: Record<string, number> = {};
  const byUnverifiedReason: Record<string, number> = {};
  const byObligationState: Record<string, number> = {};
  let withOracle = 0;
  let unverified = 0;
  for (const surface of surfaces) {
    byKind[surface.kind] = (byKind[surface.kind] ?? 0) + 1;
    if (surface.oracles.length > 0) withOracle += 1;
    if (surface.unverified) {
      unverified += 1;
      byUnverifiedReason[surface.unverified.reason] =
        (byUnverifiedReason[surface.unverified.reason] ?? 0) + 1;
    }
    if (surface.obligation) {
      byObligationState[surface.obligation.state] =
        (byObligationState[surface.obligation.state] ?? 0) + 1;
    }
  }
  return {
    totalSurfaces: surfaces.length,
    byKind: sortRecord(byKind),
    withOracle,
    unverified,
    byUnverifiedReason: sortRecord(byUnverifiedReason),
    byObligationState: sortRecord(byObligationState),
    coverageNotes: [...coverageNotes].sort(compareStrings),
  };
};

const sortRecord = (record: Record<string, number>): Record<string, number> => {
  const out: Record<string, number> = {};
  for (const key of Object.keys(record).sort(compareStrings)) out[key] = record[key]!;
  return out;
};

/** Canonical serialization: sorted, pretty-printed, trailing newline. */
export const serializeSurfaceInventory = (document: SurfaceInventoryDocument): string =>
  `${JSON.stringify(document, null, 2)}\n`;
