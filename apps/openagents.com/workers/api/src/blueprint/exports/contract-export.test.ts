import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  BLUEPRINT_CONTRACT_CONSUMERS,
  BLUEPRINT_CONTRACT_EXPORT_SEED,
  BlueprintContractExportSeed as BlueprintContractExportSeedSchema,
  blueprintContractExportSeedCoversConsumers,
  blueprintContractExportSeedHasCatalogs,
  blueprintContractExportSeedIsPrivateDataSafe,
} from './contract-export'

describe('Blueprint contract export seed', () => {
  test('decodes the seed fixture', () => {
    expect(
      S.decodeUnknownSync(BlueprintContractExportSeedSchema)(
        BLUEPRINT_CONTRACT_EXPORT_SEED,
      ),
    ).toEqual(BLUEPRINT_CONTRACT_EXPORT_SEED)
  })

  test('covers agents and Rust-side consumers', () => {
    expect(BLUEPRINT_CONTRACT_CONSUMERS).toEqual([
      'ai_agent',
      'nexus',
      'oa_node',
      'oa_workroomd',
      'probe',
      'psionic',
      'pylon',
      'treasury',
    ])
    expect(
      blueprintContractExportSeedCoversConsumers(
        BLUEPRINT_CONTRACT_EXPORT_SEED,
      ),
    ).toBe(true)
  })

  test('includes JSON Schema, OpenAPI, event, and receipt catalogs', () => {
    expect(
      blueprintContractExportSeedHasCatalogs(BLUEPRINT_CONTRACT_EXPORT_SEED),
    ).toBe(true)
    expect(BLUEPRINT_CONTRACT_EXPORT_SEED.jsonSchemas.length).toBeGreaterThan(
      10,
    )
    expect(
      BLUEPRINT_CONTRACT_EXPORT_SEED.openApi.map(item => item.path),
    ).toEqual([
      '/api/blueprint/program-registry',
      '/api/blueprint/contracts',
      '/api/blueprint/tassadar-modules',
      '/api/blueprint/program-runs',
      '/api/blueprint/action-submissions',
      '/api/blueprint/action-submissions',
      '/api/blueprint/contributions',
      '/api/blueprint/contributions',
    ])
    expect(
      BLUEPRINT_CONTRACT_EXPORT_SEED.eventCatalog.map(item => item.eventRef),
    ).toContain('event.blueprint.probe.failed.v1')
    expect(
      BLUEPRINT_CONTRACT_EXPORT_SEED.receiptCatalog.map(
        item => item.receiptRef,
      ),
    ).toContain('receipt.probe_failure')
  })

  test('does not include secret-shaped or private payload material', () => {
    expect(
      blueprintContractExportSeedIsPrivateDataSafe(
        BLUEPRINT_CONTRACT_EXPORT_SEED,
      ),
    ).toBe(true)
  })
})
