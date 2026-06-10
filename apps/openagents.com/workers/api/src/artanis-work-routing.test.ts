import { describe, expect, test } from 'vitest'

import {
  ARTANIS_WORK_ROUTING_CAPABILITIES,
  ARTANIS_WORK_ROUTING_NO_DIRECT_AUTHORITY,
  ARTANIS_WORK_ROUTING_WORK_CLASSES,
  ArtanisWorkRoutingLedgerRecord,
  ArtanisWorkRoutingProposalRecord,
  ArtanisWorkRoutingUnsafe,
  artanisWorkRoutingProjectionHasPrivateMaterial,
  exampleArtanisWorkRoutingLedger,
  projectArtanisWorkRoutingLedger,
} from './artanis-work-routing'

const nowIso = '2026-06-07T04:30:00.000Z'

const ledgerWithProposal = (
  proposal: ArtanisWorkRoutingProposalRecord,
): ArtanisWorkRoutingLedgerRecord =>
  new ArtanisWorkRoutingLedgerRecord({
    ...exampleArtanisWorkRoutingLedger,
    proposals: [proposal],
  })

const acceptedProposal =
  exampleArtanisWorkRoutingLedger.proposals.find(
    proposal => proposal.state === 'accepted',
  )!

const approvalRequiredProposal =
  exampleArtanisWorkRoutingLedger.proposals.find(
    proposal => proposal.risk === 'approval_required',
  )!

