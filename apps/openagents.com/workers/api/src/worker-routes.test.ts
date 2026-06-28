import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import type { ExactRoute } from './http/router'
import type { Env } from './index'
import { WorkerRequestLayer } from './runtime'
import {
  gatewayLegacyPathname,
  makeWorkerRouteRequest,
  shouldRedirectUnknownDocumentToHome,
} from './worker-routes'

const requestFor = (pathname: string, init: RequestInit = {}) =>
  new Request(`https://openagents.com${pathname}`, {
    headers: { accept: 'text/html', ...(init.headers ?? {}) },
    method: init.method ?? 'GET',
  })

describe('Worker document route fallback', () => {
  test('keeps the product promises document route in the app shell', () => {
    expect(
      shouldRedirectUnknownDocumentToHome(requestFor('/promises'), '/promises'),
    ).toBe(false)
  })

  test('keeps public training run document routes in the app shell', () => {
    expect(
      shouldRedirectUnknownDocumentToHome(requestFor('/tassadar'), '/tassadar'),
    ).toBe(false)
    expect(
      shouldRedirectUnknownDocumentToHome(
        requestFor('/tassadar/replay/first-real-settlement'),
        '/tassadar/replay/first-real-settlement',
      ),
    ).toBe(false)
    expect(
      shouldRedirectUnknownDocumentToHome(
        requestFor(
          '/tassadar/replay/first-real-settlement?camera=social&duration=60&hud=social',
        ),
        '/tassadar/replay/first-real-settlement',
      ),
    ).toBe(false)
    expect(
      shouldRedirectUnknownDocumentToHome(
        requestFor('/training/runs'),
        '/training/runs',
      ),
    ).toBe(false)
    expect(
      shouldRedirectUnknownDocumentToHome(
        requestFor('/training/runs/run.cs336.a1.demo'),
        '/training/runs/run.cs336.a1.demo',
      ),
    ).toBe(false)
  })

  test('keeps the components gallery document route in the app shell', () => {
    expect(
      shouldRedirectUnknownDocumentToHome(
        requestFor('/components'),
        '/components',
      ),
    ).toBe(false)
  })

  test('keeps Forge and business funnel document routes in the app shell', () => {
    expect(
      shouldRedirectUnknownDocumentToHome(requestFor('/business'), '/business'),
    ).toBe(false)
    expect(
      shouldRedirectUnknownDocumentToHome(requestFor('/forge'), '/forge'),
    ).toBe(false)
  })

  test('keeps autopilot onboarding and its legal vertical in the app shell', () => {
    expect(
      shouldRedirectUnknownDocumentToHome(
        requestFor('/autopilot'),
        '/autopilot',
      ),
    ).toBe(false)
    expect(
      shouldRedirectUnknownDocumentToHome(
        requestFor('/autopilot/legal'),
        '/autopilot/legal',
      ),
    ).toBe(false)
    expect(
      shouldRedirectUnknownDocumentToHome(
        requestFor('/autopilot/work'),
        '/autopilot/work',
      ),
    ).toBe(false)
  })

  test('redirects unclaimed autopilot vertical and deeper onboarding documents', () => {
    expect(
      shouldRedirectUnknownDocumentToHome(
        requestFor('/autopilot/foo'),
        '/autopilot/foo',
      ),
    ).toBe(true)
    expect(
      shouldRedirectUnknownDocumentToHome(
        requestFor('/autopilot/legal/foo'),
        '/autopilot/legal/foo',
      ),
    ).toBe(true)
  })

  test('keeps the Moksha document route in the app shell', () => {
    expect(
      shouldRedirectUnknownDocumentToHome(requestFor('/moksha'), '/moksha'),
    ).toBe(false)
  })

  test('keeps the OpenAgents Moksha document route in the app shell', () => {
    expect(
      shouldRedirectUnknownDocumentToHome(requestFor('/moksha2'), '/moksha2'),
    ).toBe(false)
  })

  test('keeps the standalone landing document route in the app shell', () => {
    expect(
      shouldRedirectUnknownDocumentToHome(requestFor('/landing'), '/landing'),
    ).toBe(false)
  })

  test('keeps the public Khala document route in the app shell when unauthed', () => {
    expect(
      shouldRedirectUnknownDocumentToHome(requestFor('/khala'), '/khala'),
    ).toBe(false)
  })

  test('keeps the GPT-OSS Gym document route in the app shell', () => {
    expect(
      shouldRedirectUnknownDocumentToHome(requestFor('/gym/oss'), '/gym/oss'),
    ).toBe(false)
  })

  test('keeps the public Terminal-Bench Gym document route in the app shell', () => {
    expect(shouldRedirectUnknownDocumentToHome(requestFor('/gym'), '/gym')).toBe(
      false,
    )
  })

  test('redirects unclaimed Gym document routes', () => {
    expect(
      shouldRedirectUnknownDocumentToHome(requestFor('/gym/foo'), '/gym/foo'),
    ).toBe(true)
    expect(
      shouldRedirectUnknownDocumentToHome(
        requestFor('/gym/oss/bar'),
        '/gym/oss/bar',
      ),
    ).toBe(true)
    expect(
      shouldRedirectUnknownDocumentToHome(
        requestFor('/gym/legal'),
        '/gym/legal',
      ),
    ).toBe(true)
  })

  test('keeps the Pylon document route in the app shell', () => {
    expect(
      shouldRedirectUnknownDocumentToHome(requestFor('/pylons'), '/pylons'),
    ).toBe(false)
  })

  test('serves the public legal document routes in the app shell for unauthed visitors', () => {
    expect(
      shouldRedirectUnknownDocumentToHome(requestFor('/terms'), '/terms'),
    ).toBe(false)
    expect(
      shouldRedirectUnknownDocumentToHome(requestFor('/privacy'), '/privacy'),
    ).toBe(false)
  })

  test('keeps public stats document routes in the app shell', () => {
    expect(
      shouldRedirectUnknownDocumentToHome(requestFor('/stats'), '/stats'),
    ).toBe(false)
    expect(
      shouldRedirectUnknownDocumentToHome(
        requestFor('/stats-old'),
        '/stats-old',
      ),
    ).toBe(false)
  })

  test('keeps forum thread document routes in the app shell for crawlers', () => {
    // `/forum/t/{id}` must reach the social-preview handler, not the
    // unknown-document redirect, so the injected OG/Twitter meta is served.
    expect(
      shouldRedirectUnknownDocumentToHome(
        requestFor('/forum/t/55555555-5555-4555-8555-555555555555'),
        '/forum/t/55555555-5555-4555-8555-555555555555',
      ),
    ).toBe(false)
  })

  test('does not redirect the forum OG image route (treated as a file)', () => {
    expect(
      shouldRedirectUnknownDocumentToHome(
        requestFor('/og/forum/55555555-5555-4555-8555-555555555555.svg'),
        '/og/forum/55555555-5555-4555-8555-555555555555.svg',
      ),
    ).toBe(false)
  })

  test('redirects unknown direct browser document paths to the homepage', () => {
    expect(
      shouldRedirectUnknownDocumentToHome(
        requestFor('/not-a-real-page'),
        '/not-a-real-page',
      ),
    ).toBe(true)
    expect(
      shouldRedirectUnknownDocumentToHome(requestFor('/live'), '/live'),
    ).toBe(true)
  })

  test('does not redirect API, asset, or file-like requests', () => {
    expect(
      shouldRedirectUnknownDocumentToHome(
        requestFor('/api/unknown'),
        '/api/unknown',
      ),
    ).toBe(false)
    expect(
      shouldRedirectUnknownDocumentToHome(
        requestFor('/assets/missing.js'),
        '/assets/missing.js',
      ),
    ).toBe(false)
    expect(
      shouldRedirectUnknownDocumentToHome(
        requestFor('/favicon.ico'),
        '/favicon.ico',
      ),
    ).toBe(false)
  })
})

