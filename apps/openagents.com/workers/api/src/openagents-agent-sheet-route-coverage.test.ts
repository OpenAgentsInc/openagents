import { Effect } from 'effect'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, test } from 'vitest'

import { openAgentsCapabilityManifest } from './openagents-capability-manifest'
import { openAgentsOpenApiDocument } from './openagents-openapi'

const repoRoot = resolve(import.meta.dirname, '../../..')
const liveAgentDocMarkdown = readFileSync(
  resolve(repoRoot, 'docs/live/AGENTS.md'),
  'utf8',
)

type RouteCoverageExpectation = Readonly<{
  authorityPhrases: ReadonlyArray<string>
  manifestId: string
  method: string
  path: string
  requiresIdempotency: boolean
  requiresPaymentLanguage: boolean
  requiresRedactionLanguage: boolean
  status: 'live' | 'planned_or_gated'
}>

const launchCriticalRoutes: ReadonlyArray<RouteCoverageExpectation> = [
  {
    authorityPhrases: ['public self-service agent registration'],
    manifestId: 'register_agent',
    method: 'POST',
    path: '/api/agents/register',
    requiresIdempotency: false,
    requiresPaymentLanguage: false,
    requiresRedactionLanguage: true,
    status: 'live',
  },
  {
    authorityPhrases: ['registered agent token'],
    manifestId: 'agent_home',
    method: 'GET',
    path: '/api/agents/home',
    requiresIdempotency: false,
    requiresPaymentLanguage: false,
    requiresRedactionLanguage: true,
    status: 'live',
  },
  {
    authorityPhrases: ['Idempotency-Key', 'registered agents'],
    manifestId: 'agent_hosted_search',
    method: 'POST',
    path: '/api/agents/search',
    requiresIdempotency: true,
    requiresPaymentLanguage: true,
    requiresRedactionLanguage: true,
    status: 'live',
  },
  {
    authorityPhrases: ['payment preview'],
    manifestId: 'agent_hosted_search_payment_preview',
    method: 'POST',
    path: '/api/agents/search/payments/preview',
    requiresIdempotency: true,
    requiresPaymentLanguage: true,
    requiresRedactionLanguage: true,
    status: 'live',
  },
  {
    authorityPhrases: ['payment redeem'],
    manifestId: 'agent_hosted_search_payment_redeem',
    method: 'POST',
    path: '/api/agents/search/payments/redeem',
    requiresIdempotency: true,
    requiresPaymentLanguage: true,
    requiresRedactionLanguage: true,
    status: 'live',
  },
  {
    authorityPhrases: ['pending review record only'],
    manifestId: 'submit_public_agent_proposal',
    method: 'POST',
    path: '/api/agents/proposals',
    requiresIdempotency: true,
    requiresPaymentLanguage: false,
    requiresRedactionLanguage: true,
    status: 'live',
  },
  {
    authorityPhrases: ['owner-approved public proposal rate-limit recovery'],
    manifestId: 'agent_proposal_rate_limit_recovery',
    method: 'POST',
    path: '/api/agents/proposals/rate-limit/preview',
    requiresIdempotency: true,
    requiresPaymentLanguage: true,
    requiresRedactionLanguage: true,
    status: 'live',
  },
  {
    authorityPhrases: ['owner-approved public proposal rate-limit recovery'],
    manifestId: 'redeem_public_agent_proposal_rate_limit_recovery',
    method: 'POST',
    path: '/api/agents/proposals/rate-limit/redeem',
    requiresIdempotency: true,
    requiresPaymentLanguage: true,
    requiresRedactionLanguage: true,
    status: 'live',
  },
  {
    authorityPhrases: ['public launch status'],
    manifestId: 'forum_launch_status',
    method: 'GET',
    path: '/api/forum/launch-status',
    requiresIdempotency: false,
    requiresPaymentLanguage: false,
    requiresRedactionLanguage: false,
    status: 'live',
  },
  {
    authorityPhrases: ['registered agent tokens'],
    manifestId: 'forum_topic_create',
    method: 'POST',
    path: '/api/forum/forums/{forumSlug}/topics',
    requiresIdempotency: true,
    requiresPaymentLanguage: false,
    requiresRedactionLanguage: true,
    status: 'live',
  },
  {
    authorityPhrases: ['registered agent tokens'],
    manifestId: 'forum_reply_create',
    method: 'POST',
    path: '/api/forum/topics/{topicId}/posts',
    requiresIdempotency: true,
    requiresPaymentLanguage: false,
    requiresRedactionLanguage: true,
    status: 'live',
  },
  {
    authorityPhrases: ['receipt'],
    manifestId: 'forum_receipt_lookup',
    method: 'GET',
    path: '/api/forum/receipts/{receiptRef}',
    requiresIdempotency: false,
    requiresPaymentLanguage: true,
    requiresRedactionLanguage: true,
    status: 'live',
  },
  {
    authorityPhrases: ['settlement claim'],
    manifestId: 'forum_tip_settlement_claim',
    method: 'POST',
    path: '/api/forum/receipts/{receiptRef}/settlement-claims',
    requiresIdempotency: true,
    requiresPaymentLanguage: true,
    requiresRedactionLanguage: true,
    status: 'live',
  },
  {
    authorityPhrases: ['owned Pylon registration'],
    manifestId: 'pylon_register',
    method: 'POST',
    path: '/api/pylons/register',
    requiresIdempotency: true,
    requiresPaymentLanguage: false,
    requiresRedactionLanguage: true,
    status: 'live',
  },
  {
    authorityPhrases: ['owned Pylon'],
    manifestId: 'pylon_heartbeat',
    method: 'POST',
    path: '/api/pylons/{pylonRef}/heartbeat',
    requiresIdempotency: true,
    requiresPaymentLanguage: false,
    requiresRedactionLanguage: true,
    status: 'live',
  },
  {
    authorityPhrases: ['wallet readiness'],
    manifestId: 'pylon_wallet_readiness',
    method: 'POST',
    path: '/api/pylons/{pylonRef}/wallet-readiness',
    requiresIdempotency: true,
    requiresPaymentLanguage: true,
    requiresRedactionLanguage: true,
    status: 'live',
  },
  {
    authorityPhrases: ['public Artanis report'],
    manifestId: 'public_artanis_report',
    method: 'GET',
    path: '/api/public/artanis/report',
    requiresIdempotency: false,
    requiresPaymentLanguage: false,
    requiresRedactionLanguage: true,
    status: 'live',
  },
  {
    authorityPhrases: ['public Pylon stats'],
    manifestId: 'public_pylon_stats',
    method: 'GET',
    path: '/api/public/pylon-stats',
    requiresIdempotency: false,
    requiresPaymentLanguage: true,
    requiresRedactionLanguage: true,
    status: 'live',
  },
  {
    authorityPhrases: ['public Nexus/Pylon receipt'],
    manifestId: 'public_nexus_pylon_receipt',
    method: 'GET',
    path: '/api/public/nexus-pylon/receipts/{receiptRef}',
    requiresIdempotency: false,
    requiresPaymentLanguage: true,
    requiresRedactionLanguage: true,
    status: 'live',
  },
  {
    authorityPhrases: ['Site payment discovery'],
    manifestId: 'site_payment_discovery',
    method: 'GET',
    path: '/api/sites/{siteId}/commerce/discovery',
    requiresIdempotency: false,
    requiresPaymentLanguage: true,
    requiresRedactionLanguage: true,
    status: 'live',
  },
  {
    authorityPhrases: ['payment proof'],
    manifestId: 'site_payment_proof',
    method: 'GET',
    path: '/api/sites/{siteId}/commerce/payment-proofs/{checkoutIntentRef}',
    requiresIdempotency: false,
    requiresPaymentLanguage: true,
    requiresRedactionLanguage: true,
    status: 'live',
  },
  {
    authorityPhrases: ['Site commerce review'],
    manifestId: 'site_commerce_review',
    method: 'GET',
    path: '/api/sites/{siteId}/commerce/review',
    requiresIdempotency: false,
    requiresPaymentLanguage: true,
    requiresRedactionLanguage: true,
    status: 'live',
  },
  {
    authorityPhrases: ['MDK account binding'],
    manifestId: 'site_mdk_account_binding',
    method: 'GET',
    path: '/api/sites/{siteId}/commerce/mdk-account-binding',
    requiresIdempotency: false,
    requiresPaymentLanguage: true,
    requiresRedactionLanguage: true,
    status: 'live',
  },
]

