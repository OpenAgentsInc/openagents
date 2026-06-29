import { Schema as S } from 'effect'

import {
  CodingAutopilotContinuationDecisionRecord,
  codingAutopilotContinuationDecisionRecordFromBlueprint,
} from './coding-autopilot-continuation-decisions'
import { CodingAutopilotMissionRecord } from './coding-autopilot-missions'
import {
  classifyProviderAccountFailover,
  type ProviderAccountFailoverFailureClass,
} from './provider-account-failover-policy'
import {
  PROVIDER_ACCOUNT_LEASE_POLICY_VERSION,
  type ProviderAccountLeaseCandidate,
  selectProviderAccountLeaseCandidate,
} from './provider-account-lease-policy'
import { isoTimestampAfterIso } from './runtime-primitives'

export const AUTOPILOT_ROTATION_SMOKE_VERSION =
  'autopilot-rate-limit-rotation-smoke:v1' as const

const TURN_DURATION_MILLISECONDS = 60_000

export class AutopilotRotationSmokeUnsafe extends S.TaggedErrorClass<AutopilotRotationSmokeUnsafe>()(
  'AutopilotRotationSmokeUnsafe',
  {
    reason: S.String,
  },
) {}

export class AutopilotRotationSmokeTurn extends S.Class<AutopilotRotationSmokeTurn>(
  'AutopilotRotationSmokeTurn',
)({
  accountSlug: S.String,
  artifactRef: S.String,
  buildsOnArtifactRef: S.NullOr(S.String),
  contextFingerprintAfter: S.String,
  contextFingerprintBefore: S.String,
  leaseRef: S.String,
  missionRef: S.String,
  resumedAfterRotation: S.Boolean,
  turnRef: S.String,
}) {}

export class AutopilotRotationSmokeRotation extends S.Class<AutopilotRotationSmokeRotation>(
  'AutopilotRotationSmokeRotation',
)({
  accountStateAction: S.String,
  cooldownUntil: S.NullOr(S.String),
  customerSafeStatus: S.String,
  failureClass: S.String,
  inducedAtTurnRef: S.String,
  nextAccountSlug: S.NullOr(S.String),
  nextLeaseRef: S.NullOr(S.String),
  outcome: S.Literals(['retrying', 'blocked']),
  policyVersion: S.Literal(PROVIDER_ACCOUNT_LEASE_POLICY_VERSION),
  previousAccountSlug: S.String,
  previousLeaseRef: S.String,
  receiptRef: S.String,
  selectionReason: S.String,
}) {}

export class AutopilotRateLimitRotationSmokeReceipt extends S.Class<AutopilotRateLimitRotationSmokeReceipt>(
  'AutopilotRateLimitRotationSmokeReceipt',
)({
  blockedReason: S.NullOr(S.String),
  continuationDecision: S.NullOr(CodingAutopilotContinuationDecisionRecord),
  generatedAt: S.String,
  mission: CodingAutopilotMissionRecord,
  outcome: S.Literals(['completed', 'blocked']),
  rotation: S.NullOr(AutopilotRotationSmokeRotation),
  smokeVersion: S.Literal(AUTOPILOT_ROTATION_SMOKE_VERSION),
  turns: S.Array(AutopilotRotationSmokeTurn),
}) {}

export type AutopilotRotationSmokeInducedFailure = Readonly<{
  atTurnRef: string
  failureClass: ProviderAccountFailoverFailureClass
}>

export type AutopilotRotationSmokeInput = Readonly<{
  fleet: ReadonlyArray<ProviderAccountLeaseCandidate>
  inducedFailure: AutopilotRotationSmokeInducedFailure | null
  missionSlug: string
  plannedTurnRefs: ReadonlyArray<string>
  startedAtIso: string
  workroomRef: string
}>

const safeSlugPattern = /^[a-z0-9][a-z0-9_]{0,80}$/

export const rotationContextFingerprint = (
  prior: string,
  turnRef: string,
): string => {
  const text = `${prior}|${turnRef}`
  const hash = [...text].reduce(
    (state, character) =>
      Math.imul(state ^ character.charCodeAt(0), 16_777_619) >>> 0,
    2_166_136_261,
  )

  return `ctx.${hash.toString(16).padStart(8, '0')}`
}

