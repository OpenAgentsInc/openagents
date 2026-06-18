import { describe, expect, test } from 'vitest'

import {
  deriveDataContributionTraceDigest,
  verifyDataContributionCorrectness,
} from './data-trace-marketplace-gate'
import {
  ArtanisWorkDirectionUnsafe,
  buildArtanisWorkDirectionLaborProposal,
  filterArtanisWorkRoutingProposalsIntoRequests,
  handleArtanisWorkDirectionDelivery,
  runArtanisWorkDirectionRequestTick,
  type ArtanisProgramAuthorshipVerificationVerdict,
  type ArtanisWorkDirectionDelivery,
  type ArtanisWorkDirectionRequest,
  type ArtanisWorkRoutingProposalDirection,
} from './artanis-work-directions'
import { verifyTassadarAdversarialDivergenceClaim } from './tassadar-adversarial-verification-market'

const sourceDigest =
  'sha256:3333333333333333333333333333333333333333333333333333333333333333'
const divergenceInputDigest =
  'sha256:5555555555555555555555555555555555555555555555555555555555555555'
const divergenceExpectedDigest =
  'sha256:6666666666666666666666666666666666666666666666666666666666666666'
const divergenceObservedDigest =
  'sha256:7777777777777777777777777777777777777777777777777777777777777777'
const traceRows = [
  {
    evidenceRef: 'span.public.reference_lane.prosemirror_001',
    fieldRef: 'field.public.trace_corpus.edge',
    valueDigest:
      'sha256:4444444444444444444444444444444444444444444444444444444444444444',
  },
]

const programRequest = (
  overrides: Partial<ArtanisWorkDirectionRequest> = {},
): ArtanisWorkDirectionRequest => ({
  budgetSats: 4_000,
  corpusRef: 'corpus.tassadar_trace.v0_2.w3_100m',
  deadlineRef: 'deadline.public.artanis.work_direction.soon',
  directionKind: 'program_authorship',
  moduleFamilyRef: 'module_family.public.tassadar.calm_wasm',
  objectiveRef: 'objective.public.artanis.program_author.calm_wasm_dense_001',
  repositoryRefs: ['repo.public.github.OpenAgentsInc.openagents'],
  sourceRefs: ['source.public.artanis.work_direction.c1'],
  title: 'Author a distinct CALM Wasm module',
  verificationClass: 'v1_construction',
  verificationCommandRef: 'command.public.tassadar.v1_construction_verification',
  ...overrides,
})

const datasetRequest = (
  overrides: Partial<ArtanisWorkDirectionRequest> = {},
): ArtanisWorkDirectionRequest => ({
  budgetSats: 2_500,
  corpusRef: 'corpus.tassadar_trace.reference_lane.prosemirror_v1',
  deadlineRef: 'deadline.public.artanis.work_direction.soon',
  directionKind: 'dataset_curation',
  objectiveRef: 'objective.public.artanis.dataset_curate.prosemirror_trace_001',
  repositoryRefs: ['repo.public.projects.prosemirror'],
  sourceRefs: ['source.public.projects.prosemirror.manifest'],
  title: 'Curate reference-lane trace corpus',
  verificationClass: 'v3_data_correctness',
  verificationCommandRef:
    'command.public.openagents.data_contribution.v3_correctness',
  ...overrides,
})

const adversarialRequest = (
  overrides: Partial<ArtanisWorkDirectionRequest> = {},
): ArtanisWorkDirectionRequest => ({
  budgetSats: 3_000,
  corpusRef: 'corpus.tassadar_adversarial.divergence_inputs.v1',
  deadlineRef: 'deadline.public.artanis.work_direction.soon',
  directionKind: 'adversarial_verification',
  moduleFamilyRef: 'module_family.public.tassadar.linked_dense',
  objectiveRef:
    'objective.public.artanis.adversarial_verification.linked_dense_001',
  repositoryRefs: ['repo.public.github.OpenAgentsInc.openagents'],
  sourceRefs: ['source.public.artanis.work_direction.e3'],
  title: 'Find a reproducible linked dense module divergence',
  verificationClass: 'e3_adversarial_divergence',
  verificationCommandRef: 'command.public.tassadar.e3_adversarial_divergence',
  ...overrides,
})