const manifestPath = (path: string): string => `https://openagents.com${path}`

const equivalentPathVariants = (path: string): ReadonlyArray<string> => {
  const variants = new Set([path])
  variants.add(path.replace('{forumSlug}', '{forumId}'))
  variants.add(path.replace('{forumId}', '{forumSlug}'))
  variants.add(path.replace('{receiptRef}', '{receiptId}'))
  variants.add(path.replace('{receiptId}', '{receiptRef}'))
  return [...variants]
}

const manifestEntries = (
  manifest: Awaited<ReturnType<typeof loadManifestAndOpenApi>>['manifest'],
) => [...manifest.resources, ...manifest.actions]

const loadManifestAndOpenApi = async () => {
  const manifest = await Effect.runPromise(openAgentsCapabilityManifest())
  const openApi = await Effect.runPromise(openAgentsOpenApiDocument())

  return { manifest, openApi }
}

const openApiOperation = (
  openApi: Awaited<ReturnType<typeof loadManifestAndOpenApi>>['openApi'],
  route: RouteCoverageExpectation,
) => {
  const pathItem = equivalentPathVariants(route.path)
    .map(path => openApi.paths[path])
    .find(path => path !== undefined) as Record<string, unknown> | undefined

  return pathItem?.[route.method.toLowerCase()] as
    | Record<string, unknown>
    | undefined
}

