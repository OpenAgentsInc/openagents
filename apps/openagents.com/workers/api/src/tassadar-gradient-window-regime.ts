import { Schema as S } from 'effect'

export const TassadarGradientWindowStage = S.Literals([
  'submitted',
  'quarantined',
  'recomputed',
  'replicated',
  'canary_passed',
  'promoted',
  'blocked',
])
export type TassadarGradientWindowStage =
  typeof TassadarGradientWindowStage.Type

export const TassadarGradientWindowCandidate = S.Struct({
  baseCheckpointDigest: S.String,
  compiledCoreGradientTargeted: S.Boolean,
  compiledCoreRef: S.String,
  constructionReceiptRefs: S.Array(S.String),
  contributorRef: S.String,
  curatedDataRefs: S.Array(S.String),
  datasetShardDigest: S.String,
  frozenCoreDigestAfter: S.String,
  frozenCoreDigestBefore: S.String,
  frozenParameterScopes: S.Array(S.String),
  gradientsFlowThroughTrace: S.Boolean,
  learnedInterfaceDigest: S.String,
  modelFamilyRef: S.String,
  optimizerStateDigest: S.String,
  psionicH1EvidenceRefs: S.Array(S.String),
  quarantineCheckpointDigest: S.String,
  randomSeedRef: S.String,
  sourceRefs: S.Array(S.String),
  trainableParameterScopes: S.Array(S.String),
  trainingConfigDigest: S.String,
  updateDigest: S.String,
  verificationReceiptRefs: S.Array(S.String),
  windowRef: S.String,
})
export type TassadarGradientWindowCandidate =
  typeof TassadarGradientWindowCandidate.Type

export const TassadarGradientWindowRecomputeReceipt = S.Struct({
  expectedUpdateDigest: S.String,
  passed: S.Boolean,
  receiptRefs: S.Array(S.String),
  recomputedUpdateDigest: S.String,
})
export type TassadarGradientWindowRecomputeReceipt =
  typeof TassadarGradientWindowRecomputeReceipt.Type

export const TassadarGradientWindowReplicationReceipt = S.Struct({
  passed: S.Boolean,
  receiptRefs: S.Array(S.String),
  replicaUpdateDigests: S.Array(S.String),
})
export type TassadarGradientWindowReplicationReceipt =
  typeof TassadarGradientWindowReplicationReceipt.Type

export const TassadarGradientWindowCanaryReceipt = S.Struct({
  exactRolloutPassAt1: S.Number,
  outputDigestMatchRate: S.Number,
  passed: S.Boolean,
  receiptRefs: S.Array(S.String),
  replayAcceptanceRate: S.Number,
})
export type TassadarGradientWindowCanaryReceipt =
  typeof TassadarGradientWindowCanaryReceipt.Type

export const TassadarGradientWindowReceiptBundle = S.Struct({
  canary: TassadarGradientWindowCanaryReceipt,
  promotionDecisionRefs: S.Array(S.String),
  quarantineReceiptRefs: S.Array(S.String),
  recompute: TassadarGradientWindowRecomputeReceipt,
  replication: TassadarGradientWindowReplicationReceipt,
  rollbackRefs: S.Array(S.String),
  settlementReceiptRefs: S.Array(S.String),
})
export type TassadarGradientWindowReceiptBundle =
  typeof TassadarGradientWindowReceiptBundle.Type

export type TassadarGradientWindowPromotionProjection = Readonly<{
  authority: Readonly<{
    canonicalCheckpointMutationAllowed: boolean
    compiledCoreGradientMutationAllowed: false
    directSubmissionMutationAllowed: false
    quarantineCheckpointMutationAllowed: boolean
    settlementMutationAllowed: false
  }>
  blockerRefs: ReadonlyArray<string>
  canaryReceiptRefs: ReadonlyArray<string>
  compiledCoreUnchanged: boolean
  constructionReceiptRefs: ReadonlyArray<string>
  curatedDataRefs: ReadonlyArray<string>
  gateRef: string
  promotionAllowed: boolean
  promotionDecisionRefs: ReadonlyArray<string>
  recomputeReceiptRefs: ReadonlyArray<string>
  replicationReceiptRefs: ReadonlyArray<string>
  rollbackRefs: ReadonlyArray<string>
  settlementEligible: boolean
  settlementReceiptRefs: ReadonlyArray<string>
  sourceRefs: ReadonlyArray<string>
  stage: TassadarGradientWindowStage
  verificationReceiptRefs: ReadonlyArray<string>
  windowRef: string
}>

