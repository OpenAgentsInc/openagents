/**
 * Validator verdict schema v0.1 and the four-tier validation ladder for
 * the Tassadar verified trace factory (issue #4748).
 *
 * Tier 0 — schema/hash: structural decode plus recomputing the full
 *          trace digest from the token stream. No re-execution.
 * Tier 1 — full replay: independent re-execution of the regenerated
 *          workload; required before any record from a new worker,
 *          profile, or family leaves quarantine.
 * Tier 2 — window spot-checks, sampled by worker reputation.
 * Tier 3 — random adversarial replay over already-admitted records.
 *
 * Iron rules, enforced as code rather than prose:
 *   1. never train from unverified artifacts (`trainingEligibility`);
 *   2. expected digests never ship in generation assignments
 *      (`generationAssignmentDigestViolations`);
 *   3. quarantine-before-admission for new workers
 *      (`admissionDecision`).
 */
import {
  executeTassadarNumericModel,
  TassadarNumericExecutionError,
  type TassadarAlmNumericModel,
} from '@openagentsinc/tassadar-executor'

import {
  finalOutputDigestFromTokens,
  fullTraceDigestFromTokens,
  TASSADAR_TRACE_PROFILE_VERSION,
  traceTokensFromStepOutputs,
  type TassadarTraceRecord,
  type TassadarValidationTier,
  type TassadarValidatorReceipt,
} from './trace-record'

export const TASSADAR_VALIDATOR_VERDICT_SCHEMA_VERSION =
  'validator_verdict.v0.1'
export const TASSADAR_TRACE_FACTORY_REPLAY_CLASS_ID =
  'exact_trace_replay.trace_factory.v0_1'

export type TassadarVerdictRejection = Readonly<
  | { kind: 'schema_invalid'; detail: string }
  | { kind: 'profile_unknown'; detail: string }
  | { kind: 'token_digest_mismatch'; detail: string }
  | { kind: 'final_output_digest_mismatch'; detail: string }
  | { kind: 'trace_digest_mismatch'; detail: string }
  | { kind: 'program_hash_mismatch'; detail: string }
  | { kind: 'step_count_mismatch'; detail: string }
  | { kind: 'row_mismatch'; detail: string }
  | { kind: 'execution_refused'; detail: string }
>

export type TassadarValidationMethod =
  | 'schema_hash'
  | 'full_replay'
  | 'window_spot_check'
  | 'adversarial_replay'

export type TassadarValidatorVerdict = Readonly<{
  verdictSchemaVersion: typeof TASSADAR_VALIDATOR_VERDICT_SCHEMA_VERSION
  classId: typeof TASSADAR_TRACE_FACTORY_REPLAY_CLASS_ID
  recordId: string
  programHash: string
  tier: TassadarValidationTier
  method: TassadarValidationMethod
  outcome: 'verified' | 'rejected'
  rejection: TassadarVerdictRejection | null
  validatorDeviceRef: string
  replayedSteps: number
  comparedSteps: number
  validatedAtIso: string
}>

export type TassadarValidationTierPolicy = Readonly<{
  tier: TassadarValidationTier
  method: TassadarValidationMethod
  trigger:
    | 'every_record'
    | 'new_worker_or_profile_or_family'
    | 'reputation_sampled'
    | 'random_adversarial'
  reExecution: 'none' | 'window' | 'full'
}>

/** The frozen four-tier ladder. Order is the order of escalation. */
export const TASSADAR_VALIDATION_TIER_LADDER: ReadonlyArray<TassadarValidationTierPolicy> =
  [
    { method: 'schema_hash', reExecution: 'none', tier: 0, trigger: 'every_record' },
    {
      method: 'full_replay',
      reExecution: 'full',
      tier: 1,
      trigger: 'new_worker_or_profile_or_family',
    },
    {
      method: 'window_spot_check',
      reExecution: 'window',
      tier: 2,
      trigger: 'reputation_sampled',
    },
    {
      method: 'adversarial_replay',
      reExecution: 'full',
      tier: 3,
      trigger: 'random_adversarial',
    },
  ]

export type TassadarWorkerStanding = Readonly<{
  workerRef: string
  isNewWorker: boolean
  isNewProfile: boolean
  isNewFamily: boolean
  verifiedRecordCount: number
}>

/**
 * Quarantine-before-admission: a record from a new worker, new profile,
 * or new family requires a Tier 1 full replay before admission; a
 * record from an established worker still requires Tier 0 plus at least
 * one replay-bearing verdict (Tier 1, 2, or 3).
 */
