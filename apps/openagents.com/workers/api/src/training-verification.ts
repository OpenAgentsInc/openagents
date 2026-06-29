import { Schema as S } from 'effect'

import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'
import {
  parseJsonRecord,
  parseJsonStringArray,
  recordFromUnknown,
  stringArrayFromUnknown,
} from './json-boundary'
import { isoTimestampAfterIso } from './runtime-primitives'

const TrimmedString = S.Trim
const NonEmptyTrimmedString = TrimmedString.check(S.isNonEmpty())
const PublicSafeRef = NonEmptyTrimmedString.check(
  S.isMinLength(3),
  S.isMaxLength(260),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9_.:/-]*$/),
)
const PublicSafeRefs = S.optionalKey(S.Array(PublicSafeRef))

export const TrainingVerificationClass = S.Literals([
  'deterministic_recompute',
  'exact_trace_replay',
  'freivalds_merkle',
  'seeded_replication',
  'statistical_cross_check',
])
export type TrainingVerificationClass = typeof TrainingVerificationClass.Type

export const TrainingVerificationChallengeState = S.Literals([
  'Queued',
  'Leased',
  'Retrying',
  'Verified',
  'Rejected',
  'TimedOut',
])
export type TrainingVerificationChallengeState =
  typeof TrainingVerificationChallengeState.Type

export const TrainingVerificationSamplingPolicyKind = S.Literals([
  'aggregate',
  'per_contribution',
])
export type TrainingVerificationSamplingPolicyKind =
  typeof TrainingVerificationSamplingPolicyKind.Type

export const TrainingVerificationFailureCode = S.Literals([
  'ChallengeAlreadyTerminal',
  'DimensionMismatch',
  'DigestMismatch',
  'ExecutorTraceMismatch',
  'FieldMismatch',
  'FreivaldsMismatch',
  'LeaseExpired',
  'MerkleProofInvalid',
  'OutputDigestMissing',
  'RetryBudgetExhausted',
  'RowOpeningMissing',
  'SamplePolicyRejected',
  'StatisticalThresholdFailed',
  'VerificationClassUnknown',
])
export type TrainingVerificationFailureCode =
  typeof TrainingVerificationFailureCode.Type

export const TrainingVerificationChallengeCreateRequest = S.Struct({
  commitmentRefs: PublicSafeRefs,
  contributionRef: S.optionalKey(PublicSafeRef),
  homeworkKind: PublicSafeRef,
  maxAttempts: S.optionalKey(S.Number),
  payload: S.Record(S.String, S.Unknown),
  samplingPolicy: S.optionalKey(TrainingVerificationSamplingPolicyKind),
  trainingRunRef: PublicSafeRef,
  verificationClass: TrainingVerificationClass,
  windowRef: S.optionalKey(PublicSafeRef),
})
export type TrainingVerificationChallengeCreateRequest =
  typeof TrainingVerificationChallengeCreateRequest.Type

export const TrainingVerificationChallengeLeaseRequest = S.Struct({
  leaseSeconds: S.optionalKey(S.Number),
  validatorRef: PublicSafeRef,
  verificationClass: S.optionalKey(TrainingVerificationClass),
})
export type TrainingVerificationChallengeLeaseRequest =
  typeof TrainingVerificationChallengeLeaseRequest.Type

export const TrainingVerificationChallengeRetryRequest = S.Struct({
  failureCodes: PublicSafeRefs,
  receiptRefs: PublicSafeRefs,
})
export type TrainingVerificationChallengeRetryRequest =
  typeof TrainingVerificationChallengeRetryRequest.Type

export const TrainingVerificationChallengeFinalizeRequest = S.Struct({
  receiptRefs: PublicSafeRefs,
})
export type TrainingVerificationChallengeFinalizeRequest =
  typeof TrainingVerificationChallengeFinalizeRequest.Type

export type TrainingVerificationChallengeRecord = Readonly<{
  challengeRef: string
  commitmentRefs: ReadonlyArray<string>
  contributionRef: string | null
  createdAt: string
  failureCodes: ReadonlyArray<TrainingVerificationFailureCode>
  homeworkKind: string
  id: string
  leaseExpiresAt: string | null
  leaseRef: string | null
  leasedToRef: string | null
  maxAttempts: number
  payloadJson: string
  publicProjectionJson: string
  rejectedAt: string | null
  samplingPolicy: TrainingVerificationSamplingPolicyKind
  state: TrainingVerificationChallengeState
  timedOutAt: string | null
  trainingRunRef: string
  updatedAt: string
  verdictRefs: ReadonlyArray<string>
  verificationClass: TrainingVerificationClass
  verifiedAt: string | null
  windowRef: string | null
}>

export type TrainingVerificationChallengeEventRecord = Readonly<{
  challengeRef: string
  createdAt: string
  failureCodes: ReadonlyArray<TrainingVerificationFailureCode>
  id: string
  receiptRefs: ReadonlyArray<string>
  stateFrom: TrainingVerificationChallengeState | null
  stateTo: TrainingVerificationChallengeState
  transitionKind: string
  validatorRef: string | null
}>

export type TrainingVerificationChallengeProjection = Readonly<{
  challengeRef: string
  commitmentRefs: ReadonlyArray<string>
  contributionRef: string | null
  createdAtDisplay: string
  failureCodes: ReadonlyArray<TrainingVerificationFailureCode>
  homeworkKind: string
  leaseExpiresInSeconds: number | null
  leasedToRef: string | null
  samplingPolicy: TrainingVerificationSamplingPolicyKind
  state: TrainingVerificationChallengeState
  trainingRunRef: string
  updatedAtDisplay: string
  verdictRefs: ReadonlyArray<string>
  verificationClass: TrainingVerificationClass
  windowRef: string | null
  // #5124: the two digests the exact_trace_replay verifier actually compares,
  // surfaced public-safe (sha256 hashes, no secrets) so a Rejected
  // ExecutorTraceMismatch is diagnosable — worker-stored commitment vs
  // validator-stored replay. Null for non-exact-trace classes / absent payloads.
  exactTraceCommitmentDigestRef?: string | null
  exactTraceReplayDigestRef?: string | null
}>