describe('Canonical /api gateway base alias (#6148)', () => {
  test('maps canonical /api/v1 + /api/mpp paths back to their legacy gateway path', () => {
    expect(gatewayLegacyPathname('/api/v1/models')).toBe('/v1/models')
    expect(gatewayLegacyPathname('/api/v1/chat/completions')).toBe(
      '/v1/chat/completions',
    )
    expect(gatewayLegacyPathname('/api/v1/quote')).toBe('/v1/quote')
    expect(gatewayLegacyPathname('/api/v1/gateway/glm-fleet/readiness')).toBe(
      '/v1/gateway/glm-fleet/readiness',
    )
    expect(gatewayLegacyPathname('/api/v1/inference/batches')).toBe(
      '/v1/inference/batches',
    )
    expect(gatewayLegacyPathname('/api/mpp/v1/chat/completions')).toBe(
      '/mpp/v1/chat/completions',
    )
  })

  test('leaves legacy gateway paths and ordinary /api product routes untouched', () => {
    expect(gatewayLegacyPathname('/v1/models')).toBeUndefined()
    expect(gatewayLegacyPathname('/mpp/v1/chat/completions')).toBeUndefined()
    expect(gatewayLegacyPathname('/api/billing/checkout')).toBeUndefined()
    expect(
      gatewayLegacyPathname('/api/public/product-promises'),
    ).toBeUndefined()
    // Must not match a non-gateway `/api/v1x...` lookalike or a bare `/api/v`.
    expect(gatewayLegacyPathname('/api/v1x/models')).toBeUndefined()
    expect(gatewayLegacyPathname('/api/mppx/v1')).toBeUndefined()
  })
})

