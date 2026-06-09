import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  ARTANIS_REAL_SMALL_BITCOIN_ASSIGNMENT_SMOKE_NO_AUTHORITY,
  ArtanisRealSmallBitcoinAssignmentSmokeProjection,
  ArtanisRealSmallBitcoinAssignmentSmokeUnsafe,
  issue438ArtanisRealSmallBitcoinAssignmentSmokeRecord,
  projectArtanisRealSmallBitcoinAssignmentSmoke,
} from './artanis-real-small-bitcoin-assignment-smoke'

const nowIso = '2026-06-07T09:00:00.000Z'

describe('Artanis real small-bitcoin assignment smoke', () => {
  test('projects issue 438 as public-safe retained real-assignment evidence', () => {
    const projection = projectArtanisRealSmallBitcoinAssignmentSmoke(
      issue438ArtanisRealSmallBitcoinAssignmentSmokeRecord(),
      'public',
      nowIso,
    )

    expect(S.decodeUnknownSync(
      ArtanisRealSmallBitcoinAssignmentSmokeProjection,
    )(projection)).toEqual(projection)
    expect(projection).toMatchObject({
      agentRef: 'agent_artanis',
      amountLabel: '0.00001000 bitcoin (1,000 satoshis)',
      assignmentRef: 'assignment.public.issue_438.issue_438_artanis_1780822221',
      audience: 'public',
      moneyMovement: 'real_bitcoin',
      releaseCreationAllowedByThisRecord: false,
      releasePublicationAllowed: false,
      settlementMutationAllowed: false,
      settlementReceiptRef:
        'receipt.nexus.issue_438.settlement.issue_438_artanis_1780822221',
      state: 'passed',
      walletSpendAllowed: false,
    })
    expect(projection.acceptedWorkRefs).toContain(
      'accepted_work.public.issue_438_artanis_pylon_assignment',
    )
    expect(projection.artifactProofRefs).toEqual(
      expect.arrayContaining([
        'artifact.public.issue_438.artanis_assignment_proof_manifest',
        'proof.public.issue_438.pylon_assignment.accepted_work',
      ]),
    )
    expect(projection.duplicateDispatchEvidenceRefs).toContain(
      'idempotency.public.issue_438.payment_authority.no_duplicate_spend',
    )
    expect(projection.forumUpdateRefs).toContain(
      'forum.public.artanis.nexus_pylon.release_gate_pass.issue_438_artanis_1780822221',
    )
    expect(projection.receiptPageRouteRef).toContain(
      'receipt.nexus.issue_438.settlement.issue_438_artanis_1780822221',
    )
    expect(JSON.stringify(projection)).not.toMatch(
      /lnbc|lntb|mnemonic|preimage|payment_hash|raw_payment|wallet_(config|key|material|mnemonic|secret|seed|state)/i,
    )
    expect(JSON.stringify(projection)).not.toMatch(
      /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/,
    )
  })

  test('rejects wrong agent, missing required evidence, raw payment material, and authority escalation', () => {
    const record = issue438ArtanisRealSmallBitcoinAssignmentSmokeRecord()

    expect(() =>
      projectArtanisRealSmallBitcoinAssignmentSmoke(
        {
          ...record,
          agentRef: 'agent_wrong',
        },
        'operator',
        nowIso,
      )
    ).toThrow(ArtanisRealSmallBitcoinAssignmentSmokeUnsafe)

    expect(() =>
      projectArtanisRealSmallBitcoinAssignmentSmoke(
        {
          ...record,
          acceptedWorkRefs: [],
        },
        'operator',
        nowIso,
      )
    ).toThrow(ArtanisRealSmallBitcoinAssignmentSmokeUnsafe)

    expect(() =>
      projectArtanisRealSmallBitcoinAssignmentSmoke(
        {
          ...record,
          artifactProofRefs: ['payment_hash.raw_secret'],
        },
        'operator',
        nowIso,
      )
    ).toThrow(ArtanisRealSmallBitcoinAssignmentSmokeUnsafe)

    expect(() =>
      projectArtanisRealSmallBitcoinAssignmentSmoke(
        {
          ...record,
          authority: {
            ...ARTANIS_REAL_SMALL_BITCOIN_ASSIGNMENT_SMOKE_NO_AUTHORITY,
            walletSpendAllowed: true,
          },
        },
        'operator',
        nowIso,
      )
    ).toThrow(ArtanisRealSmallBitcoinAssignmentSmokeUnsafe)
  })
})
