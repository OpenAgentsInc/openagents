import { Schema as S } from 'effect'

import {
  type TassadarExecutorTraceCloseoutEvidence,
  TassadarExecutorTraceWorkloadFamilies,
  type TassadarExecutorTraceWorkloadFamily,
} from './tassadar-executor-trace-homework'

/**
 * Worker -> validator executor-trace contribution authority (#5052, epic
 * #5051). A contributor that claimed a training window lease submits its worker
 * trace commitment through the agent-gated trace-submission route; the worker
 * half is recorded here as a PENDING contribution awaiting a distinct-device
 * validator. The validator's replay digest pairs with it in the replay-verdict
 * route, which builds the existing exact_trace_replay verification challenge.
 *
 * This store records SUBMITTED worker commitments only. It grants no
 * acceptance, payout, settlement, or public-claim authority. Replay is the
 * trust anchor: the Verified/Rejected verdict is computed by the separate
 * exact_trace_replay challenge over the worker-vs-validator digest match, never
 * trusted from the submitter. All fields are public-safe refs or bounded step
 * integers.
 */

const PublicSafeRef = S.Trim.check(
  S.isNonEmpty(),
  S.isMinLength(3),
  S.isMaxLength(260),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9_.:/-]*$/),
)
const SampledStep = S.Number.check(
  S.isInt(),
  S.isBetween({ minimum: 0, maximum: 100_000_000 }),
)
const WorkloadFamily = S.Literals([...TassadarExecutorTraceWorkloadFamilies])

const SampledWindow = S.Struct({
  endStep: SampledStep,
  startStep: SampledStep,
})

/**
 * §4.1 worker trace-submission body. Carries the worker half of the
 * exact_trace_replay closeout evidence: the trace commitment digest, the
 * worker Pylon device, the sampled window, and the assignment/worker receipt
 * refs. No validator material is accepted here (the validator is a distinct
 * device added in §4.2).
 */
export const TrainingTraceSubmissionRequest = S.Struct({
  assignmentRef: PublicSafeRef,
  pylonDeviceRef: PublicSafeRef,
  sampledWindow: SampledWindow,
  sampledWindowRef: PublicSafeRef,
  traceCommitmentDigestRef: PublicSafeRef,
  workerReceiptRef: PublicSafeRef,
  workloadFamily: WorkloadFamily,
})
export type TrainingTraceSubmissionRequest =
  typeof TrainingTraceSubmissionRequest.Type

/**
 * §4.2 validator replay-verdict body. The validator device must be DISTINCT
 * from the worker Pylon device (enforced in the route + the challenge builder).
 * The `workloadFamily` selects the pending worker contribution to pair with.
 */
export const TrainingReplayVerdictRequest = S.Struct({
  replayDigestRef: PublicSafeRef,
  validatorDeviceRef: PublicSafeRef,
  validatorReceiptRef: S.optionalKey(PublicSafeRef),
  workloadFamily: WorkloadFamily,
})
export type TrainingReplayVerdictRequest =
  typeof TrainingReplayVerdictRequest.Type

export type TrainingTraceContributionState = 'pending' | 'paired'

export type TrainingTraceContributionRecord = Readonly<{
  assignmentRef: string
  contributionRef: string
  id: string
  leaseRef: string
  publicProjectionJson: string
  pylonDeviceRef: string
  pylonRef: string
  replayDigestRef: string | null
  sampledWindow: Readonly<{ endStep: number; startStep: number }>
  sampledWindowRef: string
  state: TrainingTraceContributionState
  submittedAt: string
  traceCommitmentDigestRef: string
  trainingRunRef: string
  updatedAt: string
  validatorDeviceRef: string | null
  verificationChallengeRef: string | null
  windowRef: string
  workerReceiptRef: string
  workloadFamily: TassadarExecutorTraceWorkloadFamily
}>

export class TrainingTraceContributionStoreError extends S.TaggedErrorClass<TrainingTraceContributionStoreError>()(
  'TrainingTraceContributionStoreError',
  {
    kind: S.Literals([
      'conflict',
      'forbidden',
      'not_found',
      'storage_error',
      'validation_error',
    ]),
    reason: S.String,
  },
) {}

export const trainingTraceContributionStoreErrorFromUnknown = (
  error: unknown,
): TrainingTraceContributionStoreError =>
  error instanceof TrainingTraceContributionStoreError
    ? error
    : new TrainingTraceContributionStoreError({
        kind: 'storage_error',
        reason: error instanceof Error ? error.message : String(error),
      })

