// Live data wiring for the WEB-1 sales-landing PREVIEW route
// (`/preview/sales-landing`, GitHub issue #8565).
//
// This module is the ONLY place the preview landing talks to real systems.
// Everything here is a public, no-auth, no-spend, no-mutation GET against the
// production `openagents.com` worker's public projection endpoints — the same
// endpoints the `/stats` and `/pylons` routes and the Khala CLI already read.
// Every fetch is fail-soft: any network/parse/non-200 error resolves to `null`
// and the section renders its honest "unavailable" state rather than a
// fabricated number. No counter, price, or plan value is ever hardcoded here.

// --- Tokens served headline counter -----------------------------------------
// GET /api/public/khala-tokens-served ->
//   { schemaVersion: 'openagents.public_khala_tokens_served.v1',
//     tokensServed: number, generatedAt: string, staleness: {...} }
// Handler: workers/api/src/public-khala-tokens-served-routes.ts
export const KHALA_TOKENS_SERVED_URL = '/api/public/khala-tokens-served'

export type KhalaTokensServedSnapshot = Readonly<{
  tokensServed: number
  generatedAt?: string | undefined
}>

export const fetchKhalaTokensServed = async (
  fetchFn: typeof fetch = fetch,
  url: string = KHALA_TOKENS_SERVED_URL,
): Promise<KhalaTokensServedSnapshot | null> => {
  try {
    const response = await fetchFn(url, { headers: { accept: 'application/json' } })
    if (!response.ok) return null
    const body = (await response.json()) as Partial<KhalaTokensServedSnapshot>
    if (typeof body.tokensServed !== 'number' || !Number.isFinite(body.tokensServed)) {
      return null
    }
    return { tokensServed: body.tokensServed, generatedAt: body.generatedAt }
  } catch {
    return null
  }
}

// --- Khala Code plan catalog (public pricing source of truth) ----------------
// GET /api/public/khala-code/plans ->
//   { schemaVersion: 'openagents.khala_code.plan_catalog.v1',
//     plans: [{ planId, kind, label, tagline, priceLabel, isDefault,
//               captureExcluded, terms: string[], purchase?: {...} }], ... }
// Handler: workers/api/src/inference/khala-code-plan-routes.ts
// Source of truth: workers/api/src/inference/khala-code-plan-catalog.ts
//
// This is the only PUBLIC pricing projection. The card-checkout credit packs
// (STRIPE_CREDIT_PACKAGES_JSON) and mobile IAP catalog live server-side and are
// not exposed on any public endpoint, so they are intentionally NOT wired here;
// exposing them would require a new public projection route (owner/backend
// decision). The Free/Paid privacy plans below are the honest, config-backed
// public pricing surface.
export const KHALA_CODE_PLANS_URL = '/api/public/khala-code/plans'

export type KhalaCodePlanProjection = Readonly<{
  planId: string
  kind: 'free' | 'paid'
  label: string
  tagline: string
  priceLabel: string
  isDefault: boolean
  captureExcluded: boolean
  terms: ReadonlyArray<string>
  purchase?: Readonly<{ armed: boolean }>
}>

export type KhalaCodePlanCatalogProjection = Readonly<{
  summary?: string | undefined
  plans: ReadonlyArray<KhalaCodePlanProjection>
}>

export const fetchKhalaCodePlans = async (
  fetchFn: typeof fetch = fetch,
  url: string = KHALA_CODE_PLANS_URL,
): Promise<KhalaCodePlanCatalogProjection | null> => {
  try {
    const response = await fetchFn(url, { headers: { accept: 'application/json' } })
    if (!response.ok) return null
    const body = (await response.json()) as Partial<KhalaCodePlanCatalogProjection>
    if (!Array.isArray(body.plans) || body.plans.length === 0) return null
    return { summary: body.summary, plans: body.plans as KhalaCodePlanProjection[] }
  } catch {
    return null
  }
}

// --- Formatting -------------------------------------------------------------
// The exact placeholder every live value shows before the first client fetch
// resolves (SSR / no-JS state). Never a fabricated number.
export const LIVE_VALUE_PENDING = '—'

export const formatCount = (value: number | null | undefined): string =>
  typeof value === 'number' && Number.isFinite(value) && value >= 0
    ? Math.round(value).toLocaleString('en-US')
    : LIVE_VALUE_PENDING

// --- Approved-reuse CTA + link targets --------------------------------------
// Reused from existing, already-in-repo approved surfaces. `talkToSarah` is the
// direction named in issue #8565 (sarah.openagents.com; embedded later).
export const SALES_LANDING_LINKS = {
  talkToSarah: 'https://sarah.openagents.com',
  businessIntake: '/business#business-intake',
  khala: '/khala',
  promises: '/docs/product-promises',
  promisesJson: '/api/public/product-promises',
  forum: 'https://openagents.com/forum',
  docs: '/docs',
  stats: '/stats',
  github: 'https://github.com/OpenAgentsInc/openagents',
} as const
