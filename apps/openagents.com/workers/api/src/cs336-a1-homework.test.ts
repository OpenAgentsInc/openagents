import { describe, expect, it } from 'vitest'

import {
  type BuyModeAlertRecord,
  type BuyModeCampaignRecord,
  type BuyModeDispatcherStore,
  type BuyModeJobRecord,
  type BuyModeRelayJobRequest,
  type BuyModeRelayPublisher,
  startBuyModeCampaign,
} from './buy-mode-dispatcher'
import {
  type TrainingVerificationChallengeEventRecord,
  type TrainingVerificationChallengeRecord,
  type TrainingVerificationClass,
  type TrainingVerificationStore,
} from './training-verification'
import {
  Cs336A1HomeworkJobKind,
  Cs336A1PsionicLaneRef,
  buildCs336A1HomeworkPayload,
  cs336A1VerificationChallengeRequests,
  dispatchCs336A1HomeworkJob,
  projectCs336A1NoSpendRehearsal,
  recordCs336A1VerificationChallenges,
  verifyCs336A1NoSpendCloseout,
} from './cs336-a1-homework'

const nowIso = '2026-06-10T11:00:00.000Z'

class MemoryBuyModeStore implements BuyModeDispatcherStore {
  readonly alerts: BuyModeAlertRecord[] = []
  readonly campaigns = new Map<string, BuyModeCampaignRecord>()
  readonly jobs = new Map<string, BuyModeJobRecord>()

  latestCampaign = async () =>
    [...this.campaigns.values()].sort((left, right) =>
      right.updatedAt.localeCompare(left.updatedAt)
    )[0] ?? null

  readCampaign = async (campaignId: string) =>
    this.campaigns.get(campaignId) ?? null

  readJobByIdempotencyKeyHash = async (idempotencyKeyHash: string) =>
    [...this.jobs.values()].find(job =>
      job.idempotencyKeyHash === idempotencyKeyHash
    ) ?? null

  readJobByRequestEventId = async (requestEventId: string) =>
    [...this.jobs.values()].find(job => job.requestEventId === requestEventId) ??
    null

  readSettlementByResultEventId = async (resultEventId: string) =>
    [...this.jobs.values()].find(job => job.resultEventId === resultEventId) ??
    null

  recordAlertAndHalt = async (
    campaign: BuyModeCampaignRecord,
    alert: BuyModeAlertRecord,
  ) => {
    this.alerts.push(alert)
    this.campaigns.set(campaign.campaignId, {
      ...campaign,
      lastAlertRef: alert.reasonRef,
      state: 'halted',
      updatedAt: alert.createdAt,
    })
  }

  recordDispatch = async (
    campaign: BuyModeCampaignRecord,
    job: BuyModeJobRecord,
  ) => {
    this.campaigns.set(campaign.campaignId, {
      ...campaign,
      updatedAt: job.updatedAt,
    })
    this.jobs.set(job.jobId, job)
  }

  recordSettlement = async (
    campaign: BuyModeCampaignRecord,
    job: BuyModeJobRecord,
  ) => {
    this.campaigns.set(campaign.campaignId, campaign)
    this.jobs.set(job.jobId, job)
  }

  startCampaign = async (campaign: BuyModeCampaignRecord) => {
    this.campaigns.set(campaign.campaignId, campaign)
  }

  stopCampaign = async (
    campaign: BuyModeCampaignRecord,
    stoppedAt: string,
  ) => {
    this.campaigns.set(campaign.campaignId, {
      ...campaign,
      state: 'disabled',
      updatedAt: stoppedAt,
    })
  }
}

class FakeRelayPublisher implements BuyModeRelayPublisher {
  readonly requests: BuyModeRelayJobRequest[] = []

  publishJobRequest = async (input: BuyModeRelayJobRequest) => {
    this.requests.push(input)

    return {
      accepted: true,
      relayRef: 'relay.public.cs336_a1.test',
      requestEventId: `event.cs336_a1.${this.requests.length}`,
    }
  }
}

class MemoryVerificationStore implements TrainingVerificationStore {
  readonly challenges = new Map<string, TrainingVerificationChallengeRecord>()
  readonly events: TrainingVerificationChallengeEventRecord[] = []

  createChallenge = async (
    challenge: TrainingVerificationChallengeRecord,
    event: TrainingVerificationChallengeEventRecord,
  ) => {
    this.challenges.set(challenge.challengeRef, challenge)
    this.events.push(event)

    return challenge
  }

  leaseChallenge = async (
    challenge: TrainingVerificationChallengeRecord,
    event: TrainingVerificationChallengeEventRecord,
  ) => {
    this.challenges.set(challenge.challengeRef, challenge)
    this.events.push(event)

    return challenge
  }

  listLeaseCandidates = async (
    _nowIso: string,
    _limit: number,
    verificationClass?: TrainingVerificationClass,
  ) =>
    [...this.challenges.values()].filter(
      challenge =>
        ['Queued', 'Retrying'].includes(challenge.state) &&
        (verificationClass === undefined ||
          challenge.verificationClass === verificationClass),
    )

  readChallenge = async (challengeRef: string) =>
    this.challenges.get(challengeRef)

  transitionChallenge = async (
    challenge: TrainingVerificationChallengeRecord,
    event: TrainingVerificationChallengeEventRecord,
  ) => {
    this.challenges.set(challenge.challengeRef, challenge)
    this.events.push(event)

    return challenge
  }
}

