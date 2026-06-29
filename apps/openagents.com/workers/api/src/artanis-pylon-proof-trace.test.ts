import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  ARTANIS_PYLON_PROOF_TRACE_FIXTURES,
  ArtanisPylonProofTraceRecord,
  ArtanisPylonProofTraceUnsafe,
  artanisPylonProofTraceHasNoAuthority,
  artanisPylonProofTraceProjectionHasPrivateMaterial,
  projectArtanisPylonProofTrace,
} from './artanis-pylon-proof-trace'
import {
  OPENAGENTS_UNSAFE_REDACTION_FIXTURES,
  openAgentsSerializedValueContainsUnsafeFixture,
} from './redaction-regression-fixtures'

const nowIso = '2026-06-07T23:00:00.000Z'

const traceRecord = (
  overrides: Partial<ArtanisPylonProofTraceRecord> = {},
): ArtanisPylonProofTraceRecord =>
  S.decodeUnknownSync(ArtanisPylonProofTraceRecord)({
    ...ARTANIS_PYLON_PROOF_TRACE_FIXTURES[0]!,
    ...overrides,
  })

describe('Artanis/Pylon proof trace checker', () => {
  test('classifies a complete single-assignment proof chain as complete without authority', () => {
    const projection = projectArtanisPylonProofTrace(
      traceRecord(),
      'public',
      nowIso,
    )

    expect(projection).toMatchObject({
      acceptedWorkObserved: true,
      artifactProofObserved: true,
      dispatchObserved: true,
      paymentEvidenceObserved: true,
      publicReceiptObserved: true,
      realBitcoinMoved: true,
      sameAssignmentIdObserved: true,
      settlementEvidenceObserved: true,
      state: 'complete',
      terminalSettlementObserved: true,
    })
    expect(projection.distinctPylonRefs).toEqual([
      'pylon.public.edge.trace_alpha',
    ])
    expect(projection.missingEvidenceRefs).toEqual([])
    expect(artanisPylonProofTraceHasNoAuthority(
      projection.authority,
    )).toBe(true)
    expect(projection.authority).toMatchObject({
      pylonMutationAllowed: false,
      receiptMutationAllowed: false,
      releasePublicationAllowed: false,
      settlementMutationAllowed: false,
      walletSpendAllowed: false,
    })
    expect(JSON.stringify(projection)).not.toContain('2026-06-07T')
    expect(artanisPylonProofTraceProjectionHasPrivateMaterial(projection))
      .toBe(false)
  })

  test('keeps missing accepted work and payment evidence partial', () => {
    const projection = projectArtanisPylonProofTrace(
      traceRecord({
        pylonEvents: ARTANIS_PYLON_PROOF_TRACE_FIXTURES[0]!.pylonEvents
          .filter(event =>
            event.eventKind !== 'assignment_acceptance' &&
            event.eventKind !== 'payment_receipt'
          ),
      }),
      'public',
      nowIso,
    )

    expect(projection.state).toBe('partial')
    expect(projection.acceptedWorkObserved).toBe(false)
    expect(projection.paymentEvidenceObserved).toBe(false)
    expect(projection.missingEvidenceRefs).toEqual([
      'missing.public.artanis_pylon_proof.accepted_work',
      'missing.public.artanis_pylon_proof.payment_evidence',
    ])
    expect(projection.realBitcoinMoved).toBe(true)
    expect(projection.terminalSettlementObserved).toBe(true)
  })

  test('blocks mismatched receipt assignment ids', () => {
    const fixture = ARTANIS_PYLON_PROOF_TRACE_FIXTURES[0]!
    const projection = projectArtanisPylonProofTrace(
      traceRecord({
        receipt: {
          ...fixture.receipt!,
          assignmentRef: 'assignment.public.artanis.proof_trace.other',
        },
      }),
      'public',
      nowIso,
    )

    expect(projection.state).toBe('blocked')
    expect(projection.sameAssignmentIdObserved).toBe(false)
    expect(projection.blockerRefs).toContain(
      'blocker.public.artanis_pylon_proof.assignment_ref_mismatch',
    )
    expect(projection.missingEvidenceRefs).toContain(
      'missing.public.artanis_pylon_proof.public_receipt',
    )
  })

  test('does not allow simulation-only receipts to satisfy real bitcoin movement', () => {
    const fixture = ARTANIS_PYLON_PROOF_TRACE_FIXTURES[0]!
    const projection = projectArtanisPylonProofTrace(
      traceRecord({
        receipt: {
          ...fixture.receipt!,
          movementMode: 'simulation',
          realBitcoinMoved: false,
        },
      }),
      'public',
      nowIso,
    )

    expect(projection.state).toBe('partial')
    expect(projection.publicReceiptObserved).toBe(true)
    expect(projection.realBitcoinMoved).toBe(false)
    expect(projection.blockerRefs).toContain(
      'blocker.public.artanis_pylon_proof.simulation_only_receipt',
    )
    expect(projection.missingEvidenceRefs).toContain(
      'missing.public.artanis_pylon_proof.real_bitcoin_movement',
    )
  })

  test('redacts private refs from public projection while retaining them for operator projection', () => {
    const record = traceRecord({
      dispatch: {
        ...ARTANIS_PYLON_PROOF_TRACE_FIXTURES[0]!.dispatch,
        evidenceRefs: [
          'event.public.artanis.dispatch.assignment_proof_trace_complete_001',
          'operator.note.artanis.trace_summary',
        ],
      },
    })

    const publicProjection = projectArtanisPylonProofTrace(
      record,
      'public',
      nowIso,
    )
    const operatorProjection = projectArtanisPylonProofTrace(
      record,
      'operator',
      nowIso,
    )

    expect(publicProjection.evidenceRefs).not.toContain(
      'operator.note.artanis.trace_summary',
    )
    expect(operatorProjection.evidenceRefs).toContain(
      'operator.note.artanis.trace_summary',
    )
  })

  test('rejects raw payment, wallet, provider, private log, and timestamp material', () => {
    for (const fixture of [
      ...OPENAGENTS_UNSAFE_REDACTION_FIXTURES,
      { label: 'raw invoice', value: 'raw_invoice.lnbc1unsafe' },
      { label: 'payment hash', value: 'payment_hash.provider_secret' },
      { label: 'wallet path', value: '/Users/example/.mdk-wallet/config.json' },
      { label: 'raw timestamp', value: '2026-06-07T22:50:00.000Z' },
    ]) {
      expect(() =>
        projectArtanisPylonProofTrace(
          traceRecord({
            dispatch: {
              ...ARTANIS_PYLON_PROOF_TRACE_FIXTURES[0]!.dispatch,
              evidenceRefs: [fixture.value],
            },
          }),
          'operator',
          nowIso,
        ),
      ).toThrow(ArtanisPylonProofTraceUnsafe)
    }
  })

  test('projection avoids common unsafe serialized fixtures', () => {
    const projection = projectArtanisPylonProofTrace(
      traceRecord(),
      'public',
      nowIso,
    )

    expect(openAgentsSerializedValueContainsUnsafeFixture(projection)).toBe(
      false,
    )
  })
})
