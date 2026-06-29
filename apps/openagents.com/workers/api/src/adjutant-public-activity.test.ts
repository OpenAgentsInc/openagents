import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { handlePublicAdjutantActivityApi } from './public-adjutant-activity-routes'

type StoredActivityRow = Readonly<{
  active_deployment_status: string | null
  active_deployment_updated_at: string | null
  active_deployment_url: string | null
  assignment_agent_id: string
  assignment_archived_at: string | null
  assignment_id: string
  assignment_kind: string
  assignment_status: string
  assignment_updated_at: string
  assignment_visibility: string
  order_visibility: string | null
  order_status: string | null
  site_access_mode: string | null
  site_slug: string | null
  site_status: string | null
  site_title: string | null
  site_visibility: string | null
  software_order_id: string | null
}>

class PublicAdjutantActivityStore {
  rows: Array<StoredActivityRow> = []
}

const d1Meta = (): D1Meta & Record<string, unknown> => ({
  changed_db: false,
  changes: 0,
  duration: 0,
  last_row_id: 0,
  rows_read: 0,
  rows_written: 0,
  size_after: 0,
})

class PublicAdjutantActivityStatement implements D1PreparedStatement {
  constructor(private readonly store: PublicAdjutantActivityStore) {}

  bind(): D1PreparedStatement {
    return this
  }

  first<T = unknown>(colName: string): Promise<T | null>
  first<T = Record<string, unknown>>(): Promise<T | null>
  first<T = unknown>(): Promise<T | null> {
    return Promise.reject(new Error('D1 first should not be used'))
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    return Promise.reject(new Error('D1 run should not be used'))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    const rows = this.store.rows
      .filter(
        row =>
          row.assignment_agent_id === 'agent_adjutant' &&
          row.assignment_visibility === 'public' &&
          row.assignment_archived_at === null &&
          (row.software_order_id === null ||
            row.order_visibility === 'public') &&
          (row.site_slug === null ||
            (row.site_visibility === 'public' &&
              row.site_access_mode === 'public')),
      )
      .sort((left, right) =>
        (
          right.active_deployment_updated_at ?? right.assignment_updated_at
        ).localeCompare(
          left.active_deployment_updated_at ?? left.assignment_updated_at,
        ),
      )
      .slice(0, 20)

    return Promise.resolve({
      meta: d1Meta(),
      results: rows as Array<T>,
      success: true,
    })
  }

  raw<T = unknown[]>(options: {
    columnNames: true
  }): Promise<[string[], ...T[]]>
  raw<T = unknown[]>(options?: { columnNames?: false }): Promise<T[]>
  raw<T = unknown[]>(): Promise<[string[], ...T[]] | T[]> {
    return Promise.reject(new Error('D1 raw should not be used'))
  }
}

const publicAdjutantActivityDb = (
  store: PublicAdjutantActivityStore,
): D1Database => ({
  batch: () => Promise.reject(new Error('D1 batch should not be used')),
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: () => new PublicAdjutantActivityStatement(store),
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
})

const row = (
  input: Partial<StoredActivityRow> = {},
): StoredActivityRow => ({
  active_deployment_status: 'active',
  active_deployment_updated_at: '2026-06-05T00:06:00.000Z',
  active_deployment_url: 'https://sites.openagents.com/otec',
  assignment_agent_id: 'agent_adjutant',
  assignment_archived_at: null,
  assignment_id: 'adjutant_assignment_public_1',
  assignment_kind: 'site_generation',
  assignment_status: 'deployed',
  assignment_updated_at: '2026-06-05T00:05:00.000Z',
  assignment_visibility: 'public',
  order_status: 'delivered',
  order_visibility: 'public',
  site_access_mode: 'public',
  site_slug: 'otec',
  site_status: 'approved',
  site_title: 'OTEC Site',
  site_visibility: 'public',
  software_order_id: 'software_order_public_1',
  ...input,
})

const runRoute = (store: PublicAdjutantActivityStore): Promise<Response> =>
  Effect.runPromise(
    handlePublicAdjutantActivityApi(
      new Request('https://openagents.com/api/public/adjutant/activity'),
      { OPENAGENTS_DB: publicAdjutantActivityDb(store) },
    ),
  )

describe('public Adjutant activity API', () => {
  test('returns public milestones and excludes private or team-only rows', async () => {
    const store = new PublicAdjutantActivityStore()
    store.rows = [
      row(),
      row({
        assignment_id: 'adjutant_assignment_private',
        assignment_visibility: 'private',
        site_slug: 'private-site',
      }),
      row({
        assignment_id: 'adjutant_assignment_team_site',
        site_access_mode: 'openagents_core',
        site_slug: 'team-site',
        site_visibility: 'team',
      }),
    ]

    const response = await runRoute(store)
    const body = (await response.json()) as {
      deployedSites: ReadonlyArray<Record<string, unknown>>
      milestones: ReadonlyArray<Record<string, unknown>>
    }
    const serialized = JSON.stringify(body)

    expect(response.status).toBe(200)
    expect(body.milestones).toEqual([
      expect.objectContaining({
        label: 'Public Site deployed',
        publicRef: 'site:otec',
        siteUrl: 'https://sites.openagents.com/otec',
        stage: 'deployed',
        summary: 'OTEC Site is live.',
      }),
    ])
    expect(body.deployedSites).toEqual([
      expect.objectContaining({
        publicRef: 'site:otec',
        slug: 'otec',
        url: 'https://sites.openagents.com/otec',
      }),
    ])
    expect(serialized).not.toContain('private-site')
    expect(serialized).not.toContain('team-site')
    expect(serialized).not.toContain('request')
    expect(serialized).not.toContain('prompt')
    expect(serialized).not.toContain('payload')
  })

  test('fails closed when projected public text contains provider secret material', async () => {
    const store = new PublicAdjutantActivityStore()
    store.rows = [
      row({
        site_title: 'Bearer gho_abcdefghijklmnopqrstuvwxyz',
      }),
    ]

    const response = await runRoute(store)

    expect(response.status).toBe(500)
    await expect(response.json()).resolves.toEqual({
      error: 'public_adjutant_activity_unsafe',
    })
  })
})
