/**
 * Pinned rate catalog for the coder lanes the effectiveness suite grades, and
 * the one rule that makes the suite worth running: an unpriced lane reports
 * `unknown`, never zero.
 *
 * SOURCE OF TRUTH: the forge model catalog at `OpenAgentsInc/openagents.com`
 * `config/config.exs` under `config :openagents, model_catalog:`, projected
 * publicly by `lib/openagents/inference/models.ex` onto `GET /api/v1/models`.
 * That catalog — not the Cloudflare worker table in
 * `apps/openagents.com/workers/api/src/inference/pricing.ts` — is the one that
 * prices the ids the coder actually runs (`gemini-3.7-flash`, `ox-alpha`,
 * `gpt-5.6-luna`). The worker table prices a different model generation
 * (`sonnet`, `gemini-3.6-flash`, ...) and shares no id with the coder catalog.
 *
 * The rows below are a PINNED snapshot, following the precedent set by
 * `packages/rlm-recall-eval/src/price-catalog.ts`: the forge config is not a
 * TypeScript module this package can import, a pinned version keeps a graded
 * run reproducible from a clean checkout, and cost provenance has to be
 * auditable rather than implied. {@link pricingFromModelsPayload} re-derives
 * the same rows from a live `GET /api/v1/models` body when an operator wants
 * to score against the deployed catalog instead of the snapshot.
 *
 * TWO SEPARATE HONESTY PROBLEMS LIVE HERE, AND THEY ARE NOT THE SAME PROBLEM.
 *
 * 1. UNPRICED. `gpt-5.6-luna` — the lane the graded runs actually use most —
 *    has no `pricing` key in the forge catalog at all. The config says why:
 *    "This entry deliberately omits `pricing`, so a grant pinned to it records
 *    no estimated cost rather than a made-up zero." A local `ollama:` lane is
 *    unpriced for a different reason: it burns wall clock and electricity, not
 *    metered tokens, so no per-token rate exists to apply. Both return
 *    `usd: null`. Neither is ever priced at 0, and neither falls back to the
 *    worker's `UNKNOWN_MODEL_COST` — that constant exists so an un-tabled
 *    model is not UNDER-charged at the till, and it is explicitly "not a
 *    measured rate". Charging conservatively and measuring honestly are
 *    opposite jobs; borrowing that number here would fabricate the very
 *    figure this suite exists to report.
 *
 * 2. PLACEHOLDER. The two ids that DO carry rates carry rates the forge config
 *    marks itself: "Placeholder: the operator must set real provider rates
 *    before accepting any spend." A number derived from those rates is
 *    arithmetically correct and economically provisional. It is reported, and
 *    it is labelled {@link RateBasis} `operator_placeholder` everywhere it
 *    travels, so a threshold can refuse to score against it.
 */

/** Pinned catalog version. Bump when the rows below change. */
export const CODER_RATE_CATALOG_VERSION = "openagents.coder-rate-catalog.2026-08-25" as const;

/** Human-auditable pointer back to the forge source of truth. */
export const CODER_RATE_CATALOG_SOURCE_REF =
  "OpenAgentsInc/openagents.com config/config.exs#model_catalog" as const;

/**
 * Where a rate came from. `operator_placeholder` is a rate the forge config
 * marks as provisional; `operator_confirmed` is a rate an operator has
 * reconciled against a provider invoice. Every row is a placeholder today.
 */
export type RateBasis = "operator_placeholder" | "operator_confirmed";

/** A per-model rate row, in USD per 1,000,000 tokens. */
export interface ModelRateRow {
  readonly modelId: string;
  readonly inputUsdPerMtok: number;
  /**
   * The rate for cached-read input tokens. The forge prices cached reads at
   * this rate and charges cache WRITES as ordinary input, because a write is
   * not a read. Where the catalog declares no cached rate, it falls back to
   * the full input rate, and this field records that resolved fallback.
   */
  readonly cachedInputUsdPerMtok: number;
  readonly outputUsdPerMtok: number;
  readonly rateBasis: RateBasis;
}

