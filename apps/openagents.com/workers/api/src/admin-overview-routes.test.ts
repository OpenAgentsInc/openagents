import type { IdentityDb } from './identity-db'
import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { makeAdminOverviewHandlers } from './admin-overview-routes'

type TestSession = Readonly<{ user: Readonly<{ email: string }> }>

const executionContext = (): ExecutionContext => ({
  passThroughOnException: () => undefined,
  props: undefined,
  waitUntil: () => undefined,
})

class AdminOverviewStatement implements D1PreparedStatement {
  constructor(
    private readonly query: string,
    private readonly store: AdminOverviewDbStore,
  ) {}

  bind(): D1PreparedStatement {
    return this
  }

  first<T = unknown>(colName: string): Promise<T | null>
  first<T = Record<string, unknown>>(): Promise<T | null>
  first<T = unknown>(): Promise<T | null> {
    return Promise.reject(new Error(`Unexpected D1 first: ${this.query}`))
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    return Promise.reject(new Error(`Unexpected D1 run: ${this.query}`))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('FROM users')) {
      return Promise.resolve({
        results: this.store.users as unknown as ReadonlyArray<T>,
        success: true,
      } as D1Result<T>)
    }

    if (this.query.includes('software_order_count')) {
      // CFG-4 Domain 2 (#8519): the per-user order counts are a separate
      // D1 aggregate now that the user listing reads from Postgres.
      return Promise.resolve({
        results: this.store.users.map(user => ({
          software_order_count: user.software_order_count,
          user_id: user.user_id,
        })) as unknown as ReadonlyArray<T>,
        success: true,
      } as D1Result<T>)
    }