export type TrainingVerificationStore = Readonly<{
  createChallenge: (
    challenge: TrainingVerificationChallengeRecord,
    event: TrainingVerificationChallengeEventRecord,
  ) => Promise<TrainingVerificationChallengeRecord>
  leaseChallenge: (
    challenge: TrainingVerificationChallengeRecord,
    event: TrainingVerificationChallengeEventRecord,
  ) => Promise<TrainingVerificationChallengeRecord>
  listLeaseCandidates: (
    nowIso: string,
    limit: number,
    verificationClass?: TrainingVerificationClass,
  ) => Promise<ReadonlyArray<TrainingVerificationChallengeRecord>>
  readChallenge: (
    challengeRef: string,
  ) => Promise<TrainingVerificationChallengeRecord | undefined>
  transitionChallenge: (
    challenge: TrainingVerificationChallengeRecord,
    event: TrainingVerificationChallengeEventRecord,
  ) => Promise<TrainingVerificationChallengeRecord>
}>

export class TrainingVerificationStoreError extends S.TaggedErrorClass<TrainingVerificationStoreError>()(
  'TrainingVerificationStoreError',
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

export type TrainingVerificationVerifierInput = Readonly<{
  challenge: TrainingVerificationChallengeRecord
  payload: Record<string, unknown>
}>

export type TrainingVerificationVerdict = Readonly<{
  failureCodes: ReadonlyArray<TrainingVerificationFailureCode>
  publicDetails?: Record<string, unknown>
  state: 'Verified' | 'Rejected'
  verdictRefs: ReadonlyArray<string>
}>

// Per-class staleness dimension (Pluralis roadmap P2.2, openagents
// issue #4853). Additive and optional: a registration without a policy
// inherits the run-level max_allowed_stale default. Class overrides may
// only tighten — the acceptance-time decision clamps a looser override
// to the run contract ceiling (see training-staleness-acceptance.ts).
export type TrainingVerificationStalenessPolicy =
  | Readonly<{ kind: 'inherit_run_default' }>
  | Readonly<{ kind: 'max_steps_behind_override'; maxStepsBehind: number }>

export type TrainingVerificationRegistration = Readonly<{
  className: TrainingVerificationClass
  defaultSamplingPolicy: TrainingVerificationSamplingPolicyKind
  failureCodes: ReadonlyArray<TrainingVerificationFailureCode>
  stalenessPolicy?: TrainingVerificationStalenessPolicy
  verify: (
    input: TrainingVerificationVerifierInput,
  ) => Promise<TrainingVerificationVerdict> | TrainingVerificationVerdict
}>

export type TrainingVerificationRow = Readonly<{
  challenge_ref: string
  commitment_refs_json: string
  contribution_ref: string | null
  created_at: string
  failure_codes_json: string
  homework_kind: string
  id: string
  lease_expires_at: string | null
  lease_ref: string | null
  leased_to_ref: string | null
  max_attempts: number
  payload_json: string
  public_projection_json: string
  rejected_at: string | null
  sampling_policy: TrainingVerificationSamplingPolicyKind
  state: TrainingVerificationChallengeState
  timed_out_at: string | null
  training_run_ref: string
  updated_at: string
  verdict_refs_json: string
  verification_class: TrainingVerificationClass
  verified_at: string | null
  window_ref: string | null
}>

const uniqueRefs = (
  refs: ReadonlyArray<string> | undefined,
): ReadonlyArray<string> =>
  [
    ...new Set((refs ?? []).map(ref => ref.trim()).filter(ref => ref !== '')),
  ].sort()

const uniqueFailureCodes = (
  codes: ReadonlyArray<string> | undefined,
): ReadonlyArray<TrainingVerificationFailureCode> =>
  [
    ...new Set(
      (codes ?? [])
        .map(code => code.trim())
        .filter((code): code is TrainingVerificationFailureCode =>
          TrainingVerificationFailureCode.literals.includes(
            code as TrainingVerificationFailureCode,
          ),
        ),
    ),
  ].sort()

const jsonPayload = (payloadJson: string): Record<string, unknown> =>
  parseJsonRecord(payloadJson) ?? {}

const numericMatrix = (value: unknown): ReadonlyArray<ReadonlyArray<number>> =>
  Array.isArray(value)
    ? value
        .map(row => (Array.isArray(row) ? row : []))
        .map(row =>
          row
            .map(cell => (typeof cell === 'number' ? Math.trunc(cell) : NaN))
            .filter(Number.isFinite),
        )
    : []

const numericVector = (value: unknown): ReadonlyArray<number> =>
  Array.isArray(value)
    ? value
        .map(cell => (typeof cell === 'number' ? Math.trunc(cell) : NaN))
        .filter(Number.isFinite)
    : []

const positiveInteger = (value: unknown, fallback: number): number =>
  typeof value === 'number' && Number.isInteger(value) && value > 0
    ? value
    : fallback

const modulo = (value: number, modulus: number): number =>
  ((value % modulus) + modulus) % modulus

const matrixVectorProduct = (
  matrix: ReadonlyArray<ReadonlyArray<number>>,
  vector: ReadonlyArray<number>,
  modulus: number,
): ReadonlyArray<number> =>
  matrix.map(row =>
    modulo(
      row.reduce(
        (sum, cell, index) => sum + modulo(cell, modulus) * vector[index]!,
        0,
      ),
      modulus,
    ),
  )

const multiplyMatrices = (
  left: ReadonlyArray<ReadonlyArray<number>>,
  right: ReadonlyArray<ReadonlyArray<number>>,
  modulus: number,
): ReadonlyArray<ReadonlyArray<number>> => {
  const rightColumns = right[0]?.length ?? 0

  return left.map(row =>
    Array.from({ length: rightColumns }, (_, column) =>
      modulo(
        row.reduce(
          (sum, cell, index) =>
            sum + modulo(cell, modulus) * right[index]![column]!,
          0,
        ),
        modulus,
      ),
    ),
  )
}

const matricesSameShape = (
  left: ReadonlyArray<ReadonlyArray<number>>,
  right: ReadonlyArray<ReadonlyArray<number>>,
): boolean =>
  left.length === right.length &&
  left.every((row, index) => row.length === (right[index]?.length ?? -1))

const matrixShapeValid = (
  matrix: ReadonlyArray<ReadonlyArray<number>>,
): boolean =>
  matrix.length > 0 &&
  matrix[0] !== undefined &&
  matrix.every(row => row.length === matrix[0]!.length && row.length > 0)

const rowOpeningRefs = (value: unknown): ReadonlyArray<string> =>
  uniqueRefs(
    Array.isArray(value)
      ? value.map(item => {
          const record = recordFromUnknown(item)

          return typeof record?.rowCommitmentRef === 'string'
            ? record.rowCommitmentRef
            : ''
        })
      : [],
  )

const verdictRefFor = (
  className: TrainingVerificationClass,
  state: 'Verified' | 'Rejected',
  seed: string,
): string =>
  `verdict.training.${className}.${state.toLowerCase()}.${seed
    .replace(/[^A-Za-z0-9_.:-]/g, '_')
    .slice(0, 64)}`

const samplePolicyFailure = (
  challenge: TrainingVerificationChallengeRecord,
  payload: Record<string, unknown>,
): TrainingVerificationFailureCode | undefined => {
  const contributionRefs = uniqueRefs(
    stringArrayFromUnknown(payload.contributionRefs),
  )

  if (
    challenge.samplingPolicy === 'per_contribution' &&
    challenge.contributionRef === null &&
    contributionRefs.length === 0
  ) {
    return 'SamplePolicyRejected'
  }

  return undefined
}

const withSamplingPolicy = (
  challenge: TrainingVerificationChallengeRecord,
  payload: Record<string, unknown>,
  codes: ReadonlyArray<TrainingVerificationFailureCode>,
): ReadonlyArray<TrainingVerificationFailureCode> =>
  uniqueFailureCodes([
    ...codes,
    ...(samplePolicyFailure(challenge, payload) === undefined
      ? []
      : [samplePolicyFailure(challenge, payload)!]),
  ])

const makeVerdict = (
  className: TrainingVerificationClass,
  state: 'Verified' | 'Rejected',
  codes: ReadonlyArray<TrainingVerificationFailureCode>,
  seed: string,
  publicDetails?: Record<string, unknown>,
): TrainingVerificationVerdict => {
  const verdict: TrainingVerificationVerdict = {
    failureCodes: uniqueFailureCodes(codes),
    state,
    verdictRefs: [verdictRefFor(className, state, seed)],
  }

  return publicDetails === undefined ? verdict : { ...verdict, publicDetails }
}

export const verifyFreivaldsMerkle = (
  input: TrainingVerificationVerifierInput,
): TrainingVerificationVerdict => {
  const payload = input.payload
  const modulus = positiveInteger(payload.fieldModulus, 2_147_483_647)
  const left = numericMatrix(payload.leftMatrix)
  const right = numericMatrix(payload.rightMatrix)
  const claimed = numericMatrix(payload.claimedProductMatrix)
  const challengeVector = numericVector(payload.challengeVector)
  const failureCodes: TrainingVerificationFailureCode[] = []

  if (![left, right, claimed].every(matrixShapeValid)) {
    failureCodes.push('DimensionMismatch')
  } else if (
    left[0]!.length !== right.length ||
    claimed.length !== left.length ||
    claimed[0]!.length !== right[0]!.length
  ) {
    failureCodes.push('DimensionMismatch')
  }

  if (
    [...left.flat(), ...right.flat(), ...claimed.flat(), ...challengeVector].some(
      cell => cell < 0 || cell >= modulus,
    )
  ) {
    failureCodes.push('FieldMismatch')
  }

  if (
    matrixShapeValid(claimed) &&
    challengeVector.length !== (claimed[0]?.length ?? -1)
  ) {
    failureCodes.push('DimensionMismatch')
  }

  if (rowOpeningRefs(payload.rowOpenings).length === 0) {
    failureCodes.push('RowOpeningMissing')
  }

  const merkleProofValid = payload.merkleProofValid

  if (merkleProofValid === false) {
    failureCodes.push('MerkleProofInvalid')
  }

  if (failureCodes.length === 0) {
    const rightTimesVector = matrixVectorProduct(right, challengeVector, modulus)
    const leftSide = matrixVectorProduct(left, rightTimesVector, modulus)
    const rightSide = matrixVectorProduct(claimed, challengeVector, modulus)

    if (
      leftSide.length !== rightSide.length ||
      leftSide.some((cell, index) => cell !== rightSide[index])
    ) {
      failureCodes.push('FreivaldsMismatch')
    }

    if (payload.expectExactProduct === true) {
      const product = multiplyMatrices(left, right, modulus)

      if (
        !matricesSameShape(product, claimed) ||
        product.some((row, rowIndex) =>
          row.some((cell, columnIndex) => cell !== claimed[rowIndex]![columnIndex]),
        )
      ) {
        failureCodes.push('FreivaldsMismatch')
      }
    }
  }

  const sampledCodes = withSamplingPolicy(
    input.challenge,
    payload,
    failureCodes,
  )

  return makeVerdict(
    'freivalds_merkle',
    sampledCodes.length === 0 ? 'Verified' : 'Rejected',
    sampledCodes,
    input.challenge.challengeRef,
    {
      fieldModulus: modulus,
      rowOpeningCount: rowOpeningRefs(payload.rowOpenings).length,
    },
  )
}

export const verifyDeterministicRecompute = (
  input: TrainingVerificationVerifierInput,
): TrainingVerificationVerdict => {
  const payload = input.payload
  const expected = typeof payload.expectedDigestRef === 'string'
    ? payload.expectedDigestRef
    : typeof payload.outputDigestRef === 'string'
      ? payload.outputDigestRef
      : undefined
  const actual = typeof payload.recomputedDigestRef === 'string'
    ? payload.recomputedDigestRef
    : undefined
  const failureCodes: TrainingVerificationFailureCode[] = []

  if (expected === undefined || actual === undefined) {
    failureCodes.push('OutputDigestMissing')
  } else if (expected !== actual) {
    failureCodes.push('DigestMismatch')
  }

  const sampledCodes = withSamplingPolicy(
    input.challenge,
    payload,
    failureCodes,
  )

  return makeVerdict(
    'deterministic_recompute',
    sampledCodes.length === 0 ? 'Verified' : 'Rejected',
    sampledCodes,
    input.challenge.challengeRef,
    { digestRef: expected ?? 'digest.missing' },
  )
}

export const verifyExactTraceReplay = (
  input: TrainingVerificationVerifierInput,
): TrainingVerificationVerdict => {
  const payload = input.payload
  const expected = typeof payload.traceCommitmentDigestRef === 'string'
    ? payload.traceCommitmentDigestRef
    : undefined
  const actual = typeof payload.replayDigestRef === 'string'
    ? payload.replayDigestRef
    : undefined
  const window = recordFromUnknown(payload.sampledWindow)
  const failureCodes: TrainingVerificationFailureCode[] = []

  if (
    expected === undefined ||
    actual === undefined ||
    typeof window?.startStep !== 'number' ||
    typeof window?.endStep !== 'number'
  ) {
    failureCodes.push('OutputDigestMissing')
  } else if (
    exactTraceDigestComparable(expected) !== exactTraceDigestComparable(actual)
  ) {
    failureCodes.push('ExecutorTraceMismatch')
  }

  const sampledCodes = withSamplingPolicy(
    input.challenge,
    payload,
    failureCodes,
  )

  return makeVerdict(
    'exact_trace_replay',
    sampledCodes.length === 0 ? 'Verified' : 'Rejected',
    sampledCodes,
    input.challenge.challengeRef,
    {
      sampledWindowRef:
        typeof payload.sampledWindowRef === 'string'
          ? payload.sampledWindowRef
          : 'trace.window.redacted',
    },
  )
}

const exactTraceDigestComparable = (digestRef: string): string => {
  const prefixes = [
    'trace.tassadar.commitment.',
    'trace.tassadar.replay.',
  ] as const
  const prefix = prefixes.find(candidate => digestRef.startsWith(candidate))

  return prefix === undefined ? digestRef : digestRef.slice(prefix.length)
}

const verifyThreshold = (
  className: 'seeded_replication' | 'statistical_cross_check',
  input: TrainingVerificationVerifierInput,
): TrainingVerificationVerdict => {
  const payload = input.payload
  const observed = typeof payload.observedScore === 'number'
    ? payload.observedScore
    : NaN
  const threshold = typeof payload.minimumScore === 'number'
    ? payload.minimumScore
    : 1
  const codes = Number.isFinite(observed) && observed >= threshold
    ? []
    : ['StatisticalThresholdFailed' as const]
  const sampledCodes = withSamplingPolicy(input.challenge, payload, codes)

  return makeVerdict(
    className,
    sampledCodes.length === 0 ? 'Verified' : 'Rejected',
    sampledCodes,
    input.challenge.challengeRef,
    { observedScore: Number.isFinite(observed) ? observed : null, threshold },
  )
}

export const defaultTrainingVerificationRegistry = new Map<
  TrainingVerificationClass,
  TrainingVerificationRegistration
>([
  [
    'freivalds_merkle',
    {
      className: 'freivalds_merkle',
      defaultSamplingPolicy: 'per_contribution',
      failureCodes: [
        'DimensionMismatch',
        'FieldMismatch',
        'FreivaldsMismatch',
        'MerkleProofInvalid',
        'RowOpeningMissing',
      ],
      stalenessPolicy: { kind: 'inherit_run_default' },
      verify: verifyFreivaldsMerkle,
    },
  ],
  [
    'deterministic_recompute',
    {
      className: 'deterministic_recompute',
      defaultSamplingPolicy: 'per_contribution',
      failureCodes: ['DigestMismatch', 'OutputDigestMissing'],
      stalenessPolicy: { kind: 'inherit_run_default' },
      verify: verifyDeterministicRecompute,
    },
  ],
  [
    'exact_trace_replay',
    {
      className: 'exact_trace_replay',
      defaultSamplingPolicy: 'per_contribution',
      failureCodes: ['ExecutorTraceMismatch', 'OutputDigestMissing'],
      stalenessPolicy: { kind: 'inherit_run_default' },
      verify: verifyExactTraceReplay,
    },
  ],
  [
    'statistical_cross_check',
    {
      className: 'statistical_cross_check',
      defaultSamplingPolicy: 'aggregate',
      failureCodes: ['StatisticalThresholdFailed'],
      stalenessPolicy: { kind: 'max_steps_behind_override', maxStepsBehind: 2 },
      verify: input => verifyThreshold('statistical_cross_check', input),
    },
  ],
  [
    'seeded_replication',
    {
      className: 'seeded_replication',
      defaultSamplingPolicy: 'aggregate',
      failureCodes: ['StatisticalThresholdFailed'],
      stalenessPolicy: { kind: 'max_steps_behind_override', maxStepsBehind: 3 },
      verify: input => verifyThreshold('seeded_replication', input),
    },
  ],
])

export const trainingVerificationRegistrationFor = (
  registry: ReadonlyMap<
    TrainingVerificationClass,
    TrainingVerificationRegistration
  >,
  className: TrainingVerificationClass,
): TrainingVerificationRegistration => {
  const registration = registry.get(className)

  if (registration === undefined) {
    throw new TrainingVerificationStoreError({
      kind: 'validation_error',
      reason: `Unknown verification class: ${className}.`,
    })
  }

  return registration
}

export const runTrainingVerificationClass = async (
  input: Readonly<{
    challenge: TrainingVerificationChallengeRecord
    registry?: ReadonlyMap<
      TrainingVerificationClass,
      TrainingVerificationRegistration
    >
  }>,
): Promise<TrainingVerificationVerdict> => {
  const registry = input.registry ?? defaultTrainingVerificationRegistry
  const registration = trainingVerificationRegistrationFor(
    registry,
    input.challenge.verificationClass,
  )

  return registration.verify({
    challenge: input.challenge,
    payload: jsonPayload(input.challenge.payloadJson),
  })
}

export const publicTrainingVerificationChallengeProjection = (
  record: TrainingVerificationChallengeRecord,
  nowIso: string,
): TrainingVerificationChallengeProjection => ({
  challengeRef: record.challengeRef,
  commitmentRefs: uniqueRefs(record.commitmentRefs),
  contributionRef: record.contributionRef,
  createdAtDisplay: friendlyBlueprintMissionBriefingTime(
    record.createdAt,
    nowIso,
  ),
  failureCodes: uniqueFailureCodes(record.failureCodes),
  homeworkKind: record.homeworkKind,
  leaseExpiresInSeconds: record.leaseExpiresAt === null
    ? null
    : Math.max(
        0,
        Math.floor(
          (Date.parse(record.leaseExpiresAt) - Date.parse(nowIso)) / 1000,
        ),
      ),
  leasedToRef: record.leasedToRef,
  samplingPolicy: record.samplingPolicy,
  state: record.state,
  trainingRunRef: record.trainingRunRef,
  updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
    record.updatedAt,
    nowIso,
  ),
  verdictRefs: uniqueRefs(record.verdictRefs),
  verificationClass: record.verificationClass,
  windowRef: record.windowRef,
  exactTraceCommitmentDigestRef:
    typeof jsonPayload(record.payloadJson).traceCommitmentDigestRef === 'string'
      ? (jsonPayload(record.payloadJson).traceCommitmentDigestRef as string)
      : null,
  exactTraceReplayDigestRef:
    typeof jsonPayload(record.payloadJson).replayDigestRef === 'string'
      ? (jsonPayload(record.payloadJson).replayDigestRef as string)
      : null,
})

