import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import type { BlueprintProgramRunRecord } from '../schemas/program-run'
import {
  blueprintProgramRegistryProjection,
  blueprintProgramRegistryProjectionIsSafe,
  BlueprintProgramRegistryApiSeed as BlueprintProgramRegistryApiSeedSchema,
} from '../schemas/program-registry'
import {
  AUTOPILOT_CONTINUATION_ACTIONS,
  AUTOPILOT_CONTINUATION_MODULE_VERSIONS,
  AUTOPILOT_CONTINUATION_RELEASE_GATES,
} from './autopilot-continuation-signatures'
import {
  DELIVERY_PIPELINE_PROGRAMS,
  deliveryPipelineProgramTypeId,
} from '../delivery-pipeline-programs'
import {
  AUTOPILOT_CONTINUATION_PROGRAM_REGISTRY,
  AUTOPILOT_CONTINUATION_PROGRAM_REGISTRY_API_SEED,
  AUTOPILOT_CONTINUATION_PROGRAM_TYPES,
} from './program-registry'
import {
  BLUEPRINT_REPLAY_PROGRAM_SIGNATURE_ID,
  BLUEPRINT_REPLAY_PROGRAM_TYPE_ID,
  BLUEPRINT_REPLAY_TOOL_REF,
} from './replay-signatures'

const run: BlueprintProgramRunRecord = {
  actorRef: 'actor.operator.test',
  archivedAt: null,
  authorityBoundary: 'evidence_only',
  confidence: 0.82,
  costRef: 'cost.autopilot.continue.test',
  createdAt: '2026-06-05T00:00:00.000Z',
  directMutationDisabled: true,
  evidenceRefs: ['evidence.context_pack_1', 'evidence.failure_case_1'],
  id: 'blueprint_program_run.continue.test_1',
  idempotencyKey: 'idem.blueprint_program_run.continue.test_1',
  inputSnapshotHash: 'sha256.input_snapshot_1',
  latencyMs: 1210,
  metadata: {
    internalTrace: 'excluded from projection',
  },
  moduleVersionId: 'module_version.autopilot.continue.candidate_1',
  noDeploy: true,
  noEmail: true,
  noSourceMutation: true,
  noSpend: true,
  programSignatureId: 'program_signature.autopilot.continue.v1',
  programTypeId: 'program_type.autopilot.continue',
  purposeRef: 'purpose.autopilot.continue',
  receiptRefs: ['receipt.program_run_1'],
  routeRef: 'route.autopilot.continue',
  typedOutput: {
    privateReasoning: 'excluded from projection',
  },
  updatedAt: '2026-06-05T00:00:00.000Z',
}