export class TassadarGradientWindowUnsafe extends S.TaggedErrorClass<TassadarGradientWindowUnsafe>()(
  'TassadarGradientWindowUnsafe',
  {
    reason: S.String,
  },
) {}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const digestPattern = /^(digest\.sha256\.|sha256:)?[a-f0-9]{32,128}$/i
const unsafeGradientWindowPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer|cookie|customer[_-]?(email|name|prompt|record|value)|dataset\.(private|raw)|email[_-]?(address|body|html|raw|text)|full[_-]?(prompt|source|trace)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|payment[_-]?(hash|id|invoice|preimage|proof|raw|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private([._-]|$)|provider[_-]?(account|credential|grant|payload|secret|token)|raw([._-]|$)|repo[_-]?private|secret|seed[_-]?phrase|sk-[a-z0-9]|source[._-]?(archive|private|raw)|token|trace[._-]?(raw|full|private|payload)|wallet)/i

const decodeCandidate = S.decodeUnknownSync(TassadarGradientWindowCandidate)
const decodeReceipts = S.decodeUnknownSync(TassadarGradientWindowReceiptBundle)

const uniqueRefs = (
  refs: ReadonlyArray<string | undefined>,
): ReadonlyArray<string> =>
  [...new Set(refs.filter((ref): ref is string => ref !== undefined))]
    .map(ref => ref.trim())
    .filter(ref => ref.length > 0)
    .sort()

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string | undefined>,
): ReadonlyArray<string> => {
  const normalized = uniqueRefs(refs)
  const unsafe = normalized.find(ref =>
    !safeRefPattern.test(ref) || unsafeGradientWindowPattern.test(ref)
  )

  if (unsafe !== undefined) {
    throw new TassadarGradientWindowUnsafe({
      reason: `${label} must be public-safe refs without raw/private, provider, customer, wallet, payment, trace, prompt, or credential material.`,
    })
  }

  return normalized
}

const assertDigest = (label: string, digest: string): void => {
  if (!digestPattern.test(digest)) {
    throw new TassadarGradientWindowUnsafe({
      reason: `${label} must be a sha256-style digest.`,
    })
  }
}

const safeSuffix = (value: string): string =>
  value.replace(/[^A-Za-z0-9_.:/-]/g, '_').slice(0, 120)

const scopeTargetsCompiledCore = (scope: string): boolean => {
  const normalized = scope.toLowerCase()

  return (
    normalized.includes('compiled_exact_core') ||
    normalized.includes('compiled_core') ||
    normalized.includes('exact_core') ||
    normalized.includes('frozen_core') ||
    normalized.includes('analytic_executor') ||
    normalized.includes('tassadar_alm_numeric_executor') ||
    normalized.includes('tassadar_alm_numeric_execute')
  )
}

const finiteAtLeast = (value: number, floor: number): boolean =>
  Number.isFinite(value) && value >= floor

const receiptsPassed = (
  refs: ReadonlyArray<string>,
  passed: boolean,
): boolean => refs.length > 0 && passed

const replicationMatches = (
  candidate: TassadarGradientWindowCandidate,
  replication: TassadarGradientWindowReplicationReceipt,
): boolean =>
  replication.replicaUpdateDigests.length >= 2 &&
  replication.replicaUpdateDigests.every(
    digest => digest === candidate.updateDigest,
  )

const gateStage = (
  input: Readonly<{
    canaryOk: boolean
    coreBlocked: boolean
    promotionAllowed: boolean
    quarantineOk: boolean
    recomputeOk: boolean
    replicationOk: boolean
  }>,
): TassadarGradientWindowStage => {
  if (input.promotionAllowed) {
    return 'promoted'
  }
  if (input.coreBlocked) {
    return 'blocked'
  }
  if (
    input.quarantineOk &&
    input.recomputeOk &&
    input.replicationOk &&
    input.canaryOk
  ) {
    return 'canary_passed'
  }
  if (input.quarantineOk && input.recomputeOk && input.replicationOk) {
    return 'replicated'
  }
  if (input.quarantineOk && input.recomputeOk) {
    return 'recomputed'
  }
  if (input.quarantineOk) {
    return 'quarantined'
  }

  return 'submitted'
}