const attemptCount = (
  challenge: TrainingVerificationChallengeRecord,
): number =>
  challenge.state === 'Queued'
    ? 0
    : Math.max(0, Math.trunc(Number(jsonPayload(challenge.payloadJson).attemptCount ?? 0)))

const payloadWithAttemptCount = (
  challenge: TrainingVerificationChallengeRecord,
  nextAttemptCount: number,
): string =>
  JSON.stringify({
    ...jsonPayload(challenge.payloadJson),
    attemptCount: nextAttemptCount,
  })

export const buildTrainingVerificationChallengeRecord = (
  input: Readonly<{
    makeId: () => string
    nowIso: string
    registry?: ReadonlyMap<
      TrainingVerificationClass,
      TrainingVerificationRegistration
    >
    request: TrainingVerificationChallengeCreateRequest
  }>,
): Readonly<{
  challenge: TrainingVerificationChallengeRecord
  event: TrainingVerificationChallengeEventRecord
}> => {
  const registry = input.registry ?? defaultTrainingVerificationRegistry
  const registration = trainingVerificationRegistrationFor(
    registry,
    input.request.verificationClass,
  )
  const id = input.makeId()
  const challenge: TrainingVerificationChallengeRecord = {
    challengeRef: `training.verification.challenge.${id}`,
    commitmentRefs: uniqueRefs(input.request.commitmentRefs),
    contributionRef: input.request.contributionRef ?? null,
    createdAt: input.nowIso,
    failureCodes: [],
    homeworkKind: input.request.homeworkKind,
    id: `training_verification_challenge_${id}`,
    leaseExpiresAt: null,
    leaseRef: null,
    leasedToRef: null,
    maxAttempts: Math.max(1, Math.trunc(input.request.maxAttempts ?? 3)),
    payloadJson: JSON.stringify({ ...input.request.payload, attemptCount: 0 }),
    publicProjectionJson: '{}',
    rejectedAt: null,
    samplingPolicy:
      input.request.samplingPolicy ?? registration.defaultSamplingPolicy,
    state: 'Queued',
    timedOutAt: null,
    trainingRunRef: input.request.trainingRunRef,
    updatedAt: input.nowIso,
    verdictRefs: [],
    verificationClass: input.request.verificationClass,
    verifiedAt: null,
    windowRef: input.request.windowRef ?? null,
  }

  return {
    challenge: {
      ...challenge,
      publicProjectionJson: JSON.stringify(
        publicTrainingVerificationChallengeProjection(challenge, input.nowIso),
      ),
    },
    event: {
      challengeRef: challenge.challengeRef,
      createdAt: input.nowIso,
      failureCodes: [],
      id: `training_verification_event_${id}_queued`,
      receiptRefs: [],
      stateFrom: null,
      stateTo: 'Queued',
      transitionKind: 'challenge_queued',
      validatorRef: null,
    },
  }
}

