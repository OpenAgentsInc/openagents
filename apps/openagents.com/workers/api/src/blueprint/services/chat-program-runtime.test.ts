import {
  TASSADAR_EXECUTOR_CAPABILITY_REF,
} from '@openagentsinc/tassadar-executor'
import { LAUNCH_RECOGNITION_REPLAY_SLUG } from '@openagentsinc/proof-replay'
import {
  tassadarDenseProgramFixture,
  tassadarDenseWeightModuleDigest,
} from '@openagentsinc/tassadar-executor/dense-weight-module'
import { Effect, Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  AUTOPILOT_CONTINUATION_MODULE_VERSIONS,
  AUTOPILOT_CONTINUATION_PROGRAM_SIGNATURES,
  AUTOPILOT_CONTINUATION_RELEASE_GATES,
} from '../fixtures/autopilot-continuation-signatures'
import {
  AUTOPILOT_CONTINUATION_PROGRAM_TYPES,
  AUTOPILOT_CONTINUATION_PROGRAM_REGISTRY,
  AUTOPILOT_CONTINUATION_PROGRAM_REGISTRY_VERSION_REF,
} from '../fixtures/program-registry'
import {
  BLUEPRINT_REPLAY_PROGRAM_SIGNATURE_ID,
  BLUEPRINT_REPLAY_PROGRAM_TYPE_ID,
  BLUEPRINT_REPLAY_TOOL_REF,
} from '../fixtures/replay-signatures'
import { blueprintProgramRegistryProjection } from '../schemas/program-registry'
import {
  BLUEPRINT_TASSADAR_DENSE_FIXTURE_MODULE_REF,
  BLUEPRINT_TASSADAR_DENSE_MODULE_CLAIM_CLASS,
  BLUEPRINT_TASSADAR_MODULE_REQUIRED_TRUST_POSTURE,
} from './tassadar-module-step'
import { BlueprintProgramRunDirectEffectDenied } from './program-run-authority'
import {
  BlueprintChatProgramTurnResult,
  type BlueprintChatProgramRuntimePrimitives,
  type BlueprintChatProgramSessionResult,
  type BlueprintChatProgramSessionRuntime,
  type BlueprintChatProgramSessionSpawnInput,
  type BlueprintChatProgramTurnInput,
  executeBlueprintChatProgramTurn,
} from './chat-program-runtime'
import { makePublicProofReplayModuleRuntime } from './replay-module'

const deterministicRuntime: BlueprintChatProgramRuntimePrimitives = {
  makeLookupId: () => 'blueprint_signature_lookup.chat.test',
  makeMenuId: () => 'blueprint_tool_menu.chat.test',
  makeProgramRunId: () => 'blueprint_program_run.chat.test',
  nowIso: () => '2026-06-18T12:00:00.000Z',
}

const baseTurn = (
  overrides: Partial<BlueprintChatProgramTurnInput> = {},
): BlueprintChatProgramTurnInput => ({
  actorRef: 'actor.operator.test',
  allowedSurfaces: ['operator_dashboard'],
  backendCapabilityRefs: ['capability.autopilot.session_spawn'],
  backendKind: 'codex',
  backendProfileId: 'backend_profile.codex.local',
  contextPackRef: 'context_pack.chat.turn_1',
  costRef: 'cost.blueprint_chat.turn_1',
  inputSnapshotHash: 'sha256:chat_turn_1',
  model: 'codex.local',
  preferredFamily: 'continuation',
  programSignatureIds: ['program_signature.autopilot.continue.v1'],
  programTypeIds: ['program_type.autopilot.continue'],
  promptSummaryRef: 'prompt_summary.chat.turn_1',
  registryVersionRef: AUTOPILOT_CONTINUATION_PROGRAM_REGISTRY_VERSION_REF,
  riskCeiling: 'medium',
  routeRef: 'route.blueprint.chat',
  runnerRef: 'runner.desktop.local',
  sessionAdapter: 'codex',
  sessionLane: 'local',
  sessionObjectiveRef: 'objective_ref.chat.turn_1',
  sourceAuthorityRefs: ['source_authority.repo.openagents'],
  supportedToolRefs: [
    'tool.context_pack.read',
    'tool.action_submission.propose',
  ],
  threadRef: 'thread.chat.1',
  timeoutSeconds: 120,
  turnRef: 'turn.chat.1',
  workroomRef: 'workroom.chat.1',
  ...overrides,
})

