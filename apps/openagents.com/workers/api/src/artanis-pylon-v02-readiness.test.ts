import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  ArtanisPylonV02ReadinessProjection,
  ArtanisPylonV02ReadinessUnsafe,
  exampleArtanisPylonV02Readiness,
  projectArtanisPylonV02Readiness,
} from './artanis-pylon-v02-readiness'

const nowIso = '2026-06-06T23:50:00.000Z'

describe('Artanis Pylon v0.2 readiness', () => {
  test('projects source, release, platform, eligible, accepted, paid, and settled states', () => {
    const projection = projectArtanisPylonV02Readiness(
      exampleArtanisPylonV02Readiness(),
      'public',
      nowIso,
    )

    expect(S.decodeUnknownSync(ArtanisPylonV02ReadinessProjection)(projection))
      .toEqual(projection)
    expect(projection.agentRef).toBe('agent_artanis')
    expect(projection.readinessRef).toBe('readiness.public.artanis.pylon_v0_2')
    expect(projection.updatedAtDisplay).toBe('5 minutes ago')
    expect(projection.gates.map(gate => [gate.stage, gate.state.state]))
      .toEqual([
        ['source_ready', 'verified'],
        ['release_ready', 'blocked'],
        ['platform_ready', 'blocked'],
        ['eligible', 'planned'],
        ['accepted', 'prohibited'],
        ['paid', 'prohibited'],
        ['settled', 'prohibited'],
      ])
    expect(projection.stateCounts).toEqual([
      { count: 2, state: 'blocked' },
      { count: 1, state: 'planned' },
      { count: 3, state: 'prohibited' },
      { count: 1, state: 'verified' },
    ])
  })

  test('keeps v0.2 source readiness separate from release and platform readiness', () => {
    const projection = projectArtanisPylonV02Readiness(
      exampleArtanisPylonV02Readiness(),
      'public',
      nowIso,
    )
    const sourceReady = projection.gates.find(
      gate => gate.stage === 'source_ready',
    )
    const releaseReady = projection.gates.find(
      gate => gate.stage === 'release_ready',
    )
    const platformReady = projection.gates.find(
      gate => gate.stage === 'platform_ready',
    )

    expect(sourceReady?.state.state).toBe('verified')
    expect(sourceReady?.caveatRefs).toContain(
      'caveat.public.source_ready_not_release_ready',
    )
    expect(releaseReady?.state.state).toBe('blocked')
    expect(releaseReady?.blockerRefs).toContain(
      'blocker.public.no_pylon_v0_2_release_asset',
    )
    expect(platformReady?.releaseAssetEvidenceRefs).toContain(
      'asset.public.current.darwin_arm64_only',
    )
  })

  test('includes setup refs, readiness commands, resource-mode caveats, and safe Forum copy', () => {
    const projection = projectArtanisPylonV02Readiness(
      exampleArtanisPylonV02Readiness(),
      'public',
      nowIso,
    )
    const template = projection.forumTemplate

    expect(template.setupRefs).toEqual(
      expect.arrayContaining([
        'docs/sites/2026-06-05-pylon-local-compute-instruction-packet.md',
        'docs/sites/2026-06-05-pylon-v0-2-public-readiness-audit.md',
      ]),
    )
    expect(template.readinessCommandRefs).toEqual(
      expect.arrayContaining([
        'command.public.pylon.version',
        'command.public.pylon.status_json',
        'command.public.pylon.training_status_json',
        'command.public.pylon.balance_json',
        'command.public.pylon.history_json',
      ]),
    )
    expect(template.resourceModeCaveatRefs).toEqual(
      expect.arrayContaining([
        'caveat.public.resource_mode_background_may_not_be_enough',
        'caveat.public.resource_mode_overnight_owner_selected',
        'caveat.public.resource_mode_dedicated_requires_operator_intent',
      ]),
    )
    expect(template.bodyText).toContain('source-level LDK target readiness')
    expect(template.bodyText).toContain(
      'online, eligible, assigned, accepted, paid, and settled as separate states',
    )
    expect(JSON.stringify(projection)).not.toContain('wallet')
    expect(JSON.stringify(projection)).not.toContain('token')
    expect(JSON.stringify(projection)).not.toContain('invoice')
  })

  test('records platform caveats for macOS, Linux, WSL, and native Windows', () => {
    const projection = projectArtanisPylonV02Readiness(
      exampleArtanisPylonV02Readiness(),
      'public',
      nowIso,
    )

    expect(projection.platformGuidance.map(guidance => [
      guidance.platform,
      guidance.state,
    ])).toEqual([
      ['macos_apple_silicon', 'verified'],
      ['linux', 'blocked'],
      ['wsl_ubuntu', 'blocked'],
      ['native_windows', 'blocked'],
    ])
  })

  test('rejects broad public-ready and unconditional earnings copy', () => {
    const input = exampleArtanisPylonV02Readiness()

    expect(() =>
      projectArtanisPylonV02Readiness({
        ...input,
        forumTemplate: {
          ...input.forumTemplate,
          bodyText: 'Pylon v0.2 is ready for everyone. Run Pylon and earn money.',
        },
      }, 'public', nowIso),
    ).toThrow(ArtanisPylonV02ReadinessUnsafe)
  })

  test('rejects paid or settled readiness without public receipt chains', () => {
    const input = exampleArtanisPylonV02Readiness()

    expect(() =>
      projectArtanisPylonV02Readiness({
        ...input,
        gates: input.gates.map(gate =>
          gate.stage === 'paid'
            ? {
                ...gate,
                desiredState: 'verified',
                evidenceRefs: ['receipt.public.pylon.paid_work'],
              }
            : gate
        ),
      }, 'public', nowIso),
    ).toThrow(ArtanisPylonV02ReadinessUnsafe)
  })

  test('rejects missing readiness stages and missing platform guidance', () => {
    const input = exampleArtanisPylonV02Readiness()

    expect(() =>
      projectArtanisPylonV02Readiness({
        ...input,
        gates: input.gates.filter(gate => gate.stage !== 'settled'),
      }, 'public', nowIso),
    ).toThrow(ArtanisPylonV02ReadinessUnsafe)

    expect(() =>
      projectArtanisPylonV02Readiness({
        ...input,
        platformGuidance: input.platformGuidance.filter(
          guidance => guidance.platform !== 'wsl_ubuntu',
        ),
      }, 'public', nowIso),
    ).toThrow(ArtanisPylonV02ReadinessUnsafe)
  })
})