/**
 * The pinned rows. Values are the forge catalog's integer micro-USD per
 * million tokens converted to USD per million tokens.
 */
export const CODER_RATE_CATALOG: Readonly<Record<string, ModelRateRow>> = {
  // pricing: input 1_250_000 / output 10_000_000 / cached 100_000 micro-USD per Mtok.
  "gemini-3.7-flash": {
    modelId: "gemini-3.7-flash",
    inputUsdPerMtok: 1.25,
    cachedInputUsdPerMtok: 0.1,
    outputUsdPerMtok: 10.0,
    rateBasis: "operator_placeholder",
  },
  // pricing: input 500_000 / output 2_000_000 micro-USD per Mtok, no cached
  // rate declared, so cached reads resolve to the full input rate.
  "ox-alpha": {
    modelId: "ox-alpha",
    inputUsdPerMtok: 0.5,
    cachedInputUsdPerMtok: 0.5,
    outputUsdPerMtok: 2.0,
    rateBasis: "operator_placeholder",
  },
};

/**
 * Model ids the coder can be pinned to that the catalog deliberately leaves
 * unpriced, with the catalog's own stated reason. Listing them separates "we
 * looked and the catalog refuses to price this" from "we have never heard of
 * this id", which is a different and less confident finding.
 */
export const DELIBERATELY_UNPRICED_MODELS: Readonly<Record<string, string>> = {
  "gpt-5.6-luna":
    "the forge model catalog omits `pricing` for this id so a grant pinned to it records no estimated cost rather than a made-up zero",
};

/** Token usage offered for pricing. A `null` count is unknown, not zero. */
export interface UsageForCost {
  readonly promptTokens: number | null;
  readonly completionTokens: number | null;
  /** Cached-read input tokens, already included in `promptTokens`. */
  readonly cachedInputTokens: number;
}

/**
 * Why a cost figure is or is not known. Every disposition other than `known`
 * carries `usd: null` so an aggregate can exclude or flag it rather than
 * silently adding zero to a total.
 */
export type CostDisposition =
  | "known"
  | "unpriced_model"
  | "unmetered_local_lane"
  | "unknown_model"
  | "unknown_usage";

export interface CostResult {
  readonly usd: number | null;
  readonly disposition: CostDisposition;
  readonly rateBasis: RateBasis | null;
  /** One sentence a report can print next to an unknown. */
  readonly reason: string;
}

/**
 * Whether this trial ran on a lane that bills no metered tokens.
 *
 * The lane the run was executed on is the authority, and it is passed in
 * because it is a fact the run records rather than something to be guessed at.
 * The `ollama:` prefix is kept as a secondary signal for a caller pricing one
 * trial with no lane in hand, but it was never a good primary one: the coder's
 * own ATIF export writes the bare model name, so an id read from a trajectory
 * carries no prefix to spot and a whole local run priced as `unknown_model` —
 * which reads as a gap in the rate catalog rather than as a lane that has no
 * rates to be in it.
 */
const isUnmetered = (modelId: string, lane: string | null): boolean =>
  lane === "local" || modelId.startsWith("ollama:") || modelId.startsWith("ollama/");

/**
 * Price one trial's usage against a rate catalog.
 *
 * Unknown stays unknown. A model with no rate, a lane with no per-token rate at
 * all, or usage missing either token dimension all return `usd: null` with a
 * disposition that names the reason. Nothing here falls back to a conservative
 * default rate: this function measures, it does not charge.
 */