export const projectTassadarGradientWindowRegime = (
  input: Readonly<{
    candidate: TassadarGradientWindowCandidate
    receipts: TassadarGradientWindowReceiptBundle
  }>,
): TassadarGradientWindowPromotionProjection => {
  const candidate = decodeCandidate(input.candidate)
  const receipts = decodeReceipts(input.receipts)

  const identityRefs = assertSafeRefs('Tassadar gradient window identity refs', [
    candidate.compiledCoreRef,
    candidate.contributorRef,
    candidate.modelFamilyRef,
    candidate.randomSeedRef,
    candidate.windowRef,
    ...candidate.frozenParameterScopes,
    ...candidate.trainableParameterScopes,
  ])
  const constructionReceiptRefs = assertSafeRefs(
    'Tassadar gradient window construction receipt refs',
    candidate.constructionReceiptRefs,
  )
  const verificationReceiptRefs = assertSafeRefs(
    'Tassadar gradient window verification receipt refs',
    candidate.verificationReceiptRefs,
  )
  const curatedDataRefs = assertSafeRefs(
    'Tassadar gradient window curated data refs',
    candidate.curatedDataRefs,
  )
  const sourceRefs = assertSafeRefs('Tassadar gradient window source refs', [
    ...candidate.psionicH1EvidenceRefs,
    ...candidate.sourceRefs,
  ])
  const quarantineReceiptRefs = assertSafeRefs(
    'Tassadar gradient window quarantine receipt refs',
    receipts.quarantineReceiptRefs,
  )
  const recomputeReceiptRefs = assertSafeRefs(
    'Tassadar gradient window recompute receipt refs',
    receipts.recompute.receiptRefs,
  )
  const replicationReceiptRefs = assertSafeRefs(
    'Tassadar gradient window replication receipt refs',
    receipts.replication.receiptRefs,
  )
  const canaryReceiptRefs = assertSafeRefs(
    'Tassadar gradient window canary receipt refs',
    receipts.canary.receiptRefs,
  )
  const promotionDecisionRefs = assertSafeRefs(
    'Tassadar gradient window promotion decision refs',
    receipts.promotionDecisionRefs,
  )
  const rollbackRefs = assertSafeRefs(
    'Tassadar gradient window rollback refs',
    receipts.rollbackRefs,
  )
  const settlementReceiptRefs = assertSafeRefs(
    'Tassadar gradient window settlement receipt refs',
    receipts.settlementReceiptRefs,
  )

  const digestChecks: ReadonlyArray<readonly [string, string]> = [
    ['baseCheckpointDigest', candidate.baseCheckpointDigest],
    ['datasetShardDigest', candidate.datasetShardDigest],
    ['frozenCoreDigestAfter', candidate.frozenCoreDigestAfter],
    ['frozenCoreDigestBefore', candidate.frozenCoreDigestBefore],
    ['learnedInterfaceDigest', candidate.learnedInterfaceDigest],
    ['optimizerStateDigest', candidate.optimizerStateDigest],
    ['quarantineCheckpointDigest', candidate.quarantineCheckpointDigest],
    ['trainingConfigDigest', candidate.trainingConfigDigest],
    ['updateDigest', candidate.updateDigest],
    ['expectedUpdateDigest', receipts.recompute.expectedUpdateDigest],
    ['recomputedUpdateDigest', receipts.recompute.recomputedUpdateDigest],
    ...receipts.replication.replicaUpdateDigests.map(
      (digest, index) => [`replicaUpdateDigests.${index}`, digest] as const,
    ),
  ]
  digestChecks.forEach(([label, digest]) => assertDigest(label, digest))

  const compiledCoreUnchanged =
    candidate.frozenCoreDigestBefore === candidate.frozenCoreDigestAfter
  const trainableCoreScope = candidate.trainableParameterScopes.some(scope =>
    scopeTargetsCompiledCore(scope)
  )
  const frozenCoreScopePresent = candidate.frozenParameterScopes.some(scope =>
    scopeTargetsCompiledCore(scope)
  )
  const coreBlocked =
    !compiledCoreUnchanged ||
    candidate.compiledCoreGradientTargeted ||
    trainableCoreScope ||
    !frozenCoreScopePresent ||
    !candidate.gradientsFlowThroughTrace
  const quarantineOk = quarantineReceiptRefs.length > 0
  const recomputeOk =
    receiptsPassed(recomputeReceiptRefs, receipts.recompute.passed) &&
    receipts.recompute.expectedUpdateDigest === candidate.updateDigest &&
    receipts.recompute.recomputedUpdateDigest === candidate.updateDigest
  const replicationOk =
    receiptsPassed(replicationReceiptRefs, receipts.replication.passed) &&
    replicationMatches(candidate, receipts.replication)
  const canaryOk =
    receiptsPassed(canaryReceiptRefs, receipts.canary.passed) &&
    finiteAtLeast(receipts.canary.exactRolloutPassAt1, 1) &&
    finiteAtLeast(receipts.canary.outputDigestMatchRate, 1) &&
    finiteAtLeast(receipts.canary.replayAcceptanceRate, 1)

  const blockerRefs = uniqueRefs([
    ...(constructionReceiptRefs.length === 0
      ? ['blocker.public.tassadar_gradient_window.construction_substrate_missing']
      : []),
    ...(verificationReceiptRefs.length === 0
      ? ['blocker.public.tassadar_gradient_window.verification_substrate_missing']
      : []),
    ...(curatedDataRefs.length === 0
      ? ['blocker.public.tassadar_gradient_window.curated_data_refs_missing']
      : []),
    ...(!compiledCoreUnchanged
      ? ['blocker.public.tassadar_gradient_window.frozen_core_digest_changed']
      : []),
    ...(candidate.compiledCoreGradientTargeted || trainableCoreScope
      ? ['blocker.public.tassadar_gradient_window.compiled_core_gradient_targeted']
      : []),
    ...(!frozenCoreScopePresent
      ? ['blocker.public.tassadar_gradient_window.frozen_core_scope_missing']
      : []),
    ...(!candidate.gradientsFlowThroughTrace
      ? ['blocker.public.tassadar_gradient_window.trace_not_forward_pass']
      : []),
    ...(!quarantineOk
      ? ['blocker.public.tassadar_gradient_window.quarantine_missing']
      : []),
    ...(!recomputeOk
      ? ['blocker.public.tassadar_gradient_window.recompute_missing_or_failed']
      : []),
    ...(!replicationOk
      ? ['blocker.public.tassadar_gradient_window.replication_missing_or_failed']
      : []),
    ...(!canaryOk
      ? ['blocker.public.tassadar_gradient_window.canary_missing_or_failed']
      : []),
    ...(promotionDecisionRefs.length === 0
      ? ['blocker.public.tassadar_gradient_window.promotion_decision_missing']
      : []),
  ])
  const promotionAllowed = blockerRefs.length === 0

  return {
    authority: {
      canonicalCheckpointMutationAllowed: promotionAllowed,
      compiledCoreGradientMutationAllowed: false,
      directSubmissionMutationAllowed: false,
      quarantineCheckpointMutationAllowed: quarantineOk && !coreBlocked,
      settlementMutationAllowed: false,
    },
    blockerRefs,
    canaryReceiptRefs,
    compiledCoreUnchanged,
    constructionReceiptRefs,
    curatedDataRefs,
    gateRef:
      `gate.public.tassadar_gradient_window.${safeSuffix(candidate.windowRef)}`,
    promotionAllowed,
    promotionDecisionRefs,
    recomputeReceiptRefs,
    replicationReceiptRefs,
    rollbackRefs,
    settlementEligible: promotionAllowed && settlementReceiptRefs.length > 0,
    settlementReceiptRefs,
    sourceRefs: uniqueRefs([...identityRefs, ...sourceRefs]),
    stage: gateStage({
      canaryOk,
      coreBlocked,
      promotionAllowed,
      quarantineOk,
      recomputeOk,
      replicationOk,
    }),
    verificationReceiptRefs,
    windowRef: candidate.windowRef,
  }
}