const leaseSecondsForRequest = (
  request: TrainingVerificationChallengeLeaseRequest,
): number => {
  const leaseSeconds = request.leaseSeconds ?? 15 * 60

  if (
    !Number.isFinite(leaseSeconds) ||
    leaseSeconds < 60 ||
    leaseSeconds > 86_400
  ) {
    throw new TrainingVerificationStoreError({
      kind: 'validation_error',
      reason: 'leaseSeconds must be between 60 and 86400.',
    })
  }

  return Math.floor(leaseSeconds)
}

export const leaseTrainingVerificationChallengeRecord = (
  input: Readonly<{
    challenge: TrainingVerificationChallengeRecord
    eventId: string
    nowIso: string
    request: TrainingVerificationChallengeLeaseRequest
  }>,
): Readonly<{
  challenge: TrainingVerificationChallengeRecord
  event: TrainingVerificationChallengeEventRecord
}> => {
  if (!['Queued', 'Retrying'].includes(input.challenge.state)) {
    throw new TrainingVerificationStoreError({
      kind: 'conflict',
      reason: `Cannot lease challenge from ${input.challenge.state}.`,
    })
  }

  const nextAttemptCount = attemptCount(input.challenge) + 1
  const nextChallenge: TrainingVerificationChallengeRecord = {
    ...input.challenge,
    leaseExpiresAt: isoTimestampAfterIso(
      input.nowIso,
      leaseSecondsForRequest(input.request) * 1000,
    ),
    leaseRef: `training.verification.lease.${input.eventId}`,
    leasedToRef: input.request.validatorRef,
    payloadJson: payloadWithAttemptCount(input.challenge, nextAttemptCount),
    state: 'Leased',
    updatedAt: input.nowIso,
  }

  return {
    challenge: {
      ...nextChallenge,
      publicProjectionJson: JSON.stringify(
        publicTrainingVerificationChallengeProjection(
          nextChallenge,
          input.nowIso,
        ),
      ),
    },
    event: {
      challengeRef: input.challenge.challengeRef,
      createdAt: input.nowIso,
      failureCodes: [],
      id: `training_verification_event_${input.eventId}_leased`,
      receiptRefs: [],
      stateFrom: input.challenge.state,
      stateTo: 'Leased',
      transitionKind: 'challenge_leased',
      validatorRef: input.request.validatorRef,
    },
  }
}

