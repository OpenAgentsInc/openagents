import { describe, expect, test } from 'vitest'

import {
  MARKETPLACE_DATA_LABELING_WORK_CLASS,
  type MarketplaceWorkClassDefinition,
  MARKETPLACE_LIVE_WORK_CLASS,
  MARKETPLACE_WORK_CLASS_CATALOG,
  MARKETPLACE_WORK_CLASS_CATALOG_PROMISE,
  MARKETPLACE_WORK_CLASS_CATALOG_SCHEMA,
  MarketplaceWorkClassCatalogError,
  assertCatalogInvariants,
  getMarketplaceWorkClass,
  inertMarketplaceWorkClasses,
  isMarketplaceWorkClassLive,
  isPluginMarketplaceBeyondCodeTaskLive,
  liveMarketplaceWorkClasses,
  projectMarketplaceWorkClassCatalog,
} from './marketplace-work-class-catalog'

describe('marketplace work-class catalog', () => {
  test('code_task and data_labeling are live; remaining plugin classes are inert scaffolds', () => {
    const live = liveMarketplaceWorkClasses()
    expect(live.map(entry => entry.workClass)).toEqual([
      MARKETPLACE_LIVE_WORK_CLASS,
      MARKETPLACE_DATA_LABELING_WORK_CLASS,
    ])
    expect(isMarketplaceWorkClassLive(MARKETPLACE_LIVE_WORK_CLASS)).toBe(true)
    expect(isMarketplaceWorkClassLive(MARKETPLACE_DATA_LABELING_WORK_CLASS)).toBe(true)

    const inert = inertMarketplaceWorkClasses()
    expect(inert.length).toBeGreaterThan(0)
    for (const entry of inert) {
      expect(entry.status).toBe('inert_scaffold')
      expect(entry.workClass).not.toBe(MARKETPLACE_LIVE_WORK_CLASS)
      expect(isMarketplaceWorkClassLive(entry.workClass)).toBe(false)
    }
  })

  test('plugin marketplace beyond code_task is live at the planner contract level', () => {
    expect(isPluginMarketplaceBeyondCodeTaskLive()).toBe(true)
  })

  test('getMarketplaceWorkClass resolves known and unknown ids', () => {
    expect(getMarketplaceWorkClass('data_labeling')?.status).toBe('live')
    expect(getMarketplaceWorkClass('does_not_exist')).toBeNull()
  })

  test('every entry carries a non-empty contract', () => {
    for (const entry of MARKETPLACE_WORK_CLASS_CATALOG) {
      expect(entry.title.trim().length).toBeGreaterThan(0)
      expect(entry.requiredCapabilityRefs.length).toBeGreaterThan(0)
      expect(entry.verificationCommandRef.trim().length).toBeGreaterThan(0)
    }
  })

  test('assertCatalogInvariants accepts the shipped catalog', () => {
    expect(() => assertCatalogInvariants()).not.toThrow()
  })

  test('assertCatalogInvariants rejects a catalog with no live class beyond code_task', () => {
    const codeOnly: ReadonlyArray<MarketplaceWorkClassDefinition> =
      MARKETPLACE_WORK_CLASS_CATALOG.map(entry =>
        entry.workClass === MARKETPLACE_DATA_LABELING_WORK_CLASS
          ? { ...entry, status: 'inert_scaffold' as const }
          : entry,
      )
    expect(() => assertCatalogInvariants(codeOnly)).toThrow(
      MarketplaceWorkClassCatalogError,
    )
  })

  test('assertCatalogInvariants rejects duplicate ids', () => {
    const dupes: ReadonlyArray<MarketplaceWorkClassDefinition> = [
      ...MARKETPLACE_WORK_CLASS_CATALOG,
      ...MARKETPLACE_WORK_CLASS_CATALOG.slice(0, 1),
    ]
    expect(() => assertCatalogInvariants(dupes)).toThrow(
      MarketplaceWorkClassCatalogError,
    )
  })

  test('assertCatalogInvariants rejects a catalog missing live code_task', () => {
    const noLive: ReadonlyArray<MarketplaceWorkClassDefinition> =
      inertMarketplaceWorkClasses()
    expect(() => assertCatalogInvariants(noLive)).toThrow(
      MarketplaceWorkClassCatalogError,
    )
  })

  test('projection is honest: yellow with a live non-code work class', () => {
    const projection = projectMarketplaceWorkClassCatalog()
    expect(projection.schema).toBe(MARKETPLACE_WORK_CLASS_CATALOG_SCHEMA)
    expect(projection.promiseIds).toEqual([
      MARKETPLACE_WORK_CLASS_CATALOG_PROMISE,
    ])
    expect(projection.promiseState).toBe('yellow')
    expect(projection.inert).toBe(false)
    expect(projection.liveWorkClass).toBe(MARKETPLACE_LIVE_WORK_CLASS)
    expect(projection.liveWorkClasses).toContain(MARKETPLACE_DATA_LABELING_WORK_CLASS)
    expect(projection.pluginMarketplaceBeyondCodeTaskLive).toBe(true)
    expect(projection.unclearedBlockerRefs).toEqual([])
    expect(projection.maxStalenessSeconds).toBe(0)
  })
})
