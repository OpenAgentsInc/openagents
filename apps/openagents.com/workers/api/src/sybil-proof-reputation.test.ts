import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  SYBIL_PROOF_REPUTATION_READ_ONLY_AUTHORITY,
  SybilProofReputationPaymentGraphEdge,
  SybilProofReputationProjection,
  SybilProofReputationUnsafe,
  projectSybilProofReputation,
} from './sybil-proof-reputation'

const generatedAtIso = '2026-06-27T18:30:00.000Z'

const edge = (
  overrides: Partial<SybilProofReputationPaymentGraphEdge> = {},
): SybilProofReputationPaymentGraphEdge =>
  S.decodeUnknownSync(SybilProofReputationPaymentGraphEdge)({
    amountMsat: 250_000,
    createdAtIso: '2026-06-27T18:00:00.000Z',
    edgeRef: 'edge.public.reputation.accepted_1',
    outcome: 'accepted',
    payerRef: 'buyer.public.team_1',
    settlementRefs: ['settlement.public.btc.accepted_1'],
    subjectRef: 'pylon.public.alpha',
    verificationRefs: ['verification.public.replay.accepted_1'],
    workRef: 'work.public.accepted_1',
    ...overrides,
  })

describe('sybil-proof reputation projection', () => {
  test('scores accepted replay-verified settled work without granting authority', () => {
    const projection = projectSybilProofReputation({
      edges: [
        edge(),
        edge({
          edgeRef: 'edge.public.reputation.accepted_2',
          amountMsat: 750_000,
          settlementRefs: ['settlement.public.btc.accepted_2'],
          verificationRefs: ['verification.public.replay.accepted_2'],
          workRef: 'work.public.accepted_2',
        }),
        edge({
          edgeRef: 'edge.public.reputation.rejected_1',
          amountMsat: 100_000,
          outcome: 'rejected',
          settlementRefs: ['settlement.public.btc.rejected_1'],
          subjectRef: 'pylon.public.beta',
          verificationRefs: ['verification.public.replay.rejected_1'],
          workRef: 'work.public.rejected_1',
        }),
      ],
      generatedAtIso,
      projectionRef: 'projection.public.reputation.beta',
    })

    expect(S.decodeUnknownSync(SybilProofReputationProjection)(projection))
      .toEqual(projection)
    expect(projection.authority).toEqual(
      SYBIL_PROOF_REPUTATION_READ_ONLY_AUTHORITY,
    )
    expect(projection.authority.noDispatchAuthority).toBe(true)
    expect(projection.authority.noPayoutAuthority).toBe(true)
    expect(projection.authority.noPublicClaimUpgrade).toBe(true)
    expect(projection.authority.noSettlementAuthority).toBe(true)
    expect(projection.subjects[0]).toMatchObject({
      acceptedSettledMsat: 1_000_000,
      acceptedVerifiedOutcomeCount: 2,
      manualOverrideApplied: false,
      rejectedVerifiedOutcomeCount: 0,
      score: 900,
      subjectRef: 'pylon.public.alpha',
    })
    expect(projection.subjects[1]).toMatchObject({
      acceptedSettledMsat: 0,
      acceptedVerifiedOutcomeCount: 0,
      rejectedVerifiedOutcomeCount: 1,
      score: 0,
      subjectRef: 'pylon.public.beta',
    })
    expect(JSON.stringify(projection)).not.toContain('lnbc')
  })

  test('keeps cost-to-fake tied to actually settled verified outcomes', () => {
    const projection = projectSybilProofReputation({
      edges: [
        edge({
          amountMsat: 1_000_000,
          edgeRef: 'edge.public.reputation.real_paid_work',
          subjectRef: 'pylon.public.real_provider',
        }),
        edge({
          amountMsat: 9_000_000,
          edgeRef: 'edge.public.reputation.sybil_unverified',
          settlementRefs: [],
          subjectRef: 'pylon.public.sybil_1',
          verificationRefs: ['verification.public.replay.sybil_1'],
        }),
        edge({
          amountMsat: 9_000_000,
          edgeRef: 'edge.public.reputation.sybil_unsettled',
          settlementRefs: ['settlement.public.btc.sybil_2'],
          subjectRef: 'pylon.public.sybil_2',
          verificationRefs: [],
        }),
      ],
      generatedAtIso,
      projectionRef: 'projection.public.reputation.beta',
    })

    expect(projection.subjects.map(subject => subject.subjectRef)).toEqual([
      'pylon.public.real_provider',
      'pylon.public.sybil_1',
      'pylon.public.sybil_2',
    ])
    expect(projection.subjects[0]?.score).toBe(900)
    expect(projection.subjects[1]?.score).toBe(0)
    expect(projection.subjects[1]?.blockerRefs).toEqual([
      'blocker.public.reputation.unsettled_or_unverified:edge.public.reputation.sybil_unverified',
    ])
    expect(projection.subjects[2]?.score).toBe(0)
  })

  test('applies closed-beta manual overrides as caveated read-only evidence', () => {
    const projection = projectSybilProofReputation({
      edges: [edge({ subjectRef: 'pylon.public.alpha' })],
      generatedAtIso,
      manualOverrides: [
        {
          createdAtIso: '2026-06-27T18:10:00.000Z',
          expiresAtIso: '2026-07-27T18:10:00.000Z',
          issuedByRef: 'operator.public.reputation_beta',
          overrideRef: 'override.public.reputation.manual_beta_1',
          reasonRef: 'reason.public.reputation.closed_beta_seed',
          scoreDelta: 50,
          scoreFloor: 950,
          subjectRef: 'pylon.public.manual_seed',
        },
      ],
      projectionRef: 'projection.public.reputation.beta',
    })

    const manual = projection.subjects.find(subject =>
      subject.subjectRef === 'pylon.public.manual_seed'
    )
    expect(manual).toMatchObject({
      caveatRefs: ['caveat.public.reputation.closed_beta_manual_override'],
      manualOverrideApplied: true,
      manualOverrideRefs: ['override.public.reputation.manual_beta_1'],
      score: 950,
    })
    expect(manual?.scoreBasisRefs).toContain(
      'basis.public.reputation.closed_beta_manual_override',
    )
    expect(projection.caveatRefs).toContain(
      'caveat.public.reputation.closed_beta_manual_overrides_enabled',
    )
    expect(manual?.authority.noManualOverrideWriteAuthority).toBe(true)
  })

  test('rejects unsafe material and unaudited manual overrides', () => {
    expect(() =>
      projectSybilProofReputation({
        edges: [
          edge({
            settlementRefs: ['settlement.public.btc.lnbc123rawinvoice'],
          }),
        ],
        generatedAtIso,
        projectionRef: 'projection.public.reputation.beta',
      }),
    ).toThrow(SybilProofReputationUnsafe)

    expect(() =>
      projectSybilProofReputation({
        edges: [],
        generatedAtIso,
        manualOverrides: [
          {
            createdAtIso: '2026-06-27T18:10:00.000Z',
            issuedByRef: '',
            overrideRef: 'override.public.reputation.manual_beta_1',
            reasonRef: 'reason.public.reputation.closed_beta_seed',
            scoreFloor: 700,
            subjectRef: 'pylon.public.manual_seed',
          },
        ],
        projectionRef: 'projection.public.reputation.beta',
      }),
    ).toThrow(SybilProofReputationUnsafe)
  })
})