export const retryTrainingVerificationChallengeRecord = (
  input: Readonly<{
    challenge: TrainingVerificationChallengeRecord
    eventId: string
    nowIso: string
    request: TrainingVerificationChallengeRetryRequest
    validatorRef?: string
  }>,
): Readonly<{
  challenge: TrainingVerificationChallengeRecord
  event: TrainingVerificationChallengeEventRecord
}> => {
  if (input.challenge.state !== 'Leased') {
    throw new TrainingVerificationStoreError({
      kind: 'conflict',
      reason: `Cannot retry challenge from ${input.challenge.state}.`,
    })
  }

  const expired =
    input.challenge.leaseExpiresAt !== null &&
    Date.parse(input.challenge.leaseExpiresAt) <= Date.parse(input.nowIso)
  const nextAttemptCount = attemptCount(input.challenge)
  const exhausted = nextAttemptCount >= input.challenge.maxAttempts
  const failureCodes = uniqueFailureCodes([
    ...input.challenge.failureCodes,
    ...(input.request.failureCodes ?? []),
    ...(expired ? ['LeaseExpired' as const] : []),
    ...(exhausted ? ['RetryBudgetExhausted' as const] : []),
  ])
  const nextState: TrainingVerificationChallengeState = exhausted
    ? 'TimedOut'
    : 'Retrying'
  const nextChallenge: TrainingVerificationChallengeRecord = {
    ...input.challenge,
    failureCodes,
    leaseExpiresAt: null,
    leaseRef: null,
    leasedToRef: null,
    state: nextState,
    timedOutAt: nextState === 'TimedOut' ? input.nowIso : null,
    updatedAt: input.nowIso,
  }

  return {
    challenge: {
      ...nextChallenge,
      publicProjectionJson: JSON.stringify(
        publicTrainingVerificationChallengeProjection(
          nextChallenge,
          input.nowIso,
        ),
      ),
    },
    event: {
      challengeRef: input.challenge.challengeRef,
      createdAt: input.nowIso,
      failureCodes,
      id: `training_verification_event_${input.eventId}_retry`,
      receiptRefs: uniqueRefs(input.request.receiptRefs),
      stateFrom: input.challenge.state,
      stateTo: nextState,
      transitionKind:
        nextState === 'TimedOut' ? 'challenge_timed_out' : 'challenge_retry',
      validatorRef: input.validatorRef ?? input.challenge.leasedToRef,
    },
  }
}