const makeSessionRuntime = (
  overrides: Partial<BlueprintChatProgramSessionResult> = {},
) => {
  const calls: Array<BlueprintChatProgramSessionSpawnInput> = []
  const sessionRuntime: BlueprintChatProgramSessionRuntime = {
    spawnSession: input => {
      calls.push(input)

      return Effect.succeed({
        confidence: 0.82,
        evidenceRefs: ['evidence.session.response_1'],
        events: [
          {
            eventRef: 'event.session.spawned_1',
            evidenceRefs: ['evidence.session.spawned_1'],
            observedAt: '2026-06-18T12:00:01.000Z',
            phase: 'session.spawn',
            receiptRefs: ['receipt.session.spawned_1'],
            safeProjection: true,
            state: 'completed',
            summaryRef: 'summary.session.spawned_1',
          },
        ],
        latencyMs: 1275,
        receiptRefs: ['receipt.session.finished_1'],
        renderedResponseRef: 'rendered_response.chat.turn_1',
        requestedDirectEffects: [],
        responseRef: 'response.chat.turn_1',
        responseSummaryRef: 'response_summary.chat.turn_1',
        sessionRef: 'session.chat.turn_1',
        toolCallbackRefs: ['tool_callback.session.context_1'],
        typedOutput: {
          responseRef: 'response.chat.turn_1',
          state: 'completed',
        },
        usage: {
          completionTokens: 16,
          promptTokens: 44,
          totalTokens: 60,
          truth: 'estimated',
        },
        ...overrides,
      })
    },
  }

  return { calls, sessionRuntime }
}

const registryWithTassadarStep = () => {
  const tassadarScope = {
    access: 'evidence',
    allowedSurfaces: ['agent_api', 'operator_dashboard'],
    requiresApproval: false,
    tassadarModuleStep: {
      executionMode: 'registry_resolved',
      expectedCapabilityRef: TASSADAR_EXECUTOR_CAPABILITY_REF,
      expectedClaimClass: BLUEPRINT_TASSADAR_DENSE_MODULE_CLAIM_CLASS,
      expectedModuleDigest: tassadarDenseWeightModuleDigest,
      expectedTraceDigest: tassadarDenseProgramFixture.expectedTraceDigest,
      expectedTrustPosture: BLUEPRINT_TASSADAR_MODULE_REQUIRED_TRUST_POSTURE,
      kind: 'tassadar_module_step',
      moduleKind: 'dense_weight_module',
      moduleRef: BLUEPRINT_TASSADAR_DENSE_FIXTURE_MODULE_REF,
      registryRef: 'registry.tassadar_modules.seed.2026-06-18',
      stepRef: 'step.tassadar.chat.loop_sum_dense',
    },
    toolRef: 'tool.tassadar.module.execute',
  } as const

  return blueprintProgramRegistryProjection({
    moduleVersions: AUTOPILOT_CONTINUATION_MODULE_VERSIONS,
    programSignatures: AUTOPILOT_CONTINUATION_PROGRAM_SIGNATURES.map(
      signature =>
        signature.id === 'program_signature.autopilot.continue.v1'
          ? {
              ...signature,
              toolScopes: [...signature.toolScopes, tassadarScope],
            }
          : signature,
    ),
    programTypes: AUTOPILOT_CONTINUATION_PROGRAM_TYPES.map(programType =>
      programType.id === 'program_type.autopilot.continue'
        ? {
            ...programType,
            toolScopes: [...programType.toolScopes, tassadarScope],
          }
        : programType,
    ),
    releaseGates: AUTOPILOT_CONTINUATION_RELEASE_GATES,
  })
}

