import { describe, expect, test } from 'vitest'

import {
  ArtanisFrlmBudgetObservation,
  ArtanisFrlmBudgetPolicy,
  ArtanisFrlmBudgetPolicyUnsafe,
  artanisFrlmBudgetPolicyFromBlueprintPolicies,
  evaluateArtanisFrlmBudgetPolicy,
} from './artanis-frlm-budget-policy'

const hardPolicy = new ArtanisFrlmBudgetPolicy({
  blueprintBudgetPolicyRefs: [
    'budget-policy.public.artanis.frlm.provider_tokens',
    'budget-policy.public.artanis.frlm.recursion_depth',
  ],
  enforcement: 'hard',
  maxRecursionDepth: 3,
  maxTotalTokens: 1_000,
  policyRef: 'budget-policy.public.artanis.frlm.hard',
})

const observation = (
  input: Partial<ArtanisFrlmBudgetObservation>,
): ArtanisFrlmBudgetObservation =>
  new ArtanisFrlmBudgetObservation({
    inputTokensUsed: 400,
    outputTokensUsed: 200,
    requestedFanoutCount: 1,
    requestedRecursionDepth: 2,
    reservedOutputTokens: 300,
    ...input,
  })

describe('Artanis FRLM budget policy', () => {
  test('continues when projected tokens and requested recursion depth stay within the policy', () => {
    const decision = evaluateArtanisFrlmBudgetPolicy(
      hardPolicy,
      observation({}),
    )

    expect(decision).toMatchObject({
      blockerRefs: [],
      caveatRefs: [],
      decision: 'continue',
      maxRecursionDepth: 3,
      maxTotalTokens: 1_000,
      projectedTotalTokens: 900,
      remainingRecursionDepth: 1,
      remainingTokens: 100,
    })
  })

  test('hard-blocks fanout when the provider-token ceiling would be exceeded', () => {
    const decision = evaluateArtanisFrlmBudgetPolicy(
      hardPolicy,
      observation({ reservedOutputTokens: 401 }),
    )

    expect(decision.decision).toBe('blocked')
    expect(decision.remainingTokens).toBe(0)
    expect(decision.blockerRefs).toEqual([
      'blocker.public.artanis.frlm_budget.provider_tokens_exceeded',
    ])
    expect(decision.caveatRefs).toEqual([
      'caveat.public.artanis.frlm_budget.provider_tokens_exceeded',
    ])
  })

  test('hard-blocks recursive fanout past the maximum depth', () => {
    const decision = evaluateArtanisFrlmBudgetPolicy(
      hardPolicy,
      observation({
        requestedRecursionDepth: 4,
        reservedOutputTokens: 1,
      }),
    )

    expect(decision.decision).toBe('blocked')
    expect(decision.remainingRecursionDepth).toBe(0)
    expect(decision.blockerRefs).toEqual([
      'blocker.public.artanis.frlm_budget.recursion_depth_exceeded',
    ])
  })

  test('warns instead of blocking when a soft Blueprint policy is exceeded', () => {
    const softPolicy = new ArtanisFrlmBudgetPolicy({
      ...hardPolicy,
      enforcement: 'soft',
      policyRef: 'budget-policy.public.artanis.frlm.soft',
    })
    const decision = evaluateArtanisFrlmBudgetPolicy(
      softPolicy,
      observation({ requestedRecursionDepth: 4 }),
    )

    expect(decision.decision).toBe('warn')
    expect(decision.blockerRefs).toEqual([])
    expect(decision.caveatRefs).toEqual([
      'caveat.public.artanis.frlm_budget.recursion_depth_exceeded',
    ])
  })

  test('derives the most restrictive token and depth caps from Blueprint budget policies', () => {
    const policy = artanisFrlmBudgetPolicyFromBlueprintPolicies({
      defaultMaxRecursionDepth: 6,
      defaultMaxTotalTokens: 20_000,
      policies: [
        {
          budgetKind: 'provider_tokens',
          budgetRef: 'budget-policy.public.artanis.frlm.provider_tokens.large',
          enforcement: 'soft',
          limit: 10_000,
        },
        {
          budgetKind: 'provider_tokens',
          budgetRef: 'budget-policy.public.artanis.frlm.provider_tokens.small',
          enforcement: 'hard',
          limit: 4_000,
        },
        {
          budgetKind: 'recursion_depth',
          budgetRef: 'budget-policy.public.artanis.frlm.recursion_depth',
          enforcement: 'hard',
          limit: 2,
        },
      ],
      policyRef: 'budget-policy.public.artanis.frlm.derived',
    })

    expect(policy).toMatchObject({
      blueprintBudgetPolicyRefs: [
        'budget-policy.public.artanis.frlm.provider_tokens.large',
        'budget-policy.public.artanis.frlm.provider_tokens.small',
        'budget-policy.public.artanis.frlm.recursion_depth',
      ],
      enforcement: 'hard',
      maxRecursionDepth: 2,
      maxTotalTokens: 4_000,
      policyRef: 'budget-policy.public.artanis.frlm.derived',
    })
  })

  test('rejects invalid policy bounds and unsafe refs', () => {
    expect(() =>
      evaluateArtanisFrlmBudgetPolicy(
        new ArtanisFrlmBudgetPolicy({
          ...hardPolicy,
          maxTotalTokens: 0,
        }),
        observation({}),
      ),
    ).toThrow(ArtanisFrlmBudgetPolicyUnsafe)

    expect(() =>
      evaluateArtanisFrlmBudgetPolicy(
        new ArtanisFrlmBudgetPolicy({
          ...hardPolicy,
          policyRef: 'provider_secret.raw',
        }),
        observation({}),
      ),
    ).toThrow(ArtanisFrlmBudgetPolicyUnsafe)
  })
})
