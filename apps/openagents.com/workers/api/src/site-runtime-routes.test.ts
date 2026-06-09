import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { makeSiteRuntimeRoutes } from './site-runtime-routes'

type SiteRuntimeRow = Readonly<{
  access_mode: string
  active_deployment_id: string | null
  build_status: string | null
  deployment_id: string | null
  deployment_status: string | null
  dispatch_namespace: string | null
  external_deployment_id: string | null
  d1_binding_name: string | null
  r2_binding_name: string | null
  runtime_kind: string | null
  runtime_script_name: string | null
  site_id: string
  site_status: string
  slug: string
  static_assets_manifest_json: string | null
  version_id: string | null
  visibility: string
  worker_module_r2_key: string | null
}>

class SiteRuntimeDbStore {
  rows: Array<SiteRuntimeRow> = [
    {
      access_mode: 'public',
      active_deployment_id: 'site_deployment_otec',
      build_status: 'saved',
      deployment_id: 'site_deployment_otec_previous',
      deployment_status: 'rolled_back',
      dispatch_namespace: null,
      external_deployment_id: null,
      d1_binding_name: null,
      r2_binding_name: null,
      runtime_kind: 'omega_static_r2',
      runtime_script_name: null,
      site_id: 'site_project_otec',
      site_status: 'approved',
      slug: 'otec',
      static_assets_manifest_json: JSON.stringify({
        assets: {
          'index.html': {
            cacheControl: 'public, max-age=120',
            contentType: 'text/html; charset=utf-8',
            r2Key:
              'sites/otec/deployments/site_deployment_otec_previous/index.html',
          },
        },
      }),
      version_id: 'site_version_otec_previous',
      visibility: 'public',
      worker_module_r2_key: null,
    },
    {
    access_mode: 'public',
    active_deployment_id: 'site_deployment_otec',
    build_status: 'saved',
    deployment_id: 'site_deployment_otec',
    deployment_status: 'active',
    dispatch_namespace: null,
    external_deployment_id: null,
    d1_binding_name: null,
    r2_binding_name: null,
    runtime_kind: 'omega_static_r2',
    runtime_script_name: null,
    site_id: 'site_project_otec',
    site_status: 'approved',
    slug: 'otec',
    static_assets_manifest_json: JSON.stringify({
      assets: {
        'index.html': {
          cacheControl: 'public, max-age=120',
          contentType: 'text/html; charset=utf-8',
          r2Key: 'sites/otec/deployments/site_deployment_otec/index.html',
        },
      },
    }),
    version_id: 'site_version_otec',
    visibility: 'public',
    worker_module_r2_key: null,
    },
  ]

  get row(): SiteRuntimeRow | null {
    return this.rows.find(row => row.deployment_id === row.active_deployment_id) ?? null
  }

  set row(value: SiteRuntimeRow | null) {
    this.rows = value === null ? [] : [value]
  }
}

class SiteRuntimeStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: SiteRuntimeDbStore,
  ) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    this.values = values

    return this
  }

  first<T = unknown>(colName: string): Promise<T | null>
  first<T = Record<string, unknown>>(): Promise<T | null>
  first<T = unknown>(): Promise<T | null> {
    if (this.query.includes('FROM site_projects')) {
      const isVersionQuery = this.query.includes('site_versions.id = ?')
      const versionId = isVersionQuery ? String(this.values[0]) : null
      const slug = String(this.values[isVersionQuery ? 1 : 0])
      const row = isVersionQuery
        ? this.store.rows.find(
            row => row.slug === slug && row.version_id === versionId,
          )
        : this.store.row?.slug === slug
          ? this.store.row
          : undefined

      return Promise.resolve((row as T | undefined) ?? null)
    }

    return Promise.reject(new Error(`Unexpected D1 first: ${this.query}`))
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    return Promise.reject(new Error(`Unexpected D1 run: ${this.query}`))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
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

const siteRuntimeDb = (store: SiteRuntimeDbStore): D1Database => ({
  batch: () => Promise.reject(new Error('D1 batch should not be used')),
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => new SiteRuntimeStatement(query, store),
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
})

class MemoryR2Bucket {
  objects = new Map<string, Readonly<{ body: string; contentType: string }>>([
    [
      'sites/otec/deployments/site_deployment_otec/index.html',
      {
        body: '<!doctype html><title>OTEC</title>',
        contentType: 'text/html; charset=utf-8',
      },
    ],
    [
      'sites/otec/deployments/site_deployment_otec_previous/index.html',
      {
        body: '<!doctype html><title>OTEC previous</title>',
        contentType: 'text/html; charset=utf-8',
      },
    ],
  ])

  get(key: string): Promise<R2ObjectBody | null> {
    const object = this.objects.get(key)

    if (object === undefined) {
      return Promise.resolve(null)
    }

    return Promise.resolve({
      body: new Blob([object.body], { type: object.contentType }).stream(),
      httpEtag: '"test-etag"',
      writeHttpMetadata: headers =>
        headers.set('content-type', object.contentType),
    } as R2ObjectBody)
  }
}

class MemoryDispatchNamespace {
  requests: ReadonlyArray<Readonly<{ name: string; request: Request }>> = []

  get(name: string): Fetcher {
    return {
      fetch: (request: Request) => {
        this.requests = [...this.requests, { name, request }]

        return Promise.resolve(
          new Response(`worker:${name}:${new URL(request.url).pathname}`, {
            headers: { 'content-type': 'text/plain; charset=utf-8' },
          }),
        )
      },
    } as Fetcher
  }
}

const routes = makeSiteRuntimeRoutes({ sitesHost: 'sites.openagents.com' })

const runRoute = (
  store: SiteRuntimeDbStore,
  request: Request,
  dispatch = new MemoryDispatchNamespace(),
): Promise<Response> => {
  const route = routes.routeSiteRuntimeRequest(request, {
    ARTIFACTS: new MemoryR2Bucket() as unknown as R2Bucket,
    OPENAGENTS_DB: siteRuntimeDb(store),
    SITES_DISPATCH: dispatch as unknown as DispatchNamespace,
  })

  if (route === undefined) {
    throw new Error('route did not match')
  }

  return Effect.runPromise(route)
}

describe('site runtime routes', () => {
  test('serves the active static artifact for a public Site slug', async () => {
    const response = await runRoute(
      new SiteRuntimeDbStore(),
      new Request('https://sites.openagents.com/otec'),
    )

    expect(response.status).toBe(200)
    expect(response.headers.get('content-type')).toBe(
      'text/html; charset=utf-8',
    )
    expect(response.headers.get('cache-control')).toBe('public, max-age=120')
    await expect(response.text()).resolves.toContain('<title>OTEC</title>')
  })

  test('serves a previous saved static artifact at a dedicated version URL', async () => {
    const response = await runRoute(
      new SiteRuntimeDbStore(),
      new Request(
        'https://sites.openagents.com/otec/versions/site_version_otec_previous',
      ),
    )

    expect(response.status).toBe(200)
    expect(response.headers.get('content-type')).toBe(
      'text/html; charset=utf-8',
    )
    await expect(response.text()).resolves.toContain(
      '<title>OTEC previous</title>',
    )
  })

  test('does not serve malformed version URLs', async () => {
    const response = await runRoute(
      new SiteRuntimeDbStore(),
      new Request('https://sites.openagents.com/otec/versions/not-a-version'),
    )

    expect(response.status).toBe(404)
  })

  test('does not match non-sites hosts', () => {
    const route = routes.routeSiteRuntimeRequest(
      new Request('https://openagents.com/otec'),
      {
        ARTIFACTS: new MemoryR2Bucket() as unknown as R2Bucket,
        OPENAGENTS_DB: siteRuntimeDb(new SiteRuntimeDbStore()),
        SITES_DISPATCH:
          new MemoryDispatchNamespace() as unknown as DispatchNamespace,
      },
    )

    expect(route).toBeUndefined()
  })

  test('returns 404 for missing Sites', async () => {
    const store = new SiteRuntimeDbStore()
    store.row = null

    const response = await runRoute(
      store,
      new Request('https://sites.openagents.com/otec'),
    )

    expect(response.status).toBe(404)
    await expect(response.text()).resolves.toBe('Not found')
  })

  test('does not serve disabled deployments or disabled Site projects', async () => {
    const disabledDeploymentStore = new SiteRuntimeDbStore()
    disabledDeploymentStore.row = {
      ...disabledDeploymentStore.row!,
      deployment_status: 'disabled',
    }
    const disabledSiteStore = new SiteRuntimeDbStore()
    disabledSiteStore.row = {
      ...disabledSiteStore.row!,
      site_status: 'disabled',
    }

    const disabledDeploymentResponse = await runRoute(
      disabledDeploymentStore,
      new Request('https://sites.openagents.com/otec'),
    )
    const disabledSiteResponse = await runRoute(
      disabledSiteStore,
      new Request('https://sites.openagents.com/otec'),
    )

    expect(disabledDeploymentResponse.status).toBe(404)
    expect(disabledSiteResponse.status).toBe(404)
  })

  test('checks public access mode before serving protected Sites', async () => {
    const store = new SiteRuntimeDbStore()
    store.row = {
      ...store.row!,
      access_mode: 'owner_admins',
      visibility: 'private',
    }

    const response = await runRoute(
      store,
      new Request('https://sites.openagents.com/otec'),
    )

    expect(response.status).toBe(404)
  })

  test('redirects query-bearing Site URLs to clean public URLs', async () => {
    const response = await runRoute(
      new SiteRuntimeDbStore(),
      new Request('https://sites.openagents.com/otec?github_write=connected'),
    )

    expect(response.status).toBe(302)
    expect(response.headers.get('location')).toBe(
      'https://sites.openagents.com/otec',
    )
    expect(response.headers.get('cache-control')).toBe('no-store')
  })

  test('dispatches active Workers for Platforms Sites through the namespace', async () => {
    const store = new SiteRuntimeDbStore()
    store.row = {
      ...store.row!,
      dispatch_namespace: 'openagents-sites-production',
      external_deployment_id: 'cf-deployment-otec',
      d1_binding_name: 'SITE_DB',
      r2_binding_name: 'SITE_ASSETS',
      runtime_kind: 'workers_for_platforms',
      runtime_script_name: 'site-worker-otec',
      static_assets_manifest_json: JSON.stringify({ assets: {} }),
      worker_module_r2_key:
        'sites/site_project_otec/versions/site_version_otec/worker.mjs',
    }
    const dispatch = new MemoryDispatchNamespace()

    const response = await runRoute(
      store,
      new Request('https://sites.openagents.com/otec/dashboard', {
        method: 'POST',
      }),
      dispatch,
    )

    expect(response.status).toBe(200)
    await expect(response.text()).resolves.toBe(
      'worker:site-worker-otec:/dashboard',
    )
    expect(dispatch.requests).toHaveLength(1)
    expect(dispatch.requests[0]?.name).toBe('site-worker-otec')
    expect(new URL(dispatch.requests[0]!.request.url).pathname).toBe(
      '/dashboard',
    )
  })
})
