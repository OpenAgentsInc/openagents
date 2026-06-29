import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  CodingAutopilotRepoMemoryProjection,
  CodingAutopilotRepoMemoryRecord,
  CodingAutopilotRepoMemoryUnsafe,
  codingAutopilotRepoMemoryProjectionHasPrivateMaterial,
  exampleCodingAutopilotRepoMemoryRecords,
  projectCodingAutopilotRepoMemory,
} from './coding-autopilot-repo-memory'

const nowIso = '2026-06-06T21:05:00.000Z'

describe('Coding on Autopilot repo memory', () => {
  test('projects public and private repo memory with audience redaction', () => {
    const [publicMemory, privateMemory] = exampleCodingAutopilotRepoMemoryRecords()
    const publicProjection = projectCodingAutopilotRepoMemory(
      publicMemory!,
      'public',
      nowIso,
    )
    const publicPrivateProjection = projectCodingAutopilotRepoMemory(
      privateMemory!,
      'public',
      nowIso,
    )
    const customerPrivateProjection = projectCodingAutopilotRepoMemory(
      privateMemory!,
      'customer',
      nowIso,
    )
    const operatorProjection = projectCodingAutopilotRepoMemory(
      privateMemory!,
      'operator',
      nowIso,
    )

    expect(S.decodeUnknownSync(CodingAutopilotRepoMemoryRecord)(publicMemory))
      .toEqual(publicMemory)
    expect(S.decodeUnknownSync(CodingAutopilotRepoMemoryProjection)(publicProjection))
      .toEqual(publicProjection)
    expect(publicProjection).toMatchObject({
      confidenceBucket: 'high',
      effectiveStatus: 'active',
      keywordRoutingAllowed: false,
      memoryKind: 'accepted_fix',
      repoRef: 'repo.OpenAgentsInc.otec_public',
      retrievalMode: 'typed_selector',
      updatedAtDisplay: '5 minutes ago',
    })
    expect(publicProjection.sourceAuthorityRefs).toEqual([])
    expect(publicProjection.workroomRefs).toEqual([])
    expect(publicPrivateProjection.repoRef).toBe('repo.redacted')
    expect(customerPrivateProjection.repoRef).toBe('repo.customer_app')
    expect(customerPrivateProjection.effectiveStatus).toBe('needs_review')
    expect(operatorProjection.sourceAuthorityRefs).toEqual([
      'source_authority.test_result_review',
    ])
  })

  test('supports every required memory kind', () => {
    const kinds = [
      'accepted_fix',
      'build_command',
      'denied_path',
      'dependency_note',
      'flaky_test',
      'pr_style',
      'rejected_fix',
      'repo_convention',
      'reviewer_preference',
      'test_command',
    ] as const
    const projections = kinds.map(memoryKind =>
      projectCodingAutopilotRepoMemory({
        ...exampleCodingAutopilotRepoMemoryRecords()[0]!,
        id: `repo_memory_${memoryKind}`,
        memoryKind,
        memoryRef: `memory.repo.${memoryKind}`,
        summaryRef: `summary.repo_memory.${memoryKind}`,
      }, 'customer', nowIso),
    )

    expect(projections.map(projection => projection.memoryKind)).toEqual(kinds)
    expect(projections.every(projection => projection.keywordRoutingAllowed === false))
      .toBe(true)
  })

  test('applies expiration and retrieval-mode rules', () => {
    const expired = projectCodingAutopilotRepoMemory({
      ...exampleCodingAutopilotRepoMemoryRecords()[0]!,
      expiresAtIso: '2026-06-06T20:00:00.000Z',
    }, 'customer', nowIso)

    expect(expired.effectiveStatus).toBe('expired')
    expect(() =>
      projectCodingAutopilotRepoMemory({
        ...exampleCodingAutopilotRepoMemoryRecords()[0]!,
        retrievalMode: 'typed_selector',
        selectorRefs: [],
      }, 'customer', nowIso),
    ).toThrow(CodingAutopilotRepoMemoryUnsafe)
    expect(() =>
      projectCodingAutopilotRepoMemory({
        ...exampleCodingAutopilotRepoMemoryRecords()[1]!,
        retrievalMode: 'semantic_embedding',
        semanticIndexRefs: [],
      }, 'customer', nowIso),
    ).toThrow(CodingAutopilotRepoMemoryUnsafe)
  })

  test('does not expose raw timestamps and rejects unsafe refs', () => {
    const projection = projectCodingAutopilotRepoMemory(
      exampleCodingAutopilotRepoMemoryRecords()[0]!,
      'customer',
      nowIso,
    )
    const serialized = JSON.stringify(projection)

    expect(serialized).not.toContain('2026-06-06T21:00:00.000Z')
    expect(codingAutopilotRepoMemoryProjectionHasPrivateMaterial(projection))
      .toBe(false)
    expect(() =>
      projectCodingAutopilotRepoMemory({
        ...exampleCodingAutopilotRepoMemoryRecords()[0]!,
        evidenceRefs: ['raw_runner_payload:mission'],
      }, 'public', nowIso),
    ).toThrow(CodingAutopilotRepoMemoryUnsafe)
    expect(() =>
      projectCodingAutopilotRepoMemory({
        ...exampleCodingAutopilotRepoMemoryRecords()[0]!,
        sourceAuthorityRefs: ['provider_token:abc'],
      }, 'operator', nowIso),
    ).toThrow(CodingAutopilotRepoMemoryUnsafe)
    expect(() =>
      projectCodingAutopilotRepoMemory({
        ...exampleCodingAutopilotRepoMemoryRecords()[0]!,
        repoRef: 'private_repo:https://github.com/customer/private-repo',
      }, 'customer', nowIso),
    ).toThrow(CodingAutopilotRepoMemoryUnsafe)
    expect(() =>
      projectCodingAutopilotRepoMemory({
        ...exampleCodingAutopilotRepoMemoryRecords()[0]!,
        caveatRefs: ['ben@example.com'],
      }, 'customer', nowIso),
    ).toThrow(CodingAutopilotRepoMemoryUnsafe)
  })
})
