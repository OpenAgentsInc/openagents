import { describe, expect, test } from 'vitest'

import type { AutopilotWorkProjection, AutopilotWorkState } from '../model'
import {
  buildForgeUsageBudgetInput,
  projectForgeUsageBudget,
} from './usage-budget'

const work = (
  state: AutopilotWorkState,
  overrides: Partial<AutopilotWorkProjection> = {},
): AutopilotWorkProjection =>
  ({
    accessRequestRefs: [],
    accessRequirements: [],
    assignmentIntents: [],
    buyerPaymentProofRef: null,
    clientRequestRef: 'client.public.work_1',
    createdAt: '2026-06-16T15:00:00.000Z',
    eventStreamRef: 'event-stream.public.work_1',
    executionCloseout: null,
    fallbackLeaseIntents: [],
    funding: {},
    generatedAt: '2026-06-17T19:00:00.000Z',
    idempotent: false,
    nextAction: {
      callerActionRefs: [],
      reasonRefs: [],
      retryAfterSeconds: null,
      state,
    },
    paymentChallenge: null,
    paymentChallengeRef: null,
    placementDecision: { selectedRunnerKind: 'requester_pylon' },
    placementPolicy: {},
    promiseRef: {
      blockerRefs: [],
      promiseId: 'autopilot.mission_briefing.v1',
      registryVersion: '2026-06-15.6',
    },
    pylonAssignmentIntents: [],
    quote: {},
    repositoryAuthorities: [],
    reviewDecision: null,
    state,
    statusUrlRef: 'status.public.work_1',
    taskRefs: ['task.public.work_1'],
    tasks: [],
    updatedAt: '2026-06-17T19:00:00.000Z',
    workOrderRef: 'work_1',
    ...overrides,
  }) as AutopilotWorkProjection

