import { Effect, Schema as S } from 'effect'
import { readFileSync } from 'node:fs'
import { describe, expect, test } from 'vitest'

import { artanisPublicReportSnapshot } from './artanis-public-report'
import { openAgentsCapabilityManifest } from './openagents-capability-manifest'
import { openAgentsOpenApiDocument } from './openagents-openapi'
import {
  PublicLaunchCopyEvidenceGate,
  PublicLaunchCopyGateProjection,
  PublicLaunchCopyGateRefs,
  PublicLaunchCopyGateUnsafe,
  PublicLaunchCopySurface,
  projectPublicLaunchCopyGate,
  publicLaunchCopyProjectionHasPrivateMaterial,
} from './public-launch-copy-gate'
import { publicPylonStatsFromNexusPayload } from './public-pylon-stats'

const blockedGate = (
  gateRef: string,
  blockerRefs: ReadonlyArray<string> = [
    `blocker.public_launch_copy_fixture.${gateRef.replaceAll('.', '_')}`,
  ],
): PublicLaunchCopyEvidenceGate =>
  new PublicLaunchCopyEvidenceGate({
    blockerRefs,
    gateRef,
    state: 'blocked',
    unsafeCopyAllowed: false,
  })

const readyGate = (gateRef: string): PublicLaunchCopyEvidenceGate =>
  new PublicLaunchCopyEvidenceGate({
    blockerRefs: [],
    gateRef,
    state: 'ready',
    unsafeCopyAllowed: true,
  })

const allBlockedGates = (): ReadonlyArray<PublicLaunchCopyEvidenceGate> =>
  Object.values(PublicLaunchCopyGateRefs).map(ref => blockedGate(ref))

const surface = (
  text: string,
  evidenceRefs: ReadonlyArray<string> = [],
): PublicLaunchCopySurface =>
  new PublicLaunchCopySurface({
    evidenceRefs,
    kind: 'launch_announcement',
    surfaceRef: 'surface.public_launch_copy.test',
    text,
  })

const projectOne = (
  text: string,
  input: Readonly<{
    evidenceGates?: ReadonlyArray<PublicLaunchCopyEvidenceGate>
    evidenceRefs?: ReadonlyArray<string>
    healthFresh?: boolean
  }> = {},
): PublicLaunchCopyGateProjection =>
  projectPublicLaunchCopyGate({
    evidenceGates: input.evidenceGates ?? allBlockedGates(),
    healthFresh: input.healthFresh,
    surfaces: [surface(text, input.evidenceRefs)],
  })

