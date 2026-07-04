import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  makeSiteCompatibilityService,
  type SiteCompatibilityReceipt,
} from './sites-compatibility'
import type { AutopilotSiteProject } from './sites'
import { createAutopilotStartSiteTemplate } from './sites-start-template'

const site: AutopilotSiteProject = {
  accessMode: 'public',
  activeDeploymentId: null,
  activeVersionId: null,
  archivedAt: null,
  createdAt: '2026-06-05T00:00:00.000Z',
  id: 'site_project_compat',
  ownerUserId: 'github:operator',
  projectId: null,
  prompt: 'Compatibility test Site.',
  slug: 'compat',
  softwareOrderId: null,
  sourceRepository: {
    name: 'compat-site',
    owner: 'OpenAgentsInc',
    provider: 'github',
    ref: 'main',
  },
  status: 'draft',
  teamId: null,
  title: 'Compat',
  updatedAt: '2026-06-05T00:00:00.000Z',
  visibility: 'public',
}

class CompatibilityStore {
  events: Array<Record<string, unknown>> = []
  receipts: Array<Record<string, unknown>> = []
}

class CompatibilityStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: CompatibilityStore,
  ) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    this.values = values

    return this
  }

  first<T = unknown>(): Promise<T | null> {
    if (this.query.includes('FROM site_compatibility_checks')) {
      const [siteId] = this.values
      const row =
        this.store.receipts.find(receipt => receipt.site_id === siteId) ?? null

      return Promise.resolve(row as T | null)
    }

    return Promise.reject(new Error(`Unexpected D1 first: ${this.query}`))
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('INSERT INTO site_compatibility_checks')) {
      const [
        id,
        siteId,
        sourceKind,
        sourceRepositoryJson,
        status,
        confidence,
        packageManager,
        buildCommand,
        outputKind,
        outputPath,
        workerModulePath,
        needsD1,
        needsR2,
        needsWorkspaceAuth,
        needsPublicAuth,
        envKeysJson,
        findingsJson,
        blockersJson,
        warningsJson,
        evidenceRefsJson,
        customerSafeStatus,
        customerSafeNextAction,
        checkedByUserId,
        createdAt,
      ] = this.values

      this.store.receipts.unshift({
        blockers_json: blockersJson,
        build_command: buildCommand,
        checked_by_user_id: checkedByUserId,
        confidence,
        created_at: createdAt,
        customer_safe_next_action: customerSafeNextAction,
        customer_safe_status: customerSafeStatus,
        env_keys_json: envKeysJson,
        evidence_refs_json: evidenceRefsJson,
        findings_json: findingsJson,
        id,
        needs_d1: needsD1,
        needs_public_auth: needsPublicAuth,
        needs_r2: needsR2,
        needs_workspace_auth: needsWorkspaceAuth,
        output_kind: outputKind,
        output_path: outputPath,
        package_manager: packageManager,
        site_id: siteId,
        source_kind: sourceKind,
        source_repository_json: sourceRepositoryJson,
        status,
        warnings_json: warningsJson,
        worker_module_path: workerModulePath,
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT INTO site_events')) {
      this.store.events.push({ values: this.values })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected D1 run: ${this.query}`))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    return Promise.resolve({ results: [], success: true } as unknown as D1Result<T>)
  }

  raw<T = unknown[]>(options: {
    columnNames: true
  }): Promise<[Array<string>, ...Array<T>]>
  raw<T = unknown[]>(options?: { columnNames?: false }): Promise<Array<T>>
  raw<T = unknown[]>(): Promise<[Array<string>, ...Array<T>] | Array<T>> {
    return Promise.resolve([])
  }
}

const db = (store: CompatibilityStore): D1Database =>
  ({
    batch: () => Promise.reject(new Error('batch not used')),
    dump: () => Promise.reject(new Error('dump not used')),
    exec: () => Promise.reject(new Error('exec not used')),
    prepare: (query: string) => new CompatibilityStatement(query, store),
    withSession: () => {
      throw new Error('sessions not used')
    },
  }) as unknown as D1Database

const check = async (
  files: ReadonlyArray<{ path: string; text: string }>,
): Promise<SiteCompatibilityReceipt> => {
  const store = new CompatibilityStore()

  return Effect.runPromise(
    makeSiteCompatibilityService(db(store), {
      makeCheckId: () => 'site_compatibility_check_test',
      makeEventId: () => 'site_event_test',
      nowIso: () => '2026-06-05T00:00:00.000Z',
    }).checkCompatibility({
      actorUserId: 'github:operator',
      files,
      site,
    }),
  )
}

describe('Site compatibility checker', () => {
  test('detects static Vite projects as ready', async () => {
    const receipt = await check([
      {
        path: 'package.json',
        text: JSON.stringify({
          dependencies: { vite: '^6.0.0' },
          scripts: { build: 'vite build' },
        }),
      },
      { path: 'pnpm-lock.yaml', text: 'lockfileVersion: 9' },
      { path: 'vite.config.ts', text: 'export default {}' },
    ])

    expect(receipt).toMatchObject({
      buildCommand: 'pnpm build',
      outputKind: 'static',
      outputPath: 'dist',
      packageManager: 'pnpm',
      status: 'ready',
    })
  })

  test('detects Worker module projects and storage/auth needs', async () => {
    const receipt = await check([
      {
        path: 'package.json',
        text: JSON.stringify({
          dependencies: { hono: '^4.0.0' },
          packageManager: 'bun@1.2.0',
          scripts: { build: 'wrangler deploy --dry-run' },
        }),
      },
      { path: 'wrangler.toml', text: 'main = "src/index.ts"' },
      {
        path: 'src/index.ts',
        text: 'export default { fetch(req, env) { env.DB; env.BUCKET; env.OPENAUTH_ISSUER_URL } }',
      },
      { path: '.env.example', text: 'OPENAI_API_KEY=\nPUBLIC_URL=\n' },
    ])

    expect(receipt).toMatchObject({
      buildCommand: 'bun run build',
      envKeys: ['BUCKET', 'DB', 'OPENAI_API_KEY', 'OPENAUTH_ISSUER_URL', 'PUBLIC_URL'],
      needsD1: true,
      needsR2: true,
      needsWorkspaceAuth: true,
      outputKind: 'worker_module',
      status: 'ready',
      workerModulePath: 'src/index.ts',
    })
  })

  test('recognizes TanStack Start wrangler.jsonc projects as WfP Worker modules', async () => {
    const template = createAutopilotStartSiteTemplate({
      siteId: site.id,
      slug: site.slug,
      title: 'Compat Start Site',
    })
    const receipt = await check(template.files)

    expect(receipt).toMatchObject({
      buildCommand: 'bun run build',
      confidence: 'high',
      outputKind: 'worker_module',
      packageManager: 'bun',
      status: 'ready',
      workerModulePath: 'src/server.ts',
    })
    expect(receipt.blockers).toEqual([])
    expect(receipt.findings.map(item => item.code)).toContain(
      'worker_module_candidate',
    )
  })

  test('blocks unsupported SSR and Node runtime API projects', async () => {
    const receipt = await check([
      {
        path: 'package.json',
        text: JSON.stringify({
          dependencies: { next: '^16.0.0' },
          scripts: { build: 'next build' },
        }),
      },
      { path: 'app/page.tsx', text: 'import fs from "fs"; export default null' },
    ])

    expect(receipt.status).toBe('blocked')
    expect(receipt.blockers.map(item => item.code)).toEqual(
      expect.arrayContaining([
        'unsupported_ssr_runtime',
        'unsupported_node_runtime_api',
      ]),
    )
  })

  test('keeps env secret values out of receipts', async () => {
    const receipt = await check([
      { path: 'index.html', text: '<main></main>' },
      { path: '.env.example', text: 'OPENAI_API_KEY=sk-test-secret-value\n' },
    ])

    expect(receipt.envKeys).toEqual(['OPENAI_API_KEY'])
    expect(JSON.stringify(receipt)).not.toContain('sk-test-secret-value')
  })
})
