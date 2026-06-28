import { Schema as S } from 'effect'

import type { BlueprintObjectiveBudgetPolicyType } from './blueprint'

export const ArtanisFrlmBudgetDecisionKind = S.Literals([
  'continue',
  'warn',
  'blocked',
])
export type ArtanisFrlmBudgetDecisionKind =
  typeof ArtanisFrlmBudgetDecisionKind.Type

export class ArtanisFrlmBudgetPolicy extends S.Class<ArtanisFrlmBudgetPolicy>(
  'ArtanisFrlmBudgetPolicy',
)({
  blueprintBudgetPolicyRefs: S.Array(S.String),
  enforcement: S.Literals(['soft', 'hard']),
  maxRecursionDepth: S.Int,
  maxTotalTokens: S.Int,
  policyRef: S.String,
}) {}

export class ArtanisFrlmBudgetObservation extends S.Class<ArtanisFrlmBudgetObservation>(
  'ArtanisFrlmBudgetObservation',
)({
  inputTokensUsed: S.Int,
  outputTokensUsed: S.Int,
  requestedFanoutCount: S.Int,
  requestedRecursionDepth: S.Int,
  reservedOutputTokens: S.Int,
}) {}

export class ArtanisFrlmBudgetDecision extends S.Class<ArtanisFrlmBudgetDecision>(
  'ArtanisFrlmBudgetDecision',
)({
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  decision: ArtanisFrlmBudgetDecisionKind,
  maxRecursionDepth: S.Int,
  maxTotalTokens: S.Int,
  policyRef: S.String,
  projectedTotalTokens: S.Int,
  remainingRecursionDepth: S.Int,
  remainingTokens: S.Int,
}) {}

export class ArtanisFrlmBudgetPolicyUnsafe extends S.TaggedErrorClass<ArtanisFrlmBudgetPolicyUnsafe>()(
  'ArtanisFrlmBudgetPolicyUnsafe',
  {
    reason: S.String,
  },
) {}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/{}-]{0,260}$/
const unsafeRefPattern =
  /(auth|bearer|cookie|credential|mnemonic|private|provider[_-]?(grant|payload|secret|token)\b|raw|secret|sk-[a-z0-9]|wallet)/i

const assertSafeRef = (value: string, field: string): void => {
  if (!safeRefPattern.test(value) || unsafeRefPattern.test(value)) {
    throw new ArtanisFrlmBudgetPolicyUnsafe({
      reason: `${field} must be a public-safe ref`,
    })
  }
}

const assertNonNegativeInt = (value: number, field: string): void => {
  if (!Number.isInteger(value) || value < 0) {
    throw new ArtanisFrlmBudgetPolicyUnsafe({
      reason: `${field} must be a non-negative integer`,
    })
  }
}

const assertPositiveInt = (value: number, field: string): void => {
  if (!Number.isInteger(value) || value <= 0) {
    throw new ArtanisFrlmBudgetPolicyUnsafe({
      reason: `${field} must be a positive integer`,
    })
  }
}

export const assertArtanisFrlmBudgetPolicy = (
  policy: ArtanisFrlmBudgetPolicy,
): void => {
  assertSafeRef(policy.policyRef, 'policyRef')
  policy.blueprintBudgetPolicyRefs.forEach(ref =>
    assertSafeRef(ref, 'blueprintBudgetPolicyRefs'),
  )
  assertPositiveInt(policy.maxTotalTokens, 'maxTotalTokens')
  assertPositiveInt(policy.maxRecursionDepth, 'maxRecursionDepth')
}

const assertArtanisFrlmBudgetObservation = (
  observation: ArtanisFrlmBudgetObservation,
): void => {
  assertNonNegativeInt(observation.inputTokensUsed, 'inputTokensUsed')
  assertNonNegativeInt(observation.outputTokensUsed, 'outputTokensUsed')
  assertNonNegativeInt(observation.reservedOutputTokens, 'reservedOutputTokens')
  assertNonNegativeInt(
    observation.requestedRecursionDepth,
    'requestedRecursionDepth',
  )
  assertNonNegativeInt(observation.requestedFanoutCount, 'requestedFanoutCount')
}