export const finalizeTrainingVerificationChallengeRecord = (
  input: Readonly<{
    challenge: TrainingVerificationChallengeRecord
    eventId: string
    nowIso: string
    request: TrainingVerificationChallengeFinalizeRequest
    validatorRef?: string
    verdict: TrainingVerificationVerdict
  }>,
): Readonly<{
  challenge: TrainingVerificationChallengeRecord
  event: TrainingVerificationChallengeEventRecord
}> => {
  if (input.challenge.state !== 'Leased') {
    throw new TrainingVerificationStoreError({
      kind: 'conflict',
      reason: `Cannot finalize challenge from ${input.challenge.state}.`,
    })
  }

  const state = input.verdict.state
  const failureCodes = uniqueFailureCodes([
    ...input.challenge.failureCodes,
    ...input.verdict.failureCodes,
  ])
  const nextChallenge: TrainingVerificationChallengeRecord = {
    ...input.challenge,
    failureCodes,
    leaseExpiresAt: null,
    leaseRef: null,
    leasedToRef: null,
    publicProjectionJson: '{}',
    rejectedAt: state === 'Rejected' ? input.nowIso : null,
    state,
    updatedAt: input.nowIso,
    verdictRefs: uniqueRefs([
      ...input.challenge.verdictRefs,
      ...input.verdict.verdictRefs,
    ]),
    verifiedAt: state === 'Verified' ? input.nowIso : null,
  }

  return {
    challenge: {
      ...nextChallenge,
      publicProjectionJson: JSON.stringify(
        publicTrainingVerificationChallengeProjection(
          nextChallenge,
          input.nowIso,
        ),
      ),
    },
    event: {
      challengeRef: input.challenge.challengeRef,
      createdAt: input.nowIso,
      failureCodes,
      id: `training_verification_event_${input.eventId}_finalized`,
      receiptRefs: uniqueRefs(input.request.receiptRefs),
      stateFrom: input.challenge.state,
      stateTo: state,
      transitionKind:
        state === 'Verified' ? 'challenge_verified' : 'challenge_rejected',
      validatorRef: input.validatorRef ?? input.challenge.leasedToRef,
    },
  }
}

