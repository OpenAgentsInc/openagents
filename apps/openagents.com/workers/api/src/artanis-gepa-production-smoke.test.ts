import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  ArtanisGepaProductionSmokeProjection,
  ArtanisGepaProductionSmokeUnsafe,
  artanisProductionLaunchGateCheckInputFromGepaSmoke,
  exampleArtanisGepaProductionSmokeRecord,
  projectArtanisGepaProductionSmoke,
} from './artanis-gepa-production-smoke'
import {
  exampleArtanisProductionLaunchGateRecord,
  projectArtanisProductionLaunchGate,
} from './artanis-production-launch-gate'

const nowIso = '2026-06-08T05:50:00.000Z'

describe('Artanis Probe GEPA production-equivalent smoke', () => {
  test('projects retained SHC Harbor and Pylon closeout evidence without payout or mutation authority', () => {
    const projection = projectArtanisGepaProductionSmoke(
      exampleArtanisGepaProductionSmokeRecord(),
      nowIso,
    )

    expect(
      S.decodeUnknownSync(ArtanisGepaProductionSmokeProjection)(projection),
    ).toEqual(projection)
    expect(projection).toMatchObject({
      acceptedCloseoutCount: 1,
      campaignRef:
        'campaign.probe_gepa.stage0.live_shc_harbor_smoke.2026_06_08',
      completedMetricCalls: 2,
      mutationAuthorityAllowed: false,
      payoutClaimAllowed: false,
      rejectedCloseoutCount: 1,
      state: 'retained',
      stateLabel: 'Retained Probe GEPA Pylon production-equivalent smoke',
    })
    expect(projection.pylonRefs).toEqual([
      'pylon.demo.stage0.one',
      'pylon.demo.stage0.two',
    ])
    expect(projection.pylonAssignmentRefs).toEqual([
      'assignment.public.pylon_gepa.live_stage0.demo_1',
      'assignment.public.pylon_gepa.live_stage0.demo_2',
    ])
    expect(JSON.stringify(projection)).not.toMatch(
      /raw_|provider|wallet|payment_hash|lnbc|\/Users\/|2026-06-08T/,
    )
  })

  test('requires accepted and rejected closeout evidence with artifacts proofs and resource refs', () => {
    const base = exampleArtanisGepaProductionSmokeRecord()

    expect(() =>
      projectArtanisGepaProductionSmoke(
        {
          ...base,
          pylonCloseouts: base.pylonCloseouts.map(closeout =>
            closeout.state === 'accepted_work'
              ? {
                  ...closeout,
                  proofBundleRefs: [],
                }
              : closeout,
          ),
        },
        nowIso,
      ),
    ).toThrow(ArtanisGepaProductionSmokeUnsafe)

    expect(() =>
      projectArtanisGepaProductionSmoke(
        {
          ...base,
          rejectedCloseoutCount: 0,
        },
        nowIso,
      ),
    ).toThrow(ArtanisGepaProductionSmokeUnsafe)
  })

  test('rejects mutable authority and unsafe refs', () => {
    const base = exampleArtanisGepaProductionSmokeRecord()

    expect(() =>
      projectArtanisGepaProductionSmoke(
        {
          ...base,
          authority: {
            ...base.authority,
            noWalletSpend: false,
          },
        },
        nowIso,
      ),
    ).toThrow(ArtanisGepaProductionSmokeUnsafe)

    expect(() =>
      projectArtanisGepaProductionSmoke(
        {
          ...base,
          probeCloseoutRefs: ['raw_run_log.probe_gepa.hidden'],
        },
        nowIso,
      ),
    ).toThrow(ArtanisGepaProductionSmokeUnsafe)
  })

  test('feeds the production launch gate while scheduled-runner proof owns the separate autonomy gate', () => {
    const smokeCheckInput = artanisProductionLaunchGateCheckInputFromGepaSmoke(
      exampleArtanisGepaProductionSmokeRecord(),
      nowIso,
    )
    const gate = exampleArtanisProductionLaunchGateRecord(nowIso)
    const projection = projectArtanisProductionLaunchGate(
      {
        ...gate,
        checks: gate.checks.map(check =>
          check.category === 'production_e2e_smoke'
            ? {
                ...check,
                ...smokeCheckInput,
              }
            : check,
        ),
      },
      nowIso,
    )

    expect(smokeCheckInput).toMatchObject({
      category: 'production_e2e_smoke',
      status: 'passed',
    })
    expect(projection.blockerRefs).not.toContain(
      'blocker.public.artanis.launch_gate.production_e2e_smoke.blocked',
    )
    expect(projection.blockerRefs).not.toContain(
      'blocker.public.artanis.launch_gate.scheduled_runner.blocked',
    )
    expect(projection.canClaimBoundedStatusProjection).toBe(true)
    expect(projection.canClaimContinuouslyRunning).toBe(false)
  })
})