describe('Blueprint Program Registry projection', () => {
  test('seeds an operator-safe registry entry for every continuation action', () => {
    expect(AUTOPILOT_CONTINUATION_PROGRAM_REGISTRY.entries).toHaveLength(
      AUTOPILOT_CONTINUATION_ACTIONS.length +
        DELIVERY_PIPELINE_PROGRAMS.length +
        1,
    )

    const continuationEntries =
      AUTOPILOT_CONTINUATION_PROGRAM_REGISTRY.entries.filter(entry =>
        entry.programTypeId.startsWith('program_type.autopilot.'),
      )
    expect(continuationEntries).toHaveLength(
      AUTOPILOT_CONTINUATION_ACTIONS.length,
    )
    expect(
      continuationEntries.every(
        entry =>
          entry.safeProjection &&
          !entry.directMutationAllowed &&
          entry.programSignatureIds.length === 1 &&
          entry.moduleVersionIds.length === 1 &&
          entry.releaseGateIds.length === 1,
      ),
    ).toBe(true)

    // Every projection entry must be operator-safe and evidence-only,
    // including the delivery-pipeline programs folded in for #4980.
    expect(
      AUTOPILOT_CONTINUATION_PROGRAM_REGISTRY.entries.every(
        entry => entry.safeProjection && !entry.directMutationAllowed,
      ),
    ).toBe(true)
  })

  test('seeds the ShowReplay proof-projection signature with a replay module scope', () => {
    const entry = AUTOPILOT_CONTINUATION_PROGRAM_REGISTRY.entries.find(
      candidate => candidate.programTypeId === BLUEPRINT_REPLAY_PROGRAM_TYPE_ID,
    )
    const signature =
      AUTOPILOT_CONTINUATION_PROGRAM_REGISTRY.programSignatures.find(
        candidate => candidate.id === BLUEPRINT_REPLAY_PROGRAM_SIGNATURE_ID,
      )

    expect(entry).toMatchObject({
      directMutationAllowed: false,
      family: 'proof_projection',
      programSignatureIds: [BLUEPRINT_REPLAY_PROGRAM_SIGNATURE_ID],
      programTypeId: BLUEPRINT_REPLAY_PROGRAM_TYPE_ID,
      safeProjection: true,
    })
    expect(signature).toMatchObject({
      inputSchema: { schemaRef: 'schema.blueprint.ShowReplayInput.v1' },
      outputSchema: { schemaRef: 'schema.blueprint.ShowReplayOutput.v1' },
      supportsProofProjection: true,
    })
    expect(signature?.toolScopes[0]).toMatchObject({
      replayModule: {
        kind: 'replay_module',
      },
      toolRef: BLUEPRINT_REPLAY_TOOL_REF,
    })
  })

  test('folds delivery-pipeline programs into the registry projection', () => {
    for (const program of DELIVERY_PIPELINE_PROGRAMS) {
      const programTypeId = deliveryPipelineProgramTypeId(program.stage)
      expect(programTypeId).toBe(program.programType.id)

      const entry = AUTOPILOT_CONTINUATION_PROGRAM_REGISTRY.entries.find(
        candidate => candidate.programTypeId === programTypeId,
      )
      expect(entry).toBeDefined()
      expect(entry!.directMutationAllowed).toBe(false)
      expect(entry!.safeProjection).toBe(true)
      expect(entry!.programSignatureIds).toHaveLength(1)

      const signature =
        AUTOPILOT_CONTINUATION_PROGRAM_REGISTRY.programSignatures.find(
          candidate => candidate.programTypeId === programTypeId,
        )
      expect(signature).toBeDefined()
      expect(signature!.outputSchema.schemaRef).toBe(program.outputSchemaRef)
    }
  })

  test('decodes the API seed contract for the operator route', () => {
    expect(
      S.decodeUnknownSync(BlueprintProgramRegistryApiSeedSchema)(
        AUTOPILOT_CONTINUATION_PROGRAM_REGISTRY_API_SEED,
      ),
    ).toEqual(AUTOPILOT_CONTINUATION_PROGRAM_REGISTRY_API_SEED)
    expect(AUTOPILOT_CONTINUATION_PROGRAM_REGISTRY_API_SEED.path).toBe(
      '/api/blueprint/program-registry',
    )
  })

  test('projects run details without raw output or metadata', () => {
    const projection = blueprintProgramRegistryProjection({
      moduleVersions: AUTOPILOT_CONTINUATION_MODULE_VERSIONS,
      programSignatures:
        AUTOPILOT_CONTINUATION_PROGRAM_REGISTRY.programSignatures,
      programTypes: AUTOPILOT_CONTINUATION_PROGRAM_TYPES,
      releaseGates: AUTOPILOT_CONTINUATION_RELEASE_GATES,
      runs: [run],
    })

    expect(blueprintProgramRegistryProjectionIsSafe(projection)).toBe(true)
    expect(projection.runDetails).toHaveLength(1)
    expect(projection.runDetails[0]).toMatchObject({
      failureRefs: ['evidence.failure_case_1'],
      id: 'blueprint_program_run.continue.test_1',
      programTypeId: 'program_type.autopilot.continue',
      safeProjection: true,
    })
    expect('typedOutput' in projection.runDetails[0]!).toBe(false)
    expect('metadata' in projection.runDetails[0]!).toBe(false)
  })
})
