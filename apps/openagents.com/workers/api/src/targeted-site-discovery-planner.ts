import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect, Schema as S } from 'effect'

import type { ExaClientShape, ExaSearchInput, ExaSearchResult } from './exa'
import {
  TargetedSiteOutreachStorageError,
  TargetedSiteOutreachValidationError,
  upsertTargetedSiteProspect,
  type TargetedSiteProspectRecord,
} from './targeted-site-outreach'

export const TargetedSiteDiscoveryPlan = S.Struct({
  campaignId: S.String,
  dryRun: S.Boolean,
  exaSearch: S.Struct({
    category: S.optionalKey(S.String),
    numResults: S.Number,
    query: S.String,
    type: S.optionalKey(S.String),
  }),
  geography: S.NullOr(S.String),
  idempotencyKeyPrefix: S.String,
  maxResults: S.Number,
  qualitySignals: S.Array(S.String),
  sourceRunRef: S.String,
  vertical: S.NullOr(S.String),
})
export type TargetedSiteDiscoveryPlan = typeof TargetedSiteDiscoveryPlan.Type

export const TargetedSiteDiscoverySourceCard = S.Struct({
  campaignId: S.String,
  confidence: S.Number,
  domain: S.String,
  prospectIdempotencyKey: S.String,
  resultUrl: S.String,
  snippet: S.NullOr(S.String),
  sourceRef: S.String,
  sourceRunRef: S.String,
  title: S.NullOr(S.String),
})
export type TargetedSiteDiscoverySourceCard =
  typeof TargetedSiteDiscoverySourceCard.Type

export const TargetedSiteDiscoveryRunResult = S.Struct({
  persistedProspects: S.Array(S.Unknown),
  plan: TargetedSiteDiscoveryPlan,
  sourceCards: S.Array(TargetedSiteDiscoverySourceCard),
})
export type TargetedSiteDiscoveryRunResult = Readonly<{
  persistedProspects: ReadonlyArray<TargetedSiteProspectRecord>
  plan: TargetedSiteDiscoveryPlan
  sourceCards: ReadonlyArray<TargetedSiteDiscoverySourceCard>
}>

export type TargetedSiteDiscoveryPlanInput = Readonly<{
  campaignId: string
  dryRun?: boolean | undefined
  excludeDomains?: ReadonlyArray<string> | undefined
  geography?: string | undefined
  idempotencyKeyPrefix: string
  includeDomains?: ReadonlyArray<string> | undefined
  maxResults?: number | undefined
  qualitySignals?: ReadonlyArray<string> | undefined
  sourceRunRef: string
  vertical?: string | undefined
}>

export class TargetedSiteDiscoveryValidationError extends S.TaggedErrorClass<TargetedSiteDiscoveryValidationError>()(
  'TargetedSiteDiscoveryValidationError',
  {
    reason: S.String,
  },
) {}

export class TargetedSiteDiscoveryStorageError extends S.TaggedErrorClass<TargetedSiteDiscoveryStorageError>()(
  'TargetedSiteDiscoveryStorageError',
  {
    operation: S.String,
    reason: S.String,
  },
) {}

export type TargetedSiteDiscoveryError =
  | TargetedSiteDiscoveryStorageError
  | TargetedSiteDiscoveryValidationError
  | TargetedSiteOutreachStorageError
  | TargetedSiteOutreachValidationError

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,220}$/
const SAFE_DOMAIN_PATTERN =
  /^(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z][a-z0-9-]{1,62}$/
const PROHIBITED_TEXT_PATTERN =
  /\b(provider[_ -]?payload|provider[_ -]?account|auth[_ -]?grant|access_token|refresh_token|device_auth_id|code_verifier|token_hash|private_key|wallet_secret|mdk_access_token|payment_preimage|payment_secret|webhook_secret|gho_[a-z0-9_]+|lnbc|lntb|lnbcrt|lno1|xprv|mnemonic)\b|@/i

const compactText = (value: string, maxLength: number): string =>
  value.trim().replace(/\s+/g, ' ').slice(0, maxLength)

const textIsSafe = (value: string): boolean =>
  !containsProviderSecretMaterial(value) && !PROHIBITED_TEXT_PATTERN.test(value)