describe('Forge usage budget projection', () => {
  test('projects exact usage, known pricing, and non-authority flags', () => {
    const view = projectForgeUsageBudget({
      budgetThresholds: [
        {
          action: 'warn',
          budgetRef: 'budget.public.work_1.run_tokens',
          limitTokens: 100_000,
          policyRefs: ['policy.public.usage.warn_80'],
          state: 'within',
        },
      ],
      contextEstimateRef: 'context-estimate.public.work_1.latest',
      costEstimate: {
        costRef: 'cost.public.work_1.estimate',
        currency: 'USD',
        estimatedCostCents: 42,
        pricingRef: 'pricing.public.provider.model',
        pricingState: 'known',
      },
      generatedAt: '2026-06-17T19:00:00.000Z',
      modelRef: 'model.public.gpt_5',
      providerRef: 'provider.public.openai',
      tokenCounts: {
        cacheReadTokens: 10_000,
        cacheWriteTokens: 2_000,
        contextWindowTokens: 48_000,
        inputTokens: 30_000,
        outputTokens: 4_000,
        serverToolRequestCount: 2,
        totalTokens: 46_000,
      },
      usageRef: 'usage.public.work_1.latest',
      usageTruth: 'exact',
      workOrderRef: 'work_1',
    })

    expect(view).toMatchObject({
      authority: {
        acceptedOutcomeAuthority: false,
        budgetEnforcementAuthority: false,
        deploymentAuthority: false,
        maxOutputEscalationAuthority: false,
        pricingWriteAuthority: false,
        providerRetryAuthority: false,
        publicClaimAuthority: false,
        settlementAuthority: false,
        spendAuthorizationAuthority: false,
        workerPayoutAuthority: false,
      },
      publicSafe: true,
      status: 'within',
      usageTruth: 'exact',
      workOrderRef: 'work_1',
    })
    expect(view.tokenCounts.contextWindowTokens).toBe(48_000)
    expect(view.costEstimate?.pricingState).toBe('known')
    expect(view.blockerRefs).toEqual([])
  })

  test('keeps normal Runs empty without usage evidence', () => {
    const view = projectForgeUsageBudget(
      buildForgeUsageBudgetInput(work('queued_or_running')),
    )

    expect(view.status).toBe('empty')
    expect(view.usageRef).toBeNull()
    expect(view.blockerRefs).toEqual([])
  })

  test('blocks unknown pricing from exact cost claims', () => {
    const view = projectForgeUsageBudget({
      costEstimate: {
        costRef: 'cost.public.work_1.unknown',
        estimatedCostCents: 5,
        pricingState: 'unknown',
      },
      generatedAt: '2026-06-17T19:00:00.000Z',
      tokenCounts: { totalTokens: 1_000 },
      usageRef: 'usage.public.work_1',
      usageTruth: 'estimated',
      workOrderRef: 'work_1',
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-usage-budget-blocker:work_1:unknown-pricing-cost-not-exact',
    )
  })

  test('blocks output-only usage from context headroom projection', () => {
    const view = projectForgeUsageBudget({
      generatedAt: '2026-06-17T19:00:00.000Z',
      tokenCounts: { outputTokens: 2_000 },
      usageRef: 'usage.public.work_1.output_only',
      usageTruth: 'exact',
      workOrderRef: 'work_1',
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-usage-budget-blocker:work_1:output-only-usage-not-context-headroom',
    )
  })

  test('blocks synthetic usage and mixed token/cost thresholds', () => {
    const view = projectForgeUsageBudget({
      budgetThresholds: [
        {
          action: 'stop',
          budgetRef: 'budget.public.work_1.mixed',
          limitCostCents: 50,
          limitTokens: 10_000,
          state: 'near_limit',
        },
      ],
      generatedAt: '2026-06-17T19:00:00.000Z',
      tokenCounts: { inputTokens: 1_000, outputTokens: 500, totalTokens: 1_500 },
      usageRef: 'usage.public.work_1.synthetic',
      usageTruth: 'synthetic',
      workOrderRef: 'work_1',
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-usage-budget-blocker:work_1:synthetic-usage-not-provider-usage',
    )
    expect(view.blockerRefs.some(ref =>
      ref.includes('mixed-token-cost-threshold:budget.public.work_1.mixed')
    )).toBe(true)
  })

  test('omits unsafe private usage material before projection', () => {
    const view = projectForgeUsageBudget({
      blockerRefs: ['usage-blocker.public.safe', 'raw prompt /Users/christopher/a.md'],
      budgetThresholds: [
        {
          action: 'warn',
          budgetRef: 'budget.public.safe',
          policyRefs: ['policy.public.safe', 'provider payload sk-private'],
          state: 'within',
        },
      ],
      contextEstimateRef: '/Users/christopher/context.json',
      costEstimate: {
        costRef: 'cost.public.safe',
        currency: 'USD',
        estimatedCostCents: 12,
        pricingRef: 'raw provider payload sk-private',
        pricingState: 'known',
      },
      generatedAt: '2026-06-17T19:00:00.000Z',
      modelRef: 'model.public.safe',
      providerRef: 'provider payload sk-private',
      quotaBlockerRefs: ['quota.public.safe', 'shell log /Users/christopher/run.log'],
      rateLimitRefs: ['rate-limit.public.safe', 'raw usage /Users/christopher/u.json'],
      tokenCounts: { inputTokens: 1_000, outputTokens: 500, totalTokens: 1_500 },
      usageRef: 'usage.public.safe',
      usageTruth: 'estimated',
      workOrderRef: 'work_1',
    })
    const payload = JSON.stringify(view)

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain('usage-blocker.public.safe')
    expect(view.blockerRefs).toContain(
      'forge-usage-budget-blocker:work_1:unsafe-usage-budget-material-omitted',
    )
    expect(view.rateLimitRefs).toEqual(['rate-limit.public.safe'])
    expect(view.quotaBlockerRefs).toEqual(['quota.public.safe'])
    expect(view.budgetThresholds[0]?.policyRefs).toEqual(['policy.public.safe'])
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('raw prompt')
    expect(payload).not.toContain('raw usage')
    expect(payload).not.toContain('shell log')
    expect(payload).not.toContain('provider payload')
    expect(payload).not.toContain('sk-private')
  })
})