const v1Verdict = (
  overrides: Partial<ArtanisProgramAuthorshipVerificationVerdict> = {},
): ArtanisProgramAuthorshipVerificationVerdict => ({
  blockerRefs: [],
  constructionVerified: true,
  moduleDigest:
    'abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789',
  moduleKind: 'tassadar.calm_wasm.module',
  realBitcoinMoved: false,
  replayVerified: true,
  settlementSimulationRef:
    'settlement_simulation.public.tassadar.compiled_module_construction.v1',
  verificationClass: 'v1_construction',
  verificationRef: 'verification.public.tassadar.v1_construction.fixture',
  ...overrides,
})

const delivery = (
  overrides: Partial<ArtanisWorkDirectionDelivery> = {},
): ArtanisWorkDirectionDelivery => ({
  acceptanceEventRef: 'nostr.event.' + 'a'.repeat(64),
  directionKind: 'program_authorship',
  programVerification: v1Verdict(),
  providerActorRef: 'agent:module-author',
  resultRef: 'result.public.artanis.work_direction.program_001',
  verificationCommandRef: 'command.public.tassadar.v1_construction_verification',
  workRequestId: 'work_request_artanis_program_001',
  ...overrides,
})

const routingProposal = (
  overrides: Partial<ArtanisWorkRoutingProposalDirection> = {},
): ArtanisWorkRoutingProposalDirection => ({
  budgetSats: 2_500,
  corpusRef: 'corpus.tassadar_trace.reference_lane.prosemirror_v1',
  deadlineRef: 'deadline.public.artanis.work_direction.soon',
  directionKind: 'dataset_curation',
  objectiveRef: 'objective.public.artanis.dataset_curate.prosemirror_trace_001',
  proposalRef: 'proposal.public.artanis.work_routing.prosemirror_trace',
  repositoryRefs: ['repo.public.projects.prosemirror'],
  selectorRef: 'selector.public.artanis.semantic_work_direction.v1',
  sourceRefs: ['topic.public.forum.artanis.work_routing'],
  title: 'Curate a ProseMirror trace corpus',
  ...overrides,
})

const acceptedDataCorrectness = async () => {
  const claimedTraceDigest = await deriveDataContributionTraceDigest({
    sourceDigest,
    traceRows,
    transformRef: 'transform.public.artanis.dataset_trace.v1',
  })

  return verifyDataContributionCorrectness({
    claimedTraceDigest,
    contributionRef: 'trace.public.artanis.dataset_curate.prosemirror_001',
    derivedTraceRows: traceRows,
    provenanceRefs: ['provenance.public.reference_lane.prosemirror_001'],
    sourceDigest,
    sourceRefs: ['source.public.projects.prosemirror.manifest'],
    transformRef: 'transform.public.artanis.dataset_trace.v1',
    verificationMode: 'derived_trace_replay',
  })
}

const acceptedDivergence = () =>
  verifyTassadarAdversarialDivergenceClaim({
    claim: {
      claimRef: 'claim.public.tassadar_adversarial.divergence_artanis_001',
      claimantActorRef: 'agent:adversarial-verifier',
      claimantDeviceRef: 'device.pylon.adversarial_hunter',
      divergenceKind: 'trace_digest_mismatch',
      expectedBehaviorDigest: divergenceExpectedDigest,
      implementationRefs: [
        'implementation.public.tassadar.reference_linear',
        'implementation.public.tassadar.hull_cache',
      ],
      inputDigest: divergenceInputDigest,
      inputRef: 'input.public.tassadar_adversarial.divergence_artanis_001',
      moduleDigest:
        'abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789',
      moduleKind: 'tassadar.linked_dense.module',
      moduleRef: 'module.public.tassadar.linked_dense.canonical',
      observedBehaviorDigest: divergenceObservedDigest,
      psionicEvidenceRefs: [
        'report.public.psionic.tassadar_exactness_refusal.step_mismatch',
      ],
      sourceRefs: ['source.public.psionic.tassadar_trace_diff_report'],
      specRef: 'spec.public.tassadar.linked_dense.w3_100m',
      workRequestId: 'work_request_artanis_adversarial_001',
    },
    reproduction: {
      blockerRefs: [],
      expectedBehaviorDigest: divergenceExpectedDigest,
      inputDigest: divergenceInputDigest,
      observedBehaviorDigest: divergenceObservedDigest,
      psionicEvidenceRefs: [
        'report.public.psionic.tassadar_exactness_refusal.reproduced',
      ],
      reproduced: true,
      reproductionRef:
        'reproduction.public.tassadar_adversarial.divergence_artanis_001',
      validatorActorRef: 'agent:independent-validator',
      validatorDeviceRef: 'device.pylon.independent_validator',
      validatorReceiptRefs: [
        'receipt.public.tassadar_adversarial.validator_replay_artanis_001',
      ],
    },
  })

