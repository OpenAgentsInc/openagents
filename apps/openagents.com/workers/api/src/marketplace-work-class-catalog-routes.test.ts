import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  MARKETPLACE_DATA_LABELING_WORK_CLASS,
  MARKETPLACE_LIVE_WORK_CLASS,
  MARKETPLACE_WORK_CLASS_CATALOG_PROMISE,
  MARKETPLACE_WORK_CLASS_CATALOG_SCHEMA,
} from './marketplace-work-class-catalog'
import {
  MarketplaceWorkClassCatalogEndpoint,
  handleMarketplaceWorkClassCatalogApi,
} from './marketplace-work-class-catalog-routes'

const request = (suffix = '', method = 'GET') =>
  new Request(
    `https://openagents.com${MarketplaceWorkClassCatalogEndpoint}${suffix}`,
    { method },
  )

describe('marketplace work-class catalog route', () => {
  test('rejects non-GET methods', async () => {
    const response = await Effect.runPromise(
      handleMarketplaceWorkClassCatalogApi(request('', 'POST')),
    )
    expect(response.status).toBe(405)
  })

  test('lists the catalog honestly: yellow with a live non-code work class', async () => {
    const response = await Effect.runPromise(
      handleMarketplaceWorkClassCatalogApi(request()),
    )
    const body = (await response.json()) as {
      schema: string
      promiseIds: ReadonlyArray<string>
      promiseState: string
      inert: boolean
      liveWorkClass: string
      liveWorkClasses: ReadonlyArray<string>
      pluginMarketplaceBeyondCodeTaskLive: boolean
      unclearedBlockerRefs: ReadonlyArray<string>
      workClasses: ReadonlyArray<{ workClass: string; status: string }>
    }
    expect(body.schema).toBe(MARKETPLACE_WORK_CLASS_CATALOG_SCHEMA)
    expect(body.promiseIds).toEqual([MARKETPLACE_WORK_CLASS_CATALOG_PROMISE])
    expect(body.promiseState).toBe('yellow')
    expect(body.inert).toBe(false)
    expect(body.liveWorkClass).toBe(MARKETPLACE_LIVE_WORK_CLASS)
    expect(body.liveWorkClasses).toContain(MARKETPLACE_DATA_LABELING_WORK_CLASS)
    expect(body.pluginMarketplaceBeyondCodeTaskLive).toBe(true)
    expect(body.unclearedBlockerRefs).toEqual([])

    const live = body.workClasses.filter(entry => entry.status === 'live')
    expect(live.map(entry => entry.workClass)).toEqual([
      MARKETPLACE_LIVE_WORK_CLASS,
      MARKETPLACE_DATA_LABELING_WORK_CLASS,
    ])
  })

  test('?workClass= narrows to a single known class', async () => {
    const response = await Effect.runPromise(
      handleMarketplaceWorkClassCatalogApi(request('?workClass=data_labeling')),
    )
    const body = (await response.json()) as {
      liveWorkClasses: ReadonlyArray<string>
      workClass: { workClass: string; status: string } | null
      pluginMarketplaceBeyondCodeTaskLive: boolean
    }
    expect(body.workClass?.workClass).toBe('data_labeling')
    expect(body.workClass?.status).toBe('live')
    expect(body.liveWorkClasses).toContain(MARKETPLACE_DATA_LABELING_WORK_CLASS)
    expect(body.pluginMarketplaceBeyondCodeTaskLive).toBe(true)
  })

  test('?workClass= returns null workClass for an unknown id', async () => {
    const response = await Effect.runPromise(
      handleMarketplaceWorkClassCatalogApi(request('?workClass=nope')),
    )
    const body = (await response.json()) as {
      workClass: unknown | null
    }
    expect(body.workClass).toBeNull()
  })
})
