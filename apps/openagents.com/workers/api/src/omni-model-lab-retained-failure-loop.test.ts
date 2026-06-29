import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OMNI_MODEL_LAB_READ_ONLY_AUTHORITY,
  OmniModelLabRetainedFailureLoopProjection,
  OmniModelLabRetainedFailureLoopRecord,
  OmniModelLabRetainedFailureLoopUnsafe,
  exampleOmniModelLabRetainedFailureLoop,
  omniModelLabProjectionHasPrivateMaterial,
  projectOmniModelLabRetainedFailureLoop,
} from './omni-model-lab-retained-failure-loop'

const nowIso = '2026-06-06T22:30:00.000Z'

const loopRecord = (
  overrides: Partial<OmniModelLabRetainedFailureLoopRecord> = {},
): OmniModelLabRetainedFailureLoopRecord =>
  S.decodeUnknownSync(OmniModelLabRetainedFailureLoopRecord)({
    ...exampleOmniModelLabRetainedFailureLoop(),
    ...overrides,
  })

describe('Omni Model Lab retained-failure loop', () => {
  test('projects retained failures through candidates, evals, adapter validation, gates, rollback, and attribution without mutation authority', () => {
    const projection = projectOmniModelLabRetainedFailureLoop(
      exampleOmniModelLabRetainedFailureLoop(),
      'operator',
      nowIso,
    )

    expect(S.decodeUnknownSync(OmniModelLabRetainedFailureLoopProjection)(
      projection,
    )).toEqual(projection)
    expect(projection).toMatchObject({
      adapterInstallationAllowed: false,
      adapterValidationPassedCount: 1,
      candidateCount: 1,
      createdAtDisplay: '30 minutes ago',
      evalExecutionAllowed: false,
      evalPassedCount: 1,
      modelTrainingMutationAllowed: false,
      payoutMutationAllowed: false,
      promotionGatePassedCount: 1,
      publicClaimUpgradeAllowed: false,
      recordedAttributionCount: 1,
      retainedFailureCount: 1,
      rollbackPosture: 'ready',
      routingMutationAllowed: false,
      runtimePromotionAllowed: false,
      selfPromotionAttemptDetected: false,
      settlementMutationAllowed: false,
      state: 'attributed',
      stateLabel: 'Attributed',
      updatedAtDisplay: '5 minutes ago',
    })
    expect(projection.authority).toEqual(OMNI_MODEL_LAB_READ_ONLY_AUTHORITY)
    expect(JSON.stringify(projection)).not.toContain('2026-06-06T')
    expect(omniModelLabProjectionHasPrivateMaterial(projection)).toBe(false)
  })

  test('keeps retained, candidate, eval, adapter, gate, and attribution states separate', () => {
    const retainedOnly = loopRecord({
      adapterValidations: [],
      attributions: [],
      candidateRecords: [],
      evalReruns: [],
      promotionGates: [],
      state: 'retained',
    })
    const candidateOnly = loopRecord({
      adapterValidations: [],
      attributions: [],
      evalReruns: [],
      promotionGates: [],
      state: 'candidate_created',
    })
    const evalOnly = loopRecord({
      adapterValidations: [],
      attributions: [],
      promotionGates: [],
      state: 'eval_rerun',
    })
    const gateOnly = loopRecord({
      attributions: [],
      state: 'gate_passed',
    })

    expect(projectOmniModelLabRetainedFailureLoop(
      retainedOnly,
      'operator',
      nowIso,
    )).toMatchObject({
      candidateCount: 0,
      evalPassedCount: 0,
      retainedFailureCount: 1,
    })
    expect(projectOmniModelLabRetainedFailureLoop(
      candidateOnly,
      'operator',
      nowIso,
    )).toMatchObject({
      candidateCount: 1,
      evalPassedCount: 0,
      promotionGatePassedCount: 0,
    })
    expect(projectOmniModelLabRetainedFailureLoop(evalOnly, 'operator', nowIso))
      .toMatchObject({
        adapterValidationPassedCount: 0,
        evalPassedCount: 1,
        promotionGatePassedCount: 0,
      })
    expect(projectOmniModelLabRetainedFailureLoop(gateOnly, 'operator', nowIso))
      .toMatchObject({
        promotionGatePassedCount: 1,
        recordedAttributionCount: 0,
        runtimePromotionAllowed: false,
      })
  })

  test('requires retained failure evidence, candidate/eval linkage, passed eval evidence, promotion gates, rollback posture, and attribution receipts', () => {
    for (const badRecord of [
      loopRecord({ retainedFailures: [] }),
      loopRecord({
        retainedFailures: [
          {
            ...exampleOmniModelLabRetainedFailureLoop().retainedFailures[0]!,
            evidenceRefs: [],
          },
        ],
      }),
      loopRecord({
        candidateRecords: [
          {
            ...exampleOmniModelLabRetainedFailureLoop().candidateRecords[0]!,
            sourceFailureRefs: ['failure.public.missing'],
          },
        ],
      }),
      loopRecord({
        evalReruns: [
          {
            ...exampleOmniModelLabRetainedFailureLoop().evalReruns[0]!,
            receiptRefs: [],
          },
        ],
      }),
      loopRecord({
        adapterValidations: [
          {
            ...exampleOmniModelLabRetainedFailureLoop().adapterValidations[0]!,
            receiptRefs: [],
          },
        ],
      }),
      loopRecord({
        promotionGates: [
          {
            ...exampleOmniModelLabRetainedFailureLoop().promotionGates[0]!,
            rollbackPosture: 'missing',
          },
        ],
      }),
      loopRecord({
        promotionGates: [
          {
            ...exampleOmniModelLabRetainedFailureLoop().promotionGates[0]!,
            selfPromotionAttempt: true,
          },
        ],
      }),
      loopRecord({
        attributions: [
          {
            ...exampleOmniModelLabRetainedFailureLoop().attributions[0]!,
            receiptRefs: [],
          },
        ],
      }),
      loopRecord({ blockerRefs: [], state: 'blocked' }),
    ]) {
      expect(() =>
        projectOmniModelLabRetainedFailureLoop(badRecord, 'operator', nowIso),
      ).toThrow(OmniModelLabRetainedFailureLoopUnsafe)
    }
  })

  test('redacts private prompts, source archives, provider refs, failures, gates, receipts, and traces publicly', () => {
    const projection = projectOmniModelLabRetainedFailureLoop(
      loopRecord({
        adapterValidations: [
          {
            ...exampleOmniModelLabRetainedFailureLoop().adapterValidations[0]!,
            candidateRefs: ['candidate.private.operator_candidate'],
            providerRefs: [
              'provider.public.local_model_lab',
              'provider.private.operator_gpu',
            ],
            receiptRefs: [
              'receipt.public.adapter_validation',
              'receipt.private.operator_receipt',
            ],
          },
        ],
        candidateRecords: [
          {
            ...exampleOmniModelLabRetainedFailureLoop().candidateRecords[0]!,
            candidateRef: 'candidate.private.operator_candidate',
            sourceFailureRefs: ['failure.private.operator_trace'],
          },
        ],
        evalReruns: [
          {
            ...exampleOmniModelLabRetainedFailureLoop().evalReruns[0]!,
            candidateRefs: ['candidate.private.operator_candidate'],
            sourceFailureRefs: ['failure.private.operator_trace'],
          },
        ],
        promotionGates: [
          {
            ...exampleOmniModelLabRetainedFailureLoop().promotionGates[0]!,
            candidateRefs: ['candidate.private.operator_candidate'],
            gateRef: 'gate.private.operator_gate',
            reviewReceiptRefs: [
              'review.public.operator_approved',
              'review.private.operator_notes',
            ],
          },
        ],
        attributions: [
          {
            ...exampleOmniModelLabRetainedFailureLoop().attributions[0]!,
            candidateRefs: ['candidate.private.operator_candidate'],
          },
        ],
        retainedFailures: [
          {
            ...exampleOmniModelLabRetainedFailureLoop().retainedFailures[0]!,
            failureRef: 'failure.private.operator_trace',
            sourceRefs: [
              'source.public.failure_summary',
              'source.private.operator_archive',
            ],
            traceRefs: [
              'trace.public.failure_summary',
              'trace.private.operator_detail',
            ],
          },
        ],
        sourceRefs: [
          'source.public.model_lab_loop_summary',
          'source.private.operator_archive',
        ],
      }),
      'public',
      nowIso,
    )

    const serialized = JSON.stringify(projection)

    expect(projection.sourceRefs).toEqual([])
    expect(projection.candidateRecords).toEqual([])
    expect(projection.promotionGates).toEqual([])
    expect(projection.retainedFailures).toEqual([])
    expect(projection.adapterValidations[0]?.providerRefs).toEqual([])
    expect(projection.adapterValidations[0]?.receiptRefs).toEqual([
      'receipt.public.adapter_validation',
    ])
    expect(serialized).not.toMatch(
      /(candidate|failure|gate|provider|receipt|review|source|trace)\.private/,
    )
    expect(omniModelLabProjectionHasPrivateMaterial(projection)).toBe(false)
  })

  test('rejects mutable authority, private prompts, raw source archives, provider payloads, customer data, wallet/payment material, and raw timestamps', () => {
    for (const badInput of [
      () =>
        projectOmniModelLabRetainedFailureLoop(
          loopRecord({
            authority: {
              ...OMNI_MODEL_LAB_READ_ONLY_AUTHORITY,
              noRuntimePromotion: false,
            },
          }),
          'operator',
          nowIso,
        ),
      () =>
        projectOmniModelLabRetainedFailureLoop(
          loopRecord({ sourceRefs: ['raw_prompt.customer_request'] }),
          'operator',
          nowIso,
        ),
      () =>
        projectOmniModelLabRetainedFailureLoop(
          loopRecord({ sourceRefs: ['source_archive.private_package'] }),
          'operator',
          nowIso,
        ),
      () =>
        projectOmniModelLabRetainedFailureLoop(
          loopRecord({
            adapterValidations: [
              {
                ...exampleOmniModelLabRetainedFailureLoop()
                  .adapterValidations[0]!,
                providerRefs: ['provider_payload.raw'],
              },
            ],
          }),
          'operator',
          nowIso,
        ),
      () =>
        projectOmniModelLabRetainedFailureLoop(
          loopRecord({ sourceRefs: ['customer_email.redacted'] }),
          'operator',
          nowIso,
        ),
      () =>
        projectOmniModelLabRetainedFailureLoop(
          loopRecord({
            attributions: [
              {
                ...exampleOmniModelLabRetainedFailureLoop().attributions[0]!,
                receiptRefs: ['receipt.public.payment_hash_abcd'],
              },
            ],
          }),
          'operator',
          nowIso,
        ),
      () =>
        projectOmniModelLabRetainedFailureLoop(
          loopRecord({ sourceRefs: ['source.public.2026-06-06T22:25:00.000Z'] }),
          'operator',
          nowIso,
        ),
    ]) {
      expect(badInput).toThrow(OmniModelLabRetainedFailureLoopUnsafe)
    }
  })
})
