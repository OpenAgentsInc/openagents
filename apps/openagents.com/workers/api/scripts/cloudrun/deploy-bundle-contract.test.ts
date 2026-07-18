import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { describe, expect, test } from 'vitest'

import { shouldBundleCloudRunDependency } from '../../vite.config'
import { externalRuntimeSpecifiers } from './assert-self-contained-bundle.mjs'

describe('Cloud Run Vite Plus bundle contract', () => {
  test.each(['env-production.yaml', 'env-staging.yaml'])(
    '%s has unique top-level keys and exactly one enabled Desktop usage gate',
    fileName => {
      const contents = readFileSync(
        fileURLToPath(new URL(fileName, import.meta.url)),
        'utf8',
      )
      const entries = contents
        .split(/\r?\n/u)
        .filter(
          line => line.trim().length > 0 && !line.trimStart().startsWith('#'),
        )
        .map(line => {
          const match = /^([A-Z][A-Z0-9_]*):\s*(.*)$/u.exec(line)
          expect(
            match,
            `expected a flat top-level YAML mapping entry: ${line}`,
          ).not.toBeNull()
          return { key: match![1], value: match![2] }
        })

      const counts = new Map<string, number>()
      for (const { key } of entries) counts.set(key, (counts.get(key) ?? 0) + 1)
      expect(
        [...counts.entries()].filter(([, count]) => count !== 1),
        'duplicate YAML keys are ambiguous and must fail closed',
      ).toEqual([])
      expect(
        entries.filter(
          ({ key }) => key === 'DESKTOP_CODEX_USAGE_INGEST_ENABLED',
        ),
      ).toEqual([{ key: 'DESKTOP_CODEX_USAGE_INGEST_ENABLED', value: '"1"' }])
    },
  )

  test('packs only the self-contained Node server bundle', () => {
    const deployScript = readFileSync(
      fileURLToPath(new URL('../deploy-cloudrun.sh', import.meta.url)),
      'utf8',
    )

    expect(deployScript).toContain('vp pack src/cloudrun/server.ts')
    expect(deployScript).toContain('pnpm run build:start')
    expect(deployScript).toContain('CI=true pnpm run build:start')
    expect(deployScript).toContain('OPENAGENTS_SKIP_START_BUILD')
    expect(deployScript).toContain('apps/start/dist/cloudrun/server.mjs')
    expect(deployScript).not.toContain('build:astro')
    expect(deployScript).toContain(
      'pnpm install --frozen-lockfile --ignore-scripts',
    )
    expect(deployScript.match(/--config\.ignore-scripts=true/g)).toHaveLength(2)
    expect(deployScript).not.toContain('astro-ui')
    expect(deployScript).toContain('! -f dist-cloudrun/server.mjs')
    expect(deployScript).not.toContain('preload.mjs')
    expect(deployScript).not.toContain('cloudflare-workers-stub')
    expect(deployScript).toContain('assert-self-contained-bundle.mjs')
    expect(deployScript.match(/--config\.node-linker=hoisted/g)).toHaveLength(2)
    expect(deployScript).toContain('--filter @openagentsinc/api-worker deploy')
    expect(deployScript).toContain('pnpm install --frozen-lockfile')
    expect(deployScript).toContain('cd "$REPO_ROOT"')
    expect(deployScript).not.toContain('--deps.never-bundle')
  })

  test('ships Vite Plus split chunks beside the server entry', () => {
    const dockerfile = readFileSync(
      fileURLToPath(new URL('../../Dockerfile', import.meta.url)),
      'utf8',
    )

    expect(dockerfile).toContain('COPY dist-cloudrun/*.mjs ./dist-cloudrun/')
    expect(dockerfile).toContain(
      'COPY dist-cloudrun/node_modules ./node_modules',
    )
    expect(dockerfile).toContain(
      'COPY dist-cloudrun/start-client ./start-client',
    )
    expect(dockerfile).toContain(
      'COPY dist-cloudrun/start-server ./start-server',
    )
    expect(dockerfile).not.toContain('astro-ui')
    expect(dockerfile).not.toContain('ASTRO_UI_DIR')
  })

  test('bundles owned workspace packages like the T3 Code pack pattern', () => {
    expect(
      shouldBundleCloudRunDependency('@openagentsinc/khala-sync-server'),
    ).toBe(true)
    expect(shouldBundleCloudRunDependency('effect')).toBe(false)
    expect(shouldBundleCloudRunDependency('nostr-effect/pure')).toBe(true)
  })

  test('rejects packages absent from the slim runtime image', () => {
    expect(
      externalRuntimeSpecifiers(`
        import fs from 'node:fs'
        import net from 'net'
        import { Effect } from 'effect'
        import '@openagentsinc/runtime-platform'
        import './local.mjs'
      `),
    ).toEqual(['@openagentsinc/runtime-platform', 'effect'])
  })
})