describe('Blueprint chat-program runtime', () => {
  test('runs a chat turn through signature selection, scoped tools, session substrate, and evidence-only Program Run output', async () => {
    const harness = makeSessionRuntime()
    const result = await Effect.runPromise(
      executeBlueprintChatProgramTurn({
        registryProjection: AUTOPILOT_CONTINUATION_PROGRAM_REGISTRY,
        runtime: deterministicRuntime,
        sessionRuntime: harness.sessionRuntime,
        turn: baseTurn(),
      }),
    )

    expect(S.decodeUnknownSync(BlueprintChatProgramTurnResult)(result)).toEqual(
      result,
    )
    expect(harness.calls).toHaveLength(1)
    expect(harness.calls[0]).toMatchObject({
      adapter: 'codex',
      objectiveRef: 'objective_ref.chat.turn_1',
      verifyRefs: ['receipt.program_run'],
    })
    expect(result.lookup.programSignatureIds).toEqual([
      'program_signature.autopilot.continue.v1',
    ])
    expect(result.toolMenu.tools.map(tool => tool.toolRef)).toEqual([
      'tool.context_pack.read',
      'tool.action_submission.propose',
    ])
    expect(result.toolMenu.tools[1]).toMatchObject({
      policy: 'approval_required',
    })
    expect(result.programRun).toMatchObject({
      authorityBoundary: 'evidence_only',
      directMutationDisabled: true,
      id: 'blueprint_program_run.chat.test',
      noDeploy: true,
      noEmail: true,
      noSourceMutation: true,
      noSpend: true,
      programSignatureId: 'program_signature.autopilot.continue.v1',
      programTypeId: 'program_type.autopilot.continue',
      purposeRef: 'purpose.autopilot.continue',
    })
    expect(result.response).toMatchObject({
      contentRedacted: true,
      responseSummaryRef: 'response_summary.chat.turn_1',
      sessionRef: 'session.chat.turn_1',
    })
    expect(result.sessionSubstrateRef).toBe(
      'substrate.autopilot.session_spawn.node_state_poll',
    )
  })

  test('folds a bound Tassadar module step exact-replay evidence into the Program Run record', async () => {
    const harness = makeSessionRuntime()
    const result = await Effect.runPromise(
      executeBlueprintChatProgramTurn({
        registryProjection: registryWithTassadarStep(),
        runtime: deterministicRuntime,
        sessionRuntime: harness.sessionRuntime,
        turn: baseTurn({
          supportedToolRefs: [
            'tool.context_pack.read',
            'tool.action_submission.propose',
            'tool.tassadar.module.execute',
          ],
        }),
      }),
    )

    expect(result.toolMenu.tools.map(tool => tool.toolRef)).toContain(
      'tool.tassadar.module.execute',
    )
    expect(result.tassadarModuleStepEvidence).toHaveLength(1)
    expect(result.tassadarModuleStepEvidence[0]).toMatchObject({
      moduleKind: 'dense_weight_module',
      stepRef: 'step.tassadar.chat.loop_sum_dense',
      verdict: 'verified',
    })
    expect(result.programRun.evidenceRefs).toEqual(
      expect.arrayContaining(
        [...result.tassadarModuleStepEvidence[0]!.evidenceRefs],
      ),
    )
    expect(
      result.programRun.receiptRefs.some(ref =>
        ref.startsWith('receipt.openagents.blueprint_tassadar_step.'),
      ),
    ).toBe(true)
  })

  test('selects ShowReplay through typed proof-projection constraints and returns the real proof replay bundle', async () => {
    const harness = makeSessionRuntime()
    const turn = baseTurn({
      contextPackRef: undefined,
      preferredFamily: 'proof_projection',
      programSignatureIds: undefined,
      programTypeIds: undefined,
      promptSummaryRef: 'prompt_summary.chat.bundle_turn',
      replayIntentRef: 'intent.blueprint.show_public_bundle',
      replaySlug: LAUNCH_RECOGNITION_REPLAY_SLUG,
      replayTargetRef: 'proof_replay.launch_recognition',
      sessionObjectiveRef: 'objective_ref.chat.bundle_turn',
      supportedToolRefs: [BLUEPRINT_REPLAY_TOOL_REF],
    })

    expect(
      `${turn.promptSummaryRef} ${turn.sessionObjectiveRef} ${turn.replayIntentRef}`,
    ).not.toMatch(/\breplay\b/i)

    const result = await Effect.runPromise(
      executeBlueprintChatProgramTurn({
        registryProjection: AUTOPILOT_CONTINUATION_PROGRAM_REGISTRY,
        replayRuntime: makePublicProofReplayModuleRuntime({} as never),
        runtime: deterministicRuntime,
        sessionRuntime: harness.sessionRuntime,
        turn,
      }),
    )

    expect(result.lookup.programSignatureIds).toEqual([
      BLUEPRINT_REPLAY_PROGRAM_SIGNATURE_ID,
    ])
    expect(result.lookup.programTypeIds).toEqual([
      BLUEPRINT_REPLAY_PROGRAM_TYPE_ID,
    ])
    expect(result.toolMenu.tools.map(tool => tool.toolRef)).toEqual([
      BLUEPRINT_REPLAY_TOOL_REF,
    ])
    expect(result.replayModuleEvidence).toHaveLength(1)
    expect(result.replayModuleEvidence[0]).toMatchObject({
      bundleRef: expect.stringContaining(
        'proof_replay_bundle.launch_recognition',
      ),
      kind: 'blueprint_replay_module_evidence',
      replaySlug: LAUNCH_RECOGNITION_REPLAY_SLUG,
      sourceAuthority: 'worker_d1_public',
    })
    expect(
      (result.replayModuleEvidence[0]!.bundle as { schemaVersion?: string })
        .schemaVersion,
    ).toBe('proof_replay_bundle.v1')
    expect(
      (result.replayModuleEvidence[0]!.bundle as { events?: unknown[] }).events
        ?.length,
    ).toBeGreaterThan(0)
    expect(result.programRun.typedOutput.replayBundles).toEqual([
      expect.objectContaining({
        replaySlug: LAUNCH_RECOGNITION_REPLAY_SLUG,
      }),
    ])
    expect(result.programRun.receiptRefs).toEqual(
      expect.arrayContaining(
        result.replayModuleEvidence[0]!.receiptRefs as Array<string>,
      ),
    )
    expect(harness.calls[0]?.verifyRefs).toEqual(
      expect.arrayContaining(['receipt.public_proof_replay_bundle']),
    )
  })

  test('denies direct write effects from the Program Run path', async () => {
    const harness = makeSessionRuntime({
      requestedDirectEffects: ['deploy'],
    })

    await expect(
      Effect.runPromise(
        executeBlueprintChatProgramTurn({
          registryProjection: AUTOPILOT_CONTINUATION_PROGRAM_REGISTRY,
          runtime: deterministicRuntime,
          sessionRuntime: harness.sessionRuntime,
          turn: baseTurn(),
        }),
      ),
    ).rejects.toBeInstanceOf(BlueprintProgramRunDirectEffectDenied)
  })
})
