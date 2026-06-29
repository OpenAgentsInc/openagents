import { Effect, Schema as S } from 'effect'
import { readFileSync } from 'node:fs'
import { describe, expect, test } from 'vitest'

import { openAgentsCapabilityManifest } from './openagents-capability-manifest'
import { openAgentsOpenApiDocument } from './openagents-openapi'
import {
  PublicLaunchDashboardEndpoint,
  PublicLaunchDashboardProjection,
  projectPublicLaunchDashboard,
  publicLaunchDashboardPromiseIds,
} from './public-launch-dashboard'
import { handlePublicLaunchDashboardApi } from './public-launch-dashboard-routes'
import { publicPylonStatsFromNexusPayload } from './public-pylon-stats'

const nowUnixMs = Date.parse('2026-06-08T20:00:00.000Z')
const nowIso = '2026-06-08T20:00:00.000Z'

const freshPylonStats = () =>
  publicPylonStatsFromNexusPayload({
    as_of_unix_ms: nowUnixMs,
    nexus_accepted_work_payout_receipt_refs: [
      'receipt.nexus.public.launch_dashboard',
    ],
    nexus_accepted_work_payout_sats_paid_24h: 1,
    nexus_accepted_work_payout_sats_paid_total: 1,
    pylon_sessions_online_now: 2,
    pylons_online_now: 2,
    pylons_seen_24h: 2,
    sellable_pylons_online_now: 2,
  })

const stalePylonStats = () =>
  publicPylonStatsFromNexusPayload({
    as_of_unix_ms: Date.parse('2026-06-08T19:00:00.000Z'),
    pylon_sessions_online_now: 2,
    pylons_online_now: 2,
    pylons_seen_24h: 2,
    sellable_pylons_online_now: 2,
  })

const storeForRoute = {
  listRegistrations: () => Promise.resolve([]),
}

const transcriptPromiseCount = (): number => {
  const audit = readFileSync(
    '../../docs/2026-06-08-pylon-agentic-revenue-gap-audit.md',
    'utf8',
  )
  const sourceInventory = audit.slice(
    audit.indexOf('## Source Promise Inventory'),
    audit.indexOf('## Live Evidence Snapshot'),
  )
  const inventory = sourceInventory.match(/^([1-9]|1[0-8])\. /gm) ?? []

  return inventory.length
}