describe('Artanis work-direction requests', () => {
  test('builds a program-authorship request with V1 construction capabilities', () => {
    const proposal = buildArtanisWorkDirectionLaborProposal(programRequest())

    expect(proposal).toEqual({
      budgetSats: 4_000,
      deadlineRef: 'deadline.public.artanis.work_direction.soon',
      objectiveRef: 'objective.public.artanis.program_author.calm_wasm_dense_001',
      repositoryRefs: ['repo.public.github.OpenAgentsInc.openagents'],
      requiredCapabilityRefs: [
        'capability.openagents.artanis_work_direction.ref_only_delivery',
        'capability.openagents.tassadar.compiled_module.construct',
        'capability.openagents.tassadar.corpus.c1',
        'capability.openagents.tassadar.program_authorship.calm_wasm_module',
        'capability.openagents.tassadar.v1_construction_verification',
      ],
      title: 'Author a distinct CALM Wasm module',
      verificationCommandRef:
        'command.public.tassadar.v1_construction_verification',
    })
  })

  test('builds a dataset-curation request with V3 correctness capabilities', () => {
    const proposal = buildArtanisWorkDirectionLaborProposal(datasetRequest())

    expect(proposal.requiredCapabilityRefs).toEqual([
      'capability.openagents.artanis_work_direction.ref_only_delivery',
      'capability.openagents.data_contribution.v3_correctness',
      'capability.openagents.reference_lane.distill_to_trace_corpus',
      'capability.openagents.tassadar.dataset_curation.trace_corpus',
    ])
    expect(proposal.verificationCommandRef).toBe(
      'command.public.openagents.data_contribution.v3_correctness',
    )
  })

  test('builds an adversarial-verification request with E3 divergence capabilities', () => {
    const proposal = buildArtanisWorkDirectionLaborProposal(adversarialRequest())

    expect(proposal.requiredCapabilityRefs).toEqual([
      'capability.openagents.artanis_work_direction.ref_only_delivery',
      'capability.openagents.tassadar.adversarial_verification.divergence_input',
      'capability.openagents.tassadar.adversarial_verification.independent_reproduction',
      'capability.openagents.tassadar.e3_adversarial_divergence',
      'capability.openagents.tassadar.v1_found_defect_settlement',
    ])
    expect(proposal.verificationCommandRef).toBe(
      'command.public.tassadar.e3_adversarial_divergence',
    )
  })

  test('keeps the work-direction requester off until an operator enables it', async () => {
    let proposed = false
    const outcome = await runArtanisWorkDirectionRequestTick({
      alreadyReservedThisTickMsat: 0,
      artanisActorRef: 'agent:artanis',
      enabled: false,
      nowIso: '2026-06-18T02:00:00.000Z',
      perTickBudgetMsat: 10_000_000,
      proposeWorkDirection: async () => {
        proposed = true
        return programRequest()
      },
      recordTickReceipt: async () => {},
      reserveEscrow: async () => ({
        ok: true,
        reserveReceiptRef: 'receipt.labor_escrow.reserve.work_direction_001',
      }),
      seedBalanceAvailableMsat: 10_000_000,
      submitWorkRequest: async () => ({
        jobEventId: 'b'.repeat(64),
        topicId: 'topic_work_direction_001',
        workRequestId: 'work_request_artanis_program_001',
      }),
    })

    expect(outcome).toEqual({ kind: 'skipped', reason: 'config_disabled' })
    expect(proposed).toBe(false)
  })

  test('submits enabled work-direction requests through labor escrow', async () => {
    const submitted: unknown[] = []
    const reserved: unknown[] = []
    const outcome = await runArtanisWorkDirectionRequestTick({
      alreadyReservedThisTickMsat: 1_000_000,
      artanisActorRef: 'agent:artanis',
      enabled: true,
      nowIso: '2026-06-18T02:00:00.000Z',
      perTickBudgetMsat: 10_000_000,
      proposeWorkDirection: async () => programRequest(),
      recordTickReceipt: async () => {},
      reserveEscrow: async input => {
        reserved.push(input)
        return {
          ok: true,
          reserveReceiptRef: 'receipt.labor_escrow.reserve.work_direction_001',
        }
      },
      seedBalanceAvailableMsat: 10_000_000,
      submitWorkRequest: async input => {
        submitted.push(input)
        return {
          jobEventId: 'b'.repeat(64),
          topicId: 'topic_work_direction_001',
          workRequestId: 'work_request_artanis_program_001',
        }
      },
    })

    expect(outcome).toMatchObject({
      budgetMsat: 4_000_000,
      kind: 'requested',
      reserveReceiptRef: 'receipt.labor_escrow.reserve.work_direction_001',
    })
    expect(submitted).toHaveLength(1)
    expect(submitted[0]).toMatchObject({
      verificationCommandRef:
        'command.public.tassadar.v1_construction_verification',
    })
    expect(reserved).toEqual([
      {
        amountMsat: 4_000_000,
        jobEventId: 'b'.repeat(64),
        requesterActorRef: 'agent:artanis',
        workRequestId: 'work_request_artanis_program_001',
      },
    ])
  })

  test('filters contributor routing proposals only when operator-enabled', () => {
    const disabled = filterArtanisWorkRoutingProposalsIntoRequests({
      proposals: [routingProposal()],
    })
    const enabled = filterArtanisWorkRoutingProposalsIntoRequests({
      operatorEnabled: true,
      proposals: [routingProposal()],
    })

    expect(disabled).toMatchObject({
      enabled: false,
      requestCount: 0,
      skippedReason: 'operator_disabled',
      workRequests: [],
    })
    expect(disabled.blockerRefs).toContain(
      'blocker.public.artanis.work_directions.operator_disabled',
    )
    expect(enabled).toMatchObject({
      enabled: true,
      requestCount: 1,
      skippedReason: null,
    })
    expect(enabled.workRequests[0]).toMatchObject({
      directionKind: 'dataset_curation',
      sourceRefs: expect.arrayContaining([
        'proposal.public.artanis.work_routing.prosemirror_trace',
        'selector.public.artanis.semantic_work_direction.v1',
      ]),
      verificationClass: 'v3_data_correctness',
      verificationCommandRef:
        'command.public.openagents.data_contribution.v3_correctness',
    })
  })

  test('rejects unsafe routing proposal refs before creating funded requests', () => {
    expect(() =>
      filterArtanisWorkRoutingProposalsIntoRequests({
        operatorEnabled: true,
        proposals: [
          routingProposal({
            sourceRefs: ['source.raw_private_trace'],
          }),
        ],
      }),
    ).toThrow(ArtanisWorkDirectionUnsafe)
  })
})

