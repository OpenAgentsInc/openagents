import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  ARTANIS_LOOP_READ_ONLY_AUTHORITY,
  ArtanisLoopLedgerProjection,
  ArtanisLoopLedgerRecord,
  ArtanisLoopUnsafe,
  artanisLoopProjectionHasPrivateMaterial,
  exampleArtanisLoopLedger,
  projectArtanisLoopLedger,
} from './artanis-loop'

const nowIso = '2026-06-07T01:00:00.000Z'

const loopLedger = (
  overrides: Partial<ArtanisLoopLedgerRecord> = {},
): ArtanisLoopLedgerRecord =>
  S.decodeUnknownSync(ArtanisLoopLedgerRecord)({
    ...exampleArtanisLoopLedger(),
    ...overrides,
  })

describe('Artanis autonomous loop ledger', () => {
  test('projects closeout receipts, artifacts, Forum publication intents, next tick schedule, and no risky authority', () => {
    const projection = projectArtanisLoopLedger(
      exampleArtanisLoopLedger(),
      'public',
      nowIso,
    )

    expect(S.decodeUnknownSync(ArtanisLoopLedgerProjection)(projection))
      .toEqual(projection)
    expect(projection).toMatchObject({
      agentId: 'agent_artanis',
      createdAtDisplay: '10 minutes ago',
      deploymentAllowed: false,
      evalLaunchAllowed: false,
      forumPublishAllowed: false,
      loopCount: 1,
      paymentSpendAllowed: false,
      providerMutationAllowed: false,
      runtimePromotionAllowed: false,
      trainingLaunchAllowed: false,
      updatedAtDisplay: '4 minutes ago',
      walletSpendAllowed: false,
    })
    expect(projection.authority).toEqual(ARTANIS_LOOP_READ_ONLY_AUTHORITY)
    expect(projection.loops[0]).toMatchObject({
      active: true,
      duplicateTickRefs: [],
      scopeRef: 'scope.public.artanis.global',
      state: 'running',
      tickCount: 1,
    })
    expect(projection.loops[0]!.ticks[0]).toMatchObject({
      artifactRefs: ['artifact.public.artanis.status_packet'],
      closeoutReceiptRefs: ['receipt.public.artanis.tick_closeout'],
      forumPublicationIntentRefs: ['forum.public.artanis.status_intent'],
      nextTickDisplay: 'Just now',
      state: 'completed',
    })
    expect(JSON.stringify(projection)).not.toContain('2026-06-07T')
    expect(artanisLoopProjectionHasPrivateMaterial(projection)).toBe(false)
  })

  test('suppresses duplicate ticks by idempotency key while preserving duplicate refs for audit', () => {
    const base = exampleArtanisLoopLedger()
    const loop = base.loops[0]!
    const duplicate = {
      ...loop.ticks[0]!,
      tickRef: 'tick.public.artanis.retry_duplicate',
      updatedAtIso: '2026-06-07T00:57:00.000Z',
    }
    const projection = projectArtanisLoopLedger(
      loopLedger({
        loops: [
          {
            ...loop,
            ticks: [loop.ticks[0]!, duplicate],
          },
        ],
      }),
      'operator',
      nowIso,
    )

    expect(projection.loops[0]!.tickCount).toBe(1)
    expect(projection.loops[0]!.ticks[0]!.tickRef).toBe(
      'tick.public.artanis.20260607T0052',
    )
    expect(projection.loops[0]!.duplicateTickRefs).toEqual([
      'tick.public.artanis.retry_duplicate',
    ])
  })

  test('enforces one active loop per scope and explicit blocked/waiting state evidence', () => {
    const base = exampleArtanisLoopLedger()
    const loop = base.loops[0]!
    const waitingTick = {
      ...loop.ticks[0]!,
      actionProposals: [
        {
          actionRef: 'action.public.artanis.training_launch_proposal',
          approvalRequirementRefs: ['approval.public.training_launch_review'],
          artifactRefs: [],
          authorityReceiptRefs: ['authority.public.operator_training_review'],
          caveatRefs: ['caveat.public.requires_operator_approval'],
          evidenceRefs: ['evidence.public.model_lab_report'],
          kind: 'training_launch' as const,
          risk: 'approval_required' as const,
        },
      ],
      approvalRequirements: [
        {
          actionRef: 'action.public.artanis.training_launch_proposal',
          approvalRef: 'approval.public.training_launch_review',
          authorityRef: 'authority.public.operator_training_review',
          caveatRefs: ['caveat.public.approval_required_before_dispatch'],
          expiresAtIso: '2026-06-07T02:00:00.000Z',
          state: 'pending' as const,
        },
      ],
      closeoutReceiptRefs: [],
      nextTickAtIso: null,
      state: 'waiting_for_approval' as const,
      tickRef: 'tick.public.artanis.waiting_for_training_approval',
    }
    const blockedTick = {
      ...loop.ticks[0]!,
      blockerRefs: ['blocker.public.pylon_readiness_missing'],
      closeoutReceiptRefs: [],
      nextTickAtIso: null,
      state: 'blocked' as const,
      tickRef: 'tick.public.artanis.blocked_pylon_readiness',
    }

    const waitingProjection = projectArtanisLoopLedger(
      loopLedger({
        loops: [
          {
            ...loop,
            state: 'waiting_for_approval',
            ticks: [waitingTick],
          },
        ],
      }),
      'operator',
      nowIso,
    )
    const blockedProjection = projectArtanisLoopLedger(
      loopLedger({
        loops: [
          {
            ...loop,
            blockerRefs: ['blocker.public.pylon_readiness_missing'],
            state: 'blocked',
            ticks: [blockedTick],
          },
        ],
      }),
      'operator',
      nowIso,
    )

    expect(waitingProjection.loops[0]!.state).toBe('waiting_for_approval')
    expect(
      waitingProjection.loops[0]!.ticks[0]!.approvalRequirements,
    ).toHaveLength(1)
    expect(blockedProjection.loops[0]!.state).toBe('blocked')
    expect(blockedProjection.loops[0]!.blockerRefs).toEqual([
      'blocker.public.pylon_readiness_missing',
    ])

    expect(() =>
      projectArtanisLoopLedger(
        loopLedger({
          loops: [
            loop,
            {
              ...loop,
              loopRef: 'loop.public.artanis.second_active',
            },
          ],
        }),
        'operator',
        nowIso,
      ),
    ).toThrow(ArtanisLoopUnsafe)
    expect(() =>
      projectArtanisLoopLedger(
        loopLedger({
          loops: [
            {
              ...loop,
              blockerRefs: [],
              state: 'blocked',
            },
          ],
        }),
        'operator',
        nowIso,
      ),
    ).toThrow(ArtanisLoopUnsafe)
  })

  test('requires completed ticks to carry stable closeout refs and next schedule', () => {
    const base = exampleArtanisLoopLedger()
    const tick = base.loops[0]!.ticks[0]!

    for (const badTick of [
      { ...tick, artifactRefs: [] },
      { ...tick, closeoutReceiptRefs: [] },
      { ...tick, forumPublicationIntentRefs: [] },
      { ...tick, nextTickAtIso: null },
    ]) {
      expect(() =>
        projectArtanisLoopLedger(
          loopLedger({
            loops: [
              {
                ...base.loops[0]!,
                ticks: [badTick],
              },
            ],
          }),
          'operator',
          nowIso,
        ),
      ).toThrow(ArtanisLoopUnsafe)
    }
  })

  test('denies risky actions without authority refs and rejects unsafe material or false authority', () => {
    const base = exampleArtanisLoopLedger()
    const tick = base.loops[0]!.ticks[0]!
    const riskyWithoutAuthority = {
      ...tick,
      actionProposals: [
        {
          actionRef: 'action.public.artanis.wallet_spend',
          approvalRequirementRefs: [],
          artifactRefs: [],
          authorityReceiptRefs: [],
          caveatRefs: ['caveat.public.no_spend_authority'],
          evidenceRefs: ['evidence.public.forum_reward_context'],
          kind: 'wallet_spend' as const,
          risk: 'approval_required' as const,
        },
      ],
    }
    const riskyMarkedSafe = {
      ...tick,
      actionProposals: [
        {
          ...riskyWithoutAuthority.actionProposals[0]!,
          risk: 'safe' as const,
        },
      ],
    }

    for (const badLedger of [
      loopLedger({
        loops: [{ ...base.loops[0]!, ticks: [riskyWithoutAuthority] }],
      }),
      loopLedger({
        loops: [{ ...base.loops[0]!, ticks: [riskyMarkedSafe] }],
      }),
      loopLedger({ caveatRefs: ['provider_payload.raw'] }),
      loopLedger({
        loops: [
          {
            ...base.loops[0]!,
            ticks: [
              {
                ...tick,
                selectedContextRefs: ['raw_log.operator'],
              },
            ],
          },
        ],
      }),
      loopLedger({
        authority: {
          ...ARTANIS_LOOP_READ_ONLY_AUTHORITY,
          noWalletSpend: false,
        },
      }),
      loopLedger({
        authority: {
          ...ARTANIS_LOOP_READ_ONLY_AUTHORITY,
          noTrainingLaunch: false,
        },
      }),
    ]) {
      expect(() =>
        projectArtanisLoopLedger(badLedger, 'operator', nowIso),
      ).toThrow(ArtanisLoopUnsafe)
    }
  })
})