describe('public launch dashboard', () => {
  test('projects every transcript promise as red, yellow, or green with evidence and blockers', () => {
    const dashboard = projectPublicLaunchDashboard({
      generatedAt: nowIso,
      nowUnixMs,
      pylonStats: freshPylonStats(),
    })

    expect(
      S.decodeUnknownSync(PublicLaunchDashboardProjection)(dashboard),
    ).toEqual(dashboard)
    expect(dashboard.schemaVersion).toBe(
      'openagents.public_launch_dashboard.v1',
    )
    expect(dashboard.rows).toHaveLength(transcriptPromiseCount())
    expect(publicLaunchDashboardPromiseIds).toHaveLength(
      transcriptPromiseCount(),
    )
    expect(new Set(dashboard.rows.map(row => row.promiseId)).size).toBe(
      transcriptPromiseCount(),
    )
    expect(dashboard.rows.map(row => [row.promiseId, row.status])).toEqual([
      ['pylon_release_tomorrow', 'yellow'],
      ['first_real_model_training_run', 'yellow'],
      ['five_bitcoin_revenue_streams', 'red'],
      ['compute_revenue_modes', 'red'],
      ['data_trace_revenue', 'red'],
      ['forum_content_tipping', 'yellow'],
      ['no_wallet_knowledge_bitcoin', 'yellow'],
      ['site_referral_bitcoin_stream', 'yellow'],
      ['money_dev_kit_payments', 'yellow'],
      ['one_agent_instruction_sheet', 'green'],
      ['api_hosted_gemini', 'yellow'],
      ['agentic_labor_products', 'yellow'],
      ['pylon_cli_tui_probe_background', 'yellow'],
      ['control_center_fanout_plugin_marketplace', 'yellow'],
      ['dspy_gepa_signature_monetization', 'red'],
      ['chatgpt_claude_codex_capacity', 'red'],
      ['cursor_agent_forum_wallet', 'yellow'],
      ['prepaid_provider_capacity_monetization', 'red'],
    ])
    expect(dashboard.redCount).toBe(6)
    expect(dashboard.yellowCount).toBe(11)
    expect(dashboard.greenCount).toBe(1)
    expect(dashboard.status).toBe('red')
    expect(dashboard.rows.every(row => row.evidenceRefs.length > 0)).toBe(true)
    expect(
      dashboard.rows
        .filter(row => row.status !== 'green')
        .every(row => row.blockerRefs.length > 0),
    ).toBe(true)
    const cursorClaimRow = dashboard.rows.find(
      row => row.promiseId === 'cursor_agent_forum_wallet',
    )

    expect(cursorClaimRow?.safeCopy).toContain(
      'complete X verification for public Forum speech',
    )
    expect(cursorClaimRow?.safeCopy).toContain(
      '1000 sats claim reward still require explicit MDK setup',
    )
    expect(cursorClaimRow?.blockerRefs).toEqual(
      expect.arrayContaining([
        'blocker.launch_dashboard.claimed_public_identity.x_reward_payout_readiness',
        'blocker.launch_dashboard.claimed_public_identity.reward_policy_terms_required',
        'blocker.launch_dashboard.claimed_public_identity.reward_abuse_review_required',
        'blocker.launch_dashboard.claimed_public_identity.reward_compliance_review_required',
        'blocker.launch_dashboard.claimed_public_identity.nostr_adapter_planned',
      ]),
    )
  })

  test('stale endpoint data keeps stale-sensitive promises red or yellow', () => {
    const dashboard = projectPublicLaunchDashboard({
      generatedAt: nowIso,
      nowUnixMs,
      pylonStats: stalePylonStats(),
    })

    expect(dashboard.staleEndpointRefs).toEqual([
      'endpoint:/api/public/pylon-stats',
    ])
    expect(
      dashboard.rows
        .filter(row =>
          row.blockerRefs.includes('blocker.launch_dashboard.endpoint_stale'),
        )
        .map(row => row.status),
    ).not.toContain('green')
  })

  test('serves no-store public JSON from the route', async () => {
    const response = await Effect.runPromise(
      handlePublicLaunchDashboardApi(
        new Request(`https://openagents.com${PublicLaunchDashboardEndpoint}`),
        {
          nowUnixMs: () => nowUnixMs,
          store: storeForRoute,
        },
      ),
    )
    const body = S.decodeUnknownSync(PublicLaunchDashboardProjection)(
      await response.json(),
    )

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(body.rows).toHaveLength(18)
  })

  test('rejects unsupported methods', async () => {
    const response = await Effect.runPromise(
      handlePublicLaunchDashboardApi(
        new Request(`https://openagents.com${PublicLaunchDashboardEndpoint}`, {
          method: 'POST',
        }),
        { nowUnixMs: () => nowUnixMs, store: storeForRoute },
      ),
    )

    expect(response.status).toBe(405)
    expect(response.headers.get('allow')).toBe('GET')
  })

  test('is discoverable from the public manifest and OpenAPI', async () => {
    const [manifest, openApi] = await Promise.all([
      Effect.runPromise(openAgentsCapabilityManifest()),
      Effect.runPromise(openAgentsOpenApiDocument()),
    ])

    expect(manifest.resources).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          auth: 'public',
          href: `https://openagents.com${PublicLaunchDashboardEndpoint}`,
          id: 'public_launch_dashboard',
          method: 'GET',
        }),
      ]),
    )
    expect(openApi.paths[PublicLaunchDashboardEndpoint]).toMatchObject({
      get: expect.objectContaining({
        operationId: 'getPublicLaunchDashboard',
      }),
    })
  })
})
