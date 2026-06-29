import { Effect } from 'effect'

import { tassadarExecutorTraceVerificationChallengeRequest } from './tassadar-executor-trace-homework'
import {
  type TrainingTraceContributionRecord,
  type TrainingTraceContributionStore,
  closeoutFromPairedContribution,
  pairedContributionProjectionJson,
  trainingTraceContributionStoreErrorFromUnknown,
} from './tassadar-trace-contribution-authority'
import type {
  TrainingVerificationChallengeCreateRequest,
  TrainingVerificationChallengeRecord,
} from './training-verification'

/**
 * Worker -> validator pairing orchestration (#5053, epic #5051). Per design
 * §4.3 this ships option (B): the Artanis-paired-first path. A pending worker
 * trace contribution (#5052) needs a DISTINCT validator device assigned to it
 * so a validator can replay + submit a verdict. This module is the bounded,
 * pure-where-possible orchestration that, each tick, picks the oldest unpaired
 * pending contribution, hands it a distinct validator candidate, builds the
 * EXISTING exact_trace_replay verification challenge, and records the pairing.
 *
 * INERT BY DEFAULT. The scheduled wrapper (`runTassadarTracePairingScheduled`)
 * returns a disabled outcome unless explicitly enabled via the
 * `TASSADAR_TRACE_PAIRING=1` flag. Landing this changes no live Artanis tick
 * behavior, relaxes no `requireAdmin`, and touches no settlement/payout path.
 *
 * Trust model (unchanged): replay is the trust anchor. This orchestration never
 * trusts a submitter digest and never decides Verified/Rejected itself — it only
 * assigns a distinct validator device and builds the challenge whose
 * separate-device replay match computes the verdict. Device-distinctness
 * (`validatorDeviceRef != pylonDeviceRef`) and lease integrity are enforced here
 * AND re-enforced by the challenge builder and the agent-gated verdict route.
 */

export type TassadarValidatorCandidate = Readonly<{
  // A validator's already-computed replay digest for the candidate workload,
  // produced by re-executing the dispatched workload on the validator device.
  replayDigestRef: string
  validatorDeviceRef: string
}>

export type TassadarTracePairingRejection =
  | 'self_pair_device_not_distinct'
  | 'no_distinct_validator_candidate'
  | 'contribution_not_pending'

export type TassadarTracePairingDecision =
  | Readonly<{
      kind: 'pairable'
      challengeRequest: TrainingVerificationChallengeCreateRequest
      contributionRef: string
      pylonDeviceRef: string
      validatorDeviceRef: string
      replayDigestRef: string
    }>
  | Readonly<{
      kind: 'rejected'
      contributionRef: string
      reason: TassadarTracePairingRejection
    }>

/**
 * Pure pairing decision. Given a pending worker contribution and an ordered set
 * of candidate validators, select the first candidate whose device is DISTINCT
 * from the worker Pylon device, then build the existing exact_trace_replay
 * challenge request. No store, clock, or network. Self-pairs and already-paired
 * contributions are rejected without producing a challenge.
 */
export const decideTracePairing = (
  contribution: TrainingTraceContributionRecord,
  candidates: ReadonlyArray<TassadarValidatorCandidate>,
): TassadarTracePairingDecision => {
  if (contribution.state !== 'pending') {
    return {
      contributionRef: contribution.contributionRef,
      kind: 'rejected',
      reason: 'contribution_not_pending',
    }
  }

  const distinctCandidate = candidates.find(
    candidate => candidate.validatorDeviceRef !== contribution.pylonDeviceRef,
  )

  if (distinctCandidate === undefined) {
    return {
      contributionRef: contribution.contributionRef,
      kind: 'rejected',
      // If every candidate is the worker's own device, this is a self-pair
      // attempt; otherwise there is simply no distinct validator available.
      reason:
        candidates.length > 0
          ? 'self_pair_device_not_distinct'
          : 'no_distinct_validator_candidate',
    }
  }

  const closeout = closeoutFromPairedContribution(contribution, {
    replayDigestRef: distinctCandidate.replayDigestRef,
    validatorDeviceRef: distinctCandidate.validatorDeviceRef,
  })

  // The builder re-enforces worker != validator device; for a distinct
  // candidate this never throws.
  const challengeRequest = tassadarExecutorTraceVerificationChallengeRequest({
    closeout,
    trainingRunRef: contribution.trainingRunRef,
    windowRef: contribution.windowRef,
  })

  return {
    challengeRequest,
    contributionRef: contribution.contributionRef,
    kind: 'pairable',
    pylonDeviceRef: contribution.pylonDeviceRef,
    replayDigestRef: distinctCandidate.replayDigestRef,
    validatorDeviceRef: distinctCandidate.validatorDeviceRef,
  }
}

export type TassadarTracePairingOutcome = Readonly<{
  state: 'paired' | 'no_pending' | 'no_distinct_validator' | 'skipped'
  contributionRef: string | null
  verificationChallengeRef: string | null
  validatorDeviceRef: string | null
  reason: string | null
}>