export const requiredAdmissionTier = (
  standing: TassadarWorkerStanding,
): TassadarValidationTier =>
  standing.isNewWorker || standing.isNewProfile || standing.isNewFamily
    ? 1
    : 2

export type TassadarAdmissionDecision = Readonly<
  | { status: 'admitted'; satisfiedTier: TassadarValidationTier }
  | { status: 'quarantined'; reason: string; requiredTier: TassadarValidationTier }
  | { status: 'rejected'; rejectionKinds: ReadonlyArray<string> }
>

const verifiedReceiptTiers = (
  receipts: ReadonlyArray<TassadarValidatorReceipt>,
): ReadonlyArray<TassadarValidationTier> =>
  receipts
    .filter(receipt => receipt.outcome === 'verified')
    .map(receipt => receipt.tier)

export const admissionDecision = (
  record: TassadarTraceRecord,
  standing: TassadarWorkerStanding,
): TassadarAdmissionDecision => {
  const rejectedKinds = record.validatorReceipts
    .filter(receipt => receipt.outcome === 'rejected')
    .map(receipt => receipt.rejectionKind ?? 'unspecified')
  if (rejectedKinds.length > 0) {
    return { rejectionKinds: rejectedKinds, status: 'rejected' }
  }
  const verifiedTiers = verifiedReceiptTiers(record.validatorReceipts)
  const requiredTier = requiredAdmissionTier(standing)
  if (!verifiedTiers.includes(0)) {
    return {
      reason: 'no verified Tier 0 schema/hash receipt',
      requiredTier,
      status: 'quarantined',
    }
  }
  const satisfied = verifiedTiers.find(tier => tier >= requiredTier)
  if (satisfied === undefined) {
    return {
      reason: `no verified replay receipt at tier >= ${requiredTier} (worker ${standing.workerRef} standing requires it before admission)`,
      requiredTier,
      status: 'quarantined',
    }
  }

  return { satisfiedTier: satisfied, status: 'admitted' }
}

export type TassadarTrainingEligibility = Readonly<
  | { eligible: true; admittedTier: TassadarValidationTier }
  | { eligible: false; reason: string }
>

/**
 * Iron rule 1: never train from unverified artifacts. Eligibility
 * requires a verified Tier 0 receipt plus a verified full-replay-class
 * receipt (Tier 1 or Tier 3). There is no override parameter on
 * purpose.
 */
export const trainingEligibility = (
  record: TassadarTraceRecord,
): TassadarTrainingEligibility => {
  const verifiedTiers = verifiedReceiptTiers(record.validatorReceipts)
  if (!verifiedTiers.includes(0)) {
    return {
      eligible: false,
      reason: `record ${record.recordId} has no verified Tier 0 schema/hash receipt`,
    }
  }
  const replayTier = verifiedTiers.find(tier => tier === 1 || tier === 3)
  if (replayTier === undefined) {
    return {
      eligible: false,
      reason: `record ${record.recordId} has no verified full-replay receipt (tier 1 or 3); unverified artifacts never reach training`,
    }
  }

  return { admittedTier: replayTier, eligible: true }
}

const BANNED_GENERATION_ASSIGNMENT_KEYS = [
  'claimedtracedigest',
  'expecteddigest',
  'expected_digest',
  'expectedtracedigest',
  'expected_trace_digest',
  'expectedfinalrow',
  'expected_final_row',
  'expectedoutputs',
  'expected_outputs',
  'fulltracedigest',
  'full_trace_digest',
  'finaloutputdigest',
  'final_output_digest',
] as const

export type TassadarDigestBanViolation = Readonly<{
  path: string
  key: string
}>

/**
 * Iron rule 2: expected digests never ship in generation assignments.
 * Recursively scans an assignment payload for digest-bearing keys and
 * returns every violation, typed.
 */
export const generationAssignmentDigestViolations = (
  payload: unknown,
  path = '$',
): ReadonlyArray<TassadarDigestBanViolation> => {
  if (Array.isArray(payload)) {
    return payload.flatMap((entry, index) =>
      generationAssignmentDigestViolations(entry, `${path}[${index}]`),
    )
  }
  if (payload === null || typeof payload !== 'object') return []

  return Object.entries(payload as Record<string, unknown>).flatMap(
    ([key, value]) => {
      const normalized = key.toLowerCase()
      const own = (
        BANNED_GENERATION_ASSIGNMENT_KEYS as ReadonlyArray<string>
      ).includes(normalized)
        ? [{ key, path: `${path}.${key}` }]
        : []

      return [
        ...own,
        ...generationAssignmentDigestViolations(value, `${path}.${key}`),
      ]
    },
  )
}