export const artanisFrlmBudgetPolicyFromBlueprintPolicies = (
  input: Readonly<{
    defaultEnforcement?: 'soft' | 'hard' | undefined
    defaultMaxRecursionDepth: number
    defaultMaxTotalTokens: number
    policyRef: string
    policies: ReadonlyArray<BlueprintObjectiveBudgetPolicyType>
  }>,
): ArtanisFrlmBudgetPolicy => {
  const providerTokenPolicies = input.policies.filter(
    policy => policy.budgetKind === 'provider_tokens',
  )
  const recursionDepthPolicies = input.policies.filter(
    policy => policy.budgetKind === 'recursion_depth',
  )
  const limits = (policies: ReadonlyArray<BlueprintObjectiveBudgetPolicyType>) =>
    policies.map(policy => Math.trunc(policy.limit)).filter(limit => limit > 0)
  const tokenLimits = limits(providerTokenPolicies)
  const depthLimits = limits(recursionDepthPolicies)
  const hardPolicyPresent = input.policies.some(
    policy => policy.enforcement === 'hard',
  )

  return new ArtanisFrlmBudgetPolicy({
    blueprintBudgetPolicyRefs: input.policies.map(policy => policy.budgetRef),
    enforcement:
      hardPolicyPresent || input.defaultEnforcement === 'hard' ? 'hard' : 'soft',
    maxRecursionDepth:
      depthLimits.length === 0
        ? input.defaultMaxRecursionDepth
        : Math.min(...depthLimits),
    maxTotalTokens:
      tokenLimits.length === 0
        ? input.defaultMaxTotalTokens
        : Math.min(...tokenLimits),
    policyRef: input.policyRef,
  })
}

export const evaluateArtanisFrlmBudgetPolicy = (
  policy: ArtanisFrlmBudgetPolicy,
  observation: ArtanisFrlmBudgetObservation,
): ArtanisFrlmBudgetDecision => {
  assertArtanisFrlmBudgetPolicy(policy)
  assertArtanisFrlmBudgetObservation(observation)

  const projectedTotalTokens =
    observation.inputTokensUsed +
    observation.outputTokensUsed +
    observation.reservedOutputTokens
  const remainingTokens = Math.max(0, policy.maxTotalTokens - projectedTotalTokens)
  const remainingRecursionDepth = Math.max(
    0,
    policy.maxRecursionDepth - observation.requestedRecursionDepth,
  )
  const tokenLimitExceeded = projectedTotalTokens > policy.maxTotalTokens
  const depthLimitExceeded =
    observation.requestedFanoutCount > 0 &&
    observation.requestedRecursionDepth > policy.maxRecursionDepth
  const limitExceeded = tokenLimitExceeded || depthLimitExceeded
  const shouldBlock = policy.enforcement === 'hard' && limitExceeded

  return new ArtanisFrlmBudgetDecision({
    blockerRefs: shouldBlock
      ? [
          tokenLimitExceeded
            ? 'blocker.public.artanis.frlm_budget.provider_tokens_exceeded'
            : 'blocker.public.artanis.frlm_budget.recursion_depth_exceeded',
        ]
      : [],
    caveatRefs: limitExceeded
      ? [
          tokenLimitExceeded
            ? 'caveat.public.artanis.frlm_budget.provider_tokens_exceeded'
            : 'caveat.public.artanis.frlm_budget.recursion_depth_exceeded',
        ]
      : [],
    decision: shouldBlock ? 'blocked' : limitExceeded ? 'warn' : 'continue',
    maxRecursionDepth: policy.maxRecursionDepth,
    maxTotalTokens: policy.maxTotalTokens,
    policyRef: policy.policyRef,
    projectedTotalTokens,
    remainingRecursionDepth,
    remainingTokens,
  })
}
