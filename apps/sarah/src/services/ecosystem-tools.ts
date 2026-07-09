/**
 * KHS-9 live ecosystem product-truth tools (#8608, epic #8599).
 *
 * Sarah's KB snapshot goes stale; live truth is better. These server-side
 * tools read the PUBLIC openagents.com surfaces — the product-promise
 * registry, the Khala tokens-served counter, the pylon stats projection, and
 * the Khala Code plan catalog — so her answers cite live product truth
 * instead of pasted copy. The openagents.com Worker/monolith API remains the
 * system of record; nothing here carries authority, secrets, or writes.
 *
 * Law:
 * - SEMANTIC ROUTING ONLY (workspace invariant): promise lookup and the
 *   grounding intent match are embedding + cosine-similarity via the shared
 *   `sarahEmbedText` lane — never keyword/regex intent routing. When the
 *   embedder is unavailable the result is an honest miss, not a string-match
 *   fallback.
 * - REGISTRY SAFE-COPY ONLY: a promise's state caps how Sarah may describe
 *   it. Tool output carries `safeCopy` (never `unsafeCopy`) plus an explicit
 *   per-state caveat — yellow records always ship operator-assisted caveat
 *   wording, and non-live records ship do-not-pitch wording.
 * - FAIL-SOFT: every fetch is short-timeout, short-cached (60s), and a
 *   failure degrades to an honest `ok:false` / null — a conversation never
 *   breaks because a public endpoint is down.
 * - FLAG-GATED grounding: `maybeEcosystemGrounding` is a no-op unless
 *   SARAH_ECOSYSTEM_GROUNDING=1 (rollout safety; default off) — the brain
 *   lanes carry exactly one hook call each and prod behavior is unchanged
 *   until the owner arms the flag.
 */

import { cosineSimilarity, sarahEmbedText } from "./semantic-answer-cache.ts"

const CACHE_TTL_MS = 60_000
const FETCH_TIMEOUT_MS = 5_000
const PROMISE_MATCH_MIN_SIMILARITY_DEFAULT = 0.5
const PROMISE_MATCH_TOP_K = 3
const GROUNDING_MIN_SIMILARITY_DEFAULT = 0.72
const MAX_SAFE_COPY_LENGTH = 240

export function sarahEcosystemBaseUrl(): string {
  return (
    process.env.SARAH_ECOSYSTEM_BASE_URL?.trim().replace(/\/+$/, "") ||
    "https://openagents.com"
  )
}

let lastError: string | null = null

// ---------------------------------------------------------------------------
// Fail-soft public fetch with a 60s short cache (per path)
// ---------------------------------------------------------------------------

type FetchLike = (url: string, init?: RequestInit) => Promise<Response>
let fetchImpl: FetchLike = (url, init) => fetch(url, init)

const jsonCache = new Map<string, { at: number; value: unknown }>()

async function fetchPublicJson(path: string): Promise<unknown | null> {
  const cached = jsonCache.get(path)
  if (cached && Date.now() - cached.at < CACHE_TTL_MS) return cached.value
  const controller = new AbortController()
  const timeout = setTimeout(() => controller.abort(), FETCH_TIMEOUT_MS)
  try {
    const response = await fetchImpl(`${sarahEcosystemBaseUrl()}${path}`, {
      headers: { accept: "application/json" },
      signal: controller.signal,
    })
    if (!response.ok) {
      lastError = `ecosystem_http_${response.status}:${path}`
      return null
    }
    const value = (await response.json()) as unknown
    jsonCache.set(path, { at: Date.now(), value })
    return value
  } catch (error) {
    lastError =
      error instanceof Error && error.name === "AbortError"
        ? `ecosystem_timeout:${path}`
        : `ecosystem_unreachable:${path}`
    return null
  } finally {
    clearTimeout(timeout)
  }
}

function clip(text: string, max = MAX_SAFE_COPY_LENGTH): string {
  const normalized = text.replace(/\s+/g, " ").trim()
  if (normalized.length <= max) return normalized
  return `${normalized.slice(0, max - 1).trimEnd()}…`
}

// ---------------------------------------------------------------------------
// promise_lookup — embedding match over the LIVE promise registry
// ---------------------------------------------------------------------------

export const PROMISE_REGISTRY_PATH = "/api/public/product-promises"
export const TOKENS_SERVED_PATH = "/api/public/khala-tokens-served"
export const PYLON_STATS_PATH = "/api/public/pylon-stats"
export const PLAN_CATALOG_PATH = "/api/public/khala-code/plans"
export const AGENTS_SHEET_URL = "https://openagents.com/AGENTS.md"

type RegistryRecord = {
  promiseId?: string
  state?: string
  claim?: string
  safeCopy?: string
  authorityBoundary?: string
  blockerRefs?: string[]
}