const evidenceRefByFailureClass: Record<
  ProviderAccountFailoverFailureClass,
  string
> = {
  grant_resolution_failed: 'evidence.account_grant_resolution_failed',
  launch_timeout: 'evidence.account_launch_timeout',
  low_credits: 'evidence.account_low_credits',
  provider_outage: 'evidence.account_outage',
  quota_exhausted: 'evidence.account_quota_exhausted',
  rate_limited: 'evidence.account_rate_limit',
  runner_failure: 'evidence.account_runner_failure',
  token_invalidated: 'evidence.account_reauth_required',
  unknown_provider_failure: 'evidence.account_unknown_failure',
}

const accountSlugForCandidate = (
  candidate: ProviderAccountLeaseCandidate,
): string => {
  const slug = candidate.providerAccountRef.replace(
    /^provider-account_ref_/,
    '',
  )

  if (!safeSlugPattern.test(slug)) {
    throw new AutopilotRotationSmokeUnsafe({
      reason:
        'Fleet account refs must reduce to a lowercase safe slug for mission-safe lease refs.',
    })
  }

  return slug
}

const assertSafeSlug = (label: string, slug: string): void => {
  if (!safeSlugPattern.test(slug)) {
    throw new AutopilotRotationSmokeUnsafe({
      reason: `${label} must be a lowercase safe slug.`,
    })
  }
}

type ActiveLease = Readonly<{
  accountSlug: string
  leaseRef: string
  providerAccountRef: string
}>

const leaseRefForOrdinal = (missionSlug: string, ordinal: number): string =>
  `account_lease.${missionSlug}.lease_${ordinal}`

const applyFailureToCandidate = (
  candidate: ProviderAccountLeaseCandidate,
  failureClass: ProviderAccountFailoverFailureClass,
  now: string,
): ProviderAccountLeaseCandidate => {
  const action = classifyProviderAccountFailover(failureClass, now)

  return {
    ...candidate,
    cooldownUntil: action.cooldownUntil,
    health:
      action.health === null || action.health === 'healthy'
        ? candidate.health
        : action.health === 'requires_reauth'
          ? 'requires_reauth'
          : 'unhealthy',
    lowCredit: action.lowCredit || candidate.lowCredit,
    recentFailureClass: action.recentFailureClass,
  }
}

const retryAccountContinuationDecision = (
  input: Readonly<{
    failureClass: ProviderAccountFailoverFailureClass
    interruptedTurnRef: string
    missionRef: string
    missionSlug: string
    receiptRef: string
    updatedAtIso: string
    workroomRef: string
  }>,
): CodingAutopilotContinuationDecisionRecord =>
  codingAutopilotContinuationDecisionRecordFromBlueprint({
    customerExplanationRef: 'explanation.continuation.retry_account',
    decision: {
      action: 'retry_account',
      actionSubmissionRequiredForDirectEffects: true,
      authorityBoundary: 'evidence_only',
      confidence: 0.85,
      constraintRefs: ['constraint.use_connected_account_fleet'],
      decisionRef: `continuation_decision.${input.missionSlug}.retry_account`,
      directMutationDisabled: true,
      evidenceRefs: [evidenceRefByFailureClass[input.failureClass]],
      forbiddenDirectEffects: [
        'create_pull_request',
        'deploy',
        'mutate_source_fact',
        'send_email',
        'spend_money',
        'upgrade_public_claim',
      ],
      moduleVersionId: null,
      noDeploy: true,
      noEmail: true,
      noPublicClaimUpgrade: true,
      noSourceMutation: true,
      noSpend: true,
      programSignatureId: 'program_signature.autopilot.retry_account.v1',
      programTypeId: 'program_type.autopilot.continuation.v1',
      reason:
        'Provider account capacity failed mid-mission and the lease policy selected another connected account.',
      receiptRefs: [input.receiptRef],
      sourceAuthorityRefs: ['source_authority.account_fleet_health'],
      turnResultRef: `turn_result.${input.missionSlug}.${input.interruptedTurnRef}.interrupted`,
      workRef: input.missionRef,
    },
    guardrailState: 'needs_action_submission',
    id: `continuation_decision_record_${input.missionSlug}_retry_account`,
    missionRef: input.missionRef,
    programRunRef: `program_run.continuation.${input.missionSlug}`,
    rejectedAlternativeRefs: [
      'rejected.continue_without_account_retry',
      'rejected.stop_without_accepted_outcome',
    ],
    riskRefs: ['risk.account_rotation_needed'],
    updatedAtIso: input.updatedAtIso,
    workroomRefs: [input.workroomRef],
  })

