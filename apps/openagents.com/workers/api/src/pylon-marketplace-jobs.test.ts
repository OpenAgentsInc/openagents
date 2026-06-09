import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  PYLON_MARKETPLACE_JOB_KINDS,
  PYLON_MARKETPLACE_NO_SPEND_AUTHORITY,
  PylonMarketplaceLedgerProjection,
  PylonMarketplaceUnsafe,
  examplePylonMarketplaceLedger,
  projectPylonMarketplaceLedger,
  pylonMarketplaceProjectionHasPrivateMaterial,
} from './pylon-marketplace-jobs'

const nowIso = '2026-06-07T01:20:00.000Z'

describe('Pylon marketplace jobs', () => {
  test('projects seeded and policy-gated marketplace jobs without spend authority', () => {
    const publicProjection = projectPylonMarketplaceLedger(
      examplePylonMarketplaceLedger(),
      'public',
      nowIso,
    )
    const operatorProjection = projectPylonMarketplaceLedger(
      examplePylonMarketplaceLedger(),
      'operator',
      nowIso,
    )

    expect(S.decodeUnknownSync(PylonMarketplaceLedgerProjection)(
      publicProjection,
    )).toEqual(publicProjection)
    expect(publicProjection).toMatchObject({
      assignmentCount: 2,
      authority: PYLON_MARKETPLACE_NO_SPEND_AUTHORITY,
      externalPolicyGatedCount: 1,
      intakeCount: 2,
      updatedAtDisplay: '5 minutes ago',
    })
    expect(publicProjection.intakeRecords.map(intake => intake.source))
      .toEqual(['openagents_seeded', 'external_agent'])
    expect(publicProjection.intakeRecords[1]?.requesterRef)
      .toBe('requester.redacted')
    expect(operatorProjection.intakeRecords[1]?.requesterRef)
      .toBe('requester.private.external_agent_redacted')
    expect(publicProjection.assignmentRecords[0]).toMatchObject({
      acceptedWorkClaimAllowed: true,
      paidAssignmentClaimAllowed: true,
      payoutState: 'accepted_work',
      settlementClaimAllowed: false,
      state: 'accepted',
      stateLabel: 'Accepted',
    })
    expect(publicProjection.assignmentRecords[1]).toMatchObject({
      acceptedWorkClaimAllowed: false,
      paidAssignmentClaimAllowed: false,
      payoutState: 'planned',
      state: 'held_for_authority',
      stateLabel: 'Held for authority',
    })
    expect(pylonMarketplaceProjectionHasPrivateMaterial(publicProjection))
      .toBe(false)
  })

  test('exports the initial marketplace job kind coverage', () => {
    expect(PYLON_MARKETPLACE_JOB_KINDS).toEqual([
      'artifact_review',
      'benchmark_evaluation',
      'embedding_data_prep',
      'gepa_dspy_optimization',
      'inference',
      'lora_finetuning',
      'training',
      'validation',
    ])
  })

  test('requires external jobs to pass policy gates before triage', () => {
    const ledger = examplePylonMarketplaceLedger()
    const external = ledger.intakeRecords[1]!

    expect(() =>
      projectPylonMarketplaceLedger({
        ...ledger,
        intakeRecords: [
          ledger.intakeRecords[0]!,
          {
            ...external,
            policyGateRefs: [],
          },
        ],
      }, 'operator', nowIso),
    ).toThrow(PylonMarketplaceUnsafe)
  })

  test('requires assigned work to carry provider eligibility and Nexus/Pylon authority receipts', () => {
    const ledger = examplePylonMarketplaceLedger()
    const assigned = ledger.assignmentRecords[0]!

    expect(() =>
      projectPylonMarketplaceLedger({
        ...ledger,
        assignmentRecords: [
          {
            ...assigned,
            assignmentAuthorityRefs: [],
            providerEligibilityRefs: [],
            pylonReceiptRefs: [],
          },
        ],
      }, 'operator', nowIso),
    ).toThrow(PylonMarketplaceUnsafe)
  })

  test('keeps accepted-work payout basis tied to Nexus, Treasury, and Pylon receipts', () => {
    const ledger = examplePylonMarketplaceLedger()
    const accepted = ledger.assignmentRecords[0]!

    expect(() =>
      projectPylonMarketplaceLedger({
        ...ledger,
        assignmentRecords: [
          {
            ...accepted,
            pylonReceiptRefs: [],
          },
        ],
      }, 'operator', nowIso),
    ).toThrow(PylonMarketplaceUnsafe)
    expect(() =>
      projectPylonMarketplaceLedger({
        ...ledger,
        assignmentRecords: [
          {
            ...accepted,
            pylonReceiptRefs: ['receipt.public.omega_forum_reward.demo'],
          },
        ],
      }, 'operator', nowIso),
    ).toThrow(PylonMarketplaceUnsafe)
    expect(() =>
      projectPylonMarketplaceLedger({
        ...ledger,
        assignmentRecords: [
          {
            ...accepted,
            acceptedWorkRefs: ['accepted_work.public.generic_job_creation'],
          },
        ],
      }, 'operator', nowIso),
    ).toThrow(PylonMarketplaceUnsafe)
  })

  test('rejects private data, raw artifacts, provider tokens, runner logs, wallet material, payment material, and timestamps', () => {
    const ledger = examplePylonMarketplaceLedger()
    const intake = ledger.intakeRecords[0]!
    const assignment = ledger.assignmentRecords[0]!

    for (const unsafeLedger of [
      {
        ...ledger,
        intakeRecords: [
          {
            ...intake,
            dataRefs: ['dataset.raw.customer_payload'],
          },
        ],
      },
      {
        ...ledger,
        intakeRecords: [
          {
            ...intake,
            modelRefs: ['model.raw.weights.safetensors'],
          },
        ],
      },
      {
        ...ledger,
        assignmentRecords: [
          {
            ...assignment,
            artifactEvidenceRefs: ['raw_artifact.training_output'],
          },
        ],
      },
      {
        ...ledger,
        assignmentRecords: [
          {
            ...assignment,
            providerRefs: ['provider_token.local_pylon'],
          },
        ],
      },
      {
        ...ledger,
        assignmentRecords: [
          {
            ...assignment,
            resultEvidenceRefs: ['raw_runner_log.training_loop'],
          },
        ],
      },
      {
        ...ledger,
        assignmentRecords: [
          {
            ...assignment,
            treasuryReceiptRefs: ['wallet.secret.seed'],
          },
        ],
      },
      {
        ...ledger,
        assignmentRecords: [
          {
            ...assignment,
            nexusReceiptRefs: ['invoice.lnbc123'],
          },
        ],
      },
      {
        ...ledger,
        intakeRecords: [
          {
            ...intake,
            sourceRefs: ['source.public.2026-06-07T01:00:00.000Z'],
          },
        ],
      },
    ]) {
      expect(() =>
        projectPylonMarketplaceLedger(unsafeLedger, 'operator', nowIso),
      ).toThrow(PylonMarketplaceUnsafe)
    }
  })
})