export type TrainingTraceContributionStore = Readonly<{
  // Idempotent insert keyed by (lease_ref, workload_family). When a row already
  // exists for the pair, the existing record is returned unchanged so a retried
  // worker submission does not create a second pending contribution.
  recordWorkerContribution: (
    record: TrainingTraceContributionRecord,
  ) => Promise<TrainingTraceContributionRecord>
  readWorkerContribution: (
    leaseRef: string,
    workloadFamily: TassadarExecutorTraceWorkloadFamily,
  ) => Promise<TrainingTraceContributionRecord | undefined>
  // The read the pairing orchestration (#5053) and any "next contribution to
  // validate" surface needs: the oldest still-`pending` worker contributions
  // awaiting a distinct-device validator, optionally scoped to a training run.
  // Returns evidence only; pairing/device-distinctness is enforced by the
  // caller and the verdict route. Limit is bounded by the store.
  listPendingContributions: (
    input: Readonly<{
      limit: number
      trainingRunRef?: string
    }>,
  ) => Promise<ReadonlyArray<TrainingTraceContributionRecord>>
  // Validator-leg payout resolution backstop (#5310/#5306): map a participant's
  // device-ref to the most recent `pylon_ref` that device acted as a WORKER
  // under. A validator submits its verdict with its device-ref (its nodeId),
  // which is NOT a `pylon_ref` and so resolves to no registration directly; but
  // the same node also acts as a worker on other windows, binding its device-ref
  // to its owning `pylon_ref` here. Returns the newest such `pylon_ref` (or
  // undefined when this device never recorded a worker contribution), so the
  // owner-scoped Spark payout resolver can find the validator's OWN registered
  // target. Bound to the device's own historical worker pylon only; it never
  // crosses agent ownership and grants no authority.
  readMostRecentPylonRefByDeviceRef: (
    pylonDeviceRef: string,
  ) => Promise<string | undefined>
  // Pairs the pending worker contribution with a validator's replay digest and
  // the resulting verification-challenge ref. Conditional on the row still
  // being `pending` so a second validator cannot re-pair an already-paired
  // contribution.
  pairValidatorVerdict: (
    input: Readonly<{
      contributionRef: string
      publicProjectionJson: string
      replayDigestRef: string
      updatedAt: string
      validatorDeviceRef: string
      verificationChallengeRef: string
    }>,
  ) => Promise<TrainingTraceContributionRecord>
}>

const contributionRefFor = (
  leaseRef: string,
  workloadFamily: TassadarExecutorTraceWorkloadFamily,
): string =>
  `contribution.tassadar_executor_trace.${leaseRef}.${workloadFamily}`

export const buildTrainingTraceContributionRecord = (
  input: Readonly<{
    leaseRef: string
    makeId: () => string
    nowIso: string
    pylonRef: string
    request: TrainingTraceSubmissionRequest
    trainingRunRef: string
    windowRef: string
  }>,
): TrainingTraceContributionRecord => {
  const contributionRef = contributionRefFor(
    input.leaseRef,
    input.request.workloadFamily,
  )
  const publicProjection = {
    assignmentRef: input.request.assignmentRef,
    contributionRef,
    leaseRef: input.leaseRef,
    pylonDeviceRef: input.request.pylonDeviceRef,
    pylonRef: input.pylonRef,
    replayDigestRef: null,
    sampledWindow: input.request.sampledWindow,
    sampledWindowRef: input.request.sampledWindowRef,
    schemaVersion: 'openagents.training.trace_contribution.v1',
    state: 'pending' as const,
    traceCommitmentDigestRef: input.request.traceCommitmentDigestRef,
    trainingRunRef: input.trainingRunRef,
    validatorDeviceRef: null,
    verificationChallengeRef: null,
    windowRef: input.windowRef,
    workerReceiptRef: input.request.workerReceiptRef,
    workloadFamily: input.request.workloadFamily,
  }

  return {
    assignmentRef: input.request.assignmentRef,
    contributionRef,
    id: input.makeId(),
    leaseRef: input.leaseRef,
    publicProjectionJson: JSON.stringify(publicProjection),
    pylonDeviceRef: input.request.pylonDeviceRef,
    pylonRef: input.pylonRef,
    replayDigestRef: null,
    sampledWindow: input.request.sampledWindow,
    sampledWindowRef: input.request.sampledWindowRef,
    state: 'pending',
    submittedAt: input.nowIso,
    traceCommitmentDigestRef: input.request.traceCommitmentDigestRef,
    trainingRunRef: input.trainingRunRef,
    updatedAt: input.nowIso,
    validatorDeviceRef: null,
    verificationChallengeRef: null,
    windowRef: input.windowRef,
    workerReceiptRef: input.request.workerReceiptRef,
    workloadFamily: input.request.workloadFamily,
  }
}

/**
 * Reconstruct the full exact_trace_replay closeout evidence by pairing the
 * recorded worker half with the validator's replay digest and device. The
 * resulting struct feeds tassadarExecutorTraceVerificationChallengeRequest,
 * which itself re-enforces the worker != validator device rule.
 */