const exhaustedFleetContinuationDecision = (
  input: Readonly<{
    failureClass: ProviderAccountFailoverFailureClass
    interruptedTurnRef: string
    missionRef: string
    missionSlug: string
    receiptRef: string
    updatedAtIso: string
    workroomRef: string
  }>,
): CodingAutopilotContinuationDecisionRecord =>
  codingAutopilotContinuationDecisionRecordFromBlueprint({
    customerExplanationRef: 'explanation.continuation.account_fleet_exhausted',
    decision: {
      action: 'escalate',
      actionSubmissionRequiredForDirectEffects: true,
      authorityBoundary: 'evidence_only',
      confidence: 0.9,
      constraintRefs: ['constraint.use_connected_account_fleet'],
      decisionRef: `continuation_decision.${input.missionSlug}.escalate_account_fleet_exhausted`,
      directMutationDisabled: true,
      evidenceRefs: [evidenceRefByFailureClass[input.failureClass]],
      forbiddenDirectEffects: [
        'create_pull_request',
        'deploy',
        'mutate_source_fact',
        'send_email',
        'spend_money',
        'upgrade_public_claim',
      ],
      moduleVersionId: null,
      noDeploy: true,
      noEmail: true,
      noPublicClaimUpgrade: true,
      noSourceMutation: true,
      noSpend: true,
      programSignatureId: 'program_signature.autopilot.escalate.v1',
      programTypeId: 'program_type.autopilot.continuation.v1',
      reason:
        'No eligible second account is available, so the mission is blocked waiting for operator input instead of stalling silently.',
      receiptRefs: [input.receiptRef],
      sourceAuthorityRefs: ['source_authority.account_fleet_health'],
      turnResultRef: `turn_result.${input.missionSlug}.${input.interruptedTurnRef}.blocked`,
      workRef: input.missionRef,
    },
    guardrailState: 'blocked',
    id: `continuation_decision_record_${input.missionSlug}_escalate`,
    missionRef: input.missionRef,
    programRunRef: `program_run.continuation.${input.missionSlug}`,
    rejectedAlternativeRefs: ['rejected.silent_stall_without_typed_blocker'],
    riskRefs: [
      'risk.account_fleet_exhausted',
      'risk.account_rotation_needed',
    ],
    updatedAtIso: input.updatedAtIso,
    workroomRefs: [input.workroomRef],
  })

const missionRecord = (
  input: Readonly<{
    accountLeaseRefs: ReadonlyArray<string>
    artifactRefs: ReadonlyArray<string>
    blockerRefs: ReadonlyArray<string>
    createdAtIso: string
    missionRef: string
    missionSlug: string
    status: 'blocked' | 'delivered'
    updatedAtIso: string
    workroomRef: string
  }>,
): CodingAutopilotMissionRecord => ({
  accountLeaseRefs: input.accountLeaseRefs,
  artifactRefs: input.artifactRefs,
  assignmentRefs: [`assignment.rotation_smoke.${input.missionSlug}`],
  blockerRefs: input.blockerRefs,
  budgetRefs: [`budget.no_spend_smoke.${input.missionSlug}`],
  createdAtIso: input.createdAtIso,
  customerRefs: [],
  id: `mission_${input.missionSlug}`,
  latestBriefingRef: null,
  missionRef: input.missionRef,
  nextOrderRefs: [],
  objectiveStackRefs: ['objective.prove_rotation_context_continuity'],
  ownerRefs: ['owner_ref.openagents_operator'],
  routeScorecardRefs: [],
  status: input.status,
  teamRefs: [],
  updatedAtIso: input.updatedAtIso,
  workKind: 'coding',
  workroomRefs: [input.workroomRef],
})