describe('Artanis work routing', () => {
  test('projects bounded work proposals with capabilities, risk, resource modes, and no direct authority', () => {
    const operator = projectArtanisWorkRoutingLedger(
      exampleArtanisWorkRoutingLedger,
      'operator',
      nowIso,
    )
    const publicArtanis = projectArtanisWorkRoutingLedger(
      exampleArtanisWorkRoutingLedger,
      'public_artanis',
      nowIso,
    )
    const publicForum = projectArtanisWorkRoutingLedger(
      exampleArtanisWorkRoutingLedger,
      'public_forum',
      nowIso,
    )

    expect(operator.authority).toEqual(ARTANIS_WORK_ROUTING_NO_DIRECT_AUTHORITY)
    expect(operator.proposalCount).toBe(5)
    expect(operator.proposals.map(proposal => proposal.workClass)).toEqual([
      'benchmark_evaluation',
      'inference',
      'executor_trace_validation',
      'lora_finetuning',
      'embedding_data_prep',
    ])
    expect(operator.proposals.map(proposal => proposal.capability)).toEqual([
      'benchmark_evaluation',
      'inference',
      'executor_trace_validation',
      'lora_finetuning',
      'embedding_data_prep',
    ])
    expect(operator.riskyProposalRefs).toEqual([
      'work.public.artanis.benchmark_eval_proposed',
    ])
    expect(operator.traceableWorkRefs).toEqual([
      'assignment.public.artanis.tassadar_executor_trace.template',
      'work.public.pylon.inference.trace_001',
    ])
    expect(operator.proposals.some(
      proposal => proposal.operatorDetailRefs.length > 0,
    )).toBe(true)
    expect(publicArtanis.proposals.every(
      proposal => proposal.operatorDetailRefs.length === 0,
    )).toBe(true)
    expect(publicArtanis.traceableWorkRefs).toEqual([
      'assignment.public.artanis.tassadar_executor_trace.template',
      'work.public.pylon.inference.trace_001',
    ])
    expect(publicArtanis.proposals.find(
      proposal => proposal.state === 'accepted',
    )).toMatchObject({
      receiptRefs: ['receipt.public.artanis.pylon_inference_accepted'],
      traceableWorkRefs: ['work.public.pylon.inference.trace_001'],
    })
    expect(publicArtanis.proposals.find(
      proposal => proposal.workClass === 'executor_trace_validation',
    )).toMatchObject({
      receiptRefs: [
        'receipt.public.artanis.tassadar_executor_trace.dispatch_ready',
      ],
      risk: 'safe_read_only',
      state: 'dispatched',
      target: 'pylon',
      traceableWorkRefs: [
        'assignment.public.artanis.tassadar_executor_trace.template',
      ],
    })
    expect(publicArtanis.proposals.find(
      proposal => proposal.state === 'blocked',
    )).toMatchObject({
      blockerRefs: ['blocker.public.no_training_spend_authority'],
      publicCaveatRefs: ['caveat.public.training_blocked_without_approval'],
    })
    expect(artanisWorkRoutingProjectionHasPrivateMaterial(publicArtanis))
      .toBe(false)
    expect(artanisWorkRoutingProjectionHasPrivateMaterial(publicForum))
      .toBe(false)
    expect(JSON.stringify(publicArtanis)).not.toContain('operator.artanis')
    expect(JSON.stringify(publicArtanis)).not.toMatch(
      /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/,
    )
  })

  test('exports the covered work classes and target capabilities', () => {
    expect(ARTANIS_WORK_ROUTING_WORK_CLASSES).toEqual([
      'benchmark_evaluation',
      'embedding_data_prep',
      'executor_trace_validation',
      'gepa_dspy_optimization',
      'inference',
      'lora_finetuning',
      'training',
      'validation',
    ])
    expect(ARTANIS_WORK_ROUTING_CAPABILITIES).toEqual([
      'artifact_validation',
      'benchmark_evaluation',
      'coding_runtime_probe',
      'embedding_data_prep',
      'executor_trace_validation',
      'gepa_dspy_optimization',
      'inference',
      'lora_finetuning',
      'model_lab_evidence_review',
      'nexus_assignment',
      'pylon_training',
      'psionic_adapter_validation',
    ])
  })

  test('requires source evidence, target capability, approval, cost, and accepted-work receipts where applicable', () => {
    const missingEvidence = new ArtanisWorkRoutingProposalRecord({
      ...acceptedProposal,
      proposalRef: 'work.public.artanis.missing_evidence',
      sourceEvidenceRefs: [],
    })
    const missingCapability = new ArtanisWorkRoutingProposalRecord({
      ...acceptedProposal,
      proposalRef: 'work.public.artanis.missing_capability',
      targetCapabilityRefs: [],
    })
    const missingApproval = new ArtanisWorkRoutingProposalRecord({
      ...approvalRequiredProposal,
      approvalRequirementRefs: [],
      costCaveatRefs: [],
      proposalRef: 'work.public.artanis.missing_approval',
      spendLimitRefs: [],
    })
    const acceptedWithoutReceipts = new ArtanisWorkRoutingProposalRecord({
      ...acceptedProposal,
      proposalRef: 'work.public.artanis.accepted_without_receipts',
      receiptRefs: [],
      traceableWorkRefs: [],
    })

    expect(() =>
      projectArtanisWorkRoutingLedger(ledgerWithProposal(missingEvidence), 'operator', nowIso),
    ).toThrow(ArtanisWorkRoutingUnsafe)
    expect(() =>
      projectArtanisWorkRoutingLedger(ledgerWithProposal(missingCapability), 'operator', nowIso),
    ).toThrow(ArtanisWorkRoutingUnsafe)
    expect(() =>
      projectArtanisWorkRoutingLedger(ledgerWithProposal(missingApproval), 'operator', nowIso),
    ).toThrow(ArtanisWorkRoutingUnsafe)
    expect(() =>
      projectArtanisWorkRoutingLedger(ledgerWithProposal(acceptedWithoutReceipts), 'operator', nowIso),
    ).toThrow(ArtanisWorkRoutingUnsafe)
  })

  test('rejects direct authority and unsafe refs', () => {
    const directAuthority = new ArtanisWorkRoutingLedgerRecord({
      ...exampleArtanisWorkRoutingLedger,
      authority: {
        ...ARTANIS_WORK_ROUTING_NO_DIRECT_AUTHORITY,
        dispatchAllowed: true,
      },
    })
    const unsafeRef = new ArtanisWorkRoutingProposalRecord({
      ...acceptedProposal,
      proposalRef: 'work.public.artanis.unsafe',
      sourceEvidenceRefs: ['provider_secret.raw'],
    })

    expect(() =>
      projectArtanisWorkRoutingLedger(directAuthority, 'operator', nowIso),
    ).toThrow(ArtanisWorkRoutingUnsafe)
    expect(() =>
      projectArtanisWorkRoutingLedger(ledgerWithProposal(unsafeRef), 'operator', nowIso),
    ).toThrow(ArtanisWorkRoutingUnsafe)
  })
})
