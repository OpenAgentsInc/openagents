import { readFileSync } from 'node:fs'
import { describe, expect, test } from 'vitest'

import {
  DEFAULT_DATABASE,
  decidePendingMigrations,
  parseMigrationsList,
} from './check-pending-migrations.mjs'

describe('parseMigrationsList', () => {
  test('returns [] for the "No migrations to apply!" sentinel', () => {
    expect(parseMigrationsList('No migrations to apply!')).toEqual([])
    expect(
      parseMigrationsList(
        'Migrations have been applied!\nNo migrations to apply!\n',
      ),
    ).toEqual([])
  })

  test('extracts pending migration filenames from a wrangler table', () => {
    // The exact shape that masked the 2026-06-25 outage: 0234 pending.
    const raw = [
      '🌀 Loading...',
      '┌───────────────────────────────┐',
      '│ Name                          │',
      '├───────────────────────────────┤',
      '│ 0234_pylon_openauth_links.sql │',
      '└───────────────────────────────┘',
    ].join('\n')
    expect(parseMigrationsList(raw)).toEqual(['0234_pylon_openauth_links.sql'])
  })

  test('extracts multiple pending migrations and de-dupes', () => {
    const raw = [
      'Migrations to be applied:',
      '0234_pylon_openauth_links.sql',
      '0235_some_followup.sql',
      '0234_pylon_openauth_links.sql',
    ].join('\n')
    expect(parseMigrationsList(raw)).toEqual([
      '0234_pylon_openauth_links.sql',
      '0235_some_followup.sql',
    ])
  })

  test('ignores non-migration noise lines', () => {
    const raw = 'Fetching... \nDone in 1.2s\nNo migrations to apply!'
    expect(parseMigrationsList(raw)).toEqual([])
  })
})

describe('decidePendingMigrations', () => {
  test('zero pending => ok, exit 0', () => {
    const d = decidePendingMigrations([])
    expect(d.ok).toBe(true)
    expect(d.exitCode).toBe(0)
    expect(d.message).toContain(DEFAULT_DATABASE)
  })

  test('any pending => not ok, exit 1, names the files + the remediation', () => {
    const d = decidePendingMigrations(['0234_pylon_openauth_links.sql'])
    expect(d.ok).toBe(false)
    expect(d.exitCode).toBe(1)
    expect(d.message).toContain('0234_pylon_openauth_links.sql')
    expect(d.message).toContain('must NOT ship ahead of its schema')
    expect(d.message).toContain('wrangler d1 migrations apply')
  })
})

describe('deploy:safe package command', () => {
  const apiPackage = JSON.parse(
    readFileSync(new URL('../workers/api/package.json', import.meta.url), 'utf8'),
  )
  const deploySafe = apiPackage.scripts['deploy:safe']

  test('keeps migration-first zero-pending build upload ordering', () => {
    const expectedOrder = [
      'cd ../.. && bun run check:deploy-from-main',
      '&& bun run check:deploy &&',
      '&& cd workers/api && wrangler d1 migrations apply openagents-autopilot-staging --env staging --remote',
      '&& cd ../.. && bun run build:web',
      '&& cd workers/api && wrangler deploy --env staging --containers-rollout=none --assets ../../apps/web/dist',
      '&& cd ../.. && bun run predeploy:parallel-dispatch-smoke',
      '&& cd workers/api && wrangler d1 migrations apply openagents-autopilot --remote',
      '&& cd ../.. && bun run check:pending-migrations',
      '&& cd workers/api && wrangler deploy --containers-rollout=none --assets ../../apps/web/dist',
    ]

    expectedOrder.reduce((previousIndex, step) => {
      const index = deploySafe.indexOf(step)
      expect(index).toBeGreaterThan(previousIndex)
      return index
    }, -1)
  })

  test('disables Wrangler container rollout probing on the final upload', () => {
    expect(deploySafe).toContain(
      'wrangler deploy --containers-rollout=none --assets ../../apps/web/dist',
    )
  })

  test('runs the staging parallel-dispatch smoke before any production upload', () => {
    const stagingUpload = deploySafe.indexOf(
      'wrangler deploy --env staging --containers-rollout=none --assets ../../apps/web/dist',
    )
    const smoke = deploySafe.indexOf('bun run predeploy:parallel-dispatch-smoke')
    const productionMigration = deploySafe.indexOf(
      'wrangler d1 migrations apply openagents-autopilot --remote',
    )
    const productionUpload = deploySafe.indexOf(
      'wrangler deploy --containers-rollout=none --assets ../../apps/web/dist',
    )

    expect(stagingUpload).toBeGreaterThan(-1)
    expect(smoke).toBeGreaterThan(stagingUpload)
    expect(productionMigration).toBeGreaterThan(smoke)
    expect(productionUpload).toBeGreaterThan(productionMigration)
  })

  test('runs the khala-sync live-seam smoke after the staging upload and before any production step (#8507)', () => {
    const stagingUpload = deploySafe.indexOf(
      'wrangler deploy --env staging --containers-rollout=none --assets ../../apps/web/dist',
    )
    const seamSmoke = deploySafe.indexOf(
      'bun run predeploy:khala-sync-live-seam-smoke',
    )
    const productionMigration = deploySafe.indexOf(
      'wrangler d1 migrations apply openagents-autopilot --remote',
    )

    // The 2026-07-06 WebSocket-auth incident gate: a cookie-less bearer must
    // complete a real connectLive against staging or production never ships.
    expect(stagingUpload).toBeGreaterThan(-1)
    expect(seamSmoke).toBeGreaterThan(stagingUpload)
    expect(productionMigration).toBeGreaterThan(seamSmoke)
  })
})
