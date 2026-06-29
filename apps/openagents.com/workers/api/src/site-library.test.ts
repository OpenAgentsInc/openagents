import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  SiteLibraryForbidden,
  SiteLibraryValidationError,
  archiveSiteLibrarySite,
  deleteSiteLibrarySite,
  listSiteLibrary,
  siteIsVisibleForBuilderSession,
  updateSiteLibraryAccess,
} from './site-library'

type StoredSite = Readonly<{
  access_mode: 'owner_admins' | 'public'
  active_deployment_id: string | null
  active_version_id: string | null
  archived_at: string | null
  created_at: string
  id: string
  owner_user_id: string
  slug: string
  software_order_id: string | null
  status:
    | 'draft'
    | 'generating'
    | 'generated'
    | 'needs_review'
    | 'approved'
    | 'archived'
    | 'disabled'
  title: string
  updated_at: string
  visibility: 'private' | 'public' | 'team'
}>

type StoredDeployment = Readonly<{
  disabled_at: string | null
  id: string
  site_id: string
  status: string
  updated_at: string
  url: string
  version_id: string
}>

type StoredBuilderSession = Readonly<{
  archived_at: string | null
  id: string
  site_id: string | null
  status: string
  updated_at: string
}>

type StoredEvent = Readonly<{
  actor_user_id: string | null
  id: string
  payload_json: string | null
  site_id: string
  summary: string
  type: string
}>

class SiteLibraryStore {
  builderSessions: Array<StoredBuilderSession> = [
    {
      archived_at: null,
      id: 'site_builder_session_1',
      site_id: 'site_project_public',
      status: 'deployed',
      updated_at: '2026-06-05T18:00:00.000Z',
    },
  ]
  deployments: Array<StoredDeployment> = [
    {
      disabled_at: null,
      id: 'site_deployment_public',
      site_id: 'site_project_public',
      status: 'active',
      updated_at: '2026-06-05T18:00:00.000Z',
      url: 'https://sites.openagents.com/otec',
      version_id: 'site_version_public',
    },
  ]
  events: Array<StoredEvent> = []
  sites: Array<StoredSite> = [
    {
      access_mode: 'public',
      active_deployment_id: 'site_deployment_public',
      active_version_id: 'site_version_public',
      archived_at: null,
      created_at: '2026-06-05T18:00:00.000Z',
      id: 'site_project_public',
      owner_user_id: 'user_owner',
      slug: 'otec',
      software_order_id: 'software_order_otec',
      status: 'approved',
      title: 'OTEC',
      updated_at: '2026-06-05T18:00:00.000Z',
      visibility: 'public',
    },
    {
      access_mode: 'owner_admins',
      active_deployment_id: null,
      active_version_id: null,
      archived_at: null,
      created_at: '2026-06-05T17:00:00.000Z',
      id: 'site_project_private',
      owner_user_id: 'user_other',
      slug: 'private',
      software_order_id: null,
      status: 'draft',
      title: 'Private',
      updated_at: '2026-06-05T17:00:00.000Z',
      visibility: 'private',
    },
    {
      access_mode: 'public',
      active_deployment_id: null,
      active_version_id: null,
      archived_at: '2026-06-05T19:00:00.000Z',
      created_at: '2026-06-05T16:00:00.000Z',
      id: 'site_project_archived',
      owner_user_id: 'user_owner',
      slug: 'old',
      software_order_id: null,
      status: 'archived',
      title: 'Old',
      updated_at: '2026-06-05T19:00:00.000Z',
      visibility: 'public',
    },
  ]
}

const runtime = {
  makeEventId: () => 'site_event_test',
  nowIso: () => '2026-06-05T20:00:00.000Z',
}

class SiteLibraryStatement {
  values: ReadonlyArray<unknown> = []

  constructor(
    readonly query: string,
    private readonly store: SiteLibraryStore,
  ) {}

  bind(...values: ReadonlyArray<unknown>): SiteLibraryStatement {
    this.values = values

    return this
  }

  first<T = unknown>(colName: string): Promise<T | null>
  first<T = Record<string, unknown>>(): Promise<T | null>
  first<T = unknown>(): Promise<T | null> {
    if (this.query.includes('FROM site_projects')) {
      const siteId = String(this.values[0])
      const site = this.store.sites.find(
        item =>
          item.id === siteId &&
          item.archived_at === null &&
          item.status !== 'disabled',
      )

      return Promise.resolve(
        site === undefined ? null : (this.projectRow(site) as T),
      )
    }

    return Promise.resolve(null)
  }

