import { describe, expect, test } from 'vitest'

import {
  type MarketplaceWorkClassDefinition,
  MARKETPLACE_LIVE_WORK_CLASS,
  MARKETPLACE_PLUGIN_BEYOND_CODE_TASK_BLOCKER_REF,
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
  test('code_task is the only live class; plugin classes are inert scaffolds', () => {
    const live = liveMarketplaceWorkClasses()
    expect(live.map(entry => entry.workClass)).toEqual([
      MARKETPLACE_LIVE_WORK_CLASS,
    ])
    expect(isMarketplaceWorkClassLive(MARKETPLACE_LIVE_WORK_CLASS)).toBe(true)

    const inert = inertMarketplaceWorkClasses()
    expect(inert.length).toBeGreaterThan(0)
    for (const entry of inert) {
      expect(entry.status).toBe('inert_scaffold')
      expect(entry.workClass).not.toBe(MARKETPLACE_LIVE_WORK_CLASS)
      expect(isMarketplaceWorkClassLive(entry.workClass)).toBe(false)
    }
  })

  test('plugin marketplace beyond code_task is NOT live today', () => {
    expect(isPluginMarketplaceBeyondCodeTaskLive()).toBe(false)
  })

  test('getMarketplaceWorkClass resolves known and unknown ids', () => {
    expect(getMarketplaceWorkClass('data_labeling')?.status).toBe(
      'inert_scaffold',
    )
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

  test('assertCatalogInvariants rejects a live class beyond code_task', () => {
    const overclaiming: ReadonlyArray<MarketplaceWorkClassDefinition> = [
      ...MARKETPLACE_WORK_CLASS_CATALOG,
      {
        workClass: 'data_labeling_live',
        title: 'Data labeling (overclaim)',
        requiredCapabilityRefs: ['capability.market.data_labeling'],
        verificationCommandRef: 'command.public.market.data_labeling.audit',
        settlementStream: 'data',
        status: 'live',
      },
    ]
    expect(() => assertCatalogInvariants(overclaiming)).toThrow(
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

  test('projection is honest: yellow, inert, plugin blocker uncleared', () => {
    const projection = projectMarketplaceWorkClassCatalog()
    expect(projection.schema).toBe(MARKETPLACE_WORK_CLASS_CATALOG_SCHEMA)
    expect(projection.promiseIds).toEqual([
      MARKETPLACE_WORK_CLASS_CATALOG_PROMISE,
    ])
    expect(projection.promiseState).toBe('yellow')
    expect(projection.inert).toBe(true)
    expect(projection.liveWorkClass).toBe(MARKETPLACE_LIVE_WORK_CLASS)
    expect(projection.pluginMarketplaceBeyondCodeTaskLive).toBe(false)
    expect(projection.unclearedBlockerRefs).toContain(
      MARKETPLACE_PLUGIN_BEYOND_CODE_TASK_BLOCKER_REF,
    )
    expect(projection.maxStalenessSeconds).toBe(0)
  })
})
