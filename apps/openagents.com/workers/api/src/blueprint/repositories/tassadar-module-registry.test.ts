import {
  TASSADAR_ALM_LINKED_DENSE_MODULE_CLAIM_CLASS,
  TASSADAR_COMPILED_WEIGHT_MODULE_LISTING_REF,
} from '@openagentsinc/tassadar-executor/linked-dense-module'
import { Effect, Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  BLUEPRINT_TASSADAR_DENSE_MODULE_CLAIM_CLASS,
  BLUEPRINT_TASSADAR_MODULE_REGISTRY_VERSION_REF,
  BLUEPRINT_TASSADAR_MODULE_REQUIRED_TRUST_POSTURE,
  BlueprintTassadarModuleRegistryProjection,
  BlueprintTassadarModuleRegistryResolveError,
  blueprintTassadarModuleRegistryProjectionIsSafe,
  listBlueprintTassadarModuleRegistry,
  resolveBlueprintTassadarModuleRegistryEntry,
  seedBlueprintTassadarModuleRegistryEntries,
} from './tassadar-module-registry'

describe('Blueprint Tassadar module registry', () => {
  test('lists public-safe dense and linked module entries', async () => {
    const projection = await Effect.runPromise(
      listBlueprintTassadarModuleRegistry({
        generatedAt: '2026-06-18T00:00:00.000Z',
      }),
    )

    expect(
      S.decodeUnknownSync(BlueprintTassadarModuleRegistryProjection)(
        projection,
      ),
    ).toEqual(projection)
    expect(projection.registryVersionRef).toBe(
      BLUEPRINT_TASSADAR_MODULE_REGISTRY_VERSION_REF,
    )
    expect(projection.safeProjection).toBe(true)
    expect(blueprintTassadarModuleRegistryProjectionIsSafe(projection)).toBe(
      true,
    )
    expect(projection.modules.map(entry => entry.moduleKind)).toEqual([
      'dense_weight_module',
      'linked_dense_module',
    ])
  })

  test('resolves a linked module ref to digest, claim, and trust posture', async () => {
    const entry = await Effect.runPromise(
      resolveBlueprintTassadarModuleRegistryEntry({
        moduleRef: TASSADAR_COMPILED_WEIGHT_MODULE_LISTING_REF,
        requiredClaimClass: TASSADAR_ALM_LINKED_DENSE_MODULE_CLAIM_CLASS,
        requiredModuleKind: 'linked_dense_module',
        requiredTrustPosture: BLUEPRINT_TASSADAR_MODULE_REQUIRED_TRUST_POSTURE,
      }),
    )

    expect(entry.moduleRef).toBe(TASSADAR_COMPILED_WEIGHT_MODULE_LISTING_REF)
    expect(entry.moduleDigest).toMatch(/^[0-9a-f]{64}$/)
    expect(entry.traceDigest).toMatch(/^[0-9a-f]{64}$/)
    expect(entry.claimClass).toBe(TASSADAR_ALM_LINKED_DENSE_MODULE_CLAIM_CLASS)
    expect(entry.trustPosture).toBe(
      BLUEPRINT_TASSADAR_MODULE_REQUIRED_TRUST_POSTURE,
    )
  })

  test('returns a typed miss when the module ref is unknown', async () => {
    await expect(
      Effect.runPromise(
        resolveBlueprintTassadarModuleRegistryEntry({
          moduleRef: 'module.public.tassadar.unknown',
        }),
      ),
    ).rejects.toMatchObject({
      _tag: 'BlueprintTassadarModuleRegistryResolveError',
      kind: 'module_not_found',
    })
  })

  test('refuses claim-class and trust-posture mismatches', async () => {
    const dense = seedBlueprintTassadarModuleRegistryEntries()[0]!

    await expect(
      Effect.runPromise(
        resolveBlueprintTassadarModuleRegistryEntry({
          moduleRef: dense.moduleRef,
          requiredClaimClass: TASSADAR_ALM_LINKED_DENSE_MODULE_CLAIM_CLASS,
        }),
      ),
    ).rejects.toBeInstanceOf(BlueprintTassadarModuleRegistryResolveError)

    await expect(
      Effect.runPromise(
        resolveBlueprintTassadarModuleRegistryEntry({
          moduleRef: dense.moduleRef,
          requiredClaimClass: BLUEPRINT_TASSADAR_DENSE_MODULE_CLAIM_CLASS,
          requiredTrustPosture: 'unverified_public_claim',
        }),
      ),
    ).rejects.toMatchObject({
      _tag: 'BlueprintTassadarModuleRegistryResolveError',
      kind: 'trust_posture_refused',
    })
  })

  test('rejects unsafe injected registry projection material', async () => {
    const dense = seedBlueprintTassadarModuleRegistryEntries()[0]!

    await expect(
      Effect.runPromise(
        listBlueprintTassadarModuleRegistry({
          entries: [
            {
              ...dense,
              artifactRefs: ['raw_prompt.secret'],
            },
          ],
        }),
      ),
    ).rejects.toMatchObject({
      _tag: 'BlueprintTassadarModuleRegistryResolveError',
      kind: 'unsafe_projection',
    })
  })
})
