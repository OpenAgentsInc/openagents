import {
  TASSADAR_EXECUTOR_CAPABILITY_REF,
} from '@openagentsinc/tassadar-executor'
import {
  tassadarDenseProgramFixture,
  tassadarDenseWeightModuleDigest,
} from '@openagentsinc/tassadar-executor/dense-weight-module'
import {
  TASSADAR_ALM_LINKED_DENSE_MODULE_CLAIM_CLASS,
  TASSADAR_ALM_LINKED_DENSE_MODULE_DIGEST,
  TASSADAR_ALM_LINKED_DENSE_REQUIRED_TRUST_POSTURE,
  tassadarLinkedDenseProgramFixture,
} from '@openagentsinc/tassadar-executor/linked-dense-module'
import { Effect, Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import type { BlueprintProgramToolScope } from '../schemas/program'
import {
  BLUEPRINT_TASSADAR_MODULE_REGISTRY_VERSION_REF,
  type BlueprintTassadarModuleRegistryResolveInput,
  resolveBlueprintTassadarModuleRegistryEntry,
} from '../repositories/tassadar-module-registry'
import {
  BLUEPRINT_TASSADAR_DENSE_FIXTURE_MODULE_REF,
  BLUEPRINT_TASSADAR_DENSE_MODULE_CLAIM_CLASS,
  BLUEPRINT_TASSADAR_LINKED_FIXTURE_MODULE_REF,
  BLUEPRINT_TASSADAR_MODULE_FIXTURE_REGISTRY_REF,
  BLUEPRINT_TASSADAR_MODULE_REQUIRED_TRUST_POSTURE,
  BlueprintTassadarModuleStepEvidence,
  BlueprintTassadarModuleStepRefused,
  BlueprintTassadarModuleStepUnsafe,
  executeBlueprintTassadarModuleStep,
} from './tassadar-module-step'

const denseScope = (): BlueprintProgramToolScope => ({
  access: 'evidence',
  allowedSurfaces: ['agent_api', 'pylon_desktop'],
  requiresApproval: false,
  tassadarModuleStep: {
    executionMode: 'fixture_bound',
    expectedCapabilityRef: TASSADAR_EXECUTOR_CAPABILITY_REF,
    expectedClaimClass: BLUEPRINT_TASSADAR_DENSE_MODULE_CLAIM_CLASS,
    expectedModuleDigest: tassadarDenseWeightModuleDigest,
    expectedTraceDigest: tassadarDenseProgramFixture.expectedTraceDigest,
    expectedTrustPosture: BLUEPRINT_TASSADAR_MODULE_REQUIRED_TRUST_POSTURE,
    kind: 'tassadar_module_step',
    moduleKind: 'dense_weight_module',
    moduleRef: BLUEPRINT_TASSADAR_DENSE_FIXTURE_MODULE_REF,
    registryRef: BLUEPRINT_TASSADAR_MODULE_FIXTURE_REGISTRY_REF,
    stepRef: 'step.tassadar.loop_sum_dense',
  },
  toolRef: 'tool.tassadar.module.execute',
})

const linkedScope = (): BlueprintProgramToolScope => ({
  access: 'evidence',
  allowedSurfaces: ['agent_api', 'pylon_desktop'],
  requiresApproval: false,
  tassadarModuleStep: {
    executionMode: 'fixture_bound',
    expectedCapabilityRef: TASSADAR_EXECUTOR_CAPABILITY_REF,
    expectedClaimClass: TASSADAR_ALM_LINKED_DENSE_MODULE_CLAIM_CLASS,
    expectedModuleDigest: TASSADAR_ALM_LINKED_DENSE_MODULE_DIGEST,
    expectedTraceDigest: tassadarLinkedDenseProgramFixture.composedTraceDigest,
    expectedTrustPosture: TASSADAR_ALM_LINKED_DENSE_REQUIRED_TRUST_POSTURE,
    kind: 'tassadar_module_step',
    moduleKind: 'linked_dense_module',
    moduleRef: BLUEPRINT_TASSADAR_LINKED_FIXTURE_MODULE_REF,
    registryRef: BLUEPRINT_TASSADAR_MODULE_FIXTURE_REGISTRY_REF,
    stepRef: 'step.tassadar.linked_dense',
  },
  toolRef: 'tool.tassadar.module.execute',
})

describe('Blueprint Tassadar module step service', () => {
  test('executes a single dense module step and returns born-verified evidence', async () => {
    const evidence = await Effect.runPromise(
      executeBlueprintTassadarModuleStep(denseScope(), {
        observedAt: '2026-06-18T00:00:00.000Z',
      }),
    )

    expect(S.decodeUnknownSync(BlueprintTassadarModuleStepEvidence)(evidence))
      .toEqual(evidence)
    expect(evidence).toMatchObject({
      authorityBoundary: 'evidence_only',
      contentRedacted: true,
      directMutationDisabled: true,
      moduleKind: 'dense_weight_module',
      noDeploy: true,
      noEmail: true,
      noSourceMutation: true,
      noSpend: true,
      replayedTraceDigest: tassadarDenseProgramFixture.expectedTraceDigest,
      verdict: 'verified',
    })
    expect(evidence.result).toMatchObject({
      halted: true,
      outputValues: [15],
    })
    expect(evidence.receiptRefs).toContain(
      `receipt.openagents.blueprint_tassadar_step.${tassadarDenseProgramFixture.expectedTraceDigest.slice(0, 16)}`,
    )
  })

  test('executes a composed linked module step and returns replay receipts', async () => {
    const evidence = await Effect.runPromise(
      executeBlueprintTassadarModuleStep(linkedScope(), {
        observedAt: '2026-06-18T00:00:00.000Z',
      }),
    )

    expect(evidence).toMatchObject({
      authorityBoundary: 'evidence_only',
      contentRedacted: true,
      moduleKind: 'linked_dense_module',
      replayedTraceDigest: tassadarLinkedDenseProgramFixture.composedTraceDigest,
      verdict: 'verified',
    })
    expect(evidence.result).toMatchObject({
      compositionVerificationCleared: true,
      constituentVerificationCount: 2,
      replayVerificationCleared: true,
    })
    expect(evidence.receiptRefs).toContain(
      'receipt.openagents.tassadar_linked_dense_replay.cc1403674fc0d388',
    )
    expect(evidence.receiptRefs).toContain(
      'receipt.openagents.tassadar_linked_dense_composition.cc1403674fc0d388',
    )
  })

  test('resolves a module binding through the registry service at runtime', async () => {
    let capturedInput: BlueprintTassadarModuleRegistryResolveInput | undefined
    const evidence = await Effect.runPromise(
      executeBlueprintTassadarModuleStep(
        {
          ...linkedScope(),
          tassadarModuleStep: {
            ...linkedScope().tassadarModuleStep!,
            executionMode: 'registry_resolved',
            registryRef: BLUEPRINT_TASSADAR_MODULE_REGISTRY_VERSION_REF,
          },
        },
        {
          observedAt: '2026-06-18T00:00:00.000Z',
          resolveModule: input => {
            capturedInput = input
            return resolveBlueprintTassadarModuleRegistryEntry(input)
          },
        },
      ),
    )

    expect(capturedInput).toMatchObject({
      moduleRef: BLUEPRINT_TASSADAR_LINKED_FIXTURE_MODULE_REF,
      requiredClaimClass: TASSADAR_ALM_LINKED_DENSE_MODULE_CLAIM_CLASS,
      requiredModuleKind: 'linked_dense_module',
      requiredTrustPosture: TASSADAR_ALM_LINKED_DENSE_REQUIRED_TRUST_POSTURE,
    })
    expect(evidence.registryRef).toBe(
      BLUEPRINT_TASSADAR_MODULE_REGISTRY_VERSION_REF,
    )
    expect(evidence.verdict).toBe('verified')
  })

  test('rejects digest mismatches without issuing step receipts', async () => {
    const base = denseScope()
    const scope: BlueprintProgramToolScope = {
      ...base,
      tassadarModuleStep: {
        ...base.tassadarModuleStep!,
        expectedTraceDigest: '0'.repeat(64),
      },
    }

    const evidence = await Effect.runPromise(
      executeBlueprintTassadarModuleStep(scope),
    )

    expect(evidence.verdict).toBe('rejected')
    expect(evidence.replayedTraceDigest).toBe(
      tassadarDenseProgramFixture.expectedTraceDigest,
    )
    expect(evidence.blockerRefs).toContain(
      'blocker.public.blueprint_tassadar_step.trace_digest_mismatch',
    )
    expect(evidence.receiptRefs).toEqual([])
  })

  test('refuses a Tassadar step that attempts to carry write authority', async () => {
    await expect(
      Effect.runPromise(
        executeBlueprintTassadarModuleStep({
          ...denseScope(),
          access: 'propose_action',
          requiresApproval: true,
        }),
      ),
    ).rejects.toBeInstanceOf(BlueprintTassadarModuleStepRefused)
  })

  test('rejects private-data-shaped scope material before execution', async () => {
    const base = denseScope()
    const scope: BlueprintProgramToolScope = {
      ...base,
      tassadarModuleStep: {
        ...base.tassadarModuleStep!,
        moduleRef: 'raw_prompt.secret',
      },
    }

    await expect(
      Effect.runPromise(executeBlueprintTassadarModuleStep(scope)),
    ).rejects.toBeInstanceOf(BlueprintTassadarModuleStepUnsafe)
  })
})