const safeText = (
  value: string | undefined,
  maxLength: number,
): string | null => {
  if (value === undefined) {
    return null
  }

  const compact = compactText(value, maxLength)

  return compact === '' || !textIsSafe(compact) ? null : compact
}

const assertSafeRef = (field: string, value: string): void => {
  if (!SAFE_REF_PATTERN.test(value) || !textIsSafe(value)) {
    throw new TargetedSiteDiscoveryValidationError({
      reason: `${field} must be a public-safe ref.`,
    })
  }
}

const safeIdFragment = (value: string): string => {
  const fragment = value
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '_')
    .replace(/^_+|_+$/g, '')
    .slice(0, 80)

  return fragment === '' ? 'target' : fragment
}

const normalizedDomainFromUrl = (urlValue: string): string | null => {
  if (!textIsSafe(urlValue)) {
    return null
  }

  try {
    const url = new URL(urlValue)

    if (url.protocol !== 'https:' && url.protocol !== 'http:') {
      return null
    }

    if (url.username !== '' || url.password !== '') {
      return null
    }

    const hostname = url.hostname.toLowerCase().replace(/^www\./, '')

    return SAFE_DOMAIN_PATTERN.test(hostname) && textIsSafe(hostname)
      ? hostname
      : null
  } catch {
    return null
  }
}

const boundedMaxResults = (value: number | undefined): number => {
  if (value === undefined || !Number.isFinite(value)) {
    return 8
  }

  return Math.max(1, Math.min(25, Math.floor(value)))
}

const normalizedSignals = (
  signals: ReadonlyArray<string> | undefined,
): ReadonlyArray<string> =>
  (signals ?? [])
    .map(signal => safeText(signal, 80))
    .filter((signal): signal is string => signal !== null)
    .slice(0, 6)

const searchQuery = (
  input: TargetedSiteDiscoveryPlanInput,
  qualitySignals: ReadonlyArray<string>,
): string => {
  const parts = [
    'business website needing a modern web presence',
    safeText(input.vertical, 80),
    safeText(input.geography, 80),
    ...qualitySignals,
    'public company website',
  ].filter((part): part is string => part !== null)

  return parts.join(' ')
}

const confidenceFromResult = (result: ExaSearchResult): number => {
  const base = Number.isFinite(result.score) ? Number(result.score) : 0.5

  return Math.max(0, Math.min(1, base))
}

const sourceCardFromResult = (
  plan: TargetedSiteDiscoveryPlan,
  result: ExaSearchResult,
  index: number,
): TargetedSiteDiscoverySourceCard | null => {
  const domain = normalizedDomainFromUrl(result.url)

  if (domain === null) {
    return null
  }

  const title = safeText(result.title, 160)
  const snippet = safeText(
    result.contents?.summary ??
      result.summary ??
      result.contents?.highlights?.[0] ??
      result.highlights?.[0] ??
      result.text,
    360,
  )
  const sourceRef = `${plan.sourceRunRef}:exa:${safeIdFragment(domain)}:${index}`

  if (!textIsSafe(sourceRef)) {
    return null
  }

  return {
    campaignId: plan.campaignId,
    confidence: confidenceFromResult(result),
    domain,
    prospectIdempotencyKey: `${plan.idempotencyKeyPrefix}:${domain}`,
    resultUrl: result.url,
    snippet,
    sourceRef,
    sourceRunRef: plan.sourceRunRef,
    title,
  }
}

const dedupeCards = (
  cards: ReadonlyArray<TargetedSiteDiscoverySourceCard>,
  maxResults: number,
): ReadonlyArray<TargetedSiteDiscoverySourceCard> => {
  const seen = new Set<string>()
  const deduped: Array<TargetedSiteDiscoverySourceCard> = []

  for (const card of cards) {
    if (!seen.has(card.domain)) {
      seen.add(card.domain)
      deduped.push(card)
    }

    if (deduped.length >= maxResults) {
      break
    }
  }

  return deduped
}

