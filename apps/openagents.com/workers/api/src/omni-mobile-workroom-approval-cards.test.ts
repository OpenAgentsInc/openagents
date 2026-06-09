import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OMNI_MOBILE_WORKROOM_READ_ONLY_AUTHORITY,
  OmniMobileApprovalCardRecord,
  OmniMobileWorkroomApprovalUnsafe,
  OmniMobileWorkroomCompactRecord,
  OmniMobileWorkroomProjection,
  exampleOmniMobileApprovalCard,
  exampleOmniMobileApprovalCards,
  exampleOmniMobileWorkroom,
  projectOmniMobileWorkroom,
} from './omni-mobile-workroom-approval-cards'

const nowIso = '2026-06-06T22:30:00.000Z'

const workroomRecord = (
  overrides: Partial<OmniMobileWorkroomCompactRecord> = {},
): OmniMobileWorkroomCompactRecord =>
  S.decodeUnknownSync(OmniMobileWorkroomCompactRecord)({
    ...exampleOmniMobileWorkroom(),
    ...overrides,
  })

const cardRecord = (
  overrides: Partial<OmniMobileApprovalCardRecord> = {},
): OmniMobileApprovalCardRecord =>
  S.decodeUnknownSync(OmniMobileApprovalCardRecord)({
    ...exampleOmniMobileApprovalCard(),
    ...overrides,
  })

