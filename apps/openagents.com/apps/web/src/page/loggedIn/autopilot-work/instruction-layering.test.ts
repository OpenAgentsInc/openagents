import { describe, expect, test } from 'vitest'

import type { AutopilotWorkProjection, AutopilotWorkState } from '../model'
import {
  buildForgeInstructionLayeringInput,
  projectForgeInstructionLayering,
} from './instruction-layering'

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
    generatedAt: '2026-06-17T19:30:00.000Z',
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
    updatedAt: '2026-06-17T19:30:00.000Z',
    workOrderRef: 'work_1',
    ...overrides,
  }) as AutopilotWorkProjection

describe('Forge instruction layering projection', () => {
  test('projects ordered instruction layers with non-authority flags', () => {
    const view = projectForgeInstructionLayering({
      generatedAt: '2026-06-17T19:30:00.000Z',
      layers: [
        {
          freshness: 'fresh',
          kind: 'workspace_instruction',
          layerRef: 'instruction-layer.public.workspace',
          policyRefs: ['policy.public.workspace'],
          precedence: 8,
          redactionClass: 'public',
          sourceRefs: ['source.public.AGENTS.md'],
          state: 'applied',
          tokenEstimate: 900,
        },
        {
          freshness: 'fresh',
          kind: 'runtime_policy',
          layerRef: 'instruction-layer.public.runtime_policy',
          policyRefs: ['policy.public.runtime_safety'],
          precedence: 1,
          redactionClass: 'public',
          sourceRefs: ['source.public.runtime_policy'],
          state: 'applied',
          tokenEstimate: 1200,
        },
      ],
      projectionRef: 'instruction-projection.public.work_1.provider',
      snapshotRef: 'instruction-snapshot.public.work_1',
      versionRef: 'instruction-version.public.v1',
      workOrderRef: 'work_1',
    })

    expect(view).toMatchObject({
      authority: {
        acceptedOutcomeAuthority: false,
        deploymentAuthority: false,
        memoryWriteAuthority: false,
        modelCallAuthority: false,
        promptAssemblyAuthority: false,
        promptOverrideWriteAuthority: false,
        publicClaimAuthority: false,
        settingsWriteAuthority: false,
        settlementAuthority: false,
        skillCommandLoadAuthority: false,
        toolGrantAuthority: false,
        workerPayoutAuthority: false,
      },
      publicSafe: true,
      status: 'ready',
      workOrderRef: 'work_1',
    })
    expect(view.layers.map(layer => layer.layerRef)).toEqual([
      'instruction-layer.public.runtime_policy',
      'instruction-layer.public.workspace',
    ])
    expect(view.counts).toMatchObject({ applied: 2, total: 2 })
    expect(view.blockerRefs).toEqual([])
  })

  test('keeps normal Runs empty without instruction evidence', () => {
    const view = projectForgeInstructionLayering(
      buildForgeInstructionLayeringInput(work('queued_or_running')),
    )

    expect(view.status).toBe('empty')
    expect(view.layers).toEqual([])
    expect(view.blockerRefs).toEqual([])
  })

  test('blocks runtime policy replacement and replacement without evidence', () => {
    const view = projectForgeInstructionLayering({
      generatedAt: '2026-06-17T19:30:00.000Z',
      layers: [
        {
          kind: 'runtime_policy',
          layerRef: 'instruction-layer.public.runtime_policy',
          precedence: 1,
          state: 'replaced',
        },
        {
          kind: 'product_default',
          layerRef: 'instruction-layer.public.product_default',
          precedence: 7,
          state: 'replaced',
        },
      ],
      snapshotRef: 'instruction-snapshot.public.work_1',
      workOrderRef: 'work_1',
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs.some(ref =>
      ref.includes('runtime-policy-not-overridable')
    )).toBe(true)
    expect(view.blockerRefs.some(ref =>
      ref.includes('replacement-evidence-missing')
    )).toBe(true)
  })

  test('blocks skill or command tool grants without policy refs', () => {
    const view = projectForgeInstructionLayering({
      generatedAt: '2026-06-17T19:30:00.000Z',
      layers: [
        {
          allowedToolRefs: ['tool.public.shell'],
          kind: 'skill_instruction',
          layerRef: 'instruction-layer.public.skill',
          precedence: 10,
          state: 'appended',
        },
      ],
      snapshotRef: 'instruction-snapshot.public.work_1',
      workOrderRef: 'work_1',
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-instruction-layering-blocker:work_1:tool-grant-policy-missing:instruction-layer.public.skill',
    )
  })

  test('omits unsafe private instruction material before projection', () => {
    const view = projectForgeInstructionLayering({
      blockerRefs: [
        'instruction-blocker.public.safe',
        'prompt body /Users/christopher/private.md',
      ],
      generatedAt: '2026-06-17T19:30:00.000Z',
      layers: [
        {
          allowedToolRefs: ['tool.public.safe', 'raw instruction /Users/christopher/tool.md'],
          capabilityDeltaRefs: ['capability.public.safe'],
          kind: 'local_private_instruction',
          layerRef: 'instruction-layer.public.local_private',
          metadataRefs: ['metadata.public.safe', 'raw memory /Users/christopher/memory.md'],
          policyRefs: ['policy.public.safe', 'provider prompt sk-private'],
          precedence: 9,
          redactionClass: 'local_only',
          sourceRefs: ['source.public.safe', 'private instruction /Users/christopher/AGENTS.md'],
          state: 'applied',
        },
      ],
      projectionRef: 'instruction-projection.public.safe',
      snapshotRef: 'instruction-snapshot.public.safe',
      versionRef: 'instruction-version.public.safe',
      workOrderRef: 'work_1',
    })
    const payload = JSON.stringify(view)

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain('instruction-blocker.public.safe')
    expect(view.blockerRefs).toContain(
      'forge-instruction-layering-blocker:work_1:unsafe-instruction-material-omitted',
    )
    expect(view.layers[0]?.allowedToolRefs).toEqual(['tool.public.safe'])
    expect(view.layers[0]?.metadataRefs).toEqual(['metadata.public.safe'])
    expect(view.layers[0]?.policyRefs).toEqual(['policy.public.safe'])
    expect(view.layers[0]?.sourceRefs).toEqual(['source.public.safe'])
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('prompt body')
    expect(payload).not.toContain('raw instruction')
    expect(payload).not.toContain('raw memory')
    expect(payload).not.toContain('provider prompt')
    expect(payload).not.toContain('sk-private')
  })
})