const operationText = (operation: Record<string, unknown>): string =>
  JSON.stringify({
    description: operation.description,
    parameters: operation.parameters,
    requestBody: operation.requestBody,
    responses: operation.responses,
    security: operation.security,
    summary: operation.summary,
  }).toLowerCase()

const agentDocContainsRoute = (path: string): boolean =>
  equivalentPathVariants(path).some(
    variant =>
      liveAgentDocMarkdown.includes(variant) ||
      liveAgentDocMarkdown.includes(variant.replace('/api/', 'GET /api/')) ||
      liveAgentDocMarkdown.includes(variant.replace('/api/', 'POST /api/')),
  )

describe('OpenAgents agent sheet route coverage', () => {
  test('keeps launch-critical routes covered by AGENTS.md, manifest, and OpenAPI', async () => {
    const { manifest, openApi } = await loadManifestAndOpenApi()
    const entries = manifestEntries(manifest)

    const missing = launchCriticalRoutes.flatMap(route => {
      const manifestEntry = entries.find(
        entry =>
          entry.id === route.manifestId &&
          equivalentPathVariants(route.path)
            .map(manifestPath)
            .includes(entry.href) &&
          entry.method === route.method,
      )
      const operation = openApiOperation(openApi, route)

      return [
        ...(agentDocContainsRoute(route.path)
          ? []
          : [`AGENTS.md missing ${route.method} ${route.path}`]),
        ...(manifestEntry === undefined
          ? [
              `manifest missing ${route.manifestId} ${route.method} ${route.path}`,
            ]
          : []),
        ...(operation === undefined
          ? [`OpenAPI missing ${route.method.toLowerCase()} ${route.path}`]
          : []),
      ]
    })

    expect(missing).toEqual([])
  })

  test('keeps launch-critical route authority, idempotency, payment, and redaction language explicit', async () => {
    const { openApi } = await loadManifestAndOpenApi()

    const missing = launchCriticalRoutes.flatMap(route => {
      const operation = openApiOperation(openApi, route)
      const text =
        operation === undefined
          ? ''
          : `${operationText(operation)} ${liveAgentDocMarkdown}`.toLowerCase()

      return [
        ...route.authorityPhrases
          .filter(phrase => !text.includes(phrase.toLowerCase()))
          .map(
            phrase =>
              `${route.method} ${route.path} missing authority phrase ${phrase}`,
          ),
        ...(route.requiresIdempotency && !text.includes('idempotency-key')
          ? [`${route.method} ${route.path} missing idempotency language`]
          : []),
        ...(route.requiresPaymentLanguage &&
        !/(payment|paid|payout|settlement|l402|checkout|wallet|bitcoin|sats)/i.test(
          text,
        )
          ? [`${route.method} ${route.path} missing payment-state language`]
          : []),
        ...(route.requiresRedactionLanguage &&
        !/(redact|raw|private|secret|token|preimage|invoice|wallet material)/i.test(
          text,
        )
          ? [`${route.method} ${route.path} missing redaction language`]
          : []),
      ]
    })

    expect(missing).toEqual([])
  })

  test('keeps planned or gated launch language non-callable', async () => {
    const { manifest, openApi } = await loadManifestAndOpenApi()
    const broadApiKey = manifest.authModes.find(
      mode => mode.id === 'broad_scoped_api_key',
    )

    expect(broadApiKey).toMatchObject({ status: 'planned' })
    expect(liveAgentDocMarkdown).toContain('Planned Or Gated Surfaces')
    expect(liveAgentDocMarkdown).toContain('planned or gated')
    expect(liveAgentDocMarkdown).toContain('does not grant broad write')
    expect(openApi.paths['/api/agents/scoped-api-keys']).toBeUndefined()
  })
})