describe('Worker route dual-serve resolution (#6148)', () => {
  const env = {
    OPENAGENTS_DB: {} as unknown,
    SYNC_ROOM: {} as unknown,
  } as unknown as Env

  const ctx = {
    passThroughOnException: () => undefined,
    props: undefined,
    waitUntil: () => undefined,
  } as unknown as ExecutionContext

  // Records the exact-route path that matched, and the request.url an optional
  // path-param dispatcher observed, so the test can prove the canonical and
  // legacy paths reach the SAME handler with the SAME normalized request.
  const makeProbe = () => {
    const observed = {
      exactPath: undefined as string | undefined,
      dispatcherPathname: undefined as string | undefined,
      forumTopicId: undefined as string | undefined,
    }

    const okResponse = new Response('ok', { status: 200 })

    const exactRoute = (path: string): ExactRoute<Env> => ({
      path,
      handler: () =>
        Effect.sync(() => {
          observed.exactPath = path
          return okResponse
        }),
    })

    const exactRoutes: ReadonlyArray<ExactRoute<Env>> = [
      exactRoute('/v1/models'),
      exactRoute('/v1/chat/completions'),
      exactRoute('/v1/gateway/glm-fleet/readiness'),
      exactRoute('/mpp/v1/chat/completions'),
    ]

    const noRoute = () => undefined

    // The model-retrieve dispatcher reads request.url directly; it stands in for
    // every path-param dispatcher that re-parses the request rather than the
    // exact-route pathname. It only matches `/v1/models/:model`.
    const routeModelRetrieveRequest = (request: Request) => {
      const pathname = new URL(request.url).pathname
      if (/^\/v1\/models\/[^/]+$/.test(pathname)) {
        return Effect.sync(() => {
          observed.dispatcherPathname = pathname
          return okResponse
        })
      }
      return undefined
    }

    const dependencies = {
      cleanProductRouteRedirectLocation: () => undefined,
      exactRoutes,
      handleAssetRequest: () => Effect.succeed(okResponse),
      handleAppShellPage: () => Effect.succeed(okResponse),
      handleThreadPage: () => Effect.succeed(okResponse),
      handleForumThreadPage: (
        _request: Request,
        _env: Env,
        _ctx: ExecutionContext,
        topicId: string,
      ) =>
        Effect.sync(() => {
          observed.forumTopicId = topicId
          return new Response(
            '<!doctype html><meta property="og:title" content="First Topic">',
            {
              headers: { 'content-type': 'text/html; charset=utf-8' },
              status: 200,
            },
          )
        }),
      optionalUuid: (value: string | undefined) => value,
      routeAutopilotWorkRequest: noRoute,
      routeCloudCodingSessionRequest: noRoute,
      routeAgentGoalRequest: noRoute,
      routeAutopilotOnboardingTurnRequest: noRoute,
      routeKhalaChatRequest: noRoute,
      routeAgentOwnerClaimRequest: noRoute,
      routeCheckoutPageRequest: noRoute,
      routeTreasuryPageRequest: noRoute,
      routeAgentProposalRequest: noRoute,
      routeAgentSearchRequest: noRoute,
      routeAgentScopedGrantRequest: noRoute,
      routeAgentSiteRequest: noRoute,
      routeForumRequest: noRoute,
      routeImageGenerationRequest: noRoute,
      routeModelRetrieveRequest,
      routeMirrorCodeRunByIdRequest: noRoute,
      routeDurableInferenceReadRequest: noRoute,
      routeMulletRequest: noRoute,
      routeOmniRequest: noRoute,
      routeOnboardingRequest: noRoute,
      routeNexusPylonVisibilityRequest: noRoute,
      routePublicCardCreditSpendReceiptRequest: noRoute,
      routePublicCloudPrimitiveReceiptRequest: noRoute,
      routePublicInferenceReceiptRequest: noRoute,
      routePublicNip90MarketReceiptRequest: noRoute,
      routePublicPartnerPayoutReceiptRequest: noRoute,
      routePublicSiteReferralPayoutReceiptRequest: noRoute,
      routePublicStripeCheckoutReceiptRequest: noRoute,
      routeEcommerceCampaignReceiptRequest: noRoute,
      routeEcommerceCampaignReceiptOperatorRequest: noRoute,
      routeEcommerceCampaignSelfServeRequest: noRoute,
      routeMarketingAgencyReceiptRequest: noRoute,
      routeMarketingAgencySelfServeRequest: noRoute,
      routePylonApiRequest: noRoute,
      routeSiteCommerceRequest: noRoute,
      routeSiteReferralInspectionRequest: noRoute,
      routeSiteReferralPayoutLedgerRequest: noRoute,
      routeInferenceReferralRequest: noRoute,
      routeSiteReferralRequest: noRoute,
      routeOperatorAdjutantRequest: noRoute,
      routeOperatorArtanisChatRequest: noRoute,
      routeOperatorArtanisConsoleRequest: noRoute,
      routeOperatorEmailInspectionRequest: noRoute,
      routeOperatorOrderTriageRequest: noRoute,
      routeOperatorPylonMarketplaceRequest: noRoute,
      routeOperatorProviderAccountRequest: noRoute,
      routeOperatorSitesRequest: noRoute,
      routeProviderAccountRequest: noRoute,
      routeShareRequest: noRoute,
      routeSyncRequest: () => Effect.succeed(okResponse),
      routeHygieneLaneSettlementRequest: noRoute,
      routeFirmupLaneSettlementRequest: noRoute,
      routeTassadarTraceContributionRequest: noRoute,
      routeTraceRequest: noRoute,
      routeTeamChatRequest: noRoute,
      routeThreadFileRequest: noRoute,
      routeTrainingRunWindowRequest: noRoute,
      routeTrainingVerificationRequest: noRoute,
    }

    return { dependencies, observed }
  }

  const runRoute = async (request: Request) => {
    const { dependencies, observed } = makeProbe()
    const response = await Effect.runPromise(
      makeWorkerRouteRequest(dependencies)().pipe(
        Effect.provide(WorkerRequestLayer({ ctx, env, request })),
      ),
    )
    return { response, observed }
  }

  test('canonical /api/v1/models resolves to the same exact handler as /v1/models', async () => {
    const legacy = await runRoute(
      new Request('https://openagents.com/v1/models'),
    )
    const canonical = await runRoute(
      new Request('https://openagents.com/api/v1/models'),
    )

    expect(legacy.response.status).toBe(200)
    expect(canonical.response.status).toBe(200)
    expect(legacy.observed.exactPath).toBe('/v1/models')
    expect(canonical.observed.exactPath).toBe('/v1/models')
  })

  test('canonical /api/v1/chat/completions reaches the gateway handler (POST, not redirected)', async () => {
    const canonical = await runRoute(
      new Request('https://openagents.com/api/v1/chat/completions', {
        method: 'POST',
        body: JSON.stringify({ model: 'x', messages: [] }),
        headers: { 'content-type': 'application/json' },
      }),
    )

    // Dual-served: a 200 from the handler, never a 3xx redirect that an
    // OpenAI client would not re-POST.
    expect(canonical.response.status).toBe(200)
    expect(canonical.observed.exactPath).toBe('/v1/chat/completions')
  })

  test('canonical /api/v1/gateway/glm-fleet/readiness resolves to the readiness handler', async () => {
    const canonical = await runRoute(
      new Request('https://openagents.com/api/v1/gateway/glm-fleet/readiness'),
    )

    expect(canonical.response.status).toBe(200)
    expect(canonical.observed.exactPath).toBe(
      '/v1/gateway/glm-fleet/readiness',
    )
  })

  test('forum topic document routes reach the social preview document handler', async () => {
    const result = await runRoute(
      requestFor('/forum/t/55555555-5555-4555-8555-555555555555', {
        headers: { 'user-agent': 'Discordbot/2.0' },
      }),
    )

    expect(result.response.status).toBe(200)
    await expect(result.response.text()).resolves.toContain(
      'property="og:title" content="First Topic"',
    )
    expect(result.observed.forumTopicId).toBe(
      '55555555-5555-4555-8555-555555555555',
    )
  })

  test('canonical /api/mpp/v1/chat/completions reaches the MPP handler', async () => {
    const canonical = await runRoute(
      new Request('https://openagents.com/api/mpp/v1/chat/completions', {
        method: 'POST',
        body: JSON.stringify({ model: 'x', messages: [] }),
        headers: { 'content-type': 'application/json' },
      }),
    )

    expect(canonical.response.status).toBe(200)
    expect(canonical.observed.exactPath).toBe('/mpp/v1/chat/completions')
  })

  test('canonical /api/v1/models/:model reaches the path-param dispatcher with the normalized url', async () => {
    const legacy = await runRoute(
      new Request('https://openagents.com/v1/models/gpt-x'),
    )
    const canonical = await runRoute(
      new Request('https://openagents.com/api/v1/models/gpt-x'),
    )

    expect(legacy.observed.dispatcherPathname).toBe('/v1/models/gpt-x')
    // The dispatcher re-reads request.url, so dual-serve requires the request
    // object itself to be rewritten to the legacy path — not just the pathname
    // passed to the exact-route matcher.
    expect(canonical.observed.dispatcherPathname).toBe('/v1/models/gpt-x')
  })
})
