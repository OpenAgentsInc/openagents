import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  makeSiteBuildValidationService,
  type SiteBuildValidationReceipt,
} from './sites-build-validations'
import type { AutopilotSiteProject } from './sites'

const site: AutopilotSiteProject = {
  accessMode: 'public',
  activeDeploymentId: null,
  activeVersionId: null,
  archivedAt: null,
  createdAt: '2026-06-05T00:00:00.000Z',
  id: 'site_project_build',
  ownerUserId: 'github:operator',
  projectId: null,
  prompt: 'Build validation test Site.',
  slug: 'build',
  softwareOrderId: null,
  sourceRepository: {
    name: 'build-site',
    owner: 'OpenAgentsInc',
    provider: 'github',
    ref: 'main',
  },
  status: 'draft',
  teamId: null,
  title: 'Build',
  updatedAt: '2026-06-05T00:00:00.000Z',
  visibility: 'public',
}

class BuildValidationStore {
  events: Array<Record<string, unknown>> = []
  receipts: Array<Record<string, unknown>> = []
}

class BuildValidationStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: BuildValidationStore,
  ) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    this.values = values

    return this
  }

  first<T = unknown>(): Promise<T | null> {
    if (this.query.includes('FROM site_build_validations')) {
      const [siteId] = this.values
      const row =
        this.store.receipts.find(receipt => receipt.site_id === siteId) ?? null

      return Promise.resolve(row as T | null)
    }

    return Promise.reject(new Error(`Unexpected D1 first: ${this.query}`))
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('INSERT INTO site_build_validations')) {
      const [
        id,
        siteId,
        compatibilityCheckId,
        sourceKind,
        sourceRepositoryJson,
        sourceCommitSha,
        sourceHash,
        status,
        packageManager,
        requestedBuildCommand,
        buildCommand,
        outputKind,
        outputPath,
        workerModulePath,
        manifestJson,
        boundedLogsJson,
        logLineCount,
        logTruncated,
        findingsJson,
        blockersJson,
        warningsJson,
        evidenceRefsJson,
        customerSafeStatus,
        customerSafeNextAction,
        validatedByUserId,
        createdAt,
      ] = this.values

      this.store.receipts.unshift({
        blockers_json: blockersJson,
        bounded_logs_json: boundedLogsJson,
        build_command: buildCommand,
        compatibility_check_id: compatibilityCheckId,
        created_at: createdAt,
        customer_safe_next_action: customerSafeNextAction,
        customer_safe_status: customerSafeStatus,
        evidence_refs_json: evidenceRefsJson,
        findings_json: findingsJson,
        id,
        log_line_count: logLineCount,
        log_truncated: logTruncated,
        manifest_json: manifestJson,
        output_kind: outputKind,
        output_path: outputPath,
        package_manager: packageManager,
        requested_build_command: requestedBuildCommand,
        site_id: siteId,
        source_commit_sha: sourceCommitSha,
        source_hash: sourceHash,
        source_kind: sourceKind,
        source_repository_json: sourceRepositoryJson,
        status,
        validated_by_user_id: validatedByUserId,
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

const db = (store: BuildValidationStore): D1Database =>
  ({
    batch: () => Promise.reject(new Error('batch not used')),
    dump: () => Promise.reject(new Error('dump not used')),
    exec: () => Promise.reject(new Error('exec not used')),
    prepare: (query: string) => new BuildValidationStatement(query, store),
    withSession: () => {
      throw new Error('sessions not used')
    },
  }) as unknown as D1Database

const validate = async (
  files: ReadonlyArray<{ path: string; text: string }>,
  options: Readonly<{
    buildLogText?: string
    compatibility?: Parameters<
      ReturnType<typeof makeSiteBuildValidationService>['validateBuild']
    >[0]['compatibility']
    requestedBuildCommand?: string
    visualAssetRequirements?: Parameters<
      ReturnType<typeof makeSiteBuildValidationService>['validateBuild']
    >[0]['visualAssetRequirements']
  }> = {},
): Promise<Readonly<{ receipt: SiteBuildValidationReceipt; store: BuildValidationStore }>> => {
  const store = new BuildValidationStore()
  const receipt = await Effect.runPromise(
    makeSiteBuildValidationService(db(store), {
      makeEventId: () => 'site_event_build',
      makeValidationId: () => 'site_build_validation_test',
      nowIso: () => '2026-06-05T00:00:00.000Z',
    }).validateBuild({
      actorUserId: 'github:operator',
      files,
      site,
      sourceCommitSha: 'abc1234',
      sourceKind: 'github_import',
      ...(options.buildLogText === undefined
        ? {}
        : { buildLogText: options.buildLogText }),
      ...(options.compatibility === undefined
        ? {}
        : { compatibility: options.compatibility }),
      ...(options.requestedBuildCommand === undefined
        ? {}
        : { requestedBuildCommand: options.requestedBuildCommand }),
      ...(options.visualAssetRequirements === undefined
        ? {}
        : { visualAssetRequirements: options.visualAssetRequirements }),
    }),
  )

  return { receipt, store }
}

describe('Site build validation service', () => {
  test('passes static Vite build candidates with a source hash and manifest', async () => {
    const { receipt, store } = await validate([
      {
        path: 'package.json',
        text: JSON.stringify({
          dependencies: { vite: '^6.0.0' },
          packageManager: 'pnpm@9.0.0',
          scripts: { build: 'vite build' },
        }),
      },
      { path: 'pnpm-lock.yaml', text: 'lockfileVersion: 9' },
      { path: 'vite.config.ts', text: 'export default {}' },
      { path: 'src/main.ts', text: 'console.log(import.meta.env.PUBLIC_URL)' },
    ])

    expect(receipt).toMatchObject({
      buildCommand: 'pnpm build',
      outputKind: 'static',
      outputPath: 'dist',
      packageManager: 'pnpm',
      sourceCommitSha: 'abc1234',
      sourceKind: 'github_import',
      status: 'passed',
    })
    expect(receipt.sourceHash).toMatch(/^[a-f0-9]{64}$/)
    expect(receipt.manifest).toMatchObject({
      bindings: { d1: [], r2: [] },
      entrypoints: ['dist'],
      envKeys: ['PUBLIC_URL'],
    })
    expect(receipt.boundedLogs).toContain('Build validation status: passed.')
    expect(store.events).toHaveLength(1)
  })

  test('blocks image-required candidates that only provide CSS diagrams', async () => {
    const { receipt } = await validate(
      [
        { path: 'index.html', text: '<main><section class="diagram"></section></main>' },
        { path: 'style.css', text: '.diagram { background: linear-gradient(red, blue); }' },
      ],
      {
        visualAssetRequirements: [
          {
            kind: 'image',
            required: true,
            source: 'customer_request',
            summary: 'Customer asked to add images.',
          },
        ],
      },
    )

    expect(receipt.status).toBe('failed')
    expect(receipt.blockers.map(item => item.code)).toContain(
      'missing_required_visual_asset',
    )
  })

  test('passes image-required candidates with a real image asset', async () => {
    const { receipt } = await validate(
      [
        {
          path: 'index.html',
          text: '<main><img src="/assets/ocean-platform.webp" alt="Ocean platform"></main>',
        },
        { path: 'assets/ocean-platform.webp', text: 'binary-placeholder' },
      ],
      {
        visualAssetRequirements: [
          {
            kind: 'image',
            required: true,
            source: 'customer_request',
            summary: 'Customer asked to add images.',
          },
        ],
      },
    )

    expect(receipt.status).toBe('passed')
    expect(receipt.findings.map(item => item.code)).toContain(
      'required_visual_asset_present',
    )
  })

  test('uses compatibility blockers as build blockers', async () => {
    const { receipt } = await validate(
      [
        {
          path: 'package.json',
          text: JSON.stringify({
            dependencies: { vite: '^6.0.0' },
            scripts: { build: 'vite build' },
          }),
        },
      ],
      {
        compatibility: {
          blockers: [
            {
              code: 'unsupported_node_runtime_api',
              evidence: ['src/index.ts'],
              message: 'Node API not supported.',
              severity: 'blocker',
            },
          ],
          compatibilityCheckId: 'site_compatibility_check_1',
          outputKind: 'static',
          outputPath: 'dist',
          status: 'blocked',
        },
      },
    )

    expect(receipt.status).toBe('blocked')
    expect(receipt.compatibilityCheckId).toBe('site_compatibility_check_1')
    expect(receipt.blockers.map(item => item.code)).toContain(
      'unsupported_node_runtime_api',
    )
  })

  test('blocks SSR and Node runtime API build candidates', async () => {
    const { receipt } = await validate([
      {
        path: 'package.json',
        text: JSON.stringify({
          dependencies: { next: '^16.0.0' },
          scripts: { build: 'next build' },
        }),
      },
      { path: 'app/page.tsx', text: 'import fs from "fs"; export default null' },
    ])

    expect(receipt.status).toBe('failed')
    expect(receipt.blockers.map(item => item.code)).toEqual(
      expect.arrayContaining([
        'unsupported_ssr_runtime',
        'unsupported_node_runtime_api',
      ]),
    )
  })

  test('bounds build logs and records truncation metadata', async () => {
    const { receipt } = await validate(
      [{ path: 'index.html', text: '<main></main>' }],
      { buildLogText: Array.from({ length: 120 }, (_, index) => `line ${index}`).join('\n') },
    )

    expect(receipt.boundedLogs).toHaveLength(80)
    expect(receipt.logLineCount).toBe(120)
    expect(receipt.logTruncated).toBe(true)
  })

  test('keeps source hashes stable regardless of file order', async () => {
    const first = await validate([
      { path: 'b.txt', text: 'two' },
      { path: 'a.txt', text: 'one' },
    ])
    const second = await validate([
      { path: 'a.txt', text: 'one' },
      { path: 'b.txt', text: 'two' },
    ])

    expect(first.receipt.sourceHash).toBe(second.receipt.sourceHash)
  })

  test('rejects credential-shaped build inputs', async () => {
    await expect(
      validate([
        { path: 'index.html', text: '<main></main>' },
        { path: '.env', text: 'OPENAI_API_KEY=sk-test-secret-value\n' },
      ]),
    ).rejects.toMatchObject({
      _tag: 'SiteBuildValidationUnsafePayload',
    })
  })
})