export type PromiseLookupMatch = {
  promiseId: string
  state: string
  /** Registry safe copy ONLY — unsafeCopy never leaves this module. */
  safeCopy: string
  /** State-capped speaking instruction; yellow always carries the operator-assisted caveat. */
  caveat: string
  blockerRefs: string[]
  similarity: number
}

export type PromiseLookupResult = {
  ok: boolean
  source: string
  matches: PromiseLookupMatch[]
  error?: string
}

/**
 * The state cap, spelled out per record so a match can never be quoted wider
 * than the registry allows.
 */
export function promiseStateCaveat(state: string): string {
  switch (state) {
    case "green":
      return "Live — describe it only within the safe copy and its authority boundary."
    case "yellow":
      return "Operator-assisted / limited / gated today — always say this caveat plainly; do not present it as fully self-serve."
    case "planned":
      return "NOT live — roadmap only. Do not pitch it as available."
    case "red":
    case "degraded":
      return "NOT reliable right now — call it blocked or degraded, never available."
    case "withdrawn":
      return "Retired — do not pitch it at all."
    default:
      return "Unknown state — say OpenAgents cannot promise this yet and offer to escalate."
  }
}

// Registry-record embeddings are cached in-process keyed by the embedded
// text, so repeated lookups do not re-embed an unchanged registry.
const recordEmbeddings = new Map<string, number[]>()

function registryRecordText(record: RegistryRecord): string {
  return clip(
    `${record.promiseId ?? ""}: ${record.claim ?? record.safeCopy ?? ""}`,
    300,
  )
}

export function promiseMatchMinSimilarity(): number {
  const raw = Number(process.env.SARAH_ECOSYSTEM_PROMISE_MIN_SIMILARITY ?? NaN)
  return Number.isFinite(raw) && raw > 0 && raw <= 1
    ? raw
    : PROMISE_MATCH_MIN_SIMILARITY_DEFAULT
}

export async function promiseLookup(query: string): Promise<PromiseLookupResult> {
  const source = `${sarahEcosystemBaseUrl()}${PROMISE_REGISTRY_PATH}`
  const trimmed = query.trim()
  if (!trimmed) return { ok: false, source, matches: [], error: "empty_query" }

  const registry = (await fetchPublicJson(PROMISE_REGISTRY_PATH)) as {
    promises?: RegistryRecord[]
  } | null
  const records = (registry?.promises ?? []).filter(
    (record) => record.promiseId && record.state,
  )
  if (records.length === 0) {
    return { ok: false, source, matches: [], error: lastError ?? "registry_empty" }
  }

  // Semantic law: embedding match only. No embedder => honest miss.
  const queryEmbedding = await sarahEmbedText(trimmed, "RETRIEVAL_QUERY")
  if (!queryEmbedding) {
    return { ok: false, source, matches: [], error: "embedding_unavailable" }
  }

  const threshold = promiseMatchMinSimilarity()
  const scored: PromiseLookupMatch[] = []
  for (const record of records) {
    const text = registryRecordText(record)
    let embedding = recordEmbeddings.get(text) ?? null
    if (!embedding) {
      embedding = await sarahEmbedText(text, "RETRIEVAL_DOCUMENT")
      if (!embedding) continue
      recordEmbeddings.set(text, embedding)
    }
    const similarity = cosineSimilarity(queryEmbedding, embedding)
    if (similarity < threshold) continue
    const state = record.state ?? "unknown"
    scored.push({
      promiseId: record.promiseId!,
      state,
      safeCopy: clip(record.safeCopy ?? record.claim ?? "No safe public copy provided."),
      caveat: promiseStateCaveat(state),
      blockerRefs: (record.blockerRefs ?? []).slice(0, 3),
      similarity: Number(similarity.toFixed(4)),
    })
  }
  scored.sort((a, b) => b.similarity - a.similarity)
  return { ok: true, source, matches: scored.slice(0, PROMISE_MATCH_TOP_K) }
}

// ---------------------------------------------------------------------------
// live_stats — tokens served + pylon counts from the public projections
// ---------------------------------------------------------------------------

export type LiveStatsResult = {
  ok: boolean
  tokensServed: number | null
  tokensGeneratedAt: string | null
  pylons: {
    onlineNow: number | null
    seen24h: number | null
    registeredTotal: number | null
  }
  sources: string[]
  error?: string
}