export const timeOutTrainingVerificationChallengeRecord = (
  input: Readonly<{
    challenge: TrainingVerificationChallengeRecord
    eventId: string
    nowIso: string
    validatorRef?: string
  }>,
): Readonly<{
  challenge: TrainingVerificationChallengeRecord
  event: TrainingVerificationChallengeEventRecord
}> => {
  if (['Verified', 'Rejected', 'TimedOut'].includes(input.challenge.state)) {
    throw new TrainingVerificationStoreError({
      kind: 'conflict',
      reason: `Cannot time out terminal challenge from ${input.challenge.state}.`,
    })
  }

  const failureCodes = uniqueFailureCodes([
    ...input.challenge.failureCodes,
    'LeaseExpired',
    'RetryBudgetExhausted',
  ])
  const nextChallenge: TrainingVerificationChallengeRecord = {
    ...input.challenge,
    failureCodes,
    leaseExpiresAt: null,
    leaseRef: null,
    leasedToRef: null,
    state: 'TimedOut',
    timedOutAt: input.nowIso,
    updatedAt: input.nowIso,
  }

  return {
    challenge: {
      ...nextChallenge,
      publicProjectionJson: JSON.stringify(
        publicTrainingVerificationChallengeProjection(
          nextChallenge,
          input.nowIso,
        ),
      ),
    },
    event: {
      challengeRef: input.challenge.challengeRef,
      createdAt: input.nowIso,
      failureCodes,
      id: `training_verification_event_${input.eventId}_timeout`,
      receiptRefs: [],
      stateFrom: input.challenge.state,
      stateTo: 'TimedOut',
      transitionKind: 'challenge_timed_out',
      validatorRef: input.validatorRef ?? input.challenge.leasedToRef,
    },
  }
}

export const trainingVerificationStoreErrorFromUnknown = (
  error: unknown,
): TrainingVerificationStoreError =>
  error instanceof TrainingVerificationStoreError
    ? error
    : new TrainingVerificationStoreError({
        kind: 'storage_error',
        reason: error instanceof Error ? error.message : String(error),
      })

export const rowToTrainingVerificationChallenge = (
  row: TrainingVerificationRow,
): TrainingVerificationChallengeRecord => ({
  challengeRef: row.challenge_ref,
  commitmentRefs: parseJsonStringArray(row.commitment_refs_json),
  contributionRef: row.contribution_ref,
  createdAt: row.created_at,
  failureCodes: uniqueFailureCodes(parseJsonStringArray(row.failure_codes_json)),
  homeworkKind: row.homework_kind,
  id: row.id,
  leaseExpiresAt: row.lease_expires_at,
  leaseRef: row.lease_ref,
  leasedToRef: row.leased_to_ref,
  maxAttempts: row.max_attempts,
  payloadJson: row.payload_json,
  publicProjectionJson: row.public_projection_json,
  rejectedAt: row.rejected_at,
  samplingPolicy: row.sampling_policy,
  state: row.state,
  timedOutAt: row.timed_out_at,
  trainingRunRef: row.training_run_ref,
  updatedAt: row.updated_at,
  verdictRefs: parseJsonStringArray(row.verdict_refs_json),
  verificationClass: row.verification_class,
  verifiedAt: row.verified_at,
  windowRef: row.window_ref,
})

