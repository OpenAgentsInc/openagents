import { describe, expect, it } from 'vitest'

import {
  buildPylonApiAssignmentRecord,
  publicPylonApiAssignmentProjection,
} from './pylon-api'
import {
  buildTrainingValidatorAssignmentRequest,
  projectTrainingValidatorConsensus,
} from './training-validator-assignments'
import { buildTrainingVerificationChallengeRecord } from './training-verification'

const nowIso = '2026-06-10T12:00:00.000Z'

const buildChallenge = (contributionRef = 'contribution.training.worker_one') =>
  buildTrainingVerificationChallengeRecord({
    makeId: () => 'validator_assignment',
    nowIso,
    request: {
      commitmentRefs: ['commitment.training.public.1'],
      contributionRef,
      homeworkKind: 'cs336_a1_homework',
      payload: {
        contributionRefs: [contributionRef],
        expectedDigestRef: 'digest.training.expected',
        recomputedDigestRef: 'digest.training.expected',
      },
      trainingRunRef: 'training.run.cs336.a1.demo',
      verificationClass: 'deterministic_recompute',
      windowRef: 'window.training.public.1',
    },
  }).challenge

describe('training validator assignment bridge', () => {
  it('builds validation assignments on the Pylon rail without spend settlement', () => {
    const result = buildTrainingValidatorAssignmentRequest({
      challenge: buildChallenge(),
      nowIso,
      validatorPylonRef: 'pylon.validator_one',
      workerPylonRef: 'pylon.worker_one',
    })

    expect(result.kind).toBe('assignment_request')
    if (result.kind !== 'assignment_request') {
      return
    }

    expect(result.assignmentRequest).toMatchObject({
      campaignPaused: false,
      campaignRef: 'campaign.public.training_validator.cs336',
      forumAutoPublishAllowed: false,
      jobKind: 'validation',
      paymentMode: 'payable_pending_settlement',
      pylonRef: 'pylon.validator_one',
      requiredCapabilityRefs: ['capability.public.training_verification'],
      selectionPolicyRefs: [
        'selection.public.training_validator.dispatcher_controlled',
      ],
      spendCapRefs: [
        'spendcap.public.training_validator.small_sats_operator_required',
      ],
    })
    expect(result.assignmentRequest.codingAssignment).toMatchObject({
      homeworkKind: 'cs336_a1_homework',
      trainingRunRef: 'training.run.cs336.a1.demo',
      verificationClass: 'deterministic_recompute',
      workerPylonRef: 'pylon.worker_one',
    })
    expect(result.paymentBlockedRefs).toEqual([
      'blocker.training_validator.operator_spend_approval_required',
    ])
  })

  it('accepts the public challenge projection as the bridge challenge summary', () => {
    // The live dispatch script feeds the projection returned by
    // GET /api/training/verification/challenges/:challengeRef into the
    // bridge; this pins the summary-field contract it relies on.
    const result = buildTrainingValidatorAssignmentRequest({
      challenge: {
        challengeRef: 'training.verification.challenge.projection_summary',
        contributionRef: 'contribution.training.worker_one',
        homeworkKind: 'validator_recheck',
        samplingPolicy: 'per_contribution',
        trainingRunRef: 'run.cs336.a1.demo',
        verificationClass: 'freivalds_merkle',
        windowRef: 'training.window.cs336_a1.demo.20260611.w1',
      },
      nowIso,
      validatorPylonRef: 'pylon.validator_one',
      workerPylonRef: 'pylon.worker_one',
    })

    expect(result.kind).toBe('assignment_request')
    if (result.kind !== 'assignment_request') {
      return
    }

    expect(result.assignmentRequest.codingAssignment).toMatchObject({
      challengeRef: 'training.verification.challenge.projection_summary',
      homeworkKind: 'validator_recheck',
      verificationClass: 'freivalds_merkle',
      workerPylonRef: 'pylon.worker_one',
    })
  })

  it('makes self-validation structurally impossible', () => {
    const samePylon = buildTrainingValidatorAssignmentRequest({
      challenge: buildChallenge(),
      nowIso,
      validatorPylonRef: 'pylon.worker_one',
      workerPylonRef: 'pylon.worker_one',
    })
    expect(samePylon).toEqual({
      blockerRefs: ['blocker.training_validator.self_validation'],
      kind: 'blocked',
    })

    const contributionOwner = buildTrainingValidatorAssignmentRequest({
      challenge: buildChallenge('contribution.training.pylon.validator_one.1'),
      nowIso,
      validatorPylonRef: 'pylon.validator_one',
      workerPylonRef: 'pylon.worker_one',
    })
    expect(contributionOwner).toEqual({
      blockerRefs: ['blocker.training_validator.validator_owns_contribution'],
      kind: 'blocked',
    })
  })

  it('projects assignment requests through the existing public Pylon assignment projection', () => {
    const result = buildTrainingValidatorAssignmentRequest({
      challenge: buildChallenge(),
      nowIso,
      validatorPylonRef: 'pylon.validator_one',
      workerPylonRef: 'pylon.worker_one',
    })
    expect(result.kind).toBe('assignment_request')
    if (result.kind !== 'assignment_request') {
      return
    }

    const assignment = buildPylonApiAssignmentRecord({
      idempotencyKeyHash: 'hash.training.validator.assignment',
      makeId: () => 'validator_assignment',
      nowIso,
      ownerAgentUserId: 'dispatcher',
      request: result.assignmentRequest,
    })
    const projection = publicPylonApiAssignmentProjection(assignment, nowIso)

    expect(projection).toMatchObject({
      assignmentRef: result.assignmentRequest.assignmentRef,
      jobKind: 'validation',
      leaseExpiresInSeconds: 900,
      leaseState: 'active',
      pylonRef: 'pylon.validator_one',
      state: 'offered',
    })
    expect(projection.codingAssignment).toMatchObject({
      workerPylonRef: 'pylon.worker_one',
    })
  })

  it('requires two distinct validators before Freivalds rejection consensus', () => {
    const pending = projectTrainingValidatorConsensus({
      challengeRef: 'training.verification.challenge.freivalds',
      verificationClass: 'freivalds_merkle',
      verdicts: [
        {
          challengeRef: 'training.verification.challenge.freivalds',
          failureCodes: ['FreivaldsMismatch'],
          state: 'Rejected',
          validatorPylonRef: 'pylon.validator_one',
          verdictRef: 'verdict.training.freivalds.1',
          verificationClass: 'freivalds_merkle',
        },
        {
          challengeRef: 'training.verification.challenge.freivalds',
          failureCodes: ['FreivaldsMismatch'],
          state: 'Rejected',
          validatorPylonRef: 'pylon.validator_one',
          verdictRef: 'verdict.training.freivalds.duplicate',
          verificationClass: 'freivalds_merkle',
        },
      ],
    })
    expect(pending).toMatchObject({
      accepted: false,
      blockerRefs: ['blocker.training_validator.quorum_pending'],
      requiredValidatorCount: 2,
      state: 'quorum_pending',
    })

    const rejected = projectTrainingValidatorConsensus({
      challengeRef: 'training.verification.challenge.freivalds',
      verificationClass: 'freivalds_merkle',
      verdicts: [
        ...pending.verdictRefs.map((verdictRef, index) => ({
          challengeRef: 'training.verification.challenge.freivalds',
          failureCodes: ['FreivaldsMismatch' as const],
          state: 'Rejected' as const,
          validatorPylonRef: `pylon.validator_${index + 1}`,
          verdictRef,
          verificationClass: 'freivalds_merkle' as const,
        })),
      ],
    })
    expect(rejected).toMatchObject({
      accepted: true,
      blockerRefs: [],
      requiredValidatorCount: 2,
      state: 'consensus_rejected',
    })
  })
})