export async function liveStats(): Promise<LiveStatsResult> {
  const base = sarahEcosystemBaseUrl()
  const [tokens, pylons] = await Promise.all([
    fetchPublicJson(TOKENS_SERVED_PATH) as Promise<{
      tokensServed?: number
      generatedAt?: string
    } | null>,
    fetchPublicJson(PYLON_STATS_PATH) as Promise<{
      pylonsOnlineNow?: number
      pylonsSeen24h?: number
      pylonsRegisteredTotal?: number
    } | null>,
  ])
  const ok = tokens !== null || pylons !== null
  return {
    ok,
    tokensServed:
      typeof tokens?.tokensServed === "number" ? tokens.tokensServed : null,
    tokensGeneratedAt:
      typeof tokens?.generatedAt === "string" ? tokens.generatedAt : null,
    pylons: {
      onlineNow:
        typeof pylons?.pylonsOnlineNow === "number"
          ? pylons.pylonsOnlineNow
          : null,
      seen24h:
        typeof pylons?.pylonsSeen24h === "number" ? pylons.pylonsSeen24h : null,
      registeredTotal:
        typeof pylons?.pylonsRegisteredTotal === "number"
          ? pylons.pylonsRegisteredTotal
          : null,
    },
    sources: [`${base}${TOKENS_SERVED_PATH}`, `${base}${PYLON_STATS_PATH}`],
    ...(ok ? {} : { error: lastError ?? "stats_unavailable" }),
  }
}

// ---------------------------------------------------------------------------
// plan_catalog — the Khala Code plans projection, passed through honestly
// ---------------------------------------------------------------------------

export type PlanCatalogResult = {
  ok: boolean
  source: string
  summary: string | null
  promiseId: string | null
  plans: Array<{
    planId: string
    kind: string
    label: string
    priceLabel: string
    isDefault: boolean
    captureExcluded: boolean
    purchasable: boolean
  }>
  error?: string
}

export async function planCatalog(): Promise<PlanCatalogResult> {
  const source = `${sarahEcosystemBaseUrl()}${PLAN_CATALOG_PATH}`
  const payload = (await fetchPublicJson(PLAN_CATALOG_PATH)) as {
    catalog?: {
      summary?: string
      promiseId?: string
      plans?: Array<{
        planId?: string
        kind?: string
        label?: string
        priceLabel?: string
        isDefault?: boolean
        captureExcluded?: boolean
        purchase?: { armed?: boolean }
      }>
    }
  } | null
  const catalog = payload?.catalog
  if (!catalog) {
    return {
      ok: false,
      source,
      summary: null,
      promiseId: null,
      plans: [],
      error: lastError ?? "catalog_unavailable",
    }
  }
  return {
    ok: true,
    source,
    summary: catalog.summary ?? null,
    promiseId: catalog.promiseId ?? null,
    plans: (catalog.plans ?? [])
      .filter((plan) => plan.planId)
      .map((plan) => ({
        planId: plan.planId!,
        kind: plan.kind ?? "unknown",
        label: plan.label ?? plan.planId!,
        priceLabel: plan.priceLabel ?? "unpriced",
        isDefault: plan.isDefault === true,
        captureExcluded: plan.captureExcluded === true,
        // Honest purchasability from the fail-closed flag projection —
        // a plan without an armed purchase seam is NOT sellable.
        purchasable: plan.purchase?.armed === true,
      })),
  }
}

// ---------------------------------------------------------------------------
// Grounding injection — the useful non-tool path (Gemma lanes have no native
// function calling). Flag-gated, embedding-routed, one hook per brain lane.
// ---------------------------------------------------------------------------

export type EcosystemIntent = "promise_status" | "live_stats" | "plan_catalog"

/**
 * The small typed intent set. Matching is embedding + cosine against these
 * canonical phrasings — never a regex or keyword test on the user message.
 * Exported so tests can key fake embeddings off the exact canonical texts.
 */
export const ECOSYSTEM_GROUNDING_INTENTS: ReadonlyArray<{
  intent: EcosystemIntent
  canonical: string
}> = [
  {
    intent: "promise_status",
    canonical:
      "Is this OpenAgents product capability live right now? What is the current status of this feature, promise, or claim — is it shipped, limited, or roadmap?",
  },
  {
    intent: "live_stats",
    canonical:
      "How many tokens has Khala served so far? How big is the OpenAgents network — how many pylon nodes are online or registered?",
  },
  {
    intent: "plan_catalog",
    canonical:
      "What Khala Code plans are available? What are the free plan and paid plan terms, and which plan can I buy today?",
  },
]

const intentEmbeddings = new Map<string, number[]>()

export function sarahEcosystemGroundingEnabled(): boolean {
  return process.env.SARAH_ECOSYSTEM_GROUNDING?.trim() === "1"
}

export function groundingMinSimilarity(): number {
  const raw = Number(
    process.env.SARAH_ECOSYSTEM_GROUNDING_MIN_SIMILARITY ?? NaN,
  )
  return Number.isFinite(raw) && raw > 0 && raw <= 1
    ? raw
    : GROUNDING_MIN_SIMILARITY_DEFAULT
}