export type TassadarValidationContext = Readonly<{
  validatorDeviceRef: string
  validatedAtIso: string
  knownProfileVersions?: ReadonlyArray<string>
}>

const verdictBase = (
  record: TassadarTraceRecord,
  tier: TassadarValidationTier,
  method: TassadarValidationMethod,
  context: TassadarValidationContext,
) =>
  ({
    classId: TASSADAR_TRACE_FACTORY_REPLAY_CLASS_ID,
    method,
    programHash: record.programHash,
    recordId: record.recordId,
    tier,
    validatedAtIso: context.validatedAtIso,
    validatorDeviceRef: context.validatorDeviceRef,
    verdictSchemaVersion: TASSADAR_VALIDATOR_VERDICT_SCHEMA_VERSION,
  }) as const

const rejected = (
  record: TassadarTraceRecord,
  tier: TassadarValidationTier,
  method: TassadarValidationMethod,
  context: TassadarValidationContext,
  rejection: TassadarVerdictRejection,
  replayedSteps = 0,
  comparedSteps = 0,
): TassadarValidatorVerdict => ({
  ...verdictBase(record, tier, method, context),
  comparedSteps,
  outcome: 'rejected',
  rejection,
  replayedSteps,
})

/**
 * Tier 0: schema and hash consistency without re-execution. Verifies
 * the record's schema/profile, the offset structure, and that the
 * claimed full-trace and final-output digests are exactly what the
 * carried token stream hashes to.
 */
export const runTierZeroValidation = async (
  record: TassadarTraceRecord,
  context: TassadarValidationContext,
): Promise<TassadarValidatorVerdict> => {
  const knownProfiles = context.knownProfileVersions ?? [
    TASSADAR_TRACE_PROFILE_VERSION,
  ]
  if (!knownProfiles.includes(record.profileVersion)) {
    return rejected(record, 0, 'schema_hash', context, {
      detail: `profile ${record.profileVersion} is not in the validator's known profile set`,
      kind: 'profile_unknown',
    })
  }
  if (record.stepOffsets.length !== record.stepCount) {
    return rejected(record, 0, 'schema_hash', context, {
      detail: `step offsets length ${record.stepOffsets.length} does not equal step count ${record.stepCount}`,
      kind: 'schema_invalid',
    })
  }
  const recomputedFull = await fullTraceDigestFromTokens(
    record.programHash,
    record.traceTokenIds,
    record.stepOffsets,
    record.tokenWidth,
  )
  if (recomputedFull !== record.fullTraceDigest) {
    return rejected(record, 0, 'schema_hash', context, {
      detail: `token stream hashes to ${recomputedFull}, record claims ${record.fullTraceDigest}`,
      kind: 'token_digest_mismatch',
    })
  }
  const recomputedFinal = await finalOutputDigestFromTokens(
    record.programHash,
    record.traceTokenIds,
    record.stepOffsets,
    record.tokenWidth,
  )
  if (recomputedFinal !== record.finalOutputDigest) {
    return rejected(record, 0, 'schema_hash', context, {
      detail: `final row hashes to ${recomputedFinal}, record claims ${record.finalOutputDigest}`,
      kind: 'final_output_digest_mismatch',
    })
  }

  return {
    ...verdictBase(record, 0, 'schema_hash', context),
    comparedSteps: record.stepCount,
    outcome: 'verified',
    rejection: null,
    replayedSteps: 0,
  }
}

export type TassadarReplayWorkload = Readonly<{
  model: TassadarAlmNumericModel
  steps: ReadonlyArray<ReadonlyArray<number>>
}>