export const buildTargetedSiteDiscoveryPlan = (
  input: TargetedSiteDiscoveryPlanInput,
): TargetedSiteDiscoveryPlan => {
  assertSafeRef('campaignId', input.campaignId)
  assertSafeRef('idempotencyKeyPrefix', input.idempotencyKeyPrefix)
  assertSafeRef('sourceRunRef', input.sourceRunRef)

  const maxResults = boundedMaxResults(input.maxResults)
  const qualitySignals = normalizedSignals(input.qualitySignals)
  const query = searchQuery(input, qualitySignals)

  return {
    campaignId: input.campaignId,
    dryRun: input.dryRun ?? true,
    exaSearch: {
      category: 'company',
      numResults: maxResults,
      query,
      type: 'auto',
    },
    geography: safeText(input.geography, 80),
    idempotencyKeyPrefix: input.idempotencyKeyPrefix,
    maxResults,
    qualitySignals,
    sourceRunRef: input.sourceRunRef,
    vertical: safeText(input.vertical, 80),
  }
}

const exaInputFromPlan = (
  plan: TargetedSiteDiscoveryPlan,
  input: TargetedSiteDiscoveryPlanInput,
): ExaSearchInput => {
  const exaInput = {
    category: 'company',
    contents: {
      highlights: { maxCharacters: 240, numSentences: 2 },
      summary: { query: plan.exaSearch.query },
    },
    numResults: plan.maxResults,
    query: plan.exaSearch.query,
    type: 'auto',
  } satisfies ExaSearchInput

  return {
    ...exaInput,
    ...(input.excludeDomains === undefined
      ? {}
      : {
          excludeDomains: input.excludeDomains.map(domain =>
            domain.trim().toLowerCase(),
          ),
        }),
    ...(input.includeDomains === undefined
      ? {}
      : {
          includeDomains: input.includeDomains.map(domain =>
            domain.trim().toLowerCase(),
          ),
        }),
  }
}

export const sourceCardsFromExaResults = (
  plan: TargetedSiteDiscoveryPlan,
  results: ReadonlyArray<ExaSearchResult>,
): ReadonlyArray<TargetedSiteDiscoverySourceCard> =>
  dedupeCards(
    results
      .map((result, index) => sourceCardFromResult(plan, result, index))
      .filter((card): card is TargetedSiteDiscoverySourceCard => card !== null),
    plan.maxResults,
  )

export const runTargetedSiteDiscoveryPlan = (
  db: D1Database,
  exa: ExaClientShape,
  input: TargetedSiteDiscoveryPlanInput,
) =>
  Effect.gen(function* () {
    const plan = buildTargetedSiteDiscoveryPlan(input)
    const response = yield* exa.search(exaInputFromPlan(plan, input))
    const sourceCards = sourceCardsFromExaResults(plan, response.results)

    if (plan.dryRun) {
      return {
        persistedProspects: [],
        plan,
        sourceCards,
      } satisfies TargetedSiteDiscoveryRunResult
    }

    const persistedProspects: Array<TargetedSiteProspectRecord> = []

    for (const card of sourceCards) {
      const prospect = yield* Effect.tryPromise({
        catch: error =>
          error instanceof TargetedSiteOutreachValidationError ||
          error instanceof TargetedSiteOutreachStorageError
            ? error
            : new TargetedSiteDiscoveryStorageError({
                operation: 'targetedSiteDiscovery.upsertProspect',
                reason: 'targeted Site prospect upsert failed.',
              }),
        try: () =>
          upsertTargetedSiteProspect(db, {
            campaignId: plan.campaignId,
            companyName: card.title ?? undefined,
            discoveryConfidence: card.confidence,
            geography: plan.geography ?? undefined,
            idempotencyKey: card.prospectIdempotencyKey,
            metadata: {
              sourceRunRef: plan.sourceRunRef,
            },
            originUrl: card.resultUrl,
            reviewState: 'pending',
            siteName: card.title ?? undefined,
            sourceRef: card.sourceRef,
            suppressionState: 'unknown',
            targetDomain: card.domain,
            vertical: plan.vertical ?? undefined,
          }),
      })

      persistedProspects.push(prospect)
    }

    return {
      persistedProspects,
      plan,
      sourceCards,
    } satisfies TargetedSiteDiscoveryRunResult
  })