const bindChallenge = (
  statement: D1PreparedStatement,
  challenge: TrainingVerificationChallengeRecord,
): D1PreparedStatement =>
  statement.bind(
    challenge.id,
    challenge.challengeRef,
    challenge.trainingRunRef,
    challenge.windowRef,
    challenge.contributionRef,
    challenge.homeworkKind,
    challenge.verificationClass,
    challenge.samplingPolicy,
    challenge.state,
    attemptCount(challenge),
    challenge.maxAttempts,
    challenge.leaseRef,
    challenge.leasedToRef,
    challenge.leaseExpiresAt,
    challenge.payloadJson,
    JSON.stringify(challenge.commitmentRefs),
    JSON.stringify(challenge.failureCodes),
    JSON.stringify(challenge.verdictRefs),
    challenge.publicProjectionJson,
    challenge.createdAt,
    challenge.updatedAt,
    challenge.verifiedAt,
    challenge.rejectedAt,
    challenge.timedOutAt,
  )

const bindEvent = (
  statement: D1PreparedStatement,
  event: TrainingVerificationChallengeEventRecord,
): D1PreparedStatement =>
  statement.bind(
    event.id,
    event.challengeRef,
    event.transitionKind,
    event.stateFrom,
    event.stateTo,
    event.validatorRef,
    JSON.stringify(event.failureCodes),
    JSON.stringify(event.receiptRefs),
    event.createdAt,
  )

export const makeD1TrainingVerificationStore = (
  db: D1Database,
): TrainingVerificationStore => ({
  createChallenge: async (challenge, event) => {
    await db.batch([
      bindChallenge(
        db.prepare(
          `INSERT INTO training_verification_challenges
            (id, challenge_ref, training_run_ref, window_ref, contribution_ref,
             homework_kind, verification_class, sampling_policy, state,
             attempt_count, max_attempts, lease_ref, leased_to_ref,
             lease_expires_at, payload_json, commitment_refs_json,
             failure_codes_json, verdict_refs_json, public_projection_json,
             created_at, updated_at, verified_at, rejected_at, timed_out_at,
             archived_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
        ),
        challenge,
      ),
      bindEvent(
        db.prepare(
          `INSERT INTO training_verification_events
            (id, challenge_ref, transition_kind, state_from, state_to,
             validator_ref, failure_codes_json, receipt_refs_json, created_at,
             archived_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
        ),
        event,
      ),
    ])

    return challenge
  },
  leaseChallenge: async (challenge, event) => {
    await db.batch([
      db
        .prepare(
          `UPDATE training_verification_challenges
              SET state = ?,
                  attempt_count = ?,
                  lease_ref = ?,
                  leased_to_ref = ?,
                  lease_expires_at = ?,
                  payload_json = ?,
                  public_projection_json = ?,
                  updated_at = ?
            WHERE challenge_ref = ?
              AND archived_at IS NULL`,
        )
        .bind(
          challenge.state,
          attemptCount(challenge),
          challenge.leaseRef,
          challenge.leasedToRef,
          challenge.leaseExpiresAt,
          challenge.payloadJson,
          challenge.publicProjectionJson,
          challenge.updatedAt,
          challenge.challengeRef,
        ),
      bindEvent(
        db.prepare(
          `INSERT INTO training_verification_events
            (id, challenge_ref, transition_kind, state_from, state_to,
             validator_ref, failure_codes_json, receipt_refs_json, created_at,
             archived_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
        ),
        event,
      ),
    ])

    return challenge
  },
  listLeaseCandidates: async (nowIso, limit, verificationClass) => {
    const result =
      verificationClass === undefined
        ? await db
            .prepare(
              `SELECT *
                 FROM training_verification_challenges
                WHERE state IN ('Queued', 'Retrying')
                  AND archived_at IS NULL
                ORDER BY created_at ASC
                LIMIT ?`,
            )
            .bind(limit)
            .all<TrainingVerificationRow>()
        : await db
            .prepare(
              `SELECT *
                 FROM training_verification_challenges
                WHERE state IN ('Queued', 'Retrying')
                  AND verification_class = ?
                  AND archived_at IS NULL
                ORDER BY created_at ASC
                LIMIT ?`,
            )
            .bind(verificationClass, limit)
            .all<TrainingVerificationRow>()

    return (result.results ?? []).map(rowToTrainingVerificationChallenge)
  },
  readChallenge: async challengeRef => {
    const row = await db
      .prepare(
        `SELECT *
           FROM training_verification_challenges
          WHERE challenge_ref = ?
            AND archived_at IS NULL`,
      )
      .bind(challengeRef)
      .first<TrainingVerificationRow>()

    return row === null ? undefined : rowToTrainingVerificationChallenge(row)
  },
  transitionChallenge: async (challenge, event) => {
    await db.batch([
      db
        .prepare(
          `UPDATE training_verification_challenges
              SET state = ?,
                  lease_ref = ?,
                  leased_to_ref = ?,
                  lease_expires_at = ?,
                  failure_codes_json = ?,
                  verdict_refs_json = ?,
                  public_projection_json = ?,
                  updated_at = ?,
                  verified_at = ?,
                  rejected_at = ?,
                  timed_out_at = ?
            WHERE challenge_ref = ?
              AND archived_at IS NULL`,
        )
        .bind(
          challenge.state,
          challenge.leaseRef,
          challenge.leasedToRef,
          challenge.leaseExpiresAt,
          JSON.stringify(challenge.failureCodes),
          JSON.stringify(challenge.verdictRefs),
          challenge.publicProjectionJson,
          challenge.updatedAt,
          challenge.verifiedAt,
          challenge.rejectedAt,
          challenge.timedOutAt,
          challenge.challengeRef,
        ),
      bindEvent(
        db.prepare(
          `INSERT INTO training_verification_events
            (id, challenge_ref, transition_kind, state_from, state_to,
             validator_ref, failure_codes_json, receipt_refs_json, created_at,
             archived_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
        ),
        event,
      ),
    ])

    return challenge
  },
})
