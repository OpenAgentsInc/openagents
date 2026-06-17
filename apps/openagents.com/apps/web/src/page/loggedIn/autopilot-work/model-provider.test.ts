import { describe, expect, test } from 'vitest'

import type { AutopilotWorkProjection, AutopilotWorkState } from '../model'
import {
  buildForgeModelProviderInput,
  projectForgeModelProvider,
} from './model-provider'

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
    generatedAt: '2026-06-17T19:20:00.000Z',
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
    updatedAt: '2026-06-17T19:20:00.000Z',
    workOrderRef: 'work_1',
    ...overrides,
  }) as AutopilotWorkProjection

describe('Forge model provider projection', () => {
  test('projects selected model/provider resolution with non-authority flags', () => {
    const view = projectForgeModelProvider({
      capabilities: {
        contextWindowTokens: 128_000,
        maxOutputTokens: 16_000,
        structuredOutputSupport: true,
        toolCallSupport: true,
      },
      capabilityRefs: ['capability.public.model.gpt_5'],
      entitlementRefs: ['entitlement.public.team.provider_openai'],
      generatedAt: '2026-06-17T19:20:00.000Z',
      modelRef: 'model.public.gpt_5',
      policyRefs: ['policy.public.model.default'],
      pricingRefs: ['pricing.public.gpt_5'],
      privacyRefs: ['privacy.public.provider.openai'],
      providerFacingModelRef: 'provider-model.public.gpt_5',
      providerRef: 'provider.public.openai',
      requestedAliasRef: 'model-alias.public.best',
      resolutionRef: 'model-resolution.public.work_1',
      resolutionSource: 'settings',
      state: 'selected',
      telemetryRefs: ['telemetry-policy.public.aggregate'],
      validationRefs: ['validation.public.model.gpt_5'],
      validationState: 'passed',
      workOrderRef: 'work_1',
    })

    expect(view).toMatchObject({
      authority: {
        acceptedOutcomeAuthority: false,
        credentialAuthority: false,
        deploymentAuthority: false,
        modelCallAuthority: false,
        modelSwitchAuthority: false,
        pricingWriteAuthority: false,
        providerRetryAuthority: false,
        publicClaimAuthority: false,
        settingsWriteAuthority: false,
        settlementAuthority: false,
        streamParsingAuthority: false,
        workerPayoutAuthority: false,
      },
      publicSafe: true,
      status: 'selected',
      workOrderRef: 'work_1',
    })
    expect(view.capabilities.contextWindowTokens).toBe(128_000)
    expect(view.blockerRefs).toEqual([])
  })

  test('keeps normal Runs unknown without model-provider evidence', () => {
    const view = projectForgeModelProvider(
      buildForgeModelProviderInput(work('queued_or_running')),
    )

    expect(view.status).toBe('unknown')
    expect(view.resolutionRef).toBeNull()
    expect(view.blockerRefs).toEqual([])
  })

  test('blocks selected models without capability or entitlement evidence', () => {
    const view = projectForgeModelProvider({
      generatedAt: '2026-06-17T19:20:00.000Z',
      modelRef: 'model.public.unknown',
      providerRef: 'provider.public.openai',
      resolutionRef: 'model-resolution.public.work_1',
      state: 'selected',
      workOrderRef: 'work_1',
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-model-provider-blocker:work_1:missing-capability-evidence',
    )
    expect(view.blockerRefs).toContain(
      'forge-model-provider-blocker:work_1:missing-entitlement-evidence',
    )
  })

  test('blocks fallback selection and unavailable state without evidence', () => {
    const fallback = projectForgeModelProvider({
      capabilityRefs: ['capability.public.model.fallback'],
      entitlementRefs: ['entitlement.public.team.provider_openai'],
      generatedAt: '2026-06-17T19:20:00.000Z',
      modelRef: 'model.public.fallback',
      providerRef: 'provider.public.openai',
      resolutionRef: 'model-resolution.public.work_1',
      state: 'fallback_selected',
      workOrderRef: 'work_1',
    })
    const unavailable = projectForgeModelProvider({
      generatedAt: '2026-06-17T19:20:00.000Z',
      modelRef: 'model.public.unavailable',
      providerRef: 'provider.public.openai',
      resolutionRef: 'model-resolution.public.work_1',
      state: 'unavailable',
      validationState: 'unknown',
      workOrderRef: 'work_1',
    })

    expect(fallback.status).toBe('blocked')
    expect(fallback.blockerRefs).toContain(
      'forge-model-provider-blocker:work_1:fallback-selected-without-evidence',
    )
    expect(unavailable.status).toBe('blocked')
    expect(unavailable.blockerRefs).toContain(
      'forge-model-provider-blocker:work_1:provider-discovery-failure-not-unavailable-proof',
    )
  })

  test('omits unsafe private model provider material before projection', () => {
    const view = projectForgeModelProvider({
      blockerRefs: [
        'provider-blocker.public.safe',
        'raw request /Users/christopher/request.json',
      ],
      capabilityRefs: ['capability.public.safe', 'provider payload sk-private'],
      entitlementRefs: ['entitlement.public.safe'],
      generatedAt: '2026-06-17T19:20:00.000Z',
      modelRef: 'model.public.safe',
      policyRefs: ['policy.public.safe', 'internal codename private-model'],
      providerFacingModelRef: 'private deployment /Users/christopher/model',
      providerRef: 'provider.public.safe',
      requestedAliasRef: 'model-alias.public.safe',
      resolutionRef: 'model-resolution.public.safe',
      state: 'selected',
      validationRefs: ['validation.public.safe', 'sdk payload bearer token'],
      validationState: 'passed',
      workOrderRef: 'work_1',
    })
    const payload = JSON.stringify(view)

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain('provider-blocker.public.safe')
    expect(view.blockerRefs).toContain(
      'forge-model-provider-blocker:work_1:unsafe-model-provider-material-omitted',
    )
    expect(view.providerFacingModelRef).toBeNull()
    expect(view.capabilityRefs).toEqual(['capability.public.safe'])
    expect(view.policyRefs).toEqual(['policy.public.safe'])
    expect(view.validationRefs).toEqual(['validation.public.safe'])
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('raw request')
    expect(payload).not.toContain('provider payload')
    expect(payload).not.toContain('private deployment')
    expect(payload).not.toContain('internal codename')
    expect(payload).not.toContain('bearer token')
    expect(payload).not.toContain('sk-private')
  })
})