export const runAutopilotRateLimitRotationSmoke = (
  input: AutopilotRotationSmokeInput,
): AutopilotRateLimitRotationSmokeReceipt => {
  assertSafeSlug('missionSlug', input.missionSlug)
  input.plannedTurnRefs.forEach(turnRef =>
    assertSafeSlug('plannedTurnRefs entries', turnRef),
  )

  const missionRef = `mission.${input.missionSlug}`
  const startedAtIso = input.startedAtIso

  const initialSelection = selectProviderAccountLeaseCandidate(
    input.fleet,
    startedAtIso,
  )

  if (initialSelection.status === 'none') {
    throw new AutopilotRotationSmokeUnsafe({
      reason:
        'Rotation smoke requires at least one eligible account for the initial lease.',
    })
  }

  const initialLease: ActiveLease = {
    accountSlug: accountSlugForCandidate(initialSelection.candidate),
    leaseRef: leaseRefForOrdinal(input.missionSlug, 1),
    providerAccountRef: initialSelection.candidate.providerAccountRef,
  }

  const finished = input.plannedTurnRefs.reduce<
    Readonly<{
      activeLease: ActiveLease
      blockedReason: string | null
      continuationDecision: CodingAutopilotContinuationDecisionRecord | null
      fingerprint: string
      fleet: ReadonlyArray<ProviderAccountLeaseCandidate>
      leaseRefs: ReadonlyArray<string>
      nowIso: string
      rotation: AutopilotRotationSmokeRotation | null
      turns: ReadonlyArray<AutopilotRotationSmokeTurn>
    }>
  >(
    (state, plannedTurnRef) => {
      if (state.blockedReason !== null) {
        return state
      }

      const inducedHere =
        input.inducedFailure !== null &&
        input.inducedFailure.atTurnRef === plannedTurnRef &&
        state.rotation === null

      const afterFault = ((): typeof state => {
        if (!inducedHere || input.inducedFailure === null) {
          return state
        }

        const failureClass = input.inducedFailure.failureClass
        const action = classifyProviderAccountFailover(
          failureClass,
          state.nowIso,
        )
        const receiptRef = `receipt.account_failover.${input.missionSlug}`
        const remainingFleet = state.fleet.map(candidate =>
          candidate.providerAccountRef ===
            state.activeLease.providerAccountRef
            ? applyFailureToCandidate(candidate, failureClass, state.nowIso)
            : candidate,
        )
        const selection = selectProviderAccountLeaseCandidate(
          remainingFleet,
          state.nowIso,
        )

        if (selection.status === 'none' || !action.retryAllowed) {
          const rotation: AutopilotRotationSmokeRotation = {
            accountStateAction: action.accountStateAction,
            cooldownUntil: action.cooldownUntil,
            customerSafeStatus:
              'Work is blocked until another eligible account is available.',
            failureClass,
            inducedAtTurnRef: plannedTurnRef,
            nextAccountSlug: null,
            nextLeaseRef: null,
            outcome: 'blocked',
            policyVersion: PROVIDER_ACCOUNT_LEASE_POLICY_VERSION,
            previousAccountSlug: state.activeLease.accountSlug,
            previousLeaseRef: state.activeLease.leaseRef,
            receiptRef,
            selectionReason:
              selection.status === 'none'
                ? selection.reason
                : 'Failure class does not permit retrying on another account.',
          }

          return {
            ...state,
            blockedReason:
              'No eligible second account was available after the induced provider limit.',
            continuationDecision: exhaustedFleetContinuationDecision({
              failureClass,
              interruptedTurnRef: plannedTurnRef,
              missionRef,
              missionSlug: input.missionSlug,
              receiptRef,
              updatedAtIso: state.nowIso,
              workroomRef: input.workroomRef,
            }),
            fleet: remainingFleet,
            rotation,
          }
        }

        const nextLease: ActiveLease = {
          accountSlug: accountSlugForCandidate(selection.candidate),
          leaseRef: leaseRefForOrdinal(
            input.missionSlug,
            state.leaseRefs.length + 1,
          ),
          providerAccountRef: selection.candidate.providerAccountRef,
        }
        const rotation: AutopilotRotationSmokeRotation = {
          accountStateAction: action.accountStateAction,
          cooldownUntil: action.cooldownUntil,
          customerSafeStatus: action.customerSafeStatus,
          failureClass,
          inducedAtTurnRef: plannedTurnRef,
          nextAccountSlug: nextLease.accountSlug,
          nextLeaseRef: nextLease.leaseRef,
          outcome: 'retrying',
          policyVersion: PROVIDER_ACCOUNT_LEASE_POLICY_VERSION,
          previousAccountSlug: state.activeLease.accountSlug,
          previousLeaseRef: state.activeLease.leaseRef,
          receiptRef,
          selectionReason: selection.reason,
        }

        return {
          ...state,
          activeLease: nextLease,
          continuationDecision: retryAccountContinuationDecision({
            failureClass,
            interruptedTurnRef: plannedTurnRef,
            missionRef,
            missionSlug: input.missionSlug,
            receiptRef,
            updatedAtIso: state.nowIso,
            workroomRef: input.workroomRef,
          }),
          fleet: remainingFleet,
          leaseRefs: [...state.leaseRefs, nextLease.leaseRef],
          nowIso: isoTimestampAfterIso(
            state.nowIso,
            TURN_DURATION_MILLISECONDS,
          ),
          rotation,
        }
      })()

      if (afterFault.blockedReason !== null) {
        return afterFault
      }

      const previousTurn = afterFault.turns[afterFault.turns.length - 1]
      const contextFingerprintBefore = afterFault.fingerprint
      const turn: AutopilotRotationSmokeTurn = {
        accountSlug: afterFault.activeLease.accountSlug,
        artifactRef: `artifact.${plannedTurnRef}.${input.missionSlug}`,
        buildsOnArtifactRef: previousTurn?.artifactRef ?? null,
        contextFingerprintAfter: rotationContextFingerprint(
          contextFingerprintBefore,
          `turn.${input.missionSlug}.${plannedTurnRef}`,
        ),
        contextFingerprintBefore,
        leaseRef: afterFault.activeLease.leaseRef,
        missionRef,
        resumedAfterRotation:
          inducedHere && afterFault.rotation?.outcome === 'retrying',
        turnRef: `turn.${input.missionSlug}.${plannedTurnRef}`,
      }

      return {
        ...afterFault,
        fingerprint: turn.contextFingerprintAfter,
        nowIso: isoTimestampAfterIso(
          afterFault.nowIso,
          TURN_DURATION_MILLISECONDS,
        ),
        turns: [...afterFault.turns, turn],
      }
    },
    {
      activeLease: initialLease,
      blockedReason: null,
      continuationDecision: null,
      fingerprint: rotationContextFingerprint('genesis', missionRef),
      fleet: input.fleet,
      leaseRefs: [initialLease.leaseRef],
      nowIso: startedAtIso,
      rotation: null,
      turns: [],
    },
  )

  const outcome = finished.blockedReason === null ? 'completed' : 'blocked'

  return {
    blockedReason: finished.blockedReason,
    continuationDecision: finished.continuationDecision,
    generatedAt: finished.nowIso,
    mission: missionRecord({
      accountLeaseRefs: finished.leaseRefs,
      artifactRefs: finished.turns.map(turn => turn.artifactRef),
      blockerRefs:
        outcome === 'blocked'
          ? [`blocker.account_fleet_exhausted.${input.missionSlug}`]
          : [],
      createdAtIso: startedAtIso,
      missionRef,
      missionSlug: input.missionSlug,
      status: outcome === 'blocked' ? 'blocked' : 'delivered',
      updatedAtIso: finished.nowIso,
      workroomRef: input.workroomRef,
    }),
    outcome,
    rotation: finished.rotation,
    smokeVersion: AUTOPILOT_ROTATION_SMOKE_VERSION,
    turns: finished.turns,
  }
}