export const closeoutFromPairedContribution = (
  contribution: TrainingTraceContributionRecord,
  validator: Readonly<{ replayDigestRef: string; validatorDeviceRef: string }>,
): TassadarExecutorTraceCloseoutEvidence => ({
  assignmentRef: contribution.assignmentRef,
  pylonDeviceRef: contribution.pylonDeviceRef,
  replayDigestRef: validator.replayDigestRef,
  sampledWindow: contribution.sampledWindow,
  sampledWindowRef: contribution.sampledWindowRef,
  traceCommitmentDigestRef: contribution.traceCommitmentDigestRef,
  validatorDeviceRef: validator.validatorDeviceRef,
  workerReceiptRef: contribution.workerReceiptRef,
  workloadFamily: contribution.workloadFamily,
})

export const pairedContributionProjectionJson = (
  contribution: TrainingTraceContributionRecord,
  input: Readonly<{
    replayDigestRef: string
    validatorDeviceRef: string
    verificationChallengeRef: string
  }>,
): string =>
  JSON.stringify({
    assignmentRef: contribution.assignmentRef,
    contributionRef: contribution.contributionRef,
    leaseRef: contribution.leaseRef,
    pylonDeviceRef: contribution.pylonDeviceRef,
    pylonRef: contribution.pylonRef,
    replayDigestRef: input.replayDigestRef,
    sampledWindow: contribution.sampledWindow,
    sampledWindowRef: contribution.sampledWindowRef,
    schemaVersion: 'openagents.training.trace_contribution.v1',
    state: 'paired' as const,
    traceCommitmentDigestRef: contribution.traceCommitmentDigestRef,
    trainingRunRef: contribution.trainingRunRef,
    validatorDeviceRef: input.validatorDeviceRef,
    verificationChallengeRef: input.verificationChallengeRef,
    windowRef: contribution.windowRef,
    workerReceiptRef: contribution.workerReceiptRef,
    workloadFamily: contribution.workloadFamily,
  })

export type TrainingTraceContributionRow = Readonly<{
  archived_at: string | null
  assignment_ref: string
  contribution_ref: string
  id: string
  lease_ref: string
  public_projection_json: string
  pylon_device_ref: string
  pylon_ref: string
  replay_digest_ref: string | null
  sampled_window_end_step: number
  sampled_window_ref: string
  sampled_window_start_step: number
  state: TrainingTraceContributionState
  submitted_at: string
  trace_commitment_digest_ref: string
  training_run_ref: string
  updated_at: string
  validator_device_ref: string | null
  verification_challenge_ref: string | null
  window_ref: string
  worker_receipt_ref: string
  workload_family: TassadarExecutorTraceWorkloadFamily
}>

export const rowToTrainingTraceContribution = (
  row: TrainingTraceContributionRow,
): TrainingTraceContributionRecord => ({
  assignmentRef: row.assignment_ref,
  contributionRef: row.contribution_ref,
  id: row.id,
  leaseRef: row.lease_ref,
  publicProjectionJson: row.public_projection_json,
  pylonDeviceRef: row.pylon_device_ref,
  pylonRef: row.pylon_ref,
  replayDigestRef: row.replay_digest_ref,
  sampledWindow: {
    endStep: row.sampled_window_end_step,
    startStep: row.sampled_window_start_step,
  },
  sampledWindowRef: row.sampled_window_ref,
  state: row.state,
  submittedAt: row.submitted_at,
  traceCommitmentDigestRef: row.trace_commitment_digest_ref,
  trainingRunRef: row.training_run_ref,
  updatedAt: row.updated_at,
  validatorDeviceRef: row.validator_device_ref,
  verificationChallengeRef: row.verification_challenge_ref,
  windowRef: row.window_ref,
  workerReceiptRef: row.worker_receipt_ref,
  workloadFamily: row.workload_family,
})