const runFullReplay = async (
  record: TassadarTraceRecord,
  workload: TassadarReplayWorkload,
  tier: 1 | 3,
  method: TassadarValidationMethod,
  context: TassadarValidationContext,
): Promise<TassadarValidatorVerdict> => {
  if (workload.model.graph_digest !== record.programHash) {
    return rejected(record, tier, method, context, {
      detail: `regenerated model digest ${workload.model.graph_digest} does not match record program hash ${record.programHash}`,
      kind: 'program_hash_mismatch',
    })
  }
  if (workload.steps.length !== record.stepCount) {
    return rejected(record, tier, method, context, {
      detail: `regenerated workload has ${workload.steps.length} steps, record claims ${record.stepCount}`,
      kind: 'step_count_mismatch',
    })
  }
  try {
    const trace = await executeTassadarNumericModel(
      workload.model,
      workload.steps,
    )
    if (trace.traceDigest !== record.fullTraceDigest) {
      return rejected(
        record,
        tier,
        method,
        context,
        {
          detail: `independent replay produced ${trace.traceDigest}, record claims ${record.fullTraceDigest}`,
          kind: 'trace_digest_mismatch',
        },
        trace.stepCount,
        trace.stepCount,
      )
    }
    const tokenized = traceTokensFromStepOutputs(
      trace.stepOutputs,
      record.tokenWidth,
    )
    const replayedTokens = tokenized.tokens
    if (replayedTokens.length !== record.traceTokenIds.length) {
      return rejected(
        record,
        tier,
        method,
        context,
        {
          detail: `replayed token stream length ${replayedTokens.length} differs from record's ${record.traceTokenIds.length}`,
          kind: 'row_mismatch',
        },
        trace.stepCount,
        trace.stepCount,
      )
    }
    for (let index = 0; index < replayedTokens.length; index += 1) {
      if (replayedTokens[index] !== record.traceTokenIds[index]) {
        return rejected(
          record,
          tier,
          method,
          context,
          {
            detail: `first token divergence at token index ${index}`,
            kind: 'row_mismatch',
          },
          trace.stepCount,
          trace.stepCount,
        )
      }
    }

    return {
      ...verdictBase(record, tier, method, context),
      comparedSteps: trace.stepCount,
      outcome: 'verified',
      rejection: null,
      replayedSteps: trace.stepCount,
    }
  } catch (error: unknown) {
    if (error instanceof TassadarNumericExecutionError) {
      const refusal: TassadarNumericExecutionError = error

      return rejected(record, tier, method, context, {
        detail: refusal.message,
        kind: 'execution_refused',
      })
    }
    throw error
  }
}

/** Tier 1: full independent replay of the regenerated workload. */
export const runTierOneFullReplay = (
  record: TassadarTraceRecord,
  workload: TassadarReplayWorkload,
  context: TassadarValidationContext,
): Promise<TassadarValidatorVerdict> =>
  runFullReplay(record, workload, 1, 'full_replay', context)

/** Tier 2: replay with comparison restricted to a sampled step window. */
export const runTierTwoWindowSpotCheck = async (
  record: TassadarTraceRecord,
  workload: TassadarReplayWorkload,
  window: Readonly<{ startStep: number; endStep: number }>,
  context: TassadarValidationContext,
): Promise<TassadarValidatorVerdict> => {
  const full = await runFullReplay(
    record,
    workload,
    1,
    'window_spot_check',
    context,
  )
  const comparedSteps = Math.max(
    0,
    Math.min(window.endStep, record.stepCount) - Math.max(0, window.startStep),
  )

  return { ...full, comparedSteps, tier: 2 }
}

/**
 * Tier 3 selection: deterministic, seed-derived adversarial sampling
 * over admitted records. The selection seed is published AFTER
 * generation closes, so workers cannot predict which records face
 * adversarial replay.
 */
export const adversarialReplaySelection = (
  recordIds: ReadonlyArray<string>,
  selectionSeedHex: string,
  sampleEvery: number,
): ReadonlyArray<string> => {
  const seed = /^[0-9a-f]{1,16}$/.test(selectionSeedHex)
    ? BigInt(`0x${selectionSeedHex}`)
    : 0n

  return recordIds.filter((recordId, index) => {
    let hash = seed ^ BigInt(index + 1)
    for (const char of recordId) {
      hash = (hash * 31n + BigInt(char.charCodeAt(0))) & ((1n << 64n) - 1n)
    }

    return hash % BigInt(Math.max(1, sampleEvery)) === 0n
  })
}

/** Tier 3: full replay run under the adversarial method label. */
export const runTierThreeAdversarialReplay = (
  record: TassadarTraceRecord,
  workload: TassadarReplayWorkload,
  context: TassadarValidationContext,
): Promise<TassadarValidatorVerdict> =>
  runFullReplay(record, workload, 3, 'adversarial_replay', context)

export const receiptFromVerdict = (
  verdict: TassadarValidatorVerdict,
): TassadarValidatorReceipt => ({
  classId: verdict.classId,
  comparedSteps: verdict.comparedSteps,
  outcome: verdict.outcome,
  rejectionKind: verdict.rejection === null ? null : verdict.rejection.kind,
  replayedSteps: verdict.replayedSteps,
  tier: verdict.tier,
  validatedAtIso: verdict.validatedAtIso,
  validatorDeviceRef: verdict.validatorDeviceRef,
  verdictSchemaVersion: verdict.verdictSchemaVersion,
})
