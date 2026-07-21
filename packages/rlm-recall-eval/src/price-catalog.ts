/**
 * Versioned OpenAgents price catalog snapshot for dense-recall cost scoring.
 *
 * SOURCE OF TRUTH: the OpenAgents inference price catalog at
 * `apps/openagents.com/workers/api/src/inference/pricing.ts`
 * (`MODEL_PRICING_TABLE` cost rows) and its public projection in
 * `apps/openagents.com/workers/api/src/inference/model-catalog.ts`.
 *
 * This module is a PINNED, VERSIONED snapshot of the marginal cost rows that
 * the eval scores against. It is pinned (not a live import) for three reasons:
 *
 * - the worker price catalog is an app-private module, and a hermetic eval
 *   package must not depend on an application build surface;
 * - a pinned catalog version makes hermetic cost aggregates reproducible from a
 *   clean checkout, byte for byte;
 * - the eval must carry cost-basis provenance and never price unknown usage as
 *   zero, which requires an explicit, auditable snapshot.
 *
 * When the worker cost table changes, bump {@link RLM_EVAL_PRICE_CATALOG_VERSION}
 * and re-pin the rows below. The `costBasis` mirrors the worker
 * `ModelCostPerMtok` provenance: `list_placeholder` marks the published LIST
 * rate (Vertex Claude and Gemini rows carry the billing TODO), `verified` marks
 * a measured upstream rate (Fireworks open models, verified 2026-06-19).
 */

/** Pinned catalog version. Bump when the worker cost rows below change. */
export const RLM_EVAL_PRICE_CATALOG_VERSION = "openagents.price-catalog.2026-07-21" as const;

/** Human-auditable pointer back to the worker source of truth. */
export const RLM_EVAL_PRICE_CATALOG_SOURCE_REF =
  "apps/openagents.com/workers/api/src/inference/pricing.ts#MODEL_PRICING_TABLE" as const;

/** Cost-basis provenance, mirrored from the worker catalog. */
export type ModelCostBasis = "verified" | "list_placeholder";

/** Marginal cost row for one model, in USD per 1,000,000 tokens. */
export interface ModelCostRow {
  readonly modelId: string;
  readonly inputUsdPerMtok: number;
  readonly cachedInputUsdPerMtok: number;
  readonly outputUsdPerMtok: number;
  readonly costBasis: ModelCostBasis;
}

/**
 * Pinned marginal cost rows (USD per 1M tokens). A representative subset of the
 * worker table sufficient for the eval scored models. Cached-input fractions
 * for the Vertex Claude rows use the worker `CACHED_INPUT_FRACTION` of 0.5.
 */
export const RLM_EVAL_PRICE_CATALOG: Readonly<Record<string, ModelCostRow>> = {
  "gemini-3.5-flash": {
    modelId: "gemini-3.5-flash",
    inputUsdPerMtok: 0.075,
    cachedInputUsdPerMtok: 0.01875,
    outputUsdPerMtok: 0.3,
    costBasis: "list_placeholder",
  },
  "gpt-oss-120b": {
    modelId: "gpt-oss-120b",
    inputUsdPerMtok: 0.15,
    cachedInputUsdPerMtok: 0.015,
    outputUsdPerMtok: 0.6,
    costBasis: "verified",
  },
  sonnet: {
    modelId: "sonnet",
    inputUsdPerMtok: 3.0,
    cachedInputUsdPerMtok: 1.5,
    outputUsdPerMtok: 15.0,
    costBasis: "list_placeholder",
  },
  haiku: {
    modelId: "haiku",
    inputUsdPerMtok: 1.0,
    cachedInputUsdPerMtok: 0.5,
    outputUsdPerMtok: 5.0,
    costBasis: "list_placeholder",
  },
} as const;

/** Default scored model for the hermetic matrix (the desktop free-tier lane). */
export const RLM_EVAL_DEFAULT_MODEL_ID = "gemini-3.5-flash" as const;

/**
 * Token usage presented for cost scoring. A `null` count means the count is
 * unknown for that dimension. `completeness` mirrors the RLM usage contract.
 */
export interface UsageForCost {
  readonly inputTokens: number | null;
  readonly outputTokens: number | null;
  readonly completeness: "complete" | "partial" | "unavailable";
}

/**
 * Why a cost figure is or is not known. `known` costs carry a real USD value;
 * every other disposition returns `usd: null` so the aggregate can EXCLUDE it
 * rather than silently treat unknown usage as zero.
 */
export type CostDisposition = "known" | "unknown_usage" | "unknown_model";

export interface CostResult {
  readonly usd: number | null;
  readonly disposition: CostDisposition;
  readonly costBasis: ModelCostBasis | null;
}

/**
 * Price a usage record against the pinned catalog.
 *
 * Honesty rule: unknown usage stays unknown. If the usage completeness is not
 * `complete`, or either token dimension is `null`, or the model is not in the
 * catalog, the returned `usd` is `null` and the disposition names the reason.
 * The aggregate then excludes or labels it; it is never priced as zero.
 */
export const priceUsage = (modelId: string, usage: UsageForCost): CostResult => {
  const row = RLM_EVAL_PRICE_CATALOG[modelId];
  if (row === undefined) {
    return { usd: null, disposition: "unknown_model", costBasis: null };
  }
  if (
    usage.completeness !== "complete" ||
    usage.inputTokens === null ||
    usage.outputTokens === null
  ) {
    return { usd: null, disposition: "unknown_usage", costBasis: row.costBasis };
  }
  const usd =
    (usage.inputTokens / 1_000_000) * row.inputUsdPerMtok +
    (usage.outputTokens / 1_000_000) * row.outputUsdPerMtok;
  return { usd, disposition: "known", costBasis: row.costBasis };
};