  all<T = unknown>(): Promise<D1Result<T>> {
    if (this.query.includes('FROM site_projects')) {
      const rows = this.rowsForList().map(site => this.projectRow(site) as T)

      return Promise.resolve({ results: rows } as D1Result<T>)
    }

    return Promise.resolve({ results: [] } as unknown as D1Result<T>)
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('UPDATE site_projects')) {
      return this.updateSite() as Promise<D1Result<T>>
    }

    if (this.query.includes('UPDATE site_deployments')) {
      return this.updateDeployments() as Promise<D1Result<T>>
    }

    if (this.query.includes('UPDATE site_builder_sessions')) {
      return this.updateBuilderSessions() as Promise<D1Result<T>>
    }

    if (this.query.includes('INSERT INTO site_events')) {
      this.store.events.push({
        actor_user_id: String(this.values[4]),
        id: String(this.values[0]),
        payload_json:
          this.values[5] === null ? null : String(this.values[5]),
        site_id: String(this.values[1]),
        summary: String(this.values[3]),
        type: String(this.values[2]),
      })
    }

    return Promise.resolve({ success: true } as unknown as D1Result<T>)
  }

  private rowsForList(): ReadonlyArray<StoredSite> {
    const active = this.store.sites.filter(
      site => site.archived_at === null && site.status !== 'disabled',
    )

    if (this.query.includes("access_mode = 'public'")) {
      return active.filter(
        site => site.access_mode === 'public' && site.visibility === 'public',
      )
    }

    if (this.query.includes('owner_user_id = ?')) {
      const ownerUserId = String(this.values[0])

      return active.filter(site => site.owner_user_id === ownerUserId)
    }

    return active
  }

  private projectRow(site: StoredSite): Record<string, unknown> {
    const deployment =
      site.active_deployment_id === null
        ? undefined
        : this.store.deployments.find(item => item.id === site.active_deployment_id)

    return {
      ...site,
      active_deployment_status: deployment?.status ?? null,
      active_url: deployment?.url ?? null,
      deployment_count: this.store.deployments.filter(
        item => item.site_id === site.id,
      ).length,
      version_count: site.active_version_id === null ? 0 : 1,
    }
  }

  private updateSite(): Promise<D1Result> {
    const isAccessUpdate = this.query.includes('SET access_mode = ?')
    const siteId = String(this.values[isAccessUpdate ? 3 : 2])
    this.store.sites = this.store.sites.map(site => {
      if (site.id !== siteId || site.archived_at !== null) {
        return site
      }

      if (isAccessUpdate) {
        return {
          ...site,
          access_mode: this.values[0] as 'owner_admins' | 'public',
          updated_at: String(this.values[2]),
          visibility: this.values[1] as 'private' | 'public' | 'team',
        }
      }

      return {
        ...site,
        access_mode: 'owner_admins',
        active_deployment_id: null,
        active_version_id: null,
        archived_at: String(this.values[1]),
        status: this.query.includes("status = 'disabled'")
          ? 'disabled'
          : 'archived',
        updated_at: String(this.values[0]),
        visibility: 'private',
      }
    })

    return Promise.resolve({ success: true } as D1Result)
  }

  private updateDeployments(): Promise<D1Result> {
    const siteId = String(this.values[2])
    this.store.deployments = this.store.deployments.map(deployment =>
      deployment.site_id === siteId && deployment.status === 'active'
        ? {
            ...deployment,
            disabled_at: String(this.values[0]),
            status: 'disabled',
            updated_at: String(this.values[1]),
          }
        : deployment,
    )

    return Promise.resolve({ success: true } as D1Result)
  }

  private updateBuilderSessions(): Promise<D1Result> {
    const siteId = String(this.values[2])
    this.store.builderSessions = this.store.builderSessions.map(session =>
      session.site_id === siteId && session.archived_at === null
        ? {
            ...session,
            archived_at: String(this.values[0]),
            status: 'archived',
            updated_at: String(this.values[1]),
          }
        : session,
    )

    return Promise.resolve({ success: true } as D1Result)
  }
}