describe('CS336 A1 homework job kind', () => {
  it('builds public-safe payloads bound to Psionic demo lane and verifier classes', () => {
    const payload = buildCs336A1HomeworkPayload({
      assignmentRef: 'assignment.cs336_a1.demo.1',
      trainingRunRef: 'training.run.cs336.a1.demo',
      windowRef: 'training.window.cs336.a1.demo',
    })

    expect(payload).toMatchObject({
      jobKind: Cs336A1HomeworkJobKind,
      psionicLaneRef: Cs336A1PsionicLaneRef,
      requestSchemaRef: 'psion.cs336_a1_demo_automatic_execution_request.v1',
    })
    expect(payload.verificationBindings.map(binding => binding.verificationClass)).toEqual([
      'deterministic_recompute',
      'freivalds_merkle',
    ])
    expect(JSON.stringify(payload)).not.toMatch(/lnbc|mnemonic|secret|\/Users\//i)
  })

  it('is a first-class job kind on the Pylon assignment dispatcher rail', async () => {
    const { PylonApiAssignmentJobKind } = await import('./pylon-api')
    const { Schema } = await import('effect')

    expect(
      Schema.decodeUnknownSync(PylonApiAssignmentJobKind)(
        Cs336A1HomeworkJobKind,
      ),
    ).toBe('cs336_a1_homework')
  })

  it('dispatches no-spend A1 homework on the buy-mode rail and verifies closeout evidence', async () => {
    const buyModeStore = new MemoryBuyModeStore()
    const verificationStore = new MemoryVerificationStore()
    const relay = new FakeRelayPublisher()
    let id = 0

    await startBuyModeCampaign(buyModeStore, {
      campaignId: 'campaign.cs336_a1.no_spend',
      dailyCapMsats: 10_000,
      idempotencyKeyHash: 'idempotency.cs336_a1.start',
      nowIso,
      operatorUserId: 'operator.test',
      perJobCapMsats: 2_000,
      relayUrl: 'wss://relay.openagents.test',
      spendEnabled: false,
    })

    const dispatched = await dispatchCs336A1HomeworkJob(buyModeStore, relay, {
      amountMsats: 1000,
      assignmentRef: 'assignment.cs336_a1.demo.1',
      idempotencyKeyHash: 'idempotency.cs336_a1.dispatch',
      jobId: 'job.cs336_a1.demo.1',
      nowIso,
      providerPubkeys: ['11'.repeat(32)],
      trainingRunRef: 'training.run.cs336.a1.demo',
      windowRef: 'training.window.cs336.a1.demo',
    })

    expect(dispatched.buyModeResult.kind).toBe('dispatched')
    expect(relay.requests).toHaveLength(1)
    expect(relay.requests[0]!.content).toContain(Cs336A1HomeworkJobKind)

    const closeout = {
      assignmentRef: 'assignment.cs336_a1.demo.1',
      artifactRefs: ['artifact.cs336_a1.demo.public'],
      checkpointRefs: ['checkpoint.cs336_a1.demo.public'],
      contributionRef: 'contribution.cs336_a1.demo.1',
      metricRefs: ['metric.cs336_a1.loss.public'],
      proofRefs: ['proof.cs336_a1.no_spend.public'],
      tokenizerOutputDigestRef: 'digest.cs336_a1.tokenizer.abc',
      tokenizerRecomputedDigestRef: 'digest.cs336_a1.tokenizer.abc',
      trainingMatrixPayload: {
        challengeVector: [5, 11],
        claimedProductMatrix: [
          [19, 22],
          [43, 50],
        ],
        contributionRefs: ['contribution.cs336_a1.demo.1'],
        expectExactProduct: true,
        leftMatrix: [
          [1, 2],
          [3, 4],
        ],
        merkleProofValid: true,
        rightMatrix: [
          [5, 6],
          [7, 8],
        ],
        rowOpenings: [{ rowCommitmentRef: 'commitment.row.cs336_a1.demo.0' }],
      },
      workerReceiptRef: 'receipt.psionic.cs336_a1.worker.public',
    }
    const requests = cs336A1VerificationChallengeRequests({
      closeout,
      trainingRunRef: 'training.run.cs336.a1.demo',
      windowRef: 'training.window.cs336.a1.demo',
    })

    expect(requests.map(request => request.verificationClass)).toEqual([
      'deterministic_recompute',
      'freivalds_merkle',
    ])

    const challenges = await recordCs336A1VerificationChallenges(
      verificationStore,
      {
        closeout,
        makeId: () => `cs336_a1_${++id}`,
        nowIso,
        trainingRunRef: 'training.run.cs336.a1.demo',
        windowRef: 'training.window.cs336.a1.demo',
      },
    )
    const finalized = await verifyCs336A1NoSpendCloseout(verificationStore, {
      challenges,
      makeId: () => `cs336_a1_${++id}`,
      nowIso,
    })
    const projection = projectCs336A1NoSpendRehearsal({
      assignmentRef: 'assignment.cs336_a1.demo.1',
      buyModeResult: dispatched.buyModeResult,
      closeout,
      finalizedChallenges: finalized,
      trainingRunRef: 'training.run.cs336.a1.demo',
      windowRef: 'training.window.cs336.a1.demo',
    })

    expect(finalized.every(challenge => challenge.state === 'Verified')).toBe(true)
    expect(projection).toMatchObject({
      accepted: true,
      paidSettlementRequired: true,
    })
    expect(projection.blockerRefs).toContain(
      'blocker.cs336_a1.paid_settlement_requires_operator_spend_approval',
    )
    expect(JSON.stringify(projection)).not.toMatch(/lnbc|mnemonic|secret|\/Users\//i)
  })
})