describe('public launch copy gate', () => {
  test('blocks launch overclaims unless the matching gate is green and cited', () => {
    const projection = projectOne(
      'One install of Pylon earns bitcoin for anyone tomorrow.',
    )

    expect(
      S.decodeUnknownSync(PublicLaunchCopyGateProjection)(projection),
    ).toEqual(projection)
    expect(projection.state).toBe('blocked')
    expect(projection.violations).toEqual([
      expect.objectContaining({
        claimKind: 'pylon_broad_earning_live',
        requiredGateRefs: [PublicLaunchCopyGateRefs.pylonBroadEarning],
      }),
    ])
    expect(projection.blockerRefs).toContain(
      'blocker.public_launch_copy.pylon_broad_earning_live.gate_not_green',
    )
    expect(projection.blockerRefs).toContain(
      'blocker.public_launch_copy.pylon_broad_earning_live.evidence_ref_missing',
    )
  })

  test('allows unsafe launch wording only with green gate and evidence ref', () => {
    const projection = projectOne(
      `Pylon earning bitcoin is live for the public cohort; see ${PublicLaunchCopyGateRefs.pylonBroadEarning}.`,
      {
        evidenceGates: [
          readyGate(PublicLaunchCopyGateRefs.pylonBroadEarning),
          ...Object.values(PublicLaunchCopyGateRefs)
            .filter(ref => ref !== PublicLaunchCopyGateRefs.pylonBroadEarning)
            .map(ref => blockedGate(ref)),
        ],
        evidenceRefs: [PublicLaunchCopyGateRefs.pylonBroadEarning],
      },
    )

    expect(projection.violations).toEqual([])
    expect(projection.state).toBe('ready')
  })

  test('keeps stale health from making green gates publishable', () => {
    const projection = projectOne(
      `Pylon earning bitcoin is live for the public cohort; see ${PublicLaunchCopyGateRefs.pylonBroadEarning}.`,
      {
        evidenceGates: [readyGate(PublicLaunchCopyGateRefs.pylonBroadEarning)],
        evidenceRefs: [PublicLaunchCopyGateRefs.pylonBroadEarning],
        healthFresh: false,
      },
    )

    expect(projection.state).toBe('blocked')
    expect(projection.blockerRefs).toContain(
      'blocker.public_launch_copy.health_stale',
    )
  })

  test('denylists every launch promise area from the issue', () => {
    const cases: ReadonlyArray<readonly [string, string]> = [
      [
        'full_gepa_network_live',
        'The full GEPA network is live and paid across the fleet.',
      ],
      [
        'qwen_remote_finetune_live',
        'Fine-tune Qwen 3.6 is live on people’s devices now.',
      ],
      [
        'provider_capacity_live',
        'Provider capacity marketplace is live for paid capacity.',
      ],
      [
        'referral_sats_stream_live',
        'Referral sats stream live for every captured site referral.',
      ],
      [
        'hosted_mdk_direct_payouts_live',
        'Hosted MDK direct payouts are enabled for providers.',
      ],
      [
        'creator_spendable_settlement',
        'Creator spendable sats settlement is confirmed for Forum posts.',
      ],
      [
        'artanis_unbounded_autonomy',
        'Artanis is fully autonomous as a production administrator.',
      ],
    ]

    const claimKinds = cases.map(([_, text]) => {
      const projection = projectOne(text)

      expect(projection.state).toBe('blocked')

      return projection.violations[0]?.claimKind
    })

    expect(claimKinds).toEqual(cases.map(([claimKind]) => claimKind))
  })

  test('scans live docs, manifest, OpenAPI, Forum seed copy, and Artanis summaries', async () => {
    const manifest = await Effect.runPromise(openAgentsCapabilityManifest())
    const openApi = await Effect.runPromise(openAgentsOpenApiDocument())
    const artanisReport = artanisPublicReportSnapshot({
      nowIso: '2026-06-08T15:00:00.000Z',
      pylonStats: publicPylonStatsFromNexusPayload({
        pylons_online_now: 0,
        pylons_seen_24h: 0,
      }),
    })
    const forumSeedCopy =
      'Artanis Pylon update: Pylon is the local compute path for inference, optimization, fine-tuning/training, validation, accepted-work contribution, and planned marketplace jobs.'
    const docsScanGates = [
      readyGate(PublicLaunchCopyGateRefs.creatorSpendableSettlement),
      ...Object.values(PublicLaunchCopyGateRefs)
        .filter(
          ref => ref !== PublicLaunchCopyGateRefs.creatorSpendableSettlement,
        )
        .map(ref => blockedGate(ref)),
    ]
    const projection = projectPublicLaunchCopyGate({
      evidenceGates: docsScanGates,
      surfaces: [
        new PublicLaunchCopySurface({
          evidenceRefs: [
            'docs/live/AGENTS.md',
            PublicLaunchCopyGateRefs.creatorSpendableSettlement,
          ],
          kind: 'agents_doc',
          surfaceRef: 'surface.public.docs_live_agents',
          text: readFileSync('../../docs/live/AGENTS.md', 'utf8'),
        }),
        new PublicLaunchCopySurface({
          evidenceRefs: ['docs/live/HEARTBEAT.md'],
          kind: 'agents_doc',
          surfaceRef: 'surface.public.docs_live_heartbeat',
          text: readFileSync('../../docs/live/HEARTBEAT.md', 'utf8'),
        }),
        new PublicLaunchCopySurface({
          evidenceRefs: ['docs/live/RULES.md'],
          kind: 'agents_doc',
          surfaceRef: 'surface.public.docs_live_rules',
          text: readFileSync('../../docs/live/RULES.md', 'utf8'),
        }),
        new PublicLaunchCopySurface({
          evidenceRefs: ['docs/live/skill.json'],
          kind: 'manifest',
          surfaceRef: 'surface.public.docs_live_skill_json',
          text: readFileSync('../../docs/live/skill.json', 'utf8'),
        }),
        new PublicLaunchCopySurface({
          evidenceRefs: ['route:/.well-known/openagents.json'],
          kind: 'manifest',
          surfaceRef: 'surface.public.capability_manifest',
          text: JSON.stringify(manifest),
        }),
        new PublicLaunchCopySurface({
          evidenceRefs: [
            'route:/api/openapi.json',
            PublicLaunchCopyGateRefs.creatorSpendableSettlement,
          ],
          kind: 'openapi',
          surfaceRef: 'surface.public.openapi',
          text: JSON.stringify(openApi),
        }),
        new PublicLaunchCopySurface({
          evidenceRefs: ['seed.public.forum.artanis_pylon_update'],
          kind: 'forum_seed',
          surfaceRef: 'surface.public.forum_seed.artanis_pylon_update',
          text: forumSeedCopy,
        }),
        new PublicLaunchCopySurface({
          evidenceRefs: ['route:/api/public/artanis/report'],
          kind: 'artanis_report',
          surfaceRef: 'surface.public.artanis_report',
          text: JSON.stringify({
            authoritySummary: artanisReport.authoritySummary,
            claimStateCaveats: artanisReport.claimStateCaveats,
            pylonLaunchCommunication: artanisReport.pylonLaunchCommunication,
            pylonSummary: artanisReport.pylonSummary,
            productionLaunchGate: artanisReport.productionLaunchGate,
          }),
        }),
      ],
    })

    expect(projection.violations).toEqual([])
    expect(projection.state).toBe('ready')
    expect(publicLaunchCopyProjectionHasPrivateMaterial(projection)).toBe(false)
  })

  test('rejects unsafe private refs in copy evidence metadata', () => {
    expect(() =>
      projectPublicLaunchCopyGate({
        evidenceGates: allBlockedGates(),
        surfaces: [
          surface('Safe text only.', ['payment_preimage.private.raw']),
        ],
      }),
    ).toThrow(PublicLaunchCopyGateUnsafe)
  })
})
