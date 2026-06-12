import { BlueprintAssignmentScope } from '@openagentsinc/sync-schema'
import { describe, expect, test } from 'vitest'

import {
  AUTOPILOT_CONTINUATION_PROGRAM_REGISTRY_VERSION_REF,
} from './blueprint/fixtures/program-registry'
import {
  ProbeBlueprintAssignmentScopeUnsafe,
  assertProbeBlueprintAssignmentScopeSafe,
  buildProbeBlueprintAssignmentScope,
  probeBlueprintAssignmentScopeIsSafe,
  probeBlueprintCapabilitySupportCoversAssignmentScope,
} from './probe-blueprint-assignment-scope'

describe('Probe Blueprint assignment scope', () => {
  test('builds a safe scope from the Omega Blueprint registry', () => {
    const scope = buildProbeBlueprintAssignmentScope({
      contextPackRefs: ['context_pack.openagents.thread_1'],
      includeContractExport: true,
      includeRegistry: true,
      sourceAuthorityRefs: ['source_authority.repo.openagents.omega'],
    })

    expect(scope.registryVersionRef).toBe(
      AUTOPILOT_CONTINUATION_PROGRAM_REGISTRY_VERSION_REF,
    )
    expect(scope.programTypeRefs).toEqual(['program_type.autopilot.continue'])
    expect(scope.programSignatureRefs).toEqual([
      'program_signature.autopilot.continue.v1',
    ])
    expect(scope.moduleVersionRefs).toEqual([
      'module_version.autopilot.continue.candidate_1',
    ])
    expect(scope.releaseGateRefs).toEqual([
      'release_gate.autopilot.continue.v1',
    ])
    expect(scope.toolScopeRefs).toEqual([
      'tool.action_submission.propose',
      'tool.context_pack.read',
    ])
    expect(scope.registry).toBeDefined()
    expect(scope.contractExport).toBeDefined()
    expect(probeBlueprintAssignmentScopeIsSafe(scope)).toBe(true)
    expect(JSON.stringify(scope)).not.toMatch(
      /callback_token|provider_payload|raw_prompt|private_key|sk-[a-z0-9]/i,
    )
  })

  test('rejects unsafe refs and registry slices before dispatch', () => {
    const scope = buildProbeBlueprintAssignmentScope({ includeRegistry: true })

    expect(() =>
      assertProbeBlueprintAssignmentScopeSafe(
        new BlueprintAssignmentScope({
          ...scope,
          registryVersionRef: 'registry.autopilot.bad',
        }),
      ),
    ).toThrow(ProbeBlueprintAssignmentScopeUnsafe)
    expect(() =>
      assertProbeBlueprintAssignmentScopeSafe(
        new BlueprintAssignmentScope({
          ...scope,
          programSignatureRefs: ['program_signature.autopilot.missing.v1'],
        }),
      ),
    ).toThrow(ProbeBlueprintAssignmentScopeUnsafe)
    expect(() =>
      assertProbeBlueprintAssignmentScopeSafe(
        new BlueprintAssignmentScope({
          ...scope,
          toolScopeRefs: ['raw_prompt.private'],
        }),
      ),
    ).toThrow(ProbeBlueprintAssignmentScopeUnsafe)
  })

  test('checks runner Blueprint capability reports as a narrowing gate', () => {
    const scope = buildProbeBlueprintAssignmentScope()
    const backendScoped = buildProbeBlueprintAssignmentScope({
      backendCapabilityRefs: ['probe.backend.apple_fm_bridge'],
    })
    const support = {
      backendCapabilityRefs: [],
      moduleVersionRefs: scope.moduleVersionRefs ?? [],
      programSignatureRefs: scope.programSignatureRefs ?? [],
      programTypeRefs: scope.programTypeRefs ?? [],
      registryVersionRefs: [scope.registryVersionRef],
      safeProjection: true,
      toolRefs: scope.toolScopeRefs ?? [],
    }

    expect(probeBlueprintCapabilitySupportCoversAssignmentScope(
      support,
      scope,
    )).toBe(true)
    expect(probeBlueprintCapabilitySupportCoversAssignmentScope(
      {
        ...support,
        programSignatureRefs: [],
      },
      scope,
    )).toBe(false)
    expect(probeBlueprintCapabilitySupportCoversAssignmentScope(
      {
        ...support,
        safeProjection: false,
      },
      scope,
    )).toBe(false)
    expect(probeBlueprintCapabilitySupportCoversAssignmentScope(
      {
        ...support,
        backendCapabilityRefs: ['probe.backend.apple_fm_bridge'],
      },
      backendScoped,
    )).toBe(true)
    expect(probeBlueprintCapabilitySupportCoversAssignmentScope(
      support,
      backendScoped,
    )).toBe(false)
  })
})