    if (this.query.includes('FROM software_orders')) {
      return Promise.resolve({
        results: this.store.softwareOrders.map(
          ({ user_display_name, user_email, ...row }) => row,
        ) as unknown as ReadonlyArray<T>,
        success: true,
      } as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected D1 all: ${this.query}`))
  }

  raw<T = unknown[]>(options: {
    columnNames: true
  }): Promise<[string[], ...T[]]>
  raw<T = unknown[]>(options?: { columnNames?: false }): Promise<T[]>
  raw<T = unknown[]>(): Promise<[string[], ...T[]] | T[]> {
    return Promise.reject(new Error(`Unexpected D1 raw: ${this.query}`))
  }
}

class AdminOverviewDbStore {
  queries: Array<string> = []

  users = [
    {
      user_id: 'github:1',
      kind: 'human',
      display_name: 'Chris',
      primary_email: 'chris@openagents.com',
      github_username: 'chris',
      status: 'active',
      onboarding_step: 'complete',
      onboarding_completed_at: '2026-06-04T12:00:00.000Z',
      software_order_count: 1,
      created_at: '2026-06-04T11:00:00.000Z',
      updated_at: '2026-06-04T12:00:00.000Z',
    },
  ]

  softwareOrders = [
    {
      id: 'software_order_1',
      user_id: 'github:1',
      user_display_name: 'Chris',
      user_email: 'chris@openagents.com',
      status: 'submitted',
      visibility: 'public',
      request: 'Add an admin panel.',
      repository_full_name: 'OpenAgentsInc/autopilot-omega',
      current_run_id: null,
      site_project_id: 'site_project_otec',
      site_title: 'OTEC Site',
      site_slug: 'otec',
      site_status: 'draft',
      site_access_mode: 'public',
      site_visibility: 'public',
      site_active_version_id: 'site_version_1',
      site_active_deployment_id: 'site_deployment_1',
      site_active_url: 'https://sites.openagents.com/otec',
      site_version_count: 2,
      site_latest_version_id: 'site_version_2',
      site_latest_version_status: 'saved',
      site_latest_version_source_kind: 'autopilot_generated',
      site_latest_version_created_at: '2026-06-04T12:04:00.000Z',
      site_deployment_count: 1,
      site_latest_deployment_id: 'site_deployment_1',
      site_latest_deployment_status: 'active',
      site_latest_deployment_runtime_kind: 'omega_static_r2',
      site_latest_deployment_updated_at: '2026-06-04T12:05:00.000Z',
      site_storage_binding_count: 2,
      site_storage_binding_summary: 'd1:SITE_DB, r2:SITE_ASSETS',
      site_environment_value_count: 1,
      site_environment_key_summary: 'OPENAI_API_KEY:secret',
      site_access_grant_count: 1,
      site_latest_event_type: 'site_version.saved',
      site_latest_event_summary: 'Saved Site version site_version_2.',
      site_latest_event_created_at: '2026-06-04T12:04:00.000Z',
      site_latest_compatibility_id: 'site_compatibility_check_1',
      site_latest_compatibility_status: 'ready',
      site_latest_compatibility_customer_safe_status:
        'The Site is compatible with static hosting.',
      site_latest_compatibility_customer_safe_next_action:
        'Run build validation.',
      site_latest_compatibility_blockers_json: '[]',
      site_latest_compatibility_warnings_json: '[{"code":"manual_review"}]',
      site_latest_compatibility_created_at: '2026-06-04T12:03:00.000Z',
      site_latest_build_validation_id: 'site_build_validation_1',
      site_latest_build_validation_status: 'passed',
      site_latest_build_validation_source_hash: 'sha256:site',
      site_latest_build_validation_customer_safe_status:
        'The latest build passed.',
      site_latest_build_validation_customer_safe_next_action:
        'Review and deploy the saved version.',
      site_latest_build_validation_blockers_json: '[]',
      site_latest_build_validation_warnings_json: '[]',
      site_latest_build_validation_created_at: '2026-06-04T12:04:30.000Z',
      created_at: '2026-06-04T12:01:00.000Z',
      updated_at: '2026-06-04T12:01:00.000Z',
      archived_at: null,
    },
  ]
}

const adminOverviewDb = (store: AdminOverviewDbStore): D1Database => ({
  batch: () => Promise.reject(new Error('D1 batch should not be used')),
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => {
    store.queries.push(query)

    return new AdminOverviewStatement(query, store)
  },
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
})

// CFG-4 Domain 2 (#8519): the user listing + order-user enrichment read
// from the Postgres identity handle — served here from the same store.
const overviewIdentityDb = (store: AdminOverviewDbStore): IdentityDb => ({
  batch: () => Promise.resolve(),
  query: (sql, params = []) => {
    if (!sql.includes('FROM users')) {
      return Promise.reject(
        new Error(`unexpected identityDb query: ${sql.slice(0, 80)}`),
      )
    }
    if (sql.includes('IN (')) {
      const ids = new Set(params.map(String))
      return Promise.resolve(
        store.users
          .filter(user => ids.has(user.user_id))
          .map(user => ({
            avatar_url: null,
            created_at: user.created_at,
            deleted_at: null,
            display_name: user.display_name,
            github_id: null,
            github_username: user.github_username,
            id: user.user_id,
            kind: user.kind,
            primary_email: user.primary_email,
            status: user.status,
          })),
      )
    }
    return Promise.resolve(
      store.users.map(({ software_order_count, ...user }) => ({ ...user })),
    )
  },
})

const makeHandlers = (session: TestSession | null, store: AdminOverviewDbStore) =>
  makeAdminOverviewHandlers({
    appendRefreshedSessionCookies: response => {
      response.headers.set('x-session-refreshed', 'true')

      return response
    },
    identityDb: () => overviewIdentityDb(store),
    isOpenAgentsAdminEmail: email => email === 'chris@openagents.com',
    requireBrowserSession: () => Promise.resolve(session ?? undefined),
  })

const runOverview = (
  session: TestSession | null,
  store: AdminOverviewDbStore,
) =>
  Effect.runPromise(
    makeHandlers(session, store).handleAdminOverviewApi(
      new Request('https://openagents.com/api/admin/overview'),
      {
        OPENAGENTS_DB: adminOverviewDb(store),
      },
      executionContext(),
    ),
  )

describe('admin overview API', () => {
  test('returns unauthorized without a browser session', async () => {
    const response = await runOverview(null, new AdminOverviewDbStore())

    expect(response.status).toBe(401)
    await expect(response.json()).resolves.toEqual({ error: 'unauthorized' })
  })

  test('returns forbidden for non-admin users', async () => {
    const response = await runOverview(
      { user: { email: 'ben@openagents.com' } },
      new AdminOverviewDbStore(),
    )

    expect(response.status).toBe(403)
    await expect(response.json()).resolves.toEqual({ error: 'forbidden' })
  })

  test('returns users and software orders for the configured admin', async () => {
    const store = new AdminOverviewDbStore()
    const response = await runOverview(
      { user: { email: 'chris@openagents.com' } },
      store,
    )

    expect(response.status).toBe(200)
    expect(response.headers.get('x-session-refreshed')).toBe('true')
    expect(store.queries.join('\n')).toMatch(/\bJOIN\s+site_projects\b/i)
    await expect(response.json()).resolves.toEqual({
      users: [
        {
          userId: 'github:1',
          kind: 'human',
          displayName: 'Chris',
          email: 'chris@openagents.com',
          githubUsername: 'chris',
          status: 'active',
          onboardingStep: 'complete',
          onboardingCompletedAt: '2026-06-04T12:00:00.000Z',
          softwareOrderCount: 1,
          createdAt: '2026-06-04T11:00:00.000Z',
          updatedAt: '2026-06-04T12:00:00.000Z',
        },
      ],
      softwareOrders: [
        {
          id: 'software_order_1',
          userId: 'github:1',
          userDisplayName: 'Chris',
          userEmail: 'chris@openagents.com',
          status: 'submitted',
          visibility: 'public',
          request: 'Add an admin panel.',
          repositoryFullName: 'OpenAgentsInc/autopilot-omega',
          currentRunId: null,
          siteProjectId: 'site_project_otec',
          siteTitle: 'OTEC Site',
          siteSlug: 'otec',
          siteStatus: 'draft',
          siteAccessMode: 'public',
          siteVisibility: 'public',
          siteActiveVersionId: 'site_version_1',
          siteActiveDeploymentId: 'site_deployment_1',
          siteActiveUrl: 'https://sites.openagents.com/otec',
          siteVersionCount: 2,
          siteLatestVersionId: 'site_version_2',
          siteLatestVersionStatus: 'saved',
          siteLatestVersionSourceKind: 'autopilot_generated',
          siteLatestVersionCreatedAt: '2026-06-04T12:04:00.000Z',
          siteDeploymentCount: 1,
          siteLatestDeploymentId: 'site_deployment_1',
          siteLatestDeploymentStatus: 'active',
          siteLatestDeploymentRuntimeKind: 'omega_static_r2',
          siteLatestDeploymentUpdatedAt: '2026-06-04T12:05:00.000Z',
          siteStorageBindingCount: 2,
          siteStorageBindingSummary: 'd1:SITE_DB, r2:SITE_ASSETS',
          siteEnvironmentValueCount: 1,
          siteEnvironmentKeySummary: 'OPENAI_API_KEY:secret',
          siteAccessGrantCount: 1,
          siteLatestEventType: 'site_version.saved',
          siteLatestEventSummary: 'Saved Site version site_version_2.',
          siteLatestEventCreatedAt: '2026-06-04T12:04:00.000Z',
          siteLatestCompatibilityId: 'site_compatibility_check_1',
          siteLatestCompatibilityStatus: 'ready',
          siteLatestCompatibilityCustomerSafeStatus:
            'The Site is compatible with static hosting.',
          siteLatestCompatibilityCustomerSafeNextAction:
            'Run build validation.',
          siteLatestCompatibilityBlockerCount: 0,
          siteLatestCompatibilityWarningCount: 1,
          siteLatestCompatibilityCreatedAt: '2026-06-04T12:03:00.000Z',
          siteLatestBuildValidationId: 'site_build_validation_1',
          siteLatestBuildValidationStatus: 'passed',
          siteLatestBuildValidationSourceHash: 'sha256:site',
          siteLatestBuildValidationCustomerSafeStatus:
            'The latest build passed.',
          siteLatestBuildValidationCustomerSafeNextAction:
            'Review and deploy the saved version.',
          siteLatestBuildValidationBlockerCount: 0,
          siteLatestBuildValidationWarningCount: 0,
          siteLatestBuildValidationCreatedAt: '2026-06-04T12:04:30.000Z',
          createdAt: '2026-06-04T12:01:00.000Z',
          updatedAt: '2026-06-04T12:01:00.000Z',
          archivedAt: null,
        },
      ],
    })
  })
})