async function matchEcosystemIntent(
  message: string,
): Promise<EcosystemIntent | null> {
  const queryEmbedding = await sarahEmbedText(message, "RETRIEVAL_QUERY")
  if (!queryEmbedding) return null
  let best: { intent: EcosystemIntent; similarity: number } | null = null
  for (const candidate of ECOSYSTEM_GROUNDING_INTENTS) {
    let embedding = intentEmbeddings.get(candidate.canonical) ?? null
    if (!embedding) {
      embedding = await sarahEmbedText(candidate.canonical, "RETRIEVAL_DOCUMENT")
      if (!embedding) continue
      intentEmbeddings.set(candidate.canonical, embedding)
    }
    const similarity = cosineSimilarity(queryEmbedding, embedding)
    if (!best || similarity > best.similarity) {
      best = { intent: candidate.intent, similarity }
    }
  }
  if (!best || best.similarity < groundingMinSimilarity()) return null
  return best.intent
}

function groundingHeader(): string[] {
  return [
    "[live product truth]",
    `Fetched now from the public openagents.com APIs (agent sheet: ${AGENTS_SHEET_URL}).`,
    "Cite these values honestly; a record's state caps every claim you make about it.",
  ]
}

/**
 * The single grounding hook the two brain lanes call. Flag-off
 * (SARAH_ECOSYSTEM_GROUNDING unset) it returns null without touching the
 * embedder or the network — zero behavior change until the owner arms it.
 * Fail-soft everywhere: a broken endpoint or embedder is a silent null.
 */
export async function maybeEcosystemGrounding(
  message: string,
): Promise<string | null> {
  if (!sarahEcosystemGroundingEnabled()) return null
  const trimmed = message.trim()
  if (!trimmed) return null
  try {
    const intent = await matchEcosystemIntent(trimmed)
    if (!intent) return null

    if (intent === "live_stats") {
      const stats = await liveStats()
      if (!stats.ok) return null
      const lines = [
        ...groundingHeader(),
        `Khala tokens served (network-wide public counter): ${stats.tokensServed ?? "unavailable"}${stats.tokensGeneratedAt ? ` (as of ${stats.tokensGeneratedAt})` : ""}.`,
        `Pylon nodes — online now: ${stats.pylons.onlineNow ?? "unavailable"}, seen in 24h: ${stats.pylons.seen24h ?? "unavailable"}, registered total: ${stats.pylons.registeredTotal ?? "unavailable"}.`,
        `Sources: ${stats.sources.join(", ")}`,
      ]
      return lines.join("\n")
    }

    if (intent === "plan_catalog") {
      const catalog = await planCatalog()
      if (!catalog.ok) return null
      const lines = [
        ...groundingHeader(),
        catalog.summary
          ? `Khala Code plan catalog: ${clip(catalog.summary, 300)}`
          : "Khala Code plan catalog:",
        ...catalog.plans.map(
          (plan) =>
            `- ${plan.label} (${plan.kind}${plan.isDefault ? ", default" : ""}): ${plan.priceLabel}; capture excluded: ${plan.captureExcluded ? "yes" : "no"}; purchasable today: ${plan.purchasable ? "yes" : "NO — do not sell it"}.`,
        ),
        `Source: ${catalog.source}`,
      ]
      return lines.join("\n")
    }

    const lookup = await promiseLookup(trimmed)
    if (!lookup.ok || lookup.matches.length === 0) return null
    const lines = [
      ...groundingHeader(),
      "Closest live promise-registry records for this question:",
      ...lookup.matches.map(
        (match) =>
          `- ${match.promiseId} [${match.state}]: ${match.safeCopy} — ${match.caveat}`,
      ),
      `Source: ${lookup.source}`,
    ]
    return lines.join("\n")
  } catch (error) {
    lastError = error instanceof Error ? error.message : String(error)
    return null
  }
}

// ---------------------------------------------------------------------------
// Ops visibility + test hooks
// ---------------------------------------------------------------------------

export function sarahEcosystemStatus() {
  return {
    groundingEnabled: sarahEcosystemGroundingEnabled(),
    groundingMinSimilarity: groundingMinSimilarity(),
    promiseMatchMinSimilarity: promiseMatchMinSimilarity(),
    baseUrl: sarahEcosystemBaseUrl(),
    cachedEndpoints: jsonCache.size,
    lastError,
  }
}

export function __setSarahEcosystemFetchForTest(fn: FetchLike | null): void {
  fetchImpl = fn ?? ((url, init) => fetch(url, init))
}

export function __resetSarahEcosystemForTest(): void {
  fetchImpl = (url, init) => fetch(url, init)
  jsonCache.clear()
  recordEmbeddings.clear()
  intentEmbeddings.clear()
  lastError = null
}