export const makeD1TrainingTraceContributionStore = (
  db: D1Database,
): TrainingTraceContributionStore => {
  const readByLeaseWorkload = async (
    leaseRef: string,
    workloadFamily: TassadarExecutorTraceWorkloadFamily,
  ): Promise<TrainingTraceContributionRecord | undefined> => {
    const row = await db
      .prepare(
        `SELECT *
           FROM training_trace_contributions
          WHERE lease_ref = ?
            AND workload_family = ?
            AND archived_at IS NULL`,
      )
      .bind(leaseRef, workloadFamily)
      .first<TrainingTraceContributionRow>()

    return row === null ? undefined : rowToTrainingTraceContribution(row)
  }
  const readByContributionRef = async (
    contributionRef: string,
  ): Promise<TrainingTraceContributionRecord | undefined> => {
    const row = await db
      .prepare(
        `SELECT *
           FROM training_trace_contributions
          WHERE contribution_ref = ?
            AND archived_at IS NULL`,
      )
      .bind(contributionRef)
      .first<TrainingTraceContributionRow>()

    return row === null ? undefined : rowToTrainingTraceContribution(row)
  }

  const listPending = async (
    input: Readonly<{ limit: number; trainingRunRef?: string }>,
  ): Promise<ReadonlyArray<TrainingTraceContributionRecord>> => {
    const boundedLimit = Math.max(1, Math.min(50, Math.trunc(input.limit)))
    const statement =
      input.trainingRunRef === undefined
        ? db
            .prepare(
              `SELECT *
                 FROM training_trace_contributions
                WHERE state = 'pending'
                  AND archived_at IS NULL
                ORDER BY submitted_at ASC
                LIMIT ?`,
            )
            .bind(boundedLimit)
        : db
            .prepare(
              `SELECT *
                 FROM training_trace_contributions
                WHERE state = 'pending'
                  AND archived_at IS NULL
                  AND training_run_ref = ?
                ORDER BY submitted_at ASC
                LIMIT ?`,
            )
            .bind(input.trainingRunRef, boundedLimit)

    const result = await statement.all<TrainingTraceContributionRow>()

    return (result.results ?? []).map(rowToTrainingTraceContribution)
  }

  const readMostRecentPylonRefByDeviceRef = async (
    pylonDeviceRef: string,
  ): Promise<string | undefined> => {
    const trimmed = pylonDeviceRef.trim()

    if (trimmed === '') {
      return undefined
    }

    const row = await db
      .prepare(
        `SELECT pylon_ref
           FROM training_trace_contributions
          WHERE pylon_device_ref = ?
            AND archived_at IS NULL
          ORDER BY submitted_at DESC
          LIMIT 1`,
      )
      .bind(trimmed)
      .first<{ pylon_ref: string }>()

    return row === null ? undefined : row.pylon_ref
  }

  return {
    readMostRecentPylonRefByDeviceRef,
    listPendingContributions: listPending,
    readWorkerContribution: readByLeaseWorkload,
    recordWorkerContribution: async record => {
      // Idempotent by (lease_ref, workload_family): an INSERT OR IGNORE keeps
      // the first pending contribution and a retried submission re-reads it.
      await db
        .prepare(
          `INSERT OR IGNORE INTO training_trace_contributions
            (id, contribution_ref, lease_ref, window_ref, training_run_ref,
             pylon_ref, workload_family, assignment_ref, pylon_device_ref,
             trace_commitment_digest_ref, sampled_window_ref,
             sampled_window_start_step, sampled_window_end_step,
             worker_receipt_ref, state, validator_device_ref, replay_digest_ref,
             verification_challenge_ref, public_projection_json, submitted_at,
             updated_at, archived_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'pending',
                   NULL, NULL, NULL, ?, ?, ?, NULL)`,
        )
        .bind(
          record.id,
          record.contributionRef,
          record.leaseRef,
          record.windowRef,
          record.trainingRunRef,
          record.pylonRef,
          record.workloadFamily,
          record.assignmentRef,
          record.pylonDeviceRef,
          record.traceCommitmentDigestRef,
          record.sampledWindowRef,
          record.sampledWindow.startStep,
          record.sampledWindow.endStep,
          record.workerReceiptRef,
          record.publicProjectionJson,
          record.submittedAt,
          record.updatedAt,
        )
        .run()

      const stored = await readByLeaseWorkload(
        record.leaseRef,
        record.workloadFamily,
      )

      if (stored === undefined) {
        throw new TrainingTraceContributionStoreError({
          kind: 'storage_error',
          reason: 'Worker trace contribution could not be read back.',
        })
      }

      return stored
    },
    pairValidatorVerdict: async input => {
      const result = await db
        .prepare(
          `UPDATE training_trace_contributions
              SET state = 'paired',
                  validator_device_ref = ?,
                  replay_digest_ref = ?,
                  verification_challenge_ref = ?,
                  public_projection_json = ?,
                  updated_at = ?
            WHERE contribution_ref = ?
              AND state = 'pending'
              AND archived_at IS NULL`,
        )
        .bind(
          input.validatorDeviceRef,
          input.replayDigestRef,
          input.verificationChallengeRef,
          input.publicProjectionJson,
          input.updatedAt,
          input.contributionRef,
        )
        .run()

      if ((result.meta?.changes ?? 0) === 0) {
        throw new TrainingTraceContributionStoreError({
          kind: 'conflict',
          reason:
            'Worker trace contribution is not pending (already paired or missing).',
        })
      }

      const stored = await readByContributionRef(input.contributionRef)

      if (stored === undefined) {
        throw new TrainingTraceContributionStoreError({
          kind: 'storage_error',
          reason: 'Paired trace contribution could not be read back.',
        })
      }

      return stored
    },
  }
}
