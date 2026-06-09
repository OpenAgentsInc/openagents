import { describe, expect, test } from 'vitest'

const readinessCli = await import('./artanis-production-readiness.mjs')

const jsonResponse = (value: unknown) => ({
  json: async () => value,
  ok: true,
  status: 200,
})

const textResponse = (value: string) => ({
  ok: true,
  status: 200,
  text: async () => value,
})

describe('Artanis production readiness CLI helpers', () => {
  test('parses read-only verifier flags and redacts credentials', () => {
    const parsed = readinessCli.parseReadinessArgs([
      '--base-url',
      'https://openagents.com',
      '--d1-tables',
      'artanis_runtime_snapshots,artanis_loop_records',
      '--scheduled-runner',
      'disabled',
      '--source-commit',
      'commit.public.autopilot_omega.abc123',
    ])

    expect(parsed.flags.get('base-url')).toBe('https://openagents.com')
    expect(parsed.flags.get('scheduled-runner')).toBe('disabled')
    expect(readinessCli.redactSecrets(
      'Authorization: Bearer oa_agent_secret and OPENAGENTS_ADMIN_API_TOKEN=secret',
    )).toBe(
      'Authorization: Bearer <redacted> and OPENAGENTS_ADMIN_API_TOKEN=<redacted>',
    )
  })

  test('builds an observation from public reads and caller-supplied evidence', async () => {
    const parsed = readinessCli.parseReadinessArgs([
      '--d1-tables',
      [
        'artanis_approval_gates',
        'artanis_forum_publication_intents',
        'artanis_health_snapshots',
        'artanis_loop_records',
        'artanis_loop_ticks',
        'artanis_nexus_pylon_adapter_dispatches',
        'artanis_runtime_snapshots',
        'artanis_work_routing_proposals',
      ].join(','),
      '--latest-pylon-release-tag',
      'pylon-v0.1.23',
      '--production-smoke-ref',
      'smoke.public.artanis.production_equivalent.1',
      '--pylon-v02-release-assets',
      '2',
      '--pylon-v02-release-tag',
      'pylon-v0.2.0',
      '--scheduled-runner',
      'enabled',
      '--source-commit',
      'commit.public.autopilot_omega.abc123',
    ])
    const fetchFn = async (url: URL) => {
      if (url.pathname === '/api/public/artanis/report') {
        return jsonResponse({
          autonomousLoop: {},
          forumRewardSmoke: {},
          healthSummary: {},
          productionLaunchGate: {},
          pylonLaunchCommunication: {},
        })
      }

      if (url.pathname === '/api/public/pylon-stats') {
        return jsonResponse({ status: 'ready' })
      }

      if (url.pathname.startsWith('/api/forum/topics/')) {
        return jsonResponse({ posts: [{ postId: 'post_1' }] })
      }

      if (url.pathname === '/artanis') {
        return textResponse('Artanis Pylon public surface')
      }

      return { ok: false, status: 404, json: async () => null }
    }

    const observation = await readinessCli.buildObservationFromPublicReads(
      parsed,
      {},
      fetchFn,
    )
    const summary = readinessCli.summarizeObservation(observation)

    expect(observation).toMatchObject({
      artanisPageReachable: true,
      latestPylonReleaseTag: 'pylon-v0.1.23',
      productionSmokeRef: 'smoke.public.artanis.production_equivalent.1',
      pylonStatsStatus: 'fresh',
      pylonV02ReleaseTag: 'pylon-v0.2.0',
      scheduledRunnerEnabled: true,
      sourceCommitRef: 'commit.public.autopilot_omega.abc123',
      statusTopicPostCount: 1,
    })
    expect(summary.state).toBe('ready')
    expect(summary.authority).toMatchObject({
      d1MutationAllowed: false,
      deploymentAllowed: false,
      forumMutationAllowed: false,
      gitHubReleaseMutationAllowed: false,
      pylonDispatchAllowed: false,
      schedulerMutationAllowed: false,
      walletSpendAllowed: false,
    })
  })

  test('summarizes blocked state without mutating anything', () => {
    const summary = readinessCli.summarizeObservation({
      artanisPageReachable: false,
      d1TableNames: null,
      latestPylonReleaseTag: 'pylon-v0.1.23',
      productionSmokeRef: null,
      publicReportFields: ['autonomousLoop'],
      pylonStatsStatus: 'unavailable',
      pylonV02ReleaseAssetCount: 0,
      pylonV02ReleaseTag: null,
      scheduledRunnerEnabled: false,
      sourceCommitRef: null,
      statusTopicPostCount: null,
    })

    expect(summary.state).toBe('blocked')
    expect(summary.blockers).toEqual(
      expect.arrayContaining([
        'missing_report_field:productionLaunchGate',
        'pylon_v0_2_release_not_shipped',
        'production_smoke_missing',
        'scheduler_not_ready',
      ]),
    )
    expect(summary.checks).toEqual(
      expect.arrayContaining([
        { id: 'd1_persistence', status: 'unavailable' },
        { id: 'pylon_stats', status: 'unavailable' },
        { id: 'scheduled_runner_state', status: 'blocked' },
      ]),
    )
  })
})
