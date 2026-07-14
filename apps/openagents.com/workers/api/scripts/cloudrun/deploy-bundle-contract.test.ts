import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { describe, expect, test } from 'vitest'

import { shouldBundleCloudRunDependency } from '../../vite.config'
import { externalRuntimeSpecifiers } from './assert-self-contained-bundle.mjs'

describe('Cloud Run Vite Plus bundle contract', () => {
  test('preserves the server bundle while packing preload', () => {
    const deployScript = readFileSync(
      fileURLToPath(new URL('../deploy-cloudrun.sh', import.meta.url)),
      'utf8',
    )

    expect(deployScript).toMatch(
      /vp pack src\/cloudrun\/preload\.ts[\s\S]*?--no-clean/,
    )
    expect(deployScript).toContain('! -f dist-cloudrun/server.mjs')
    expect(deployScript).toContain('! -f dist-cloudrun/preload.mjs')
    expect(deployScript).toContain('assert-self-contained-bundle.mjs')
    expect(deployScript).toContain(
      'pnpm --config.node-linker=hoisted',
    )
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
  })

  test('bundles owned workspace packages like the T3 Code pack pattern', () => {
    expect(
      shouldBundleCloudRunDependency('@openagentsinc/khala-sync-server'),
    ).toBe(true)
    expect(shouldBundleCloudRunDependency('effect')).toBe(false)
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