describe('Omni mobile workroom approval cards', () => {
  test('projects compact mobile state and approval counts without mutation authority', () => {
    const projection = projectOmniMobileWorkroom(
      exampleOmniMobileWorkroom(),
      exampleOmniMobileApprovalCards(),
      'customer',
      nowIso,
    )

    expect(S.decodeUnknownSync(OmniMobileWorkroomProjection)(projection)).toEqual(
      projection,
    )
    expect(projection).toMatchObject({
      approvalMutationAllowed: false,
      artifactCount: 2,
      audience: 'customer',
      blockedApprovalCount: 1,
      criticalApprovalCount: 1,
      evidenceRefCount: 3,
      executionMutationAllowed: false,
      expiredApprovalCount: 1,
      pendingApprovalCount: 1,
      providerMutationAllowed: false,
      publicClaimUpgradeAllowed: false,
      receiptCount: 1,
      runnerLaunchAllowed: false,
      status: 'waiting_review',
      statusLabel: 'Waiting for review',
      updatedAtDisplay: '5 minutes ago',
      workKind: 'site',
    })
    expect(projection.authority).toEqual(
      OMNI_MOBILE_WORKROOM_READ_ONLY_AUTHORITY,
    )
    expect(projection.approvalCards[0]).toMatchObject({
      actionKind: 'crm_send',
      approvalRequired: true,
      expiresAtDisplay: 'Expires in 1 hour',
      expiryState: 'active',
      riskLabel: 'Medium risk',
      stateLabel: 'Pending approval',
      updatedAtDisplay: '5 minutes ago',
    })
    expect(projection.approvalCards[1]).toMatchObject({
      actionKind: 'payment',
      expiresAtDisplay: 'Expired 10 minutes ago',
      expiryState: 'expired',
      riskLabel: 'High risk',
      stateLabel: 'Expired',
    })
    expect(JSON.stringify(projection)).not.toContain('2026-06-06T')
  })

  test('requires risk evidence, explicit approvals, and server authority caveats', () => {
    for (const badCard of [
      cardRecord({
        evidenceRefs: [],
        riskLevel: 'high',
      }),
      cardRecord({
        approvalRequirement: 'not_required',
        riskLevel: 'critical',
      }),
      cardRecord({
        actionKind: 'provider_action',
        approvalRequirement: 'not_required',
        riskLevel: 'medium',
      }),
      cardRecord({
        serverAuthorityCaveatRefs: [],
      }),
    ]) {
      expect(() =>
        projectOmniMobileWorkroom(
          exampleOmniMobileWorkroom(),
          [badCard],
          'operator',
          nowIso,
        ),
      ).toThrow(OmniMobileWorkroomApprovalUnsafe)
    }
  })

  test('requires receipts for approved and executed cards, blockers for blocked cards, and expiry for expired cards', () => {
    for (const badCard of [
      cardRecord({
        approvalReceiptRefs: [],
        state: 'approved',
      }),
      cardRecord({
        approvalReceiptRefs: ['approval.public.operator_approved'],
        executionReceiptRefs: [],
        state: 'executed',
      }),
      cardRecord({
        expiresAtIso: null,
        state: 'expired',
      }),
      cardRecord({
        blockedReasonRefs: [],
        state: 'blocked',
      }),
    ]) {
      expect(() =>
        projectOmniMobileWorkroom(
          exampleOmniMobileWorkroom(),
          [badCard],
          'operator',
          nowIso,
        ),
      ).toThrow(OmniMobileWorkroomApprovalUnsafe)
    }
  })

  test('redacts private provider, wallet, idempotency, approval, and artifact refs from public and agent projections', () => {
    const projection = projectOmniMobileWorkroom(
      workroomRecord({
        artifactRefs: [
          'artifact.public.site_revision',
          'artifact.private.operator_diff_summary',
        ],
        providerStateRefs: [
          'provider.public.model_route_ready',
          'provider.private.operator_route_notes',
        ],
        receiptRefs: [
          'receipt.public.revision_ready_email',
          'receipt.private.operator_receipt',
        ],
        titleRef: 'title.private.operator_workroom',
        walletStateRefs: [
          'wallet.public.no_live_payment',
          'wallet.private.operator_wallet_notes',
        ],
      }),
      [
        cardRecord({
          approvalReceiptRefs: [
            'approval.public.operator_approved',
            'approval.private.operator_notes',
          ],
          artifactRefs: [
            'artifact.public.site_revision',
            'artifact.private.operator_diff_summary',
          ],
          idempotencyKeyRef: 'idempotency.private.operator_key_ref',
          serverAuthorityCaveatRefs: [
            'server_authority.public.operator_gate_required',
            'server_authority.private.operator_policy',
          ],
          titleRef: 'title.private.operator_card',
        }),
      ],
      'public',
      nowIso,
    )

    const serialized = JSON.stringify(projection)

    expect(projection.titleRef).toBe('title.redacted')
    expect(projection.artifactRefs).toEqual(['artifact.public.site_revision'])
    expect(projection.providerStateRefs).toEqual([])
    expect(projection.walletStateRefs).toEqual([])
    expect(projection.approvalCards[0]?.idempotencyKeyRef).toBe(
      'idempotency.redacted',
    )
    expect(projection.approvalCards[0]?.titleRef).toBe('title.redacted')
    expect(projection.approvalCards[0]?.serverAuthorityCaveatRefs).toEqual([
      'server_authority.public.operator_gate_required',
    ])
    expect(serialized).not.toMatch(
      /(approval|artifact|idempotency|provider|receipt|server_authority|title|wallet)\.private/,
    )

    const agentProjection = projectOmniMobileWorkroom(
      exampleOmniMobileWorkroom(),
      [exampleOmniMobileApprovalCard()],
      'agent',
      nowIso,
    )

    expect(JSON.stringify(agentProjection)).not.toMatch(
      /(idempotency|provider|wallet)\.private/,
    )
  })

  test('rejects side-effect authority, private refs, payment material, and raw timestamps', () => {
    for (const badInput of [
      () =>
        projectOmniMobileWorkroom(
          workroomRecord({
            authority: {
              ...OMNI_MOBILE_WORKROOM_READ_ONLY_AUTHORITY,
              noApprovalMutation: false,
            },
          }),
          [],
          'operator',
          nowIso,
        ),
      () =>
        projectOmniMobileWorkroom(
          workroomRecord({
            sourceRefs: ['source.public.2026-06-06T22:25:00.000Z'],
          }),
          [],
          'operator',
          nowIso,
        ),
      () =>
        projectOmniMobileWorkroom(
          exampleOmniMobileWorkroom(),
          [
            cardRecord({
              receiptRefs: ['receipt.public.payment_hash_abcd'],
            }),
          ],
          'operator',
          nowIso,
        ),
      () =>
        projectOmniMobileWorkroom(
          exampleOmniMobileWorkroom(),
          [
            cardRecord({
              sourceRefs: ['github.com/openagents/private-repo'],
            }),
          ],
          'operator',
          nowIso,
        ),
    ]) {
      expect(badInput).toThrow(OmniMobileWorkroomApprovalUnsafe)
    }
  })
})