const siteLibraryDb = (store: SiteLibraryStore): D1Database =>
  ({
    batch: (statements: Array<D1PreparedStatement>) =>
      Promise.all(
        statements.map((statement: D1PreparedStatement) =>
          (statement as unknown as SiteLibraryStatement).run(),
        ),
      ),
    dump: () => Promise.resolve(new ArrayBuffer(0)),
    exec: () => Promise.resolve({ count: 0, duration: 0 }),
    prepare: (query: string) =>
      new SiteLibraryStatement(query, store) as unknown as D1PreparedStatement,
    withSession: () => siteLibraryDb(store),
  }) as unknown as D1Database

describe('Site library', () => {
  test('lists mine and public without archived or disabled Sites', async () => {
    const store = new SiteLibraryStore()
    const db = siteLibraryDb(store)
    const mine = await Effect.runPromise(
      listSiteLibrary(db, {
        actorUserId: 'user_owner',
        isAdmin: false,
        scope: 'mine',
      }),
    )
    const publicSites = await Effect.runPromise(
      listSiteLibrary(db, {
        actorUserId: 'user_other',
        isAdmin: false,
        scope: 'public',
      }),
    )

    expect(mine.sites.map(site => site.id)).toEqual(['site_project_public'])
    expect(publicSites.sites.map(site => site.id)).toEqual([
      'site_project_public',
    ])
    expect(publicSites.sites[0]?.canManage).toBe(false)
  })

  test('enforces owner and admin authority for visibility updates', async () => {
    const store = new SiteLibraryStore()
    const db = siteLibraryDb(store)
    await expect(
      Effect.runPromise(
        updateSiteLibraryAccess(db, runtime, {
          accessMode: 'owner_admins',
          actorUserId: 'user_stranger',
          isAdmin: false,
          siteId: 'site_project_public',
          visibility: 'private',
        }),
      ),
    ).rejects.toBeInstanceOf(SiteLibraryForbidden)

    const updated = await Effect.runPromise(
      updateSiteLibraryAccess(db, runtime, {
        accessMode: 'owner_admins',
        actorUserId: 'user_owner',
        isAdmin: false,
        siteId: 'site_project_public',
        visibility: 'private',
      }),
    )

    expect(updated.visibility).toBe('private')
    await expect(
      Effect.runPromise(
        updateSiteLibraryAccess(db, runtime, {
          accessMode: 'public',
          actorUserId: 'user_owner',
          isAdmin: false,
          siteId: 'site_project_public',
          visibility: 'public',
        }),
      ),
    ).rejects.toBeInstanceOf(SiteLibraryValidationError)
  })

  test('archives Sites and hides stale builder sessions', async () => {
    const store = new SiteLibraryStore()
    const db = siteLibraryDb(store)
    const archived = await Effect.runPromise(
      archiveSiteLibrarySite(db, runtime, {
        actorUserId: 'user_owner',
        idempotencyKey: 'archive-site-project-public',
        isAdmin: false,
        siteId: 'site_project_public',
      }),
    )
    const publicSites = await Effect.runPromise(
      listSiteLibrary(db, {
        actorUserId: 'user_owner',
        isAdmin: false,
        scope: 'public',
      }),
    )
    const visibleForBuilder = await Effect.runPromise(
      siteIsVisibleForBuilderSession(db, 'site_project_public'),
    )

    expect(archived.status).toBe('archived')
    expect(archived.activeUrl).toBeNull()
    expect(publicSites.sites).toEqual([])
    expect(store.deployments[0]?.status).toBe('disabled')
    expect(store.builderSessions[0]?.status).toBe('archived')
    expect(visibleForBuilder).toBe(false)
  })

  test('lets admins delete another owner Site as a disabling soft delete', async () => {
    const store = new SiteLibraryStore()
    const db = siteLibraryDb(store)
    const deleted = await Effect.runPromise(
      deleteSiteLibrarySite(db, runtime, {
        actorUserId: 'user_admin',
        idempotencyKey: 'delete-site-project-private',
        isAdmin: true,
        siteId: 'site_project_private',
      }),
    )

    expect(deleted.status).toBe('disabled')
    expect(deleted.visibility).toBe('private')
    expect(store.sites.find(site => site.id === 'site_project_private')?.status)
      .toBe('disabled')
  })
})