export const priceUsage = (
  modelId: string | null,
  usage: UsageForCost,
  catalog: Readonly<Record<string, ModelRateRow>> = CODER_RATE_CATALOG,
  lane: string | null = null,
): CostResult => {
  if (modelId === null || modelId === "") {
    return {
      usd: null,
      disposition: "unknown_model",
      rateBasis: null,
      reason: "the trial records no model id, so no rate can be selected",
    };
  }
  if (isUnmetered(modelId, lane)) {
    return {
      usd: null,
      disposition: "unmetered_local_lane",
      rateBasis: null,
      reason: `${modelId} ran on the local lane, which bills no metered tokens, so it has no per-token cost`,
    };
  }
  const row = catalog[modelId];
  if (row === undefined) {
    const declared = DELIBERATELY_UNPRICED_MODELS[modelId];
    return {
      usd: null,
      disposition: declared === undefined ? "unknown_model" : "unpriced_model",
      rateBasis: null,
      reason:
        declared === undefined
          ? `${modelId} is absent from the rate catalog, so its cost is unknown`
          : `${modelId} is unpriced: ${declared}`,
    };
  }
  if (usage.promptTokens === null || usage.completionTokens === null) {
    return {
      usd: null,
      disposition: "unknown_usage",
      rateBasis: row.rateBasis,
      reason: `${modelId} has a rate but the trial reports no token counts, so its cost is unknown`,
    };
  }
  if (usage.cachedInputTokens > usage.promptTokens) {
    return {
      usd: null,
      disposition: "unknown_usage",
      rateBasis: row.rateBasis,
      reason: `${modelId} reports ${String(usage.cachedInputTokens)} cached-read tokens inside ${String(usage.promptTokens)} prompt tokens, which cannot both be true`,
    };
  }
  const uncachedInput = usage.promptTokens - usage.cachedInputTokens;
  const usd =
    (uncachedInput / 1_000_000) * row.inputUsdPerMtok +
    (usage.cachedInputTokens / 1_000_000) * row.cachedInputUsdPerMtok +
    (usage.completionTokens / 1_000_000) * row.outputUsdPerMtok;
  return {
    usd,
    disposition: "known",
    rateBasis: row.rateBasis,
    reason: `${modelId} priced from the ${row.rateBasis === "operator_placeholder" ? "placeholder" : "confirmed"} catalog rate`,
  };
};

/**
 * Re-derive rate rows from a live `GET /api/v1/models` body, so a run can be
 * scored against the catalog a deployment actually serves rather than the
 * pinned snapshot.
 *
 * The forge omits the `pricing` key entirely for an unpriced model. That
 * absence is the signal, and it is preserved: such an id yields no row, so
 * {@link priceUsage} reports it unknown rather than zero. Rates arrive as
 * integer micro-USD per million tokens and are converted to USD per million.
 */
export const pricingFromModelsPayload = (
  payload: unknown,
): Readonly<Record<string, ModelRateRow>> => {
  const rows: Record<string, ModelRateRow> = {};
  const models = readArray(readField(payload, "models") ?? readField(payload, "data"));
  for (const model of models) {
    const id = readString(readField(model, "id"));
    if (id === null) continue;
    const pricing = readField(model, "pricing");
    if (pricing === undefined || pricing === null) continue;
    const input = readNumber(readField(pricing, "input_per_million_tokens"));
    const output = readNumber(readField(pricing, "output_per_million_tokens"));
    if (input === null || output === null) continue;
    const cached = readNumber(readField(pricing, "cached_input_per_million_tokens"));
    rows[id] = {
      modelId: id,
      inputUsdPerMtok: input / 1_000_000,
      // No declared cached rate means cached reads bill at the input rate.
      cachedInputUsdPerMtok: (cached ?? input) / 1_000_000,
      outputUsdPerMtok: output / 1_000_000,
      // A served catalog states rates, never their provenance. It cannot
      // promise an operator reconciled them, so the basis stays placeholder.
      rateBasis: "operator_placeholder",
    };
  }
  return rows;
};

const readField = (value: unknown, key: string): unknown =>
  typeof value === "object" && value !== null ? (value as Record<string, unknown>)[key] : undefined;

const readArray = (value: unknown): ReadonlyArray<unknown> => (Array.isArray(value) ? value : []);

const readString = (value: unknown): string | null =>
  typeof value === "string" && value !== "" ? value : null;

const readNumber = (value: unknown): number | null =>
  typeof value === "number" && Number.isFinite(value) ? value : null;