const skipped = (reason: string): TassadarTracePairingOutcome => ({
  contributionRef: null,
  reason,
  state: 'skipped',
  validatorDeviceRef: null,
  verificationChallengeRef: null,
})

export type TassadarTracePairingDependencies = Readonly<{
  // Resolves the available distinct validator candidates for a specific pending
  // worker contribution. Implementations enforce that they never hand back the
  // worker's own device; this orchestration ALSO rejects a self-pair as a
  // defence-in-depth check before building any challenge.
  resolveValidatorCandidates: (
    contribution: TrainingTraceContributionRecord,
  ) => Promise<ReadonlyArray<TassadarValidatorCandidate>>
  createVerificationChallenge: (
    request: TrainingVerificationChallengeCreateRequest,
  ) => Promise<TrainingVerificationChallengeRecord>
  store: TrainingTraceContributionStore
  nowIso: string
  trainingRunRef?: string
}>

/**
 * Run one pairing tick: pick the oldest pending contribution, hand it a distinct
 * validator, build the existing exact_trace_replay challenge, and record the
 * pairing through the store's conditional `pairValidatorVerdict` (which is
 * `WHERE state = 'pending'`, so a double-pair fails closed). At most one
 * contribution is paired per tick.
 */
export const runTassadarTracePairing = async (
  deps: TassadarTracePairingDependencies,
): Promise<TassadarTracePairingOutcome> => {
  const pending = await deps.store.listPendingContributions({
    limit: 1,
    ...(deps.trainingRunRef === undefined
      ? {}
      : { trainingRunRef: deps.trainingRunRef }),
  })

  const contribution = pending[0]

  if (contribution === undefined) {
    return {
      contributionRef: null,
      reason: 'no_pending_contribution',
      state: 'no_pending',
      validatorDeviceRef: null,
      verificationChallengeRef: null,
    }
  }

  const candidates = await deps.resolveValidatorCandidates(contribution)
  const decision = decideTracePairing(contribution, candidates)

  if (decision.kind === 'rejected') {
    return {
      contributionRef: decision.contributionRef,
      reason: decision.reason,
      state:
        decision.reason === 'contribution_not_pending'
          ? 'skipped'
          : 'no_distinct_validator',
      validatorDeviceRef: null,
      verificationChallengeRef: null,
    }
  }

  const challenge = await deps.createVerificationChallenge(
    decision.challengeRequest,
  )

  const paired = await deps.store.pairValidatorVerdict({
    contributionRef: decision.contributionRef,
    publicProjectionJson: pairedContributionProjectionJson(contribution, {
      replayDigestRef: decision.replayDigestRef,
      validatorDeviceRef: decision.validatorDeviceRef,
      verificationChallengeRef: challenge.challengeRef,
    }),
    replayDigestRef: decision.replayDigestRef,
    updatedAt: deps.nowIso,
    validatorDeviceRef: decision.validatorDeviceRef,
    verificationChallengeRef: challenge.challengeRef,
  })

  return {
    contributionRef: paired.contributionRef,
    reason: null,
    state: 'paired',
    validatorDeviceRef: decision.validatorDeviceRef,
    verificationChallengeRef: challenge.challengeRef,
  }
}

/**
 * Flag-gated scheduled wrapper. OFF by default: returns a disabled outcome
 * unless `enabled` (wired to `TASSADAR_TRACE_PAIRING === '1'`) is true. Mirrors
 * the inert-by-default shape of `runArtanisAdminTickScheduled`, so landing this
 * does not change the live Artanis tick until the #5061 dry-run deliberately
 * enables it. All errors are caught into a skipped outcome so the tick never
 * crashes.
 */
export const runTassadarTracePairingScheduled = (
  deps: Readonly<{
    enabled: boolean
    resolveValidatorCandidates: TassadarTracePairingDependencies['resolveValidatorCandidates']
    createVerificationChallenge: TassadarTracePairingDependencies['createVerificationChallenge']
    store: TrainingTraceContributionStore
    nowIso: string
    trainingRunRef?: string
  }>,
): Effect.Effect<TassadarTracePairingOutcome, never> =>
  deps.enabled
    ? Effect.tryPromise({
        catch: trainingTraceContributionStoreErrorFromUnknown,
        try: () =>
          runTassadarTracePairing({
            createVerificationChallenge: deps.createVerificationChallenge,
            nowIso: deps.nowIso,
            resolveValidatorCandidates: deps.resolveValidatorCandidates,
            store: deps.store,
            ...(deps.trainingRunRef === undefined
              ? {}
              : { trainingRunRef: deps.trainingRunRef }),
          }),
      }).pipe(
        Effect.catch(error => Effect.succeed(skipped(`pairing_error:${error.kind}`))),
      )
    : Effect.succeed(skipped('trace_pairing_disabled'))