describe('Artanis work-direction acceptance gates', () => {
  test('settles program-authorship work only after V1 construction verification passes', async () => {
    const lifecycle: unknown[] = []
    const releases: unknown[] = []
    const outcome = await handleArtanisWorkDirectionDelivery(delivery(), {
      recordLifecycle: async input => {
        lifecycle.push(input)
      },
      recordTickReceipt: async () => {},
      refundEscrow: async () => {
        throw new Error('passing V1 verdict should not refund')
      },
      releaseEscrow: async input => {
        releases.push(input)
        return {
          ok: true,
          releaseReceiptRef: 'receipt.labor_escrow.release.work_direction_001',
        }
      },
    })

    expect(outcome).toMatchObject({
      kind: 'settled',
      lifecycleKinds: ['delivered', 'accepted', 'settled'],
      releaseReceiptRef: 'receipt.labor_escrow.release.work_direction_001',
    })
    expect(outcome.verificationGate).toMatchObject({
      releaseAllowed: true,
      status: 'accepted',
      verificationClass: 'v1_construction',
    })
    expect(releases).toEqual([
      {
        acceptanceEventRef: 'nostr.event.' + 'a'.repeat(64),
        providerActorRef: 'agent:module-author',
        workRequestId: 'work_request_artanis_program_001',
      },
    ])
    expect(lifecycle).toEqual([
      {
        lifecycleKind: 'delivered',
        receiptRef: 'result.public.artanis.work_direction.program_001',
        workRequestId: 'work_request_artanis_program_001',
      },
      {
        lifecycleKind: 'accepted',
        receiptRef: 'nostr.event.' + 'a'.repeat(64),
        workRequestId: 'work_request_artanis_program_001',
      },
      {
        lifecycleKind: 'settled',
        receiptRef: 'receipt.labor_escrow.release.work_direction_001',
        workRequestId: 'work_request_artanis_program_001',
      },
    ])
  })

  test('refunds program-authorship work when V1 verification fails', async () => {
    const refunds: unknown[] = []
    const outcome = await handleArtanisWorkDirectionDelivery(
      delivery({
        programVerification: v1Verdict({
          constructionVerified: false,
          verificationRef: 'verification.public.tassadar.v1_construction.failed',
        }),
      }),
      {
        recordLifecycle: async () => {},
        recordTickReceipt: async () => {},
        refundEscrow: async input => {
          refunds.push(input)
          return {
            ok: true,
            refundReceiptRef: 'receipt.labor_escrow.refund.work_direction_001',
          }
        },
        releaseEscrow: async () => {
          throw new Error('failing V1 verdict should not release')
        },
      },
    )

    expect(outcome).toMatchObject({
      kind: 'rejected_refunded',
      lifecycleKinds: ['delivered'],
      refundReceiptRef: 'receipt.labor_escrow.refund.work_direction_001',
    })
    expect(outcome.verificationGate.blockerRefs).toContain(
      'blocker.public.artanis.work_direction.v1_construction_failed',
    )
    expect(refunds).toEqual([
      {
        reasonRef:
          'blocker.public.artanis.work_direction.v1_construction_failed',
        workRequestId: 'work_request_artanis_program_001',
      },
    ])
  })

  test('settles dataset-curation work only after V3 data correctness passes', async () => {
    const correctness = await acceptedDataCorrectness()
    const releases: unknown[] = []
    const outcome = await handleArtanisWorkDirectionDelivery(
      delivery({
        dataCorrectnessVerification: correctness,
        directionKind: 'dataset_curation',
        programVerification: undefined,
        providerActorRef: 'agent:dataset-curator',
        resultRef: 'result.public.artanis.work_direction.dataset_001',
        verificationCommandRef:
          'command.public.openagents.data_contribution.v3_correctness',
        workRequestId: 'work_request_artanis_dataset_001',
      }),
      {
        recordLifecycle: async () => {},
        recordTickReceipt: async () => {},
        refundEscrow: async () => {
          throw new Error('passing V3 verdict should not refund')
        },
        releaseEscrow: async input => {
          releases.push(input)
          return {
            ok: true,
            releaseReceiptRef: 'receipt.labor_escrow.release.work_direction_002',
          }
        },
      },
    )

    expect(outcome).toMatchObject({
      kind: 'settled',
      releaseReceiptRef: 'receipt.labor_escrow.release.work_direction_002',
    })
    expect(outcome.verificationGate).toMatchObject({
      releaseAllowed: true,
      status: 'accepted',
      verificationClass: 'v3_data_correctness',
      verificationReceiptRefs: correctness.correctnessReceiptRefs,
    })
    expect(releases).toEqual([
      {
        acceptanceEventRef: 'nostr.event.' + 'a'.repeat(64),
        providerActorRef: 'agent:dataset-curator',
        workRequestId: 'work_request_artanis_dataset_001',
      },
    ])
  })

  test('settles adversarial-verification work only after independent divergence reproduction passes', async () => {
    const releases: unknown[] = []
    const outcome = await handleArtanisWorkDirectionDelivery(
      delivery({
        adversarialVerification: acceptedDivergence(),
        dataCorrectnessVerification: undefined,
        directionKind: 'adversarial_verification',
        programVerification: undefined,
        providerActorRef: 'agent:adversarial-verifier',
        resultRef: 'result.public.artanis.work_direction.adversarial_001',
        verificationCommandRef:
          'command.public.tassadar.e3_adversarial_divergence',
        workRequestId: 'work_request_artanis_adversarial_001',
      }),
      {
        recordLifecycle: async () => {},
        recordTickReceipt: async () => {},
        refundEscrow: async () => {
          throw new Error('passing E3 verdict should not refund')
        },
        releaseEscrow: async input => {
          releases.push(input)
          return {
            ok: true,
            releaseReceiptRef:
              'receipt.labor_escrow.release.work_direction_adversarial_001',
          }
        },
      },
    )

    expect(outcome).toMatchObject({
      kind: 'settled',
      releaseReceiptRef:
        'receipt.labor_escrow.release.work_direction_adversarial_001',
    })
    expect(outcome.verificationGate).toMatchObject({
      releaseAllowed: true,
      status: 'accepted',
      verificationClass: 'e3_adversarial_divergence',
    })
    expect(releases).toEqual([
      {
        acceptanceEventRef: 'nostr.event.' + 'a'.repeat(64),
        providerActorRef: 'agent:adversarial-verifier',
        workRequestId: 'work_request_artanis_adversarial_001',
      },
    ])
  })

  test('refunds adversarial-verification work when the divergence is not reproduced', async () => {
    const refunds: unknown[] = []
    const failed = {
      ...acceptedDivergence(),
      blockerRefs: [
        'blocker.public.tassadar_adversarial.validator_did_not_reproduce',
      ],
      reproducible: false,
      settlementEligible: false,
      status: 'rejected_false_claim' as const,
      verificationReceiptRefs: [],
    }
    const outcome = await handleArtanisWorkDirectionDelivery(
      delivery({
        adversarialVerification: failed,
        dataCorrectnessVerification: undefined,
        directionKind: 'adversarial_verification',
        programVerification: undefined,
        providerActorRef: 'agent:adversarial-verifier',
        resultRef: 'result.public.artanis.work_direction.adversarial_false_001',
        verificationCommandRef:
          'command.public.tassadar.e3_adversarial_divergence',
        workRequestId: 'work_request_artanis_adversarial_false_001',
      }),
      {
        recordLifecycle: async () => {},
        recordTickReceipt: async () => {},
        refundEscrow: async input => {
          refunds.push(input)
          return {
            ok: true,
            refundReceiptRef:
              'receipt.labor_escrow.refund.work_direction_adversarial_001',
          }
        },
        releaseEscrow: async () => {
          throw new Error('false E3 claim should not release')
        },
      },
    )

    expect(outcome).toMatchObject({
      kind: 'rejected_refunded',
      lifecycleKinds: ['delivered'],
      refundReceiptRef:
        'receipt.labor_escrow.refund.work_direction_adversarial_001',
    })
    expect(outcome.verificationGate).toMatchObject({
      releaseAllowed: false,
      status: 'rejected',
      verificationClass: 'e3_adversarial_divergence',
    })
    expect(refunds).toEqual([
      {
        reasonRef:
          'blocker.public.tassadar_adversarial.validator_did_not_reproduce',
        workRequestId: 'work_request_artanis_adversarial_false_001',
      },
    ])
  })

  test('holds dataset-curation work for validator-review V3 remainder', async () => {
    const correctness = await verifyDataContributionCorrectness({
      claimedTraceDigest:
        'sha256:5555555555555555555555555555555555555555555555555555555555555555',
      contributionRef: 'trace.public.artanis.dataset_curate.review_001',
      provenanceRefs: ['provenance.public.reference_lane.prosemirror_001'],
      sourceDigest,
      sourceRefs: ['source.public.projects.prosemirror.manifest'],
      transformRef: 'transform.public.artanis.dataset_trace.v1',
      validatorReviewRefs: ['validator_review.public.artanis.dataset_001'],
      verificationMode: 'validator_review_required',
    })
    const refunds: unknown[] = []
    const outcome = await handleArtanisWorkDirectionDelivery(
      delivery({
        dataCorrectnessVerification: correctness,
        directionKind: 'dataset_curation',
        programVerification: undefined,
        providerActorRef: 'agent:dataset-curator',
        resultRef: 'result.public.artanis.work_direction.dataset_review_001',
        verificationCommandRef:
          'command.public.openagents.data_contribution.v3_correctness',
        workRequestId: 'work_request_artanis_dataset_review_001',
      }),
      {
        recordLifecycle: async () => {},
        recordTickReceipt: async () => {},
        refundEscrow: async input => {
          refunds.push(input)
          return {
            ok: true,
            refundReceiptRef:
              'receipt.labor_escrow.refund.work_direction_review_001',
          }
        },
        releaseEscrow: async () => {
          throw new Error('validator review should not release escrow')
        },
      },
    )

    expect(outcome).toMatchObject({
      kind: 'rejected_refunded',
      lifecycleKinds: ['delivered'],
    })
    expect(outcome.verificationGate).toMatchObject({
      releaseAllowed: false,
      status: 'needs_validator_review',
      validatorReviewRefs: ['validator_review.public.artanis.dataset_001'],
    })
    expect(refunds).toEqual([
      {
        reasonRef: 'blocker.public.data_market.validator_review_required',
        workRequestId: 'work_request_artanis_dataset_review_001',
      },
    ])
  })
})
