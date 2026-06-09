import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OPENAGENTS_NATIVE_CONTRACT_CONSUMERS,
  OPENAGENTS_NATIVE_CONTRACT_REF_KINDS,
  OPENAGENTS_NATIVE_CONTRACT_REGISTRY_V1,
  OpenAgentsNativeContractRegistry,
  OpenAgentsNativeContractRegistryProjection,
  OpenAgentsNativeContractRegistryUnsafe,
  openAgentsNativeContractEntryCarriesAuthorityBoundary,
  openAgentsNativeContractEntryIsEvidenceOnly,
  openAgentsNativeContractRegistryCoversConsumers,
  openAgentsNativeContractRegistryCoversRefKinds,
  openAgentsNativeContractRegistryProjectionHasPrivateMaterial,
  projectOpenAgentsNativeContractRegistry,
} from './native-contract-registry'
import {
  OPENAGENTS_UNSAFE_REDACTION_FIXTURES,
  openAgentsSerializedValueContainsUnsafeFixture,
} from './redaction-regression-fixtures'

const nowIso = '2026-06-07T00:00:00.000Z'

describe('OpenAgents native contract registry', () => {
  test('decodes the seed registry and covers required consumers and ref kinds', () => {
    expect(S.decodeUnknownSync(OpenAgentsNativeContractRegistry)(
      OPENAGENTS_NATIVE_CONTRACT_REGISTRY_V1,
    )).toEqual(OPENAGENTS_NATIVE_CONTRACT_REGISTRY_V1)
    expect(OPENAGENTS_NATIVE_CONTRACT_CONSUMERS).toEqual([
      'ai_agent',
      'nexus',
      'oa_node',
      'oa_workroomd',
      'omega_worker',
      'probe',
      'psionic',
      'pylon',
      'treasury',
    ])
    expect(OPENAGENTS_NATIVE_CONTRACT_REF_KINDS).toEqual([
      'artifact',
      'assignment',
      'capability',
      'heartbeat',
      'lifecycle_event',
      'policy',
      'receipt',
      'redaction',
      'route',
    ])
    expect(openAgentsNativeContractRegistryCoversConsumers(
      OPENAGENTS_NATIVE_CONTRACT_REGISTRY_V1,
    )).toBe(true)
    expect(openAgentsNativeContractRegistryCoversRefKinds(
      OPENAGENTS_NATIVE_CONTRACT_REGISTRY_V1,
    )).toBe(true)
  })

  test('distinguishes evidence events from authority-boundary action contracts', () => {
    const heartbeat = OPENAGENTS_NATIVE_CONTRACT_REGISTRY_V1.entries.find(
      entry => entry.refKind === 'heartbeat',
    )
    const assignment = OPENAGENTS_NATIVE_CONTRACT_REGISTRY_V1.entries.find(
      entry => entry.refKind === 'assignment',
    )
    const receipt = OPENAGENTS_NATIVE_CONTRACT_REGISTRY_V1.entries.find(
      entry => entry.refKind === 'receipt',
    )

    expect(heartbeat).toBeDefined()
    expect(assignment).toBeDefined()
    expect(receipt).toBeDefined()
    expect(openAgentsNativeContractEntryIsEvidenceOnly(heartbeat!)).toBe(true)
    expect(openAgentsNativeContractEntryCarriesAuthorityBoundary(assignment!))
      .toBe(true)
    expect(openAgentsNativeContractEntryCarriesAuthorityBoundary(receipt!))
      .toBe(true)
  })

  test('projects public-safe registry data with friendly times and no private material', () => {
    const projection = projectOpenAgentsNativeContractRegistry(
      OPENAGENTS_NATIVE_CONTRACT_REGISTRY_V1,
      'public',
      nowIso,
    )

    expect(S.decodeUnknownSync(OpenAgentsNativeContractRegistryProjection)(
      projection,
    )).toEqual(projection)
    expect(projection.entryCount).toBe(9)
    expect(projection.evidenceOnlyCount).toBe(7)
    expect(projection.authorityActionCount).toBe(2)
    expect(projection.createdAtDisplay).toBe('10 minutes ago')
    expect(JSON.stringify(projection)).not.toContain('2026-06-06T')
    expect(openAgentsNativeContractRegistryProjectionHasPrivateMaterial(
      projection,
    )).toBe(false)
    expect(openAgentsSerializedValueContainsUnsafeFixture(projection)).toBe(
      false,
    )
  })

  test('rejects unsafe refs before projection', () => {
    for (const fixture of OPENAGENTS_UNSAFE_REDACTION_FIXTURES) {
      expect(() =>
        projectOpenAgentsNativeContractRegistry(
          {
            ...OPENAGENTS_NATIVE_CONTRACT_REGISTRY_V1,
            entries: [
              {
                ...OPENAGENTS_NATIVE_CONTRACT_REGISTRY_V1.entries[0]!,
                sourceAuthorityRefs: [fixture.value],
              },
            ],
          },
          'operator',
          nowIso,
        ),
      ).toThrow(OpenAgentsNativeContractRegistryUnsafe)
    }
  })
})