export const verifyAutopilotRotationContinuity = (
  receipt: AutopilotRateLimitRotationSmokeReceipt,
): ReadonlyArray<string> => {
  const violations: Array<string> = []
  const missionRef = receipt.mission.missionRef
  const genesisFingerprint = rotationContextFingerprint('genesis', missionRef)

  receipt.turns.forEach((turn, index) => {
    if (turn.missionRef !== missionRef) {
      violations.push(
        `Turn ${turn.turnRef} ran under mission ${turn.missionRef} instead of ${missionRef}.`,
      )
    }

    const previous = receipt.turns[index - 1]

    if (index === 0) {
      if (turn.contextFingerprintBefore !== genesisFingerprint) {
        violations.push(
          'First turn did not start from the mission genesis context fingerprint.',
        )
      }

      if (turn.buildsOnArtifactRef !== null) {
        violations.push(
          'First turn claims to build on an artifact that cannot exist yet.',
        )
      }

      return
    }

    if (previous === undefined) {
      return
    }

    if (turn.contextFingerprintBefore !== previous.contextFingerprintAfter) {
      violations.push(
        `Turn ${turn.turnRef} broke the context fingerprint chain across the previous turn.`,
      )
    }

    if (turn.buildsOnArtifactRef !== previous.artifactRef) {
      violations.push(
        `Turn ${turn.turnRef} does not build on the previous turn's artifact.`,
      )
    }
  })

  receipt.turns.forEach(turn => {
    if (!receipt.mission.artifactRefs.includes(turn.artifactRef)) {
      violations.push(
        `Mission record dropped artifact ${turn.artifactRef}; pre-rotation work was not preserved.`,
      )
    }

    if (!receipt.mission.accountLeaseRefs.includes(turn.leaseRef)) {
      violations.push(
        `Mission record dropped account lease ${turn.leaseRef}.`,
      )
    }
  })

  const rotation = receipt.rotation

  if (rotation !== null && rotation.outcome === 'retrying') {
    if (rotation.nextAccountSlug === rotation.previousAccountSlug) {
      violations.push('Rotation did not switch to a different account.')
    }

    if (
      !receipt.mission.accountLeaseRefs.includes(rotation.previousLeaseRef) ||
      rotation.nextLeaseRef === null ||
      !receipt.mission.accountLeaseRefs.includes(rotation.nextLeaseRef)
    ) {
      violations.push(
        'Mission record does not show both the pre-rotation and post-rotation account leases.',
      )
    }

    const resumedTurn = receipt.turns.find(turn => turn.resumedAfterRotation)

    if (resumedTurn === undefined) {
      violations.push('No turn resumed under the rotated account lease.')
    } else {
      if (resumedTurn.leaseRef !== rotation.nextLeaseRef) {
        violations.push(
          'The resumed turn did not run under the rotated account lease.',
        )
      }

      const resumedIndex = receipt.turns.indexOf(resumedTurn)
      const lastPreRotationTurn = receipt.turns[resumedIndex - 1]

      if (
        lastPreRotationTurn !== undefined &&
        resumedTurn.contextFingerprintBefore !==
          lastPreRotationTurn.contextFingerprintAfter
      ) {
        violations.push(
          'The resumed turn restarted from zero instead of continuing the pre-rotation context.',
        )
      }
    }

    if (receipt.continuationDecision === null) {
      violations.push('Rotation happened without a continuation decision.')
    } else {
      if (
        receipt.continuationDecision.selectedContinuationAction !==
        'retry_account'
      ) {
        violations.push(
          'Rotation continuation decision is not a retry_account decision.',
        )
      }

      if (receipt.continuationDecision.missionRef !== missionRef) {
        violations.push(
          'Rotation continuation decision points at a different mission.',
        )
      }
    }
  }

  if (receipt.outcome === 'blocked') {
    if (receipt.mission.status !== 'blocked') {
      violations.push('Blocked outcome did not set the mission to blocked.')
    }

    if (receipt.mission.blockerRefs.length === 0) {
      violations.push('Blocked outcome left no typed blocker ref behind.')
    }

    if (receipt.continuationDecision === null) {
      violations.push(
        'Blocked outcome stalled silently without a continuation decision.',
      )
    }

    if (receipt.blockedReason === null) {
      violations.push('Blocked outcome carries no typed blocked reason.')
    }
  }

  if (receipt.outcome === 'completed' && receipt.mission.status !== 'delivered') {
    violations.push('Completed outcome did not deliver the mission.')
  }

  return violations
}
