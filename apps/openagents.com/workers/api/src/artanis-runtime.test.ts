import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  ARTANIS_RUNTIME_READ_ONLY_AUTHORITY,
  ArtanisRuntimeProjection,
  ArtanisRuntimeRecord,
  ArtanisRuntimeUnsafe,
  artanisRuntimeProjectionHasPrivateMaterial,
  exampleArtanisRuntime,
  projectArtanisRuntime,
} from './artanis-runtime'

const nowIso = '2026-06-07T00:45:00.000Z'

const runtimeRecord = (
  overrides: Partial<ArtanisRuntimeRecord> = {},
): ArtanisRuntimeRecord =>
  S.decodeUnknownSync(ArtanisRuntimeRecord)({
    ...exampleArtanisRuntime(),
    ...overrides,
  })

describe('Artanis standalone runtime contract', () => {
  test('projects agent_artanis as standalone, operator-steerable, autonomous, and non-authoritative publicly', () => {
    const projection = projectArtanisRuntime(
      exampleArtanisRuntime(),
      'public',
      nowIso,
    )

    expect(S.decodeUnknownSync(ArtanisRuntimeProjection)(projection))
      .toEqual(projection)
    expect(projection).toMatchObject({
      adapterInstallAllowed: false,
      agentId: 'agent_artanis',
      agentRef: 'artanis',
      audience: 'public',
      createdAtDisplay: '15 minutes ago',
      deploymentAllowed: false,
      differsFromAdjutant: true,
      differsFromGenericPublicAgent: true,
      displayName: 'Artanis',
      mode: 'standalone_autonomous',
      operatorSteerable: true,
      paymentSpendAllowed: false,
      providerMutationAllowed: false,
      publicClaimUpgradeAllowed: false,
      runtimePromotionAllowed: false,
      settlementMutationAllowed: false,
      standalone: true,
      state: 'running',
      trainingLaunchAllowed: false,
      updatedAtDisplay: '9 minutes ago',
      walletSpendAllowed: false,
    })
    expect(projection.authority).toEqual(ARTANIS_RUNTIME_READ_ONLY_AUTHORITY)
    expect(projection.goalRefs).toEqual(['goal.public.artanis.pylon_model_lab'])
    expect(projection.forumRefs).toEqual(['forum.public.artanis.status'])
    expect(projection.modelLabRefs).toEqual([
      'model_lab.public.autopilot_continual_learning',
    ])
    expect(projection.pylonRefs).toEqual(['pylon.public.v0_2_readiness'])
    expect(projection.nexusRefs).toEqual(['nexus.public.pylon_work_market'])
    expect(projection.campaignRefs).toEqual([
      'campaign.public.pylon_r10_episode_232',
    ])
    expect(projection.privateEvidenceRefs).toEqual([])
    expect(JSON.stringify(projection)).not.toContain('2026-06-07T')
    expect(artanisRuntimeProjectionHasPrivateMaterial(projection)).toBe(false)
  })

  test('keeps private evidence visible only to team and operator projections', () => {
    const publicProjection = projectArtanisRuntime(
      exampleArtanisRuntime(),
      'public',
      nowIso,
    )
    const agentProjection = projectArtanisRuntime(
      exampleArtanisRuntime(),
      'agent',
      nowIso,
    )
    const teamProjection = projectArtanisRuntime(
      exampleArtanisRuntime(),
      'team',
      nowIso,
    )
    const operatorProjection = projectArtanisRuntime(
      exampleArtanisRuntime(),
      'operator',
      nowIso,
    )

    expect(publicProjection.privateEvidenceRefs).toEqual([])
    expect(agentProjection.privateEvidenceRefs).toEqual([])
    expect(teamProjection.privateEvidenceRefs).toEqual([
      'evidence.private.artanis.operator_loop_packet',
    ])
    expect(operatorProjection.privateEvidenceRefs).toEqual([
      'evidence.private.artanis.operator_loop_packet',
    ])
  })

  test('rejects generic public agents, Adjutant identity, missing standalone refs, dirty URLs, blocked state without blockers, and approval-waiting state without steering refs', () => {
    for (const badRecord of [
      runtimeRecord({ agentId: 'agent_generic' }),
      runtimeRecord({ agentRef: 'adjutant', displayName: 'Adjutant' }),
      runtimeRecord({ goalRefs: [] }),
      runtimeRecord({ workLoopRefs: [] }),
      runtimeRecord({ privateEvidenceRefs: [] }),
      runtimeRecord({ publicProjectionRefs: [] }),
      runtimeRecord({ forumRefs: [] }),
      runtimeRecord({ modelLabRefs: [] }),
      runtimeRecord({ pylonRefs: [] }),
      runtimeRecord({ nexusRefs: [] }),
      runtimeRecord({ campaignRefs: [] }),
      runtimeRecord({ adjutantBoundaryRefs: [] }),
      runtimeRecord({ genericAgentBoundaryRefs: [] }),
      runtimeRecord({
        publicUrls: ['https://openagents.com/artanis?token=secret'],
      }),
      runtimeRecord({ blockerRefs: [], state: 'blocked' }),
      runtimeRecord({
        operatorSteeringRefs: [],
        state: 'waiting_for_approval',
      }),
    ]) {
      expect(() =>
        projectArtanisRuntime(badRecord, 'operator', nowIso),
      ).toThrow(ArtanisRuntimeUnsafe)
    }
  })

  test('redacts private public-facing refs while preserving clean public URLs', () => {
    const projection = projectArtanisRuntime(
      runtimeRecord({
        campaignRefs: ['campaign.private.operator_campaign'],
        forumRefs: ['forum.private.operator_status'],
        goalRefs: ['goal.private.operator_goal'],
        modelLabRefs: ['model_lab.private.operator_loop'],
        nexusRefs: ['nexus.private.operator_market'],
        publicProjectionRefs: ['projection.private.operator_status'],
        pylonRefs: ['pylon.private.operator_node'],
        runtimeRef: 'runtime.private.operator_artanis',
        workLoopRefs: ['loop.private.operator_tick'],
      }),
      'public',
      nowIso,
    )

    const serialized = JSON.stringify(projection)

    expect(projection.runtimeRef).toBe('runtime.redacted.artanis')
    expect(projection.goalRefs).toEqual([])
    expect(projection.publicUrls).toEqual([
      'https://openagents.com/agents/artanis',
      'https://openagents.com/artanis',
    ])
    expect(serialized).not.toContain('.private')
    expect(serialized).not.toContain('operator_campaign')
    expect(serialized).not.toContain('operator_status')
    expect(serialized).not.toContain('operator_goal')
    expect(artanisRuntimeProjectionHasPrivateMaterial(projection)).toBe(false)
  })

  test('rejects provider, runner, wallet, payment, customer, private repo, secret, raw prompt, raw log, raw timestamp, and false authority material', () => {
    for (const badRecord of [
      runtimeRecord({ caveatRefs: ['provider_payload.raw'] }),
      runtimeRecord({ blockerRefs: ['runner_payload.raw'] }),
      runtimeRecord({ campaignRefs: ['wallet.spend.secret'] }),
      runtimeRecord({ pylonRefs: ['payment_preimage.raw'] }),
      runtimeRecord({ goalRefs: ['customer_email.ben@example.com'] }),
      runtimeRecord({ modelLabRefs: ['github.com/org/private'] }),
      runtimeRecord({ privateEvidenceRefs: ['secret.artanis_token'] }),
      runtimeRecord({ privateEvidenceRefs: ['raw_prompt.operator'] }),
      runtimeRecord({ privateEvidenceRefs: ['raw_log.operator'] }),
      runtimeRecord({ caveatRefs: ['caveat.public.2026-06-07T00:00:00'] }),
      runtimeRecord({
        authority: {
          ...ARTANIS_RUNTIME_READ_ONLY_AUTHORITY,
          noWalletSpend: false,
        },
      }),
      runtimeRecord({
        authority: {
          ...ARTANIS_RUNTIME_READ_ONLY_AUTHORITY,
          noTrainingLaunch: false,
        },
      }),
      runtimeRecord({
        authority: {
          ...ARTANIS_RUNTIME_READ_ONLY_AUTHORITY,
          noRuntimePromotion: false,
        },
      }),
    ]) {
      expect(() =>
        projectArtanisRuntime(badRecord, 'operator', nowIso),
      ).toThrow(ArtanisRuntimeUnsafe)
    }
  })
})
